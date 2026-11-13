//! Downloads: song audio through yt-dlp, and small files like covers over HTTP.
//!
//! rustypipe's own stream download no longer works (its signature deobfuscation is out of date)
//! and YouTube refuses to stream its audio URLs straight into mpv, so every song is downloaded
//! with yt-dlp first and then played from the cache.

use std::path::{Path, PathBuf};

use tokio::process::Command;

/// Audio formats yt-dlp may leave in the cache, in order of preference.
const AUDIO_EXTENSIONS: [&str; 4] = ["webm", "opus", "m4a", "mp4"];

/// Where Encore keeps its cache: `$XDG_CACHE_HOME/encore`, or `~/.cache/encore`.
pub fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("encore")
}

/// A finished download of `id` in `dir`, if there is one.
pub fn find_cached(dir: &Path, id: &str) -> Option<PathBuf> {
    AUDIO_EXTENSIONS
        .iter()
        .map(|extension| dir.join(format!("{id}.{extension}")))
        .find(|path| path.exists())
}

/// The yt-dlp arguments that download the best audio of `id` into `dir` and print the file path.
pub fn ytdlp_args(id: &str, dir: &Path) -> Vec<String> {
    let template = dir.join("%(id)s.%(ext)s");
    vec![
        "--no-config".to_string(),
        "--no-playlist".to_string(),
        "--quiet".to_string(),
        "--no-warnings".to_string(),
        "-f".to_string(),
        "bestaudio[ext=webm]/bestaudio".to_string(),
        "-o".to_string(),
        template.to_string_lossy().into_owned(),
        "--print".to_string(),
        "after_move:filepath".to_string(),
        format!("https://music.youtube.com/watch?v={id}"),
    ]
}

/// Where downloaded (offline) songs live: kept until the user removes them.
pub fn offline_dir() -> PathBuf {
    crate::account::data_dir().join("offline")
}

/// Returns the audio of `id`: the offline copy if there is one, else the cache, else a new
/// download into the cache.
pub async fn audio(id: String) -> Result<PathBuf, String> {
    if let Some(path) = find_cached(&offline_dir(), &id) {
        return Ok(path);
    }
    audio_into(id, cache_dir().join("audio")).await
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
    if let Some(cached) = find_cached(&cache_dir().join("audio"), &id) {
        let name = cached.file_name().ok_or("bad cache file name")?;
        let target = offline.join(name);
        tokio::fs::copy(&cached, &target)
            .await
            .map_err(|error| error.to_string())?;
        return Ok(target);
    }
    audio_into(id, offline).await
}

/// Downloads the audio of `id` into `dir` unless it is already there.
async fn audio_into(id: String, dir: PathBuf) -> Result<PathBuf, String> {
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(path) = find_cached(&dir, &id) {
        return Ok(path);
    }

    let output = Command::new("yt-dlp")
        .args(ytdlp_args(&id, &dir))
        .output()
        .await
        .map_err(|error| format!("could not run yt-dlp: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .last()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .ok_or_else(|| "yt-dlp did not report the downloaded file".to_string())
}

/// Downloads a small file, such as a cover image.
///
/// Images are kept here once downloaded, so they show at once on the next start.
pub async fn bytes(url: String) -> Result<Vec<u8>, String> {
    let cached = cache_dir()
        .join("images")
        .join(crate::offline::image_file_name(&url));
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
        let args = ytdlp_args("abc", Path::new("/cache"));
        assert!(args.contains(&"bestaudio[ext=webm]/bestaudio".to_string()));
        assert!(args.contains(&"/cache/%(id)s.%(ext)s".to_string()));
        assert_eq!(
            args.last().map(String::as_str),
            Some("https://music.youtube.com/watch?v=abc")
        );
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
            audio(id).await.expect("audio"),
            path,
            "offline copy is not preferred"
        );
        if !already_offline {
            std::fs::remove_file(path).expect("cleanup");
        }
    }
}
