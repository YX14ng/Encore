//! Downloads: song audio through yt-dlp, and small files like covers over HTTP.
//!
//! rustypipe's own stream download no longer works (its signature deobfuscation is out of date)
//! and YouTube refuses to stream its audio URLs straight into mpv, so every song is downloaded
//! with yt-dlp first and then played from the cache.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rust_i18n::t;
use serde::{Deserialize, Serialize};

/// Audio formats yt-dlp may leave in the cache, in order of preference.
const AUDIO_EXTENSIONS: [&str; 4] = ["webm", "opus", "m4a", "mp4"];

/// The picture of a music video: up to 720p, preferring VP9 in WebM, without sound (the song
/// plays from its own file).
const VIDEO_FORMAT: &str = "bestvideo[height<=720][ext=webm]/bestvideo[height<=720]";

/// How long the session cookies handed to yt-dlp say they last, in seconds. The file is deleted
/// once yt-dlp is done, so this only has to outlast one download.
const COOKIE_LIFETIME: u64 = 24 * 60 * 60;

/// How good the downloaded audio is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    /// Opus around 50 to 70 kbit/s, for slow or metered connections.
    Low,
    /// The best audio YouTube gives anyone: Opus around 130 kbit/s.
    #[default]
    Normal,
    /// The best audio the signed-in account gets: Opus around 256 kbit/s with YouTube Music
    /// Premium, else the same as Normal. Downloads carry the session.
    High,
}

impl Quality {
    /// Every quality, in the order the audio screen shows them.
    pub const ALL: [Self; 3] = [Self::Low, Self::Normal, Self::High];

    /// The quality's name, in the interface's language.
    pub fn label(self) -> Cow<'static, str> {
        match self {
            Self::Low => t!("audio.quality_low"),
            Self::Normal => t!("audio.quality_normal"),
            Self::High => t!("audio.quality_high"),
        }
    }

    /// The cache folder for this quality, so songs of one quality are not played for another.
    fn cache_folder(self) -> &'static str {
        match self {
            Self::Low => "audio-low",
            Self::Normal => "audio",
            Self::High => "audio-high",
        }
    }

    /// yt-dlp's format choice. Normal and High ask for the same thing; with the session,
    /// YouTube offers Premium accounts a better format, and yt-dlp takes it.
    fn format(self) -> &'static str {
        match self {
            Self::Low => "bestaudio[ext=webm][abr<=80]/worstaudio",
            // With Premium the best is ~256 kbit/s; Normal stays at the ~130 kbit/s of everyone.
            Self::Normal => "bestaudio[ext=webm][abr<=170]/bestaudio[abr<=170]/bestaudio",
            Self::High => "bestaudio[ext=webm]/bestaudio",
        }
    }
}

/// Where a song is downloaded from: the quality, and the session's `Cookie` header when signed
/// in. YouTube puts an ad before songs asked for without Premium, and its stream only works once
/// the ad would have played (yt-dlp waits about six seconds); with Premium there is none.
/// High without a session downloads Normal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source {
    pub quality: Quality,
    pub cookie: Option<String>,
}

impl Source {
    /// The quality that is really downloaded.
    fn quality(&self) -> Quality {
        match (self.quality, &self.cookie) {
            (Quality::High, None) => Quality::Normal,
            (quality, _) => quality,
        }
    }
}

pub use crate::platform::cache_dir;

/// A finished download of `id` in `dir`, if there is one.
pub fn find_cached(dir: &Path, id: &str) -> Option<PathBuf> {
    AUDIO_EXTENSIONS
        .iter()
        .map(|extension| dir.join(format!("{id}.{extension}")))
        .find(|path| path.exists())
}

/// Where image `url` is kept once downloaded.
fn image_cache_path(url: &str) -> PathBuf {
    cache_dir()
        .join("images")
        .join(crate::offline::image_file_name(url))
}

/// An image downloaded before, read from the disk cache.
pub fn cached_image(url: &str) -> Option<Vec<u8>> {
    std::fs::read(image_cache_path(url)).ok()
}

