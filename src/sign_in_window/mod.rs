//! The sign-in window: a small browser of Encore's own where the user signs in to YouTube
//! Music, the way Pear Desktop does. Its session belongs to Encore alone. A session copied from
//! the user's browser stops working as soon as that browser renews its cookies, often within
//! minutes; nothing renews this one behind Encore's back.
//!
//! The window runs in a second Encore process (`encore --sign-in-window`), so its main loop
//! never meets iced's: WebKitGTK on Linux, WebView2 on Windows. Once YouTube Music opens signed
//! in, it writes the YouTube cookies as one `Cookie` header line to its standard output, which
//! only the Encore that started it reads, and closes.
//!
//! Its browser profile (the Google account's sign-in) is kept in a private folder, so when
//! YouTube stops accepting the session Encore can get a new one by itself: it runs the same
//! page with no window (`--silent`), and Google, still knowing the account, sends it straight
//! back to YouTube Music signed in. Only when Google wants the password again does the user see
//! the window. Signing out deletes the profile.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use rust_i18n::t;

/// The command line flag that makes Encore show the sign-in window instead of the player.
pub const FLAG: &str = "--sign-in-window";

/// The flag, after `FLAG`, that signs in again with no window.
const SILENT_FLAG: &str = "--silent";

/// The exit codes of the sign-in process.
const SIGNED_IN: i32 = 0;
const CLOSED: i32 = 1;
/// Google wants the user (the password, a confirmation): only the window can do it.
const NEEDS_YOU: i32 = 3;

/// How long signing in with no window may take before it gives up.
const SILENT_TIME: Duration = Duration::from_secs(30);

/// The interface language the window speaks, passed on from the Encore that opens it.
const LANGUAGE_VAR: &str = "ENCORE_SIGN_IN_LANGUAGE";

/// Google's sign-in page, going on to YouTube Music through YouTube's own sign-in step (which
/// sets the youtube.com cookies), as music.youtube.com's "Sign in" button does.
const SIGN_IN_URL: &str = "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube\
    &passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue\
    %26app%3Ddesktop%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F";

const MUSIC_URL: &str = "https://music.youtube.com/";

/// The web view the window is made of, named when it cannot start.
#[cfg(windows)]
const ENGINE: &str = "WebView2";
#[cfg(not(windows))]
const ENGINE: &str = "WebKitGTK";

/// Where the browser profile (the Google sign-in) is kept, readable by the user only.
fn profile_dir() -> PathBuf {
    crate::platform::data_dir().join("web")
}

/// Deletes the browser profile, when the user signs out.
pub fn forget_profile() {
    // Tests must never touch the user's data.
    if !cfg!(test) {
        let _ = std::fs::remove_dir_all(profile_dir());
    }
}

/// Whether a browser profile was kept from an earlier sign-in. Tests never see the user's.
pub fn has_profile() -> bool {
    !cfg!(test) && profile_dir().is_dir()
}

/// Why signing in with no window did not work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Silent {
    /// Google wants the user: the window has to open.
    NeedsYou,
    Failed(String),
}

/// Gets a new YouTube session with no window, from the Google sign-in the profile keeps.
pub async fn sign_in_silently() -> Result<String, Silent> {
    let encore = std::env::current_exe().map_err(|error| Silent::Failed(error.to_string()))?;
    let child = tokio::process::Command::new(encore)
        .args([FLAG, SILENT_FLAG])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(SILENT_TIME * 2, child)
        .await
        .map_err(|_| Silent::Failed("timed out".to_string()))?
        .map_err(|error| Silent::Failed(error.to_string()))?;
    let header = String::from_utf8_lossy(&output.stdout).trim().to_string();
    match output.status.code() {
        Some(SIGNED_IN) if has_session(&header) => Ok(header),
        Some(NEEDS_YOU) => Err(Silent::NeedsYou),
        code => Err(Silent::Failed(format!("exit code {code:?}"))),
    }
}

/// Runs the sign-in process: the window, or with `--silent` none. Returns its exit code.
pub fn run() -> i32 {
    let silent = std::env::args().nth(2).as_deref() == Some(SILENT_FLAG);
    show(silent)
}

/// Opens the sign-in window and waits until the user signed in or closed it. Returns the
/// YouTube cookies as a `Cookie` header line.
pub async fn sign_in() -> Result<String, String> {
    let encore = std::env::current_exe()
        .map_err(|error| t!("sign_in_window.no_program", error = error).into_owned())?;
    let output = tokio::process::Command::new(encore)
        .arg(FLAG)
        .env(LANGUAGE_VAR, &*rust_i18n::locale())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| t!("sign_in_window.open_failed", error = error).into_owned())?;
    let header = String::from_utf8_lossy(&output.stdout).trim().to_string();
    match output.status.code() {
        Some(SIGNED_IN) if has_session(&header) => Ok(header),
        Some(SIGNED_IN) | Some(CLOSED) => Err(t!("sign_in_window.closed").into_owned()),
        _ => Err(t!("sign_in_window.no_webkit", engine = ENGINE).into_owned()),
    }
}

#[cfg(target_os = "linux")]
mod gtk;
#[cfg(windows)]
mod webview2;

#[cfg(target_os = "linux")]
use gtk::show;
#[cfg(windows)]
use webview2::show;

/// Whether `url` is one of Google's sign-in forms (the email, the password, a check): with no
/// window, nobody can fill it in. A Google sign-in that is kept passes through other pages.
fn wants_the_user(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://accounts.google.com/") else {
        return false;
    };
    rest.starts_with("v3/signin/")
        || rest.starts_with("signin/")
        || rest.starts_with("InteractiveLogin")
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

    #[test]
    fn google_sign_in_forms_need_the_user() {
        assert!(wants_the_user(
            "https://accounts.google.com/v3/signin/identifier?continue=x"
        ));
        assert!(wants_the_user(
            "https://accounts.google.com/v3/signin/challenge/pwd"
        ));
        assert!(!wants_the_user(
            "https://accounts.google.com/ServiceLogin?passive=true"
        ));
        assert!(!wants_the_user("https://accounts.google.com/CheckCookie"));
        assert!(!wants_the_user("https://music.youtube.com/"));
    }
}
