//! Chinese (Simplified and Traditional) to pinyin with tone marks, a syllable per character.
//! A character with several readings gets its most common one.

use pinyin::ToPinyin;

/// Whether `c` is a Chinese character (also used for Japanese kanji).
pub fn is_han(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '\u{F900}'..='\u{FAFF}')
}

/// `line` in pinyin; other characters are kept, punctuation joined to the syllable before it.
pub fn romanize(line: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    // The word being read is not Chinese (Latin letters, digits).
    let mut in_word = false;
    for c in line.chars() {
        if let Some(syllable) = c.to_pinyin() {
            words.push(syllable.with_tone().to_owned());
            in_word = false;
        } else if c.is_whitespace() {
            in_word = false;
        } else if c.is_alphanumeric() && !in_word {
            words.push(c.to_string());
            in_word = true;
        } else if let Some(last) = words.last_mut() {
            last.push(super::latin_punctuation(c));
        } else {
            words.push(super::latin_punctuation(c).to_string());
            in_word = c.is_alphanumeric();
        }
    }
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_pinyin_with_tones() {
        assert_eq!(romanize("我们一起去看海"), "wǒ men yī qǐ qù kàn hǎi");
        assert_eq!(
            romanize("後來 我總算學會了"),
            "hòu lái wǒ zǒng suàn xué huì le"
        );
        assert_eq!(romanize("你好，世界！"), "nǐ hǎo, shì jiè!");
        assert_eq!(romanize("I love 你 2"), "I love nǐ 2");
    }
}
