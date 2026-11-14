//! Reading a session from a browser. YouTube Music only serves the library (history, saved
//! songs, playlists) to requests that carry the session cookies of a signed-in browser. Encore
//! signs in through its own window (`sign_in_window`); as other ways, it asks yt-dlp to read the
//! cookies from the browser the user picks, or reads a cookies.txt, keeps only the YouTube ones
//! and hands them to rustypipe.
//!
//! The cookies give full access to the Google account while they are valid. They stay on this
//! computer, in Encore's data folder, readable only by the user.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tokio::process::Command;

/// A video that has been on YouTube since 2005; yt-dlp needs some URL to run against.
const PROBE_URL: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw";

/// The browsers Encore can read a YouTube session from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Browser {
    Zen,
    Firefox,
    Chromium,
    Chrome,
    Brave,
}

/// Every browser, with its name, in the order they are offered.
pub const BROWSERS: [(Browser, &str); 5] = [
    (Browser::Zen, "Zen"),
    (Browser::Firefox, "Firefox"),
    (Browser::Chromium, "Chromium"),
    (Browser::Chrome, "Chrome"),
    (Browser::Brave, "Brave"),
];

/// Why reading a browser's session failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// The browser has no signed-in YouTube session (yet).
    NoSession,
    /// Something else went wrong: no profile, yt-dlp failed, ...
    Failed(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::NoSession => write!(
                f,
                "no YouTube session was found there; sign in to music.youtube.com in that browser first"
            ),
            ReadError::Failed(error) => write!(f, "{error}"),
        }
    }
}

impl Browser {
    /// The value for yt-dlp's `--cookies-from-browser`. Zen is Firefox-based but unknown to
    /// yt-dlp, so it is passed as Firefox with the path of its profile.
    pub fn ytdlp_spec(self, home: &Path) -> Result<String, String> {
        Ok(match self {
            Browser::Zen => format!("firefox:{}", zen_profile(home)?.display()),
            Browser::Firefox => "firefox".to_string(),
            Browser::Chromium => "chromium".to_string(),
            Browser::Chrome => "chrome".to_string(),
            Browser::Brave => "brave".to_string(),
        })
    }
}

/// Where Encore keeps data that must survive cache cleaning: `$XDG_DATA_HOME/encore`, or
/// `~/.local/share/encore`.
pub fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir)
        .join("encore")
}

/// Creates `dir` readable only by the user.
pub fn private_dir(dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())
}

/// The folders where Zen keeps its profiles: `~/.config/zen` in recent versions, `~/.zen` in
/// older ones, and the same two inside the Flatpak sandbox. Profiles sit right inside them or
/// inside a `Profiles` subfolder.
fn zen_profile_dirs(home: &Path) -> Vec<PathBuf> {
    let roots = [
        home.join(".config/zen"),
        home.join(".zen"),
        home.join(".var/app/app.zen_browser.zen/config/zen"),
        home.join(".var/app/app.zen_browser.zen/.zen"),
    ];
    roots
        .iter()
        .flat_map(|root| [root.clone(), root.join("Profiles")])
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
}

/// The Zen profile in use: among the profiles that have cookies, the most recently written one.
pub fn zen_profile(home: &Path) -> Result<PathBuf, String> {
    zen_profile_dirs(home)
        .into_iter()
        .filter_map(|profile| {
            let modified = std::fs::metadata(profile.join("cookies.sqlite"))
                .and_then(|metadata| metadata.modified())
                .ok()?;
            Some((modified, profile))
        })
        .max_by_key(|(modified, _): &(SystemTime, PathBuf)| *modified)
        .map(|(_, profile)| profile)
        .ok_or_else(|| "no Zen profile with cookies was found".to_string())
}

/// Keeps the comment header and the `youtube.com` cookies of a Netscape cookies file.
/// HttpOnly cookies are written as `#HttpOnly_.youtube.com\t…`, so they are not comments.
pub fn youtube_only(cookies_txt: &str) -> String {
    cookies_txt
        .lines()
        .filter(|line| {
            if !line.contains('\t') {
                return line.starts_with('#') || line.trim().is_empty();
            }
            let domain = line.split('\t').next().unwrap_or_default();
            let domain = domain.strip_prefix("#HttpOnly_").unwrap_or(domain);
            domain == "youtube.com" || domain.ends_with(".youtube.com")
        })
        .map(|line| format!("{line}\n"))
        .collect()
}

/// Whether a cookies file holds a signed-in YouTube session.
pub fn has_session(cookies_txt: &str) -> bool {
    cookies_txt
        .lines()
        .filter(|line| line.contains('\t'))
        .any(|line| line.split('\t').nth(5) == Some("SAPISID"))
}

