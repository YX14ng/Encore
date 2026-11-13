//! YouTube Music: searching the catalog with rustypipe and downloading audio with yt-dlp.
//!
//! rustypipe's own stream download no longer works (its signature deobfuscation is out of date),
//! so audio goes through yt-dlp, which keeps up with YouTube's changes.

use std::path::{Path, PathBuf};

use rustypipe::client::RustyPipe;
use rustypipe::model::TrackItem;
use tokio::process::Command;

/// The widest cover worth downloading for the player card and the blurred background.
const COVER_MAX_WIDTH: u32 = 544;

/// Audio formats yt-dlp may leave in the cache, in order of preference.
const AUDIO_EXTENSIONS: [&str; 4] = ["webm", "opus", "m4a", "mp4"];

/// A track found by a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u32,
    pub cover_url: Option<String>,
}

impl SearchTrack {
    fn from_item(item: &TrackItem) -> Self {
        let artist = item
            .artists
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        Self {
            id: item.id.clone(),
            title: item.name.clone(),
            artist,
            album: item
                .album
                .as_ref()
                .map(|album| album.name.clone())
                .unwrap_or_default(),
            duration: item.duration.unwrap_or(0),
            cover_url: pick_cover(
                item.cover
                    .iter()
                    .map(|thumbnail| (thumbnail.width, thumbnail.url.as_str())),
                COVER_MAX_WIDTH,
            ),
        }
    }
}

/// Where Encore keeps its cache: `$XDG_CACHE_HOME/encore`, or `~/.cache/encore`.
pub fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("encore")
}

/// A rustypipe client that keeps its data in Encore's cache.
pub fn client() -> Result<RustyPipe, String> {
    let storage = cache_dir().join("rustypipe");
    std::fs::create_dir_all(&storage).map_err(|error| error.to_string())?;
    RustyPipe::builder()
        .storage_dir(storage)
        .build()
        .map_err(|error| error.to_string())
}

/// Searches YouTube Music for songs.
pub async fn search(rp: RustyPipe, query: String) -> Result<Vec<SearchTrack>, String> {
    let result = rp
        .query()
        .music_search_tracks(&query)
        .await
        .map_err(|error| error.to_string())?;
    Ok(result
        .items
        .items
        .iter()
        .map(SearchTrack::from_item)
        .collect())
}

/// From `(width, url)` pairs, the largest cover that is at most `max_width` wide, or the
/// smallest one if all are wider.
pub fn pick_cover<'a>(
    covers: impl Iterator<Item = (u32, &'a str)> + Clone,
    max_width: u32,
) -> Option<String> {
    let fitting = covers
        .clone()
        .filter(|(width, _)| *width <= max_width)
        .max_by_key(|(width, _)| *width);
    fitting
        .or_else(|| covers.min_by_key(|(width, _)| *width))
        .map(|(_, url)| url.to_string())
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
pub async fn download_audio(id: String) -> Result<PathBuf, String> {
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
pub async fn fetch_bytes(url: String) -> Result<Vec<u8>, String> {
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
    fn picks_the_largest_cover_that_fits() {
        let covers = [(60, "a"), (226, "b"), (544, "c"), (1200, "d")];
        assert_eq!(pick_cover(covers.into_iter(), 544).as_deref(), Some("c"));
    }

    #[test]
    fn falls_back_to_the_smallest_cover() {
        let covers = [(1200, "big"), (800, "smaller")];
        assert_eq!(
            pick_cover(covers.into_iter(), 544).as_deref(),
            Some("smaller")
        );
        assert_eq!(pick_cover(std::iter::empty(), 544), None);
    }

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

    /// Hits the network and decodes audio silently: `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore = "needs the network, yt-dlp and libmpv"]
    async fn searches_downloads_and_plays() {
        let rp = client().expect("client");
        let results = search(rp, "Daft Punk Get Lucky".to_string())
            .await
            .expect("search");
        let first = results.first().expect("at least one result");
        let path = download_audio(first.id.clone()).await.expect("download");
        let audio = crate::audio::Audio::muted().expect("mpv");
        audio.play(&path).expect("play");
        std::thread::sleep(std::time::Duration::from_secs(3));
        assert!(
            audio.position().unwrap_or(0.0) > 0.5,
            "audio did not advance"
        );
    }
}
