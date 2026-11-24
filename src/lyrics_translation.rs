//! Lyrics translated into the language of the interface, small under each line, like YouTube
//! Music's Translate button. YouTube Music translates them itself (with Google Translate): a
//! recent version of its phone app gets, with a song's lyrics, a token that asks for them in the
//! language of the request, one translation per line with words. There is no token for a song
//! already in that language. Each song's translation is kept on disk, per language.

use std::path::PathBuf;

use iced::Task;
use rustypipe::client::RustyPipe;
use serde_json::{Value, json};

use crate::lyrics::Lyrics;
use crate::lyrics_source::{app_browse, cache_dir, find};
use crate::{App, Message, i18n};

/// A version of the phone app that is offered translations (older ones are not). If YouTube
/// stops answering it, songs simply show no translation.
const APP_VERSION: &str = "8.44.54";

/// A song's lyrics translated into a language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    pub video_id: String,
    /// YouTube's code ("hl") for the language.
    pub language: &'static str,
    /// A translation per line with words, in order; none when the song has no translation.
    pub lines: Vec<String>,
}

/// The lyrics of song `video_id` translated into `language` (YouTube's code), an empty list if
/// YouTube Music offers no translation, or `None` if asking failed.
pub async fn fetch(
    video_id: String,
    language: &'static str,
    rp: Option<RustyPipe>,
) -> Option<Vec<String>> {
    let cached = cache_path(&video_id, language);
    if let Some(lines) = std::fs::read_to_string(&cached)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
    {
        return Some(lines);
    }
    // Offline, only a translation kept on disk shows.
    let lines = from_youtube(&rp?, &video_id, language).await?;
    if let Some(dir) = cached.parent()
        && std::fs::create_dir_all(dir).is_ok()
        && let Ok(json) = serde_json::to_string(&lines)
    {
        let _ = std::fs::write(&cached, json);
    }
    Some(lines)
}

fn cache_path(video_id: &str, language: &str) -> PathBuf {
    cache_dir().join(format!("{video_id}.{language}.translation.json"))
}

async fn from_youtube(rp: &RustyPipe, video_id: &str, language: &str) -> Option<Vec<String>> {
    let Some(lyrics_id) = rp.query().music_details(video_id).await.ok()?.lyrics_id else {
        return Some(Vec::new());
    };
    let lyrics = app_browse(json!({ "browseId": lyrics_id }), APP_VERSION, language).await?;
    let Some(token) = find(&lyrics, "translationContinuationToken").and_then(Value::as_str) else {
        return Some(Vec::new());
    };
    let answer = app_browse(json!({ "continuation": token }), APP_VERSION, language).await?;
    // A token with no translations behind it is not kept: a later try may get them.
    let lines = translated_lines(&answer);
    (!lines.is_empty()).then_some(lines)
}

