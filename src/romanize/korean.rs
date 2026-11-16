//! Hangul to the Revised Romanization of Korean, with the sound changes between syllables that
//! the system writes: a final consonant moving to a following vowel ("hangugeo"), ㅎ silent
//! before a vowel ("joahae") or making the next consonant aspirated ("chuka", "jota"),
//! nasalization ("hamnida", "dongnip"), ㄹ assimilation ("silla") and palatalization
//! ("gachi"). Tensing is not written, as in the system itself.

/// Hangul syllables are numbered initial × 588 + vowel × 28 + final from U+AC00.
const FIRST: u32 = 0xAC00;
const LAST: u32 = 0xD7A3;

#[rustfmt::skip]
const INITIALS: [&str; 19] = [
    "g", "kk", "n", "d", "tt", "r", "m", "b", "pp", "s", "ss", "", "j", "jj", "ch", "k", "t", "p",
    "h",
];
#[rustfmt::skip]
const VOWELS: [&str; 21] = [
    "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "wo", "we",
    "wi", "yu", "eu", "ui", "i",
];
/// How each final consonant sounds before a pause or a consonant.
#[rustfmt::skip]
const FINALS: [&str; 28] = [
    "", "k", "k", "k", "n", "n", "n", "t", "l", "k", "m", "l", "l", "l", "p", "l", "m", "p", "p",
    "t", "t", "ng", "t", "t", "k", "t", "p", "t",
];

// Initials by index.
const I_G: usize = 0;
const I_N: usize = 2;
const I_D: usize = 3;
const I_R: usize = 5;
const I_M: usize = 6;
const I_S: usize = 9;
const I_NONE: usize = 11;
const I_J: usize = 12;
const I_H: usize = 18;
// The vowel ㅣ.
const V_I: usize = 20;

fn syllable(c: char) -> Option<(usize, usize, usize)> {
    let code = c as u32;
    if !(FIRST..=LAST).contains(&code) {
        return None;
    }
    let index = (code - FIRST) as usize;
    Some((index / 588, (index % 588) / 28, index % 28))
}

/// Whether `c` is a Hangul syllable.
pub fn is_hangul(c: char) -> bool {
    syllable(c).is_some()
}

/// How final `last` and the next syllable's initial `next` (with vowel `vowel`) are written:
/// the end of this syllable and the start of the next.
fn boundary(last: usize, next: usize, vowel: usize) -> (&'static str, &'static str) {
    if next == I_NONE {
        // The final moves to the vowel that follows.
        let palatal = vowel == V_I;
        return match last {
            0 => ("", ""),
            1 => ("", "g"),
            2 => ("", "kk"),
            3 => ("k", "s"),
            4 => ("", "n"),
            5 => ("n", "j"),
            6 => ("", "n"),
            7 if palatal => ("", "j"),
            7 => ("", "d"),
            8 => ("", "r"),
            9 => ("l", "g"),
            10 => ("l", "m"),
            11 => ("l", "b"),
            12 => ("l", "s"),
            13 if palatal => ("l", "ch"),
            13 => ("l", "t"),
            14 => ("l", "p"),
            15 => ("", "r"),
            16 => ("", "m"),
            17 => ("", "b"),
            18 => ("p", "s"),
            19 => ("", "s"),
            20 => ("", "ss"),
            21 => ("ng", ""),
            22 => ("", "j"),
            23 => ("", "ch"),
            24 => ("", "k"),
            25 if palatal => ("", "ch"),
            25 => ("", "t"),
            26 => ("", "p"),
            _ => ("", ""),
        };
    }
    // ㅎ, ㄶ and ㅀ make a following ㄱ, ㄷ or ㅈ aspirated.
    if matches!(last, 6 | 15 | 27) {
        let kept = match last {
            6 => "n",
            15 => "l",
            _ => "",
        };
        match next {
            I_G => return (kept, "k"),
            I_D => return (kept, "t"),
            I_J => return (kept, "ch"),
            I_S if last == 27 => return ("", "ss"),
            I_N if last == 27 => return ("n", "n"),
            _ => {}
        }
    }
    let sound = FINALS[last];
    // A k, t or p sound makes a following ㅎ aspirated.
    if next == I_H {
        match sound {
            "k" => return ("", "k"),
            "t" if matches!(last, 22 | 23) => return ("", "ch"),
            "t" => return ("", "t"),
            "p" => return ("", "p"),
            _ => {}
        }
    }
    match (sound, next) {
        // ㄹ next to ㄴ or ㄹ.
        ("n" | "l", I_R) | ("l", I_N) => ("l", "l"),
        // Nasalization before ㄴ and ㅁ, and of ㄹ after other consonants.
        ("k", I_N | I_M) => ("ng", INITIALS[next]),
        ("t", I_N | I_M) => ("n", INITIALS[next]),
        ("p", I_N | I_M) => ("m", INITIALS[next]),
        ("k", I_R) => ("ng", "n"),
        ("p", I_R) => ("m", "n"),
        ("m" | "ng", I_R) => (sound, "n"),
        _ => (sound, INITIALS[next]),
    }
}

/// `line` with its Hangul romanized; other characters are kept.
pub fn romanize(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut onset: Option<&str> = None;
    for (index, &c) in chars.iter().enumerate() {
        let Some((initial, vowel, last)) = syllable(c) else {
            onset = None;
            out.push(c);
            continue;
        };
        out.push_str(onset.take().unwrap_or(INITIALS[initial]));
        out.push_str(VOWELS[vowel]);
        match chars.get(index + 1).copied().and_then(syllable) {
            Some((next, next_vowel, _)) => {
                let (end, start) = boundary(last, next, next_vowel);
                out.push_str(end);
                onset = Some(start);
            }
            None => out.push_str(FINALS[last]),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_sound_changes() {
        for (hangul, roman) in [
            ("사랑해요", "saranghaeyo"),
            ("밤양갱", "bamyanggaeng"),
            ("좋아해", "joahae"),
            ("같이 있어줘", "gachi isseojwo"),
            ("한국어 노래", "hangugeo norae"),
            ("학교", "hakgyo"),
            ("별빛이 내린다", "byeolbichi naerinda"),
            ("감사합니다", "gamsahamnida"),
            ("신라", "silla"),
            ("좋다", "jota"),
            ("많이", "mani"),
            ("독립", "dongnip"),
            ("축하해", "chukahae"),
            ("닭", "dak"),
        ] {
            assert_eq!(romanize(hangul), roman, "{hangul}");
        }
    }

    #[test]
    fn keeps_what_is_not_hangul() {
        assert_eq!(romanize("Love 사랑!"), "Love sarang!");
    }
}
