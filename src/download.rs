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

/// Downloads the audio of `id` into the cache, or returns the cached file.
pub async fn audio(id: String) -> Result<PathBuf, String> {
    let dir = cache_dir().join("audio");
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
pub async fn bytes(url: String) -> Result<Vec<u8>, String> {
    let response = reqwest::get(&url)
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| error.to_string())?;
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
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
}
