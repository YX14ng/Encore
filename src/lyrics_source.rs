//! Finding lyrics for a song, from YouTube Music alone: the synced lyrics its phone app shows
//! (line by line, mostly from Musixmatch) when it has them, else the plain lyrics of its Lyrics
//! tab. A disk cache keeps each song's lyrics, so each one is looked up once.
//!
//! rustypipe reads the plain lyrics but has no phone app client, so the synced ones are asked
//! with the same `browse` request the YouTube Music app (`ANDROID_MUSIC`) makes, without the
//! session: lyrics are the same for everyone.

use std::path::PathBuf;

use rustypipe::client::RustyPipe;
use serde_json::{Value, json};

use crate::download;
use crate::lyrics::{LyricLine, Lyrics};
use crate::youtube::SearchTrack;

const BROWSE_URL: &str = "https://music.youtube.com/youtubei/v1/browse?prettyPrint=false";
/// The YouTube Music phone app that the synced lyrics are asked as. It only names the client;
/// if YouTube stops answering this version, songs fall back to plain lyrics.
const APP_CLIENT: &str = "ANDROID_MUSIC";
const APP_VERSION: &str = "7.21.50";

/// Finds lyrics for `track`. Never fails: the worst case is `Lyrics::Missing`.
pub async fn fetch(track: SearchTrack, rp: Option<RustyPipe>) -> Lyrics {
    let cached = cache_path(&track.id);
    if let Some(lyrics) = std::fs::read_to_string(&cached)
        .ok()
        .and_then(|json| serde_json::from_str::<Lyrics>(&json).ok())
    {
        return lyrics;
    }

    let found = match rp {
        Some(rp) => from_youtube(&rp, &track.id).await,
        None => None,
    };
    let lyrics = found.unwrap_or(Lyrics::Missing);

    // Missing lyrics are not cached: a request may have failed, and a later try may find them.
    if lyrics != Lyrics::Missing
        && let Some(dir) = cached.parent()
        && std::fs::create_dir_all(dir).is_ok()
        && let Ok(json) = serde_json::to_string(&lyrics)
    {
        let _ = std::fs::write(&cached, json);
    }
    lyrics
}

/// The cache file of a song's lyrics. The ".ytm" keeps lyrics found on LRCLIB by older versions
/// from being shown.
fn cache_path(video_id: &str) -> PathBuf {
    download::cache_dir()
        .join("lyrics")
        .join(format!("{video_id}.ytm.json"))
}

/// The song's synced lyrics, else its plain ones. A song without a Lyrics tab has none.
async fn from_youtube(rp: &RustyPipe, video_id: &str) -> Option<Lyrics> {
    let q = rp.query();
    let lyrics_id = q.music_details(video_id).await.ok()?.lyrics_id?;
    if let Some(lines) = synced(&lyrics_id).await {
        return Some(Lyrics::Synced(lines));
    }
    let lyrics = q.music_lyrics(lyrics_id).await.ok()?;
    Some(Lyrics::Plain(
        lyrics.body.lines().map(str::to_string).collect(),
    ))
}

/// The synced lines behind `lyrics_id` (an "MPLYt…" id), as the phone app gets them.
async fn synced(lyrics_id: &str) -> Option<Vec<LyricLine>> {
    let body = json!({
        "browseId": lyrics_id,
        "context": { "client": {
            "clientName": APP_CLIENT,
            "clientVersion": APP_VERSION,
            "androidSdkVersion": 30,
            "hl": "en",
        } },
    });
    let answer: Value = reqwest::Client::new()
        .post(BROWSE_URL)
        .json(&body)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    let lines = timed_lines(&answer);
    (!lines.is_empty()).then_some(lines)
}

/// The lines of a `timedLyricsModel` answer, each with when it starts.
fn timed_lines(answer: &Value) -> Vec<LyricLine> {
    let Some(Value::Array(items)) = find(answer, "timedLyricsData") else {
        return Vec::new();
    };
    let mut lines: Vec<LyricLine> = items
        .iter()
        .filter_map(|item| {
            let text = item.get("lyricLine")?.as_str()?.trim().to_string();
            let start = item.pointer("/cueRange/startTimeMilliseconds")?;
            // YouTube writes the times as text.
            let start_ms = match start {
                Value::String(text) => text.parse().ok()?,
                other => u32::try_from(other.as_u64()?).ok()?,
            };
            Some(LyricLine { start_ms, text })
        })
        .collect();
    lines.sort_by_key(|line| line.start_ms);
    lines
}

/// The first value under `key`, anywhere in `value`.
fn find<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map
            .get(key)
            .or_else(|| map.values().find_map(|inner| find(inner, key))),
        Value::Array(items) => items.iter().find_map(|inner| find(inner, key)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_apps_timed_lyrics() {
        let answer = json!({ "contents": { "elementRenderer": { "newElement": { "type": {
            "componentType": { "model": { "timedLyricsModel": { "lyricsData": {
                "timedLyricsData": [
                    { "lyricLine": "We're no strangers to love",
                      "cueRange": { "startTimeMilliseconds": "18870",
                                    "endTimeMilliseconds": "22850" } },
                    { "lyricLine": "\u{266a}",
                      "cueRange": { "startTimeMilliseconds": "0",
                                    "endTimeMilliseconds": "18870" } },
                    { "lyricLine": "broken" },
                ],
                "sourceMessage": "Source: Musixmatch",
            } } } } } } } } });
        let lines = timed_lines(&answer);
        let read: Vec<(u32, &str)> = lines
            .iter()
            .map(|line| (line.start_ms, line.text.as_str()))
            .collect();
        assert_eq!(
            read,
            [(0, "\u{266a}"), (18_870, "We're no strangers to love")]
        );
        assert!(timed_lines(&json!({ "contents": {} })).is_empty());
    }

    /// Asks YouTube Music for a well-known song's lyrics, signed out:
    /// `cargo test -- --ignored finds_synced_lyrics`.
    #[tokio::test]
    #[ignore = "asks YouTube Music"]
    async fn finds_synced_lyrics() {
        let rp = RustyPipe::builder().no_storage().build().expect("client");
        // Rick Astley's "Never Gonna Give You Up", the song.
        let lyrics = from_youtube(&rp, "lYBUbBu4W08").await;
        let lines = lyrics.as_ref().map_or(0, |lyrics| lyrics.synced().len());
        assert!(lines > 20, "{lyrics:?}");
    }
}