/// The yt-dlp arguments that download the audio of `id` in `quality` into `dir` and print the
/// file path, with the session in `cookies` (a cookies.txt) if given.
pub fn ytdlp_args(id: &str, dir: &Path, quality: Quality, cookies: Option<&Path>) -> Vec<String> {
    let template = dir.join("%(id)s.%(ext)s");
    let mut args = vec![
        "--no-config".to_string(),
        "--no-playlist".to_string(),
        "--quiet".to_string(),
        "--no-warnings".to_string(),
        "-f".to_string(),
        quality.format().to_string(),
        "-o".to_string(),
        template.to_string_lossy().into_owned(),
        "--print".to_string(),
        "after_move:filepath".to_string(),
    ];
    if let Some(cookies) = cookies {
        args.push("--cookies".to_string());
        args.push(cookies.to_string_lossy().into_owned());
    }
    args.push(format!("https://music.youtube.com/watch?v={id}"));
    args
}

/// A Netscape cookies file (what yt-dlp reads) with the cookies of a `Cookie` header, for
/// `.youtube.com`, valid until `expires` (seconds since 1970).
fn cookies_txt(header: &str, expires: u64) -> String {
    let mut file = String::from("# Netscape HTTP Cookie File\n");
    for pair in header.split(';') {
        if let Some((name, value)) = pair.trim().split_once('=')
            && !name.is_empty()
        {
            file.push_str(&format!(
                ".youtube.com\tTRUE\t/\tTRUE\t{expires}\t{name}\t{value}\n"
            ));
        }
    }
    file
}

/// Writes the session for one yt-dlp run into a file only the user can read. The caller
/// deletes it once yt-dlp is done.
fn write_cookies(header: &str) -> Result<PathBuf, String> {
    static RUNS: AtomicU64 = AtomicU64::new(0);
    let dir = crate::account::data_dir().join("yt-dlp");
    crate::account::private_dir(&dir)?;
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let path = dir.join(format!("cookies-{}-{run}.txt", std::process::id()));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    crate::platform::write_private(&path, cookies_txt(header, now + COOKIE_LIFETIME).as_bytes())
        .map_err(|error| t!("download.session_not_written", error = error).into_owned())?;
    Ok(path)
}

/// Where downloaded (offline) songs live: kept until the user removes them.
pub fn offline_dir() -> PathBuf {
    crate::account::data_dir().join("offline")
}

/// Returns the audio of `id`: the offline copy if there is one, else the cache, else a new
/// download into the cache.
pub async fn audio(id: String, source: Source) -> Result<PathBuf, String> {
    if let Some(path) = find_cached(&offline_dir(), &id) {
        return Ok(path);
    }
    let quality = source.quality();
    let dir = cache_dir().join(quality.cache_folder());
    let cookie = source.cookie.filter(|_| quality == Quality::High);
    audio_into(id, dir, quality, cookie.as_deref()).await
}

/// Downloads the picture of video `id` alone (up to 720p, no sound) into the cache, unless it
/// is already there.
pub async fn video(id: String) -> Result<PathBuf, String> {
    let dir = cache_dir().join("video");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(path) = find_cached(&dir, &id) {
        return Ok(path);
    }
    let template = dir.join("%(id)s.%(ext)s");
    let output = crate::platform::command("yt-dlp")
        .args([
            "--no-config",
            "--no-playlist",
            "--quiet",
            "--no-warnings",
            "-f",
        ])
        .arg(VIDEO_FORMAT)
        .arg("-o")
        .arg(&template)
        .args(["--print", "after_move:filepath"])
        .arg(format!("https://www.youtube.com/watch?v={id}"))
        .output()
        .await
        .map_err(|error| t!("download.ytdlp_not_run", error = error).into_owned())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .last()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .ok_or_else(|| t!("download.video_not_reported").into_owned())
}

/// Whether song `id` would play without downloading: kept offline, or in the cache of the
/// quality `source` downloads.
pub fn is_cached(id: &str, source: &Source) -> bool {
    find_cached(&offline_dir(), id).is_some()
        || find_cached(&cache_dir().join(source.quality().cache_folder()), id).is_some()
}

