//! Renewing the YouTube Music session while Encore is open, the way an open YouTube tab does:
//! at start and then about every ten minutes (whatever Google asks for), Google is asked for
//! new session cookies (rustypipe's `user_auth_rotate_cookie`), which replace the saved ones.
//!
//! Only sessions from Encore's sign-in window are renewed. A session copied from a browser is
//! still the browser's: renewing it would supersede the browser's cookies and sign it out.
//!
//! Each renewal belongs to a run; signing in or out starts a new run, so the waits of an old
//! one end there.
//!
//! When YouTube stops accepting the session anyway, Encore first signs in again with no window
//! (see `sign_in_window`), once; only if Google wants the user does it ask. What happens goes
//! to `session.log` in the cache.

use std::time::Duration;

use iced::Task;
use rustypipe::error::{AuthError, Error};

use crate::{App, Message};

/// The wait before trying again when renewing failed for another reason than the session
/// (offline, YouTube busy).
const RETRY: Duration = Duration::from_secs(600);

/// The state of the renewals.
#[derive(Debug, Default)]
pub struct Renewal {
    /// The current run; renewals of other runs are dropped.
    run: u64,
    /// The sign-in in progress is the sign-in window's.
    from_window: bool,
    /// Signing in again with no window was tried since the session last worked.
    retried: bool,
}

/// Why a renewal failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Google no longer accepts the session.
    Rejected,
    /// Anything else; the session may still be fine.
    Other(String),
}

/// What happens with the renewals.
#[derive(Debug, Clone)]
pub enum Event {
    /// The wait of the run is over: time to renew.
    Due(u64),
    /// A renewal of the run finished: the wait until the next one, or why it failed.
    Renewed(u64, Result<Duration, Failure>),
}

impl App {
    /// Remembers whether the sign-in about to finish is the sign-in window's.
    pub(crate) fn signing_in_from_window(&mut self, from_window: bool) {
        self.renewal.from_window = from_window;
    }

    /// Whether the sign-in that just finished was the sign-in window's.
    pub(crate) fn signed_in_from_window(&self) -> bool {
        self.renewal.from_window
    }

    /// Starts renewing the saved session, if it is Encore's own.
    pub(crate) fn start_renewal(&mut self) -> Task<Message> {
        self.renewal.run += 1;
        if !self.session_log.renewable {
            return Task::none();
        }
        self.renew(self.renewal.run)
    }

    /// Stops renewing (signed out, or the session is no longer accepted).
    pub(crate) fn stop_renewal(&mut self) {
        self.renewal.run += 1;
    }

