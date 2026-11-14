//! The sign-in window: a small browser of Encore's own where the user signs in to YouTube
//! Music, the way Pear Desktop does. Its session belongs to Encore alone. A session copied from
//! the user's browser stops working as soon as that browser renews its cookies, often within
//! minutes; nothing renews this one behind Encore's back.
//!
//! The window runs in a second Encore process (`encore --sign-in-window`), so GTK's main loop
//! never meets iced's. It keeps its cookies in memory only. Once YouTube Music opens signed in,
//! it writes the YouTube cookies as one `Cookie` header line to its standard output, which only
//! the Encore that started it reads, and closes.

use std::cell::Cell;
use std::io::{IsTerminal, Write};
use std::process::Stdio;
use std::rc::Rc;

use gtk::prelude::*;
use webkit2gtk::{CookieManagerExt, LoadEvent, WebContext, WebViewExt, WebsiteDataManagerExt};

/// The command line flag that makes Encore show the sign-in window instead of the player.
pub const FLAG: &str = "--sign-in-window";

/// Google's sign-in page, going on to YouTube Music through YouTube's own sign-in step (which
/// sets the youtube.com cookies), as music.youtube.com's "Sign in" button does.
const SIGN_IN_URL: &str = "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube\
    &passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue\
    %26app%3Ddesktop%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F";

const MUSIC_URL: &str = "https://music.youtube.com/";

/// Opens the sign-in window and waits until the user signed in or closed it. Returns the
/// YouTube cookies as a `Cookie` header line.
pub async fn sign_in() -> Result<String, String> {
    let encore = std::env::current_exe()
        .map_err(|error| format!("could not find Encore's own program: {error}"))?;
    let output = tokio::process::Command::new(encore)
        .arg(FLAG)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| format!("could not open the sign-in window: {error}"))?;
    let header = String::from_utf8_lossy(&output.stdout).trim().to_string();
    match output.status.code() {
        Some(0) if has_session(&header) => Ok(header),
        Some(0) | Some(1) => Err("the sign-in window was closed before signing in".to_string()),
        _ => Err("the sign-in window could not start (is WebKitGTK installed?)".to_string()),
    }
}

/// Shows the sign-in window. Returns the process's exit code: 0 once signed in (the cookies
/// went to standard output), 1 if the user closed the window, 2 if it could not start.
pub fn run() -> i32 {
    // It prints the session cookies, so it only runs for the Encore that reads them.
    if std::io::stdout().is_terminal() || gtk::init().is_err() {
        return 2;
    }
    let signed_in = Rc::new(Cell::new(false));
    let context = WebContext::new_ephemeral();
    let view = webkit2gtk::WebView::with_context(&context);
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Sign in to YouTube Music \u{2014} Encore");
    window.set_default_size(480, 720);
    window.add(&view);
    window.connect_destroy(|_| gtk::main_quit());

    let done = signed_in.clone();
    view.connect_load_changed(move |view, event| {
        let on_music = view.uri().is_some_and(|uri| uri.starts_with(MUSIC_URL));
        if event != LoadEvent::Finished || !on_music || done.get() {
            return;
        }
        let Some(manager) = view
            .website_data_manager()
            .and_then(|data| data.cookie_manager())
        else {
            return;
        };
        let done = done.clone();
        manager.cookies(MUSIC_URL, None::<&gtk::gio::Cancellable>, move |result| {
            let Ok(mut cookies) = result else {
                return;
            };
            let pairs: Vec<(String, String)> = cookies
                .iter_mut()
                .filter_map(|cookie| {
                    Some((cookie.name()?.to_string(), cookie.value()?.to_string()))
                })
                .collect();
            let Some(header) = cookie_header(&pairs) else {
                // YouTube Music opened, but not signed in yet.
                return;
            };
            let mut out = std::io::stdout();
            if writeln!(out, "{header}").and_then(|()| out.flush()).is_ok() {
                done.set(true);
            }
            gtk::main_quit();
        });
    });

    view.load_uri(SIGN_IN_URL);
    window.show_all();
    gtk::main();
    if signed_in.get() { 0 } else { 1 }
}

/// The cookies as a `Cookie` header line, keeping the first of each name, or `None` without a
/// signed-in session (YouTube signs requests with SAPISID).
fn cookie_header(cookies: &[(String, String)]) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    let header = cookies
        .iter()
        .filter(|(name, _)| !name.is_empty() && seen.insert(name.as_str()))
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ");
    has_session(&header).then_some(header)
}

/// Whether a `Cookie` header line holds a signed-in session.
fn has_session(header: &str) -> bool {
    header.split(';').any(|pair| {
        pair.trim()
            .strip_prefix("SAPISID=")
            .is_some_and(|value| !value.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn builds_the_cookie_header_once_signed_in() {
        let cookies = pairs(&[
            ("YSC", "a"),
            ("SAPISID", "b"),
            ("__Secure-3PSID", "c"),
            ("YSC", "older"),
        ]);
        assert_eq!(
            cookie_header(&cookies).as_deref(),
            Some("YSC=a; SAPISID=b; __Secure-3PSID=c")
        );
    }

    #[test]
    fn no_header_before_signing_in() {
        assert_eq!(cookie_header(&pairs(&[("YSC", "a"), ("PREF", "b")])), None);
        assert_eq!(cookie_header(&pairs(&[("SAPISID", "")])), None);
        assert!(
            !has_session("__Secure-3PAPISID=x"),
            "only SAPISID itself counts"
        );
    }
}
