//! Finding lyrics for a song: LRCLIB (free, community-made synced lyrics) first, then the plain
//! lyrics YouTube Music shows, with a disk cache so each song is looked up once.

use std::path::PathBuf;

use rustypipe::client::RustyPipe;
use serde_json::Value;

use crate::download;
use crate::lyrics::{self, Lyrics};
use crate::youtube::SearchTrack;

/// LRCLIB asks clients to say who they are.
const USER_AGENT: &str = "Encore (desktop YouTube Music player)";
/// How far a result's length may be from the song's and still count as the same recording.
const DURATION_TOLERANCE: f64 = 4.0;

/// One answer from LRCLIB.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub duration: f64,
    pub instrumental: bool,
    pub synced: Option<String>,
    pub plain: Option<String>,
}

impl Entry {
    fn from_json(value: &Value) -> Option<Self> {
        let text = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty())
                .map(str::to_string)
        };
        Some(Self {
            duration: value.get("duration").and_then(Value::as_f64)?,
            instrumental: value
                .get("instrumental")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            synced: text("syncedLyrics"),
            plain: text("plainLyrics"),
        })
    }

    fn lyrics(&self) -> Option<Lyrics> {
        if self.instrumental {
            return Some(Lyrics::Instrumental);
        }
        if let Some(synced) = &self.synced {
            let lines = lyrics::parse_lrc(synced);
            if !lines.is_empty() {
                return Some(Lyrics::Synced(lines));
            }
        }
        self.plain.as_deref().map(plain_lines)
    }
}

/// Finds lyrics for `track`. Never fails: the worst case is `Lyrics::Missing`.
pub async fn fetch(track: SearchTrack, rp: Option<RustyPipe>) -> Lyrics {
    let cached = cache_path(&track.id);
    if let Some(lyrics) = std::fs::read_to_string(&cached)
        .ok()
        .and_then(|json| serde_json::from_str::<Lyrics>(&json).ok())
    {
        return lyrics;
    }

    let mut found = from_lrclib(&track).await;
    if found.is_none()
        && let Some(rp) = rp
    {
        found = from_youtube(&rp, &track.id).await;
    }
    let lyrics = found.unwrap_or(Lyrics::Missing);

    // Missing lyrics are not cached: LRCLIB grows, and a later try may find them.
    if lyrics != Lyrics::Missing
        && let Some(dir) = cached.parent()
        && std::fs::create_dir_all(dir).is_ok()
        && let Ok(json) = serde_json::to_string(&lyrics)
    {
        let _ = std::fs::write(&cached, json);
    }
    lyrics
}

fn cache_path(video_id: &str) -> PathBuf {
    download::cache_dir()
        .join("lyrics")
        .join(format!("{video_id}.json"))
}

async fn from_lrclib(track: &SearchTrack) -> Option<Lyrics> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .ok()?;
    let artist = first_artist(&track.artist);

    for title in title_variants(&track.title) {
        // The exact lookup needs the length; without it, go straight to searching.
        if track.duration > 0 {
            let duration = track.duration.to_string();
            let mut query = vec![
                ("track_name", title.as_str()),
                ("artist_name", artist),
                ("duration", duration.as_str()),
            ];
            if !track.album.is_empty() {
                query.push(("album_name", track.album.as_str()));
            }
            if let Some(value) = get_json(&client, "https://lrclib.net/api/get", &query).await
                && let Some(lyrics) = Entry::from_json(&value).and_then(|entry| entry.lyrics())
            {
                return Some(lyrics);
            }
        }

        let query = [("track_name", title.as_str()), ("artist_name", artist)];
        if let Some(Value::Array(results)) =
            get_json(&client, "https://lrclib.net/api/search", &query).await
        {
            let entries: Vec<Entry> = results.iter().filter_map(Entry::from_json).collect();
            if let Some(lyrics) = best_match(&entries, track.duration).and_then(Entry::lyrics) {
                return Some(lyrics);
            }
        }
    }
    None
}

async fn get_json(client: &reqwest::Client, url: &str, query: &[(&str, &str)]) -> Option<Value> {
    let response = client.get(url).query(query).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json().await.ok()
}

async fn from_youtube(rp: &RustyPipe, video_id: &str) -> Option<Lyrics> {
    let q = rp.query();
    let details = q.music_details(video_id).await.ok()?;
    let lyrics = q.music_lyrics(details.lyrics_id?).await.ok()?;
    Some(plain_lines(&lyrics.body))
}

fn plain_lines(text: &str) -> Lyrics {
    Lyrics::Plain(text.lines().map(str::to_string).collect())
}

