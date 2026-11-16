//! Romanized lyrics: Japanese, Korean and Chinese lines written out in the Latin alphabet
//! (romaji, Revised Romanization, pinyin), shown small under each line of the lyrics.
//!
//! A song is taken as Japanese if any line has kana, else Korean if any has Hangul, else Chinese
//! if any has Chinese characters; a song in the language of the interface is left alone, since
//! whoever reads that language does not need it.

mod chinese;
mod japanese;
mod kana;
mod korean;

use crate::lyrics::Lyrics;

/// The writing systems that are romanized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Japanese,
    Korean,
    Chinese,
}

impl Script {
    /// The script of the language of the interface `code` (a code of `i18n::LANGUAGES`).
    pub fn of_language(code: &str) -> Option<Self> {
        match code {
            "ja" => Some(Script::Japanese),
            "ko" => Some(Script::Korean),
            "zh-CN" | "zh-TW" => Some(Script::Chinese),
            _ => None,
        }
    }

    /// The script a song's `lines` are written in, if it is romanized.
    pub fn of_lines<'a>(lines: impl Iterator<Item = &'a str> + Clone) -> Option<Self> {
        let has = |test: fn(char) -> bool| lines.clone().any(|line| line.chars().any(test));
        if has(kana::is_kana) {
            Some(Script::Japanese)
        } else if has(korean::is_hangul) {
            Some(Script::Korean)
        } else if has(chinese::is_han) {
            Some(Script::Chinese)
        } else {
            None
        }
    }

    /// `line` romanized.
    pub fn romanize(self, line: &str) -> String {
        match self {
            Script::Japanese => japanese::romanize(line),
            Script::Korean => korean::romanize(line),
            Script::Chinese => chinese::romanize(line),
        }
    }
}

/// Whether `c` is punctuation, Latin or East Asian.
fn is_punctuation(c: char) -> bool {
    c.is_ascii_punctuation() || latin_punctuation(c) != c
}

/// The Latin punctuation for East Asian punctuation ("，" is ",", "」" is a quote); other
/// characters unchanged.
fn latin_punctuation(c: char) -> char {
    match c {
        // Full-width forms of ASCII.
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        '、' => ',',
        '。' => '.',
        '「' | '」' | '『' | '』' => '"',
        '〜' => '~',
        '　' => ' ',
        _ => c,
    }
}

/// The romanized lines of `lyrics`, one per line (empty for lines with nothing to romanize), or
/// `None` when the song is not in a romanized script or is in the interface's (`interface`, a
/// language code).
pub fn lines(lyrics: &Lyrics, interface: &str) -> Option<Vec<String>> {
    let texts: Vec<&str> = match lyrics {
        Lyrics::Synced(lines) => lines.iter().map(|line| line.text.as_str()).collect(),
        Lyrics::Plain(lines) => lines.iter().map(String::as_str).collect(),
        Lyrics::Loading | Lyrics::Instrumental | Lyrics::Missing => return None,
    };
    let script = Script::of_lines(texts.iter().copied())?;
    if Script::of_language(interface) == Some(script) {
        return None;
    }
    Some(
        texts
            .into_iter()
            .map(|text| {
                let romanized = script.romanize(text);
                if romanized == text {
                    String::new()
                } else {
                    romanized
                }
            })
            .collect(),
    )
}

impl crate::App {
    /// Romanizes the lyrics that are showing, or drops the romanization when the plugin is off.
    pub(crate) fn romanize_lyrics(&mut self) {
        self.romanized = if self.plugins.romanize {
            lines(&self.lyrics, &rust_i18n::locale())
        } else {
            None
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::LyricLine;

    fn synced(texts: &[&str]) -> Lyrics {
        Lyrics::Synced(
            texts
                .iter()
                .enumerate()
                .map(|(index, text)| LyricLine {
                    start_ms: index as u32 * 1000,
                    text: (*text).to_owned(),
                })
                .collect(),
        )
    }

    #[test]
    fn a_line_with_kana_makes_the_song_japanese() {
        // The first line alone would pass for Chinese.
        let lyrics = synced(&["東京", "君の名前を", "Oh yeah"]);
        assert_eq!(
            lines(&lyrics, "es"),
            Some(vec![
                "toukyou".into(),
                "kimi no namae wo".into(),
                String::new()
            ])
        );
    }

    #[test]
    fn songs_in_the_interface_language_are_left_alone() {
        let lyrics = synced(&["我们一起去看海"]);
        assert_eq!(lines(&lyrics, "zh-CN"), None);
        assert_eq!(
            lines(&lyrics, "ja"),
            Some(vec!["wǒ men yī qǐ qù kàn hǎi".into()])
        );
        assert_eq!(lines(&synced(&["Hello", "world"]), "en"), None);
        assert_eq!(lines(&Lyrics::Loading, "en"), None);
    }

    #[test]
    fn korean_is_found_by_its_hangul() {
        let lyrics = Lyrics::Plain(vec!["사랑해요".into()]);
        assert_eq!(lines(&lyrics, "en"), Some(vec!["saranghaeyo".into()]));
    }
}