/// Reads the YouTube session cookies of `browser` through yt-dlp.
pub async fn read_from_browser(browser: Browser) -> Result<String, ReadError> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(ReadError::Failed("HOME is not set".to_string()))?;
    let spec = browser.ytdlp_spec(&home).map_err(ReadError::Failed)?;

    let dir = data_dir();
    private_dir(&dir).map_err(ReadError::Failed)?;
    let raw = dir.join("cookies-from-browser.txt");

    let output = Command::new("yt-dlp")
        .args(["--no-config", "--quiet", "--no-warnings", "--skip-download"])
        .args(["--cookies-from-browser", &spec])
        .arg("--cookies")
        .arg(&raw)
        .arg(PROBE_URL)
        .output()
        .await;

    // The raw file has every cookie of the browser: read what is needed and delete it at once.
    let all = std::fs::read_to_string(&raw);
    let _ = std::fs::remove_file(&raw);

    let output =
        output.map_err(|error| ReadError::Failed(format!("could not run yt-dlp: {error}")))?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ReadError::Failed(error));
    }
    let cookies = youtube_only(&all.map_err(|error| ReadError::Failed(error.to_string()))?);
    if !has_session(&cookies) {
        return Err(ReadError::NoSession);
    }
    Ok(cookies)
}

/// Opens `url` in the default browser.
pub fn open_in_browser(url: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not open the browser: {error}"))
}

/// Reads a cookies.txt exported by hand (plan B), keeping only the YouTube cookies.
pub fn read_from_file(path: &str) -> Result<String, String> {
    let path = path.trim();
    let path = match path.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(rest))
            .ok_or("HOME is not set")?,
        None => PathBuf::from(path),
    };
    let all =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let cookies = youtube_only(&all);
    if !has_session(&cookies) {
        return Err("that file has no signed-in YouTube session".to_string());
    }
    Ok(cookies)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Netscape HTTP Cookie File\n\
        .youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\tabc\n\
        #HttpOnly_.youtube.com\tTRUE\t/\tTRUE\t0\t__Secure-3PSID\tdef\n\
        .google.com\tTRUE\t/\tTRUE\t0\tSID\tghi\n\
        .example.com\tTRUE\t/\tFALSE\t0\ttracker\tjkl\n\
        music.youtube.com\tFALSE\t/\tFALSE\t0\tPREF\tmno\n";

    #[test]
    fn keeps_only_youtube_cookies() {
        let kept = youtube_only(SAMPLE);
        assert!(kept.contains("SAPISID"));
        assert!(kept.contains("#HttpOnly_.youtube.com"));
        assert!(kept.contains("music.youtube.com"));
        assert!(!kept.contains("google.com"));
        assert!(!kept.contains("example.com"));
        assert!(kept.starts_with("# Netscape HTTP Cookie File"));
    }

    #[test]
    fn a_session_needs_sapisid() {
        assert!(has_session(&youtube_only(SAMPLE)));
        assert!(!has_session(".youtube.com\tTRUE\t/\tTRUE\t0\tPREF\tx\n"));
    }

    #[test]
    fn zen_is_read_as_a_firefox_profile() {
        let home = std::env::temp_dir().join(format!("encore-zen-test-{}", std::process::id()));
        let old = home.join(".zen/old.default");
        let new = home.join(".zen/new.default-release");
        std::fs::create_dir_all(&old).expect("old profile");
        std::fs::create_dir_all(&new).expect("new profile");
        std::fs::write(old.join("cookies.sqlite"), b"old").expect("old cookies");
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(new.join("cookies.sqlite"), b"new").expect("new cookies");

        assert_eq!(
            Browser::Zen.ytdlp_spec(&home),
            Ok(format!("firefox:{}", new.display()))
        );
        assert_eq!(Browser::Chrome.ytdlp_spec(&home), Ok("chrome".to_string()));
        std::fs::remove_dir_all(&home).expect("cleanup");
    }

    #[test]
    fn finds_zen_profiles_in_the_xdg_config_folder() {
        let home = std::env::temp_dir().join(format!("encore-zen-xdg-{}", std::process::id()));
        let profile = home.join(".config/zen/abc.Default (release)");
        std::fs::create_dir_all(&profile).expect("profile");
        std::fs::write(profile.join("cookies.sqlite"), b"x").expect("cookies");
        assert_eq!(zen_profile(&home), Ok(profile));
        std::fs::remove_dir_all(&home).expect("cleanup");
    }

    #[test]
    fn zen_without_profiles_is_an_error() {
        let home = std::env::temp_dir().join("encore-no-zen-here");
        assert!(Browser::Zen.ytdlp_spec(&home).is_err());
    }
}
