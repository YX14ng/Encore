//! The sign-in window: a small browser of Encore's own where the user signs in to YouTube
//! Music, the way Pear Desktop does. Its session belongs to Encore alone. A session copied from
//! the user's browser stops working as soon as that browser renews its cookies, often within
//! minutes; nothing renews this one behind Encore's back.
//!
//! The window runs in a second Encore process (`encore --sign-in-window`), so its main loop
//! never meets iced's: WebKitGTK on Linux, WebView2 on Windows. It keeps its cookies in memory
//! only. Once YouTube Music opens signed in,
//! it writes the YouTube cookies as one `Cookie` header line to its standard output, which only
//! the Encore that started it reads, and closes.

use std::process::Stdio;

use rust_i18n::t;

/// The command line flag that makes Encore show the sign-in window instead of the player.
pub const FLAG: &str = "--sign-in-window";

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
        Some(0) if has_session(&header) => Ok(header),
        Some(0) | Some(1) => Err(t!("sign_in_window.closed").into_owned()),
        _ => Err(t!("sign_in_window.no_webkit", engine = ENGINE).into_owned()),
    }
}

#[cfg(target_os = "linux")]
mod gtk;
#[cfg(windows)]
mod webview2;

#[cfg(target_os = "linux")]
pub use gtk::run;
#[cfg(windows)]
pub use webview2::run;

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