/// Keeps a song offline: copies it from the cache if it is there, else downloads it.
pub async fn keep_offline(id: String) -> Result<PathBuf, String> {
    let offline = offline_dir();
    if let Some(path) = find_cached(&offline, &id) {
        return Ok(path);
    }
    tokio::fs::create_dir_all(&offline)
        .await
        .map_err(|error| error.to_string())?;
    // The best copy already in the cache, whatever quality is chosen now.
    let cached = [Quality::High, Quality::Normal, Quality::Low]
        .into_iter()
        .find_map(|quality| find_cached(&cache_dir().join(quality.cache_folder()), &id));
    if let Some(cached) = cached {
        let name = cached
            .file_name()
            .ok_or_else(|| t!("download.bad_cache_name").into_owned())?;
        let target = offline.join(name);
        tokio::fs::copy(&cached, &target)
            .await
            .map_err(|error| error.to_string())?;
        return Ok(target);
    }
    audio_into(id, offline, Quality::Normal, None).await
}

/// Downloads the audio of `id` into `dir` unless it is already there.
async fn audio_into(
    id: String,
    dir: PathBuf,
    quality: Quality,
    cookie: Option<&str>,
) -> Result<PathBuf, String> {
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(path) = find_cached(&dir, &id) {
        return Ok(path);
    }

    let cookies = cookie.map(write_cookies).transpose()?;
    let output = crate::platform::command("yt-dlp")
        .args(ytdlp_args(&id, &dir, quality, cookies.as_deref()))
        .output()
        .await;
    if let Some(cookies) = &cookies {
        // yt-dlp writes the cookies back when it ends; the session must not linger on disk.
        let _ = std::fs::remove_file(cookies);
    }
    let output =
        output.map_err(|error| t!("download.ytdlp_not_run", error = error).into_owned())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .last()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .ok_or_else(|| t!("download.file_not_reported").into_owned())
}

