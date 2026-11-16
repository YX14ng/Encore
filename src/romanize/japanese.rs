//! Japanese to romaji. Kanji have to be read in context (君 is "kimi", not "kun"), so the line
//! is split into words with lindera and its IPADIC dictionary, which gives each word's reading
//! and part of speech; the readings are then written in romaji (`kana.rs`).
//!
//! Words are separated by spaces, with endings joined to the word they belong to ("yonde",
//! "ikanai"), and the particles は, へ and を written as they sound ("wa", "e", "wo").

use std::borrow::Cow;
use std::sync::LazyLock;

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;

use super::kana;

/// The word splitter, made the first time a Japanese line is read. `None` if the embedded
/// dictionary could not be opened; lines are then romanized kana by kana.
static SEGMENTER: LazyLock<Option<Segmenter>> = LazyLock::new(|| {
    load_dictionary("embedded://ipadic")
        .ok()
        .map(|dictionary| Segmenter::new(Mode::Normal, dictionary, None))
});

/// Words IPADIC reads one character at a time, and wrong ("ichinin"), with their reading.
const READINGS: [(&str, &str); 2] = [("一人", "ひとり"), ("二人", "ふたり")];

/// One word of a line, as IPADIC describes it.
struct Word {
    surface: String,
    /// Part of speech: 名詞, 動詞, 助詞, 助動詞, 記号…
    pos: String,
    /// The finer part of speech: 接続助詞, 接尾…
    detail: String,
    /// The reading in katakana, when the dictionary knows the word.
    reading: Option<String>,
}

/// `line` in romaji.
pub fn romanize(line: &str) -> String {
    match words(line) {
        Some(words) => join(&words),
        None => kana::to_romaji(line),
    }
}

fn words(line: &str) -> Option<Vec<Word>> {
    let segmenter = SEGMENTER.as_ref()?;
    let line = READINGS
        .iter()
        .fold(line.to_owned(), |line, (word, reading)| {
            line.replace(word, reading)
        });
    let mut tokens = segmenter.segment(Cow::Owned(line)).ok()?;
    Some(
        tokens
            .iter_mut()
            .map(|token| {
                let surface = token.surface.to_string();
                let details = token.details();
                let field = |i: usize| details.get(i).copied().unwrap_or("*").to_owned();
                let reading = Some(field(7)).filter(|reading| reading != "*");
                Word {
                    surface,
                    pos: field(0),
                    detail: field(1),
                    reading,
                }
            })
            .filter(|word| !word.surface.trim().is_empty())
            .collect(),
    )
}

/// The words in romaji, spaced. The readings of a word and its endings are joined before they
/// are written in romaji, so a ッ at the end of one part doubles the start of the next
/// (だっ + た is "datta").
fn join(words: &[Word]) -> String {
    let mut spoken: Vec<String> = Vec::new();
    let mut previous_pos = "";
    for word in words {
        let reading = match (word.pos.as_str(), word.surface.as_str()) {
            // Particles written as they sound.
            ("助詞", "は") => "ワ",
            ("助詞", "へ") => "エ",
            _ => word.reading.as_deref().unwrap_or(&word.surface),
        };
        let inflecting = matches!(previous_pos, "動詞" | "形容詞" | "助動詞");
        // The contracted てる, ちゃう… go with the verb ("aishiteru").
        let contracted = word.pos == "動詞"
            && word.detail == "非自立"
            && word.surface.starts_with(['て', 'で', 'ち', 'じ']);
        let ending = (inflecting
            && (word.pos == "助動詞"
                || (word.pos == "助詞" && word.detail == "接続助詞")
                || contracted))
            || word.detail == "接尾"
            || word.pos == "記号"
            || word.surface.chars().all(super::is_punctuation);
        match spoken.last_mut() {
            Some(last) if ending => last.push_str(reading),
            _ => spoken.push(reading.to_owned()),
        }
        previous_pos = &word.pos;
    }
    spoken
        .iter()
        .map(|word| kana::to_romaji(word))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_kanji_in_context() {
        assert_eq!(
            romanize("君の名前を呼んでいる"),
            "kimi no namae wo yonde iru"
        );
        assert_eq!(romanize("東京は今日も雨"), "toukyou wa kyou mo ame");
        assert_eq!(
            romanize("僕らは今日も歩いていく"),
            "bokura wa kyou mo aruite iku"
        );
        assert_eq!(romanize("行かない"), "ikanai");
        assert_eq!(romanize("さよならだけだった"), "sayonara dake datta");
        assert_eq!(romanize("愛してる"), "aishiteru");
        assert_eq!(romanize("二人だけの空"), "futari dake no sora");
        assert_eq!(romanize("一人きり"), "hitori kiri");
        assert_eq!(romanize("ありがとう、さようなら！"), "arigatou, sayounara!");
        assert_eq!(romanize("Ready? 行こう！"), "Ready? ikou!");
    }

    #[test]
    fn keeps_what_is_not_japanese() {
        assert_eq!(romanize("ライラック"), "rairakku");
        assert_eq!(romanize("Hello 世界"), "Hello sekai");
    }
}