/// The lines of a `musicLyricsContinuation` answer.
fn translated_lines(answer: &Value) -> Vec<String> {
    let Some(Value::Array(items)) = find(answer, "lyricsTranslations") else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| {
            item.get("translatedLyricText")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .collect()
}

/// `translation` paired with the lines of `lyrics`: one per line, empty for lines without words
/// ("♪", blank ones) and lines that translate to themselves. `None` if the counts do not match,
/// so no line ever shows another's translation.
pub fn aligned(lyrics: &Lyrics, translation: &[String]) -> Option<Vec<String>> {
    if translation.is_empty() {
        return None;
    }
    let mut next = translation.iter();
    let lines = lyrics
        .texts()?
        .into_iter()
        .map(|text| {
            if !text.chars().any(char::is_alphanumeric) {
                return Some(String::new());
            }
            let translated = next.next()?;
            Some(if translated.as_str() == text.trim() {
                String::new()
            } else {
                translated.clone()
            })
        })
        .collect::<Option<Vec<String>>>()?;
    next.next().is_none().then_some(lines)
}

impl App {
    /// Asks for the translation of song `video_id` into the language of the interface, when the
    /// plugin is on and it is not there yet.
    pub(crate) fn fetch_translation(&self, video_id: &str) -> Task<Message> {
        let language = i18n::youtube_code();
        let have = self
            .translation
            .as_ref()
            .is_some_and(|had| had.video_id == video_id && had.language == language);
        if !self.plugins.translate || have {
            return Task::none();
        }
        let rp = self.online.client();
        let video_id = video_id.to_string();
        Task::perform(fetch(video_id.clone(), language, rp), move |lines| {
            Message::TranslationLoaded(Translation {
                video_id: video_id.clone(),
                language,
                lines: lines.unwrap_or_default(),
            })
        })
    }

    /// The translation of the song playing, after the language or the plugin changed.
    pub(crate) fn retranslate(&mut self) -> Task<Message> {
        self.translate_lyrics();
        match self.online.playing_track() {
            Some(track) => self.fetch_translation(&track.id.clone()),
            None => Task::none(),
        }
    }

    pub(crate) fn translation_loaded(&mut self, translation: Translation) {
        let playing = self
            .online
            .playing_track()
            .is_some_and(|track| track.id == translation.video_id);
        if playing && translation.language == i18n::youtube_code() {
            self.translation = Some(translation);
            self.translate_lyrics();
        }
    }

    /// Pairs the translation with the lyrics showing, or drops it when the plugin is off.
    pub(crate) fn translate_lyrics(&mut self) {
        self.translated = self
            .translation
            .as_ref()
            .filter(|translation| {
                self.plugins.translate && translation.language == i18n::youtube_code()
            })
            .and_then(|translation| aligned(&self.lyrics, &translation.lines));
    }

    /// A new song: its translation is not known yet.
    pub(crate) fn forget_translation(&mut self) {
        self.translation = None;
        self.translated = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::LyricLine;

    fn strings(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|text| text.to_string()).collect()
    }

    #[test]
    fn reads_the_translated_lines() {
        let answer = json!({ "continuationContents": { "musicLyricsContinuation": {
            "lyricsTranslations": [
                { "translatedLyricText": "We're no strangers to love " },
                { "translatedLyricText": "You know the rules" },
            ],
        } } });
        assert_eq!(
            translated_lines(&answer),
            ["We're no strangers to love", "You know the rules"]
        );
        assert!(translated_lines(&json!({})).is_empty());
    }

    #[test]
    fn lines_without_words_get_no_translation() {
        let lyrics = Lyrics::Synced(
            ["\u{266a}", "正しさとは", "Oh yeah"]
                .iter()
                .enumerate()
                .map(|(index, text)| LyricLine {
                    start_ms: index as u32,
                    text: text.to_string(),
                })
                .collect(),
        );
        assert_eq!(
            aligned(&lyrics, &strings(&["What is right", "Oh yeah"])),
            Some(strings(&["", "What is right", ""]))
        );
        let plain = Lyrics::Plain(strings(&["正しさとは", "", "愚かさとは"]));
        assert_eq!(
            aligned(&plain, &strings(&["Right", "Foolish"])),
            Some(strings(&["Right", "", "Foolish"]))
        );
    }

    #[test]
    fn a_translation_that_does_not_match_is_not_shown() {
        let plain = Lyrics::Plain(strings(&["一", "二"]));
        assert_eq!(aligned(&plain, &strings(&["One"])), None, "too few");
        assert_eq!(
            aligned(&plain, &strings(&["1", "2", "3"])),
            None,
            "too many"
        );
        assert_eq!(aligned(&plain, &[]), None);
        assert_eq!(aligned(&Lyrics::Missing, &strings(&["One"])), None);
    }

    /// Asks YouTube Music for the translations of a song with plain lyrics and one with synced
    /// lyrics, signed out, and pairs them with those lyrics:
    /// `cargo test -- --ignored translates_lyrics`.
    #[tokio::test]
    #[ignore = "asks YouTube Music"]
    async fn translates_lyrics() {
        let rp = RustyPipe::builder().no_storage().build().expect("client");
        // Ado's "Usseewa" and Rick Astley's "Never Gonna Give You Up".
        for video_id in ["Qp3b-RXtz4w", "lYBUbBu4W08"] {
            let lyrics = crate::lyrics_source::from_youtube(&rp, video_id)
                .await
                .expect("lyrics");
            let lines = from_youtube(&rp, video_id, "es-419")
                .await
                .expect("translation");
            assert!(aligned(&lyrics, &lines).is_some(), "{video_id}: {lines:?}");
        }
    }
}