    pub(crate) fn renewal_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Due(run) if run == self.renewal.run => self.renew(run),
            Event::Renewed(run, result) if run == self.renewal.run => {
                let wait = match result {
                    Ok(wait) => {
                        // The session works: if it is lost again, signing in quietly may help.
                        self.renewal.retried = false;
                        crate::notes::session(format!("renewed; next in {} s", wait.as_secs()));
                        wait
                    }
                    Err(Failure::Rejected) => return self.session_rejected(),
                    Err(Failure::Other(error)) => {
                        crate::notes::session(format!(
                            "renewing failed ({error}); again in {} s",
                            RETRY.as_secs()
                        ));
                        RETRY
                    }
                };
                Task::perform(async move { tokio::time::sleep(wait).await }, move |()| {
                    Message::Renewal(Event::Due(run))
                })
            }
            // A run that was stopped.
            Event::Due(_) | Event::Renewed(..) => Task::none(),
        }
    }

    /// YouTube no longer accepts the saved session. An Encore session whose Google sign-in is
    /// kept is signed in again with no window, once; otherwise, or if that fails, the user is
    /// asked to sign in again.
    pub(crate) fn session_rejected(&mut self) -> Task<Message> {
        self.stop_renewal();
        crate::notes::session("YouTube no longer accepts the session");
        let quietly = self.session_log.renewable
            && !self.renewal.retried
            && crate::sign_in_window::has_profile();
        if !quietly {
            self.session_lost();
            return Task::none();
        }
        self.renewal.retried = true;
        crate::notes::session("signing in again with no window");
        Task::perform(
            crate::sign_in_window::sign_in_silently(),
            Message::SilentSignIn,
        )
    }

    /// Signing in again with no window finished: the new session is loaded, or the user is
    /// asked to sign in.
    pub(crate) fn silent_sign_in_done(
        &mut self,
        result: Result<String, crate::sign_in_window::Silent>,
    ) -> Task<Message> {
        match result {
            Ok(cookie) => {
                crate::notes::session("signed in again with no window");
                let Some(client) = self.online.client() else {
                    return Task::none();
                };
                // The saved library stays on screen while the new session loads it.
                self.signing_in_from_window(true);
                Task::perform(
                    crate::library::sign_in_with_cookie(client, cookie),
                    Message::SignedIn,
                )
            }
            Err(why) => {
                crate::notes::session(format!("signing in with no window failed: {why:?}"));
                self.session_lost();
                Task::none()
            }
        }
    }

    fn renew(&self, run: u64) -> Task<Message> {
        let Some(client) = self.online.client() else {
            return Task::none();
        };
        Task::perform(
            async move {
                client
                    .user_auth_rotate_cookie()
                    .await
                    .map_err(|error| match error {
                        Error::Auth(AuthError::NoLogin) => Failure::Rejected,
                        error => Failure::Other(error.to_string()),
                    })
            },
            move |result| Message::Renewal(Event::Renewed(run, result)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An app whose saved session is or is not Encore's own. The tasks are only built, never
    /// run, so nothing reaches YouTube.
    fn app(renewable: bool) -> App {
        let mut app = App::new();
        app.session_log.renewable = renewable;
        app
    }

    #[test]
    fn only_sessions_from_the_sign_in_window_are_renewed() {
        assert_eq!(app(false).start_renewal().units(), 0);
        assert_eq!(app(true).start_renewal().units(), 1);
    }

    #[test]
    fn renewing_goes_on_after_the_wait_google_asks_for() {
        let mut app = app(true);
        let _ = app.start_renewal();
        let run = app.renewal.run;
        let wait = app.renewal_event(Event::Renewed(run, Ok(Duration::from_secs(600))));
        assert_eq!(wait.units(), 1);
        assert_eq!(app.renewal_event(Event::Due(run)).units(), 1);
    }

    #[test]
    fn a_stopped_run_is_dropped() {
        let mut app = app(true);
        let _ = app.start_renewal();
        let old = app.renewal.run;
        app.stop_renewal();
        assert_eq!(app.renewal_event(Event::Due(old)).units(), 0);
        let _ = app.renewal_event(Event::Renewed(old, Err(Failure::Rejected)));
        assert!(!app.session_expired, "an old run's answer changes nothing");
    }

    #[test]
    fn a_rejected_renewal_asks_to_sign_in_again() {
        let mut app = app(true);
        let _ = app.start_renewal();
        let run = app.renewal.run;
        let retry = app.renewal_event(Event::Renewed(run, Err(Failure::Other("offline".into()))));
        assert_eq!(
            retry.units(),
            1,
            "failing for another reason tries again later"
        );
        assert!(!app.session_expired);
        let _ = app.renewal_event(Event::Renewed(run, Err(Failure::Rejected)));
        assert!(app.session_expired);
        assert_ne!(app.renewal.run, run, "renewing stops");
    }

    /// Renews the real saved session once and checks YouTube still accepts it. A wrong renewal
    /// signs Encore out, so run it only with the user's go-ahead:
    /// `cargo test -- --ignored renews_the_real_session`.
    #[tokio::test]
    #[ignore = "renews the signed-in session"]
    async fn renews_the_real_session() {
        let rp = crate::youtube::client().expect("client");
        let before = rp.user_auth_cookie_header().expect("a saved session");
        let wait = rp.user_auth_rotate_cookie().await;
        let after = rp.user_auth_cookie_header().expect("still a saved session");
        let accepted = rp.user_auth_check_cookie().await;
        assert!(wait.is_ok(), "renewing failed: {:?}", wait.err());
        assert!(accepted.is_ok(), "YouTube refused the renewed session");
        assert_ne!(before, after, "Google sent no new cookies");
    }

    #[test]
    fn a_failed_quiet_sign_in_asks_the_user() {
        let mut app = app(true);
        let task = app.silent_sign_in_done(Err(crate::sign_in_window::Silent::NeedsYou));
        assert_eq!(task.units(), 0);
        assert!(app.session_expired);
    }

    #[test]
    fn a_working_session_may_be_signed_in_quietly_again_later() {
        let mut app = app(true);
        let _ = app.start_renewal();
        app.renewal.retried = true;
        let run = app.renewal.run;
        let _ = app.renewal_event(Event::Renewed(run, Ok(Duration::from_secs(600))));
        assert!(!app.renewal.retried);
    }
}