/// The first credited artist: LRCLIB lists one, YouTube Music joins them with commas.
pub fn first_artist(artists: &str) -> &str {
    artists.split(',').next().unwrap_or(artists).trim()
}

/// The ways a YouTube Music title might be written on LRCLIB: as is, without "(feat. …)" or
/// "(Remaster)", and each side of "Original - Romanized" titles like "うっせぇわ - Usseewa".
pub fn title_variants(title: &str) -> Vec<String> {
    let mut variants = vec![title.trim().to_string()];
    let without_brackets = strip_brackets(title);
    variants.push(without_brackets.clone());
    if let Some((left, right)) = without_brackets.split_once(" - ") {
        variants.push(left.trim().to_string());
        variants.push(right.trim().to_string());
    }
    variants.retain(|variant| !variant.is_empty());
    variants.dedup();
    let mut seen = std::collections::HashSet::new();
    variants.retain(|variant| seen.insert(variant.clone()));
    variants
}

fn strip_brackets(title: &str) -> String {
    let mut out = String::new();
    let mut depth = 0_u32;
    for character in title.chars() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(character),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Among search results, the one with words whose length is closest to the song's. With an
/// unknown length (0), the first result with synced lyrics, else the first with any words.
pub fn best_match(entries: &[Entry], duration: u32) -> Option<&Entry> {
    let usable = entries
        .iter()
        .filter(|entry| entry.instrumental || entry.synced.is_some() || entry.plain.is_some());
    if duration == 0 {
        let mut usable: Vec<&Entry> = usable.collect();
        usable.sort_by_key(|entry| entry.synced.is_none());
        return usable.first().copied();
    }
    usable
        .filter(|entry| (entry.duration - f64::from(duration)).abs() <= DURATION_TOLERANCE)
        .min_by(|a, b| {
            let distance = |entry: &Entry| (entry.duration - f64::from(duration)).abs();
            (a.synced.is_none(), distance(a))
                .partial_cmp(&(b.synced.is_none(), distance(b)))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(duration: f64, synced: bool) -> Entry {
        Entry {
            duration,
            instrumental: false,
            synced: synced.then(|| "[00:01.00] la".to_string()),
            plain: Some("la".to_string()),
        }
    }

    #[test]
    fn title_variants_cover_romanized_and_featured_titles() {
        assert_eq!(
            title_variants("うっせぇわ - Usseewa"),
            vec!["うっせぇわ - Usseewa", "うっせぇわ", "Usseewa"]
        );
        assert_eq!(
            title_variants("Get Lucky (feat. Pharrell Williams)"),
            vec!["Get Lucky (feat. Pharrell Williams)", "Get Lucky"]
        );
        assert_eq!(title_variants("Odo"), vec!["Odo"]);
    }

    #[test]
    fn uses_the_first_artist() {
        assert_eq!(first_artist("Daft Punk, Pharrell Williams"), "Daft Punk");
        assert_eq!(first_artist("Ado"), "Ado");
    }

    #[test]
    fn prefers_synced_results_of_the_right_length() {
        let entries = [entry(300.0, true), entry(249.0, false), entry(247.0, true)];
        assert_eq!(best_match(&entries, 248), Some(&entries[2]));
        assert_eq!(best_match(&[entry(300.0, true)], 248), None);
        assert_eq!(best_match(&entries, 0), Some(&entries[0]));
    }

    #[test]
    fn reads_an_lrclib_answer() {
        let value: Value = serde_json::from_str(
            r#"{"duration":248.0,"instrumental":false,"plainLyrics":"a\nb",
                "syncedLyrics":"[00:30.64] a\n[00:35.11] b"}"#,
        )
        .expect("json");
        let lyrics = Entry::from_json(&value).and_then(|entry| entry.lyrics());
        assert_eq!(lyrics.map(|lyrics| lyrics.synced().len()), Some(2));
    }

    /// Asks LRCLIB for a well-known song: `cargo test -- --ignored finds_synced_lyrics`.
    #[tokio::test]
    #[ignore = "needs the network"]
    async fn finds_synced_lyrics() {
        let track = SearchTrack {
            id: "test-get-lucky".to_string(),
            title: "Get Lucky (Radio Edit - feat. Pharrell Williams and Nile Rodgers)".to_string(),
            artist: "Daft Punk, Pharrell Williams, Nile Rodgers".to_string(),
            album: String::new(),
            duration: 248,
            cover_url: None,
            entry: None,
        };
        let lyrics = from_lrclib(&track).await.expect("lyrics");
        assert!(lyrics.synced().len() > 10, "got {lyrics:?}");
    }
}