/// Downloads a small file, such as a cover image.
///
/// Images are kept here once downloaded, so they show at once on the next start.
pub async fn bytes(url: String) -> Result<Vec<u8>, String> {
    let cached = image_cache_path(&url);
    if let Ok(bytes) = tokio::fs::read(&cached).await {
        return Ok(bytes);
    }

    let response = reqwest::get(&url)
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| error.to_string())?;
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;

    // A failed write only means the image is downloaded again next time.
    if let Some(dir) = cached.parent()
        && tokio::fs::create_dir_all(dir).await.is_ok()
    {
        let _ = tokio::fs::write(&cached, &bytes).await;
    }
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_yt_dlp_for_audio_only() {
        let args = ytdlp_args("abc", Path::new("/cache"), Quality::Normal, None);
        assert!(
            args.iter()
                .any(|arg| arg.starts_with("bestaudio[ext=webm]"))
        );
        let template = Path::new("/cache").join("%(id)s.%(ext)s");
        assert!(args.contains(&template.to_string_lossy().into_owned()));
        assert!(!args.contains(&"--cookies".to_string()));
        assert_eq!(
            args.last().map(String::as_str),
            Some("https://music.youtube.com/watch?v=abc")
        );
        let low = ytdlp_args("abc", Path::new("/cache"), Quality::Low, None);
        assert!(low.contains(&"bestaudio[ext=webm][abr<=80]/worstaudio".to_string()));
        // With the session Premium's ~256 kbit/s is there too; Normal does not take it.
        let normal = ytdlp_args(
            "abc",
            Path::new("/c"),
            Quality::Normal,
            Some(Path::new("/s")),
        );
        assert!(
            normal
                .iter()
                .any(|arg| arg.starts_with("bestaudio[ext=webm][abr<=170]"))
        );
    }

    #[test]
    fn downloads_carry_the_session() {
        let args = ytdlp_args(
            "abc",
            Path::new("/c"),
            Quality::High,
            Some(Path::new("/s.txt")),
        );
        let at = args
            .iter()
            .position(|arg| arg == "--cookies")
            .expect("--cookies");
        assert_eq!(args[at + 1], "/s.txt");
        let signed_out = Source {
            quality: Quality::High,
            cookie: None,
        };
        assert_eq!(signed_out.quality(), Quality::Normal);
    }

    #[test]
    fn writes_the_session_as_a_cookies_file() {
        let file = cookies_txt("SID=a=b; HSID=c;broken; =x", 99);
        assert_eq!(
            file,
            "# Netscape HTTP Cookie File\n\
             .youtube.com\tTRUE\t/\tTRUE\t99\tSID\ta=b\n\
             .youtube.com\tTRUE\t/\tTRUE\t99\tHSID\tc\n"
        );
    }

    #[test]
    fn settings_name_the_quality_in_lowercase() {
        let json = serde_json::to_string(&Quality::High).expect("json");
        assert_eq!(json, r#""high""#);
    }

    #[test]
    fn finds_only_finished_downloads() {
        let dir = std::env::temp_dir().join(format!("encore-cache-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(dir.join("abc.webm.part"), b"partial").expect("part file");
        assert_eq!(find_cached(&dir, "abc"), None);
        std::fs::write(dir.join("abc.webm"), b"done").expect("final file");
        assert_eq!(find_cached(&dir, "abc"), Some(dir.join("abc.webm")));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// Downloads the bitrate of a song in `source`'s quality into a fresh folder, measured
    /// like the player's badge.
    async fn kbps_of(id: &str, source: &Source) -> u32 {
        let dir = std::env::temp_dir().join(format!("encore-quality-{}", std::process::id()));
        let cookie = source.cookie.as_deref();
        let path = audio_into(id.to_string(), dir.clone(), source.quality(), cookie)
            .await
            .expect("download");
        let bytes = std::fs::metadata(&path).expect("file").len();
        let seconds = crate::youtube::client()
            .expect("client")
            .query()
            .music_details(id)
            .await
            .ok()
            .and_then(|details| details.track.duration)
            .expect("duration");
        let _ = std::fs::remove_dir_all(dir);
        (bytes as f64 * 8.0 / f64::from(seconds) / 1000.0).round() as u32
    }

    /// Downloads a song in Low, signed out:
    /// `cargo test -- --ignored downloads_low_quality`.
    #[tokio::test]
    #[ignore = "downloads from YouTube"]
    async fn downloads_low_quality() {
        let kbps = kbps_of("lYBUbBu4W08", &Source::default()).await;
        let low = Source {
            quality: Quality::Low,
            cookie: None,
        };
        let low_kbps = kbps_of("lYBUbBu4W08", &low).await;
        assert!(
            low_kbps < 90 && kbps > 100,
            "low {low_kbps} kbit/s, normal {kbps} kbit/s"
        );
    }

    /// Downloads a song in High with the saved session. It uses the account, so run it only
    /// with the user's go-ahead: `cargo test -- --ignored downloads_high_quality`.
    #[tokio::test]
    #[ignore = "downloads with the signed-in session"]
    async fn downloads_high_quality() {
        let cookie = crate::youtube::client()
            .expect("client")
            .user_auth_cookie_header();
        assert!(cookie.is_some(), "not signed in");
        let high = Source {
            quality: Quality::High,
            cookie,
        };
        let kbps = kbps_of("lYBUbBu4W08", &high).await;
        let leftovers = std::fs::read_dir(crate::account::data_dir().join("yt-dlp"))
            .map(|dir| dir.count())
            .unwrap_or(0);
        assert_eq!(leftovers, 0, "a cookies file stayed behind ({kbps} kbit/s)");
        assert!(kbps > 200, "{kbps} kbit/s: is the account Premium?");
    }

    /// Copies a cached song into the offline folder and removes it again:
    /// `cargo test -- --ignored keeps_a_cached_song_offline`.
    #[tokio::test]
    #[ignore = "needs a song in the cache"]
    async fn keeps_a_cached_song_offline() {
        let cached = std::fs::read_dir(cache_dir().join("audio"))
            .expect("cache dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "webm"))
            .expect("a cached song");
        let id = cached
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("id")
            .to_string();
        let already_offline = find_cached(&offline_dir(), &id).is_some();

        let path = keep_offline(id.clone()).await.expect("keep offline");
        assert!(path.starts_with(offline_dir()));
        assert_eq!(
            audio(id, Source::default()).await.expect("audio"),
            path,
            "offline copy is not preferred"
        );
        if !already_offline {
            std::fs::remove_file(path).expect("cleanup");
        }
    }
}
