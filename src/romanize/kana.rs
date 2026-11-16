//! Kana to romaji, in modified Hepburn the way lyrics are usually written out: long vowels
//! doubled ("toukyou", "teeze"), ン before a vowel marked ("shin'ichi").

/// Two kana read as one sound, checked before single kana.
#[rustfmt::skip]
const DIGRAPHS: [(&str, &str); 66] = [
    ("キャ", "kya"), ("キュ", "kyu"), ("キョ", "kyo"),
    ("シャ", "sha"), ("シュ", "shu"), ("ショ", "sho"), ("シェ", "she"),
    ("チャ", "cha"), ("チュ", "chu"), ("チョ", "cho"), ("チェ", "che"),
    ("ニャ", "nya"), ("ニュ", "nyu"), ("ニョ", "nyo"),
    ("ヒャ", "hya"), ("ヒュ", "hyu"), ("ヒョ", "hyo"),
    ("ミャ", "mya"), ("ミュ", "myu"), ("ミョ", "myo"),
    ("リャ", "rya"), ("リュ", "ryu"), ("リョ", "ryo"),
    ("ギャ", "gya"), ("ギュ", "gyu"), ("ギョ", "gyo"),
    ("ジャ", "ja"), ("ジュ", "ju"), ("ジョ", "jo"), ("ジェ", "je"),
    ("ヂャ", "ja"), ("ヂュ", "ju"), ("ヂョ", "jo"),
    ("ビャ", "bya"), ("ビュ", "byu"), ("ビョ", "byo"),
    ("ピャ", "pya"), ("ピュ", "pyu"), ("ピョ", "pyo"),
    ("ファ", "fa"), ("フィ", "fi"), ("フェ", "fe"), ("フォ", "fo"),
    ("ヴァ", "va"), ("ヴィ", "vi"), ("ヴェ", "ve"), ("ヴォ", "vo"),
    ("ウィ", "wi"), ("ウェ", "we"), ("ウォ", "wo"),
    ("ティ", "ti"), ("ディ", "di"), ("トゥ", "tu"), ("ドゥ", "du"),
    ("デュ", "dyu"), ("テュ", "tyu"),
    ("ツァ", "tsa"), ("ツィ", "tsi"), ("ツェ", "tse"), ("ツォ", "tso"),
    ("イェ", "ye"), ("クァ", "kwa"), ("クィ", "kwi"), ("クェ", "kwe"), ("クォ", "kwo"),
    ("グァ", "gwa"),
];

/// Single kana (katakana; hiragana is turned into katakana first).
#[rustfmt::skip]
const MONOGRAPHS: [(char, &str); 86] = [
    ('ア', "a"), ('イ', "i"), ('ウ', "u"), ('エ', "e"), ('オ', "o"),
    ('カ', "ka"), ('キ', "ki"), ('ク', "ku"), ('ケ', "ke"), ('コ', "ko"),
    ('サ', "sa"), ('シ', "shi"), ('ス', "su"), ('セ', "se"), ('ソ', "so"),
    ('タ', "ta"), ('チ', "chi"), ('ツ', "tsu"), ('テ', "te"), ('ト', "to"),
    ('ナ', "na"), ('ニ', "ni"), ('ヌ', "nu"), ('ネ', "ne"), ('ノ', "no"),
    ('ハ', "ha"), ('ヒ', "hi"), ('フ', "fu"), ('ヘ', "he"), ('ホ', "ho"),
    ('マ', "ma"), ('ミ', "mi"), ('ム', "mu"), ('メ', "me"), ('モ', "mo"),
    ('ヤ', "ya"), ('ユ', "yu"), ('ヨ', "yo"),
    ('ラ', "ra"), ('リ', "ri"), ('ル', "ru"), ('レ', "re"), ('ロ', "ro"),
    ('ワ', "wa"), ('ヰ', "i"), ('ヱ', "e"), ('ヲ', "wo"), ('ン', "n"),
    ('ガ', "ga"), ('ギ', "gi"), ('グ', "gu"), ('ゲ', "ge"), ('ゴ', "go"),
    ('ザ', "za"), ('ジ', "ji"), ('ズ', "zu"), ('ゼ', "ze"), ('ゾ', "zo"),
    ('ダ', "da"), ('ヂ', "ji"), ('ヅ', "zu"), ('デ', "de"), ('ド', "do"),
    ('バ', "ba"), ('ビ', "bi"), ('ブ', "bu"), ('ベ', "be"), ('ボ', "bo"),
    ('パ', "pa"), ('ピ', "pi"), ('プ', "pu"), ('ペ', "pe"), ('ポ', "po"),
    ('ヴ', "vu"),
    ('ァ', "a"), ('ィ', "i"), ('ゥ', "u"), ('ェ', "e"), ('ォ', "o"),
    ('ャ', "ya"), ('ュ', "yu"), ('ョ', "yo"), ('ヮ', "wa"), ('ヵ', "ka"), ('ヶ', "ke"),
    ('・', " "),
];

/// Whether `c` is hiragana or katakana (the long vowel mark included).
pub fn is_kana(c: char) -> bool {
    matches!(c, '\u{3041}'..='\u{3096}' | '\u{30A1}'..='\u{30FA}' | 'ー')
}

/// Katakana for hiragana; anything else unchanged.
fn katakana(c: char) -> char {
    match c {
        '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
        _ => c,
    }
}

/// `text` with its kana in romaji; other characters are kept.
pub fn to_romaji(text: &str) -> String {
    let chars: Vec<char> = text.chars().map(katakana).collect();
    let mut out = String::new();
    let mut double_next = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let pair: String = chars[i..chars.len().min(i + 2)].iter().collect();
        let (sound, used) = match DIGRAPHS.iter().find(|(kana, _)| *kana == pair) {
            Some((_, sound)) => (Some(*sound), 2),
            None => (
                MONOGRAPHS
                    .iter()
                    .find(|(kana, _)| *kana == c)
                    .map(|(_, sound)| *sound),
                1,
            ),
        };
        i += used;
        match (c, sound) {
            // ッ doubles the next consonant ("chi" after it gives "tchi").
            ('ッ', _) => double_next = true,
            // ー lengthens the vowel before it.
            ('ー', _) => {
                if let Some(vowel) = out.chars().last().filter(|c| "aeiou".contains(*c)) {
                    out.push(vowel);
                }
            }
            (_, Some(sound)) => {
                if double_next {
                    double_next = false;
                    match sound.strip_prefix("ch") {
                        Some(_) => out.push('t'),
                        None => out.extend(sound.chars().next().filter(|c| !"aeiou".contains(*c))),
                    }
                }
                // "shin'ichi", not "shinichi": ン before a vowel or y.
                if out.ends_with('n')
                    && i > used
                    && chars[i - used - 1] == 'ン'
                    && sound.starts_with(['a', 'e', 'i', 'o', 'u', 'y'])
                {
                    out.push('\'');
                }
                out.push_str(sound);
            }
            (other, None) => {
                double_next = false;
                out.push(super::latin_punctuation(other));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_kana_in_hepburn() {
        assert_eq!(to_romaji("トウキョウ"), "toukyou");
        assert_eq!(to_romaji("テーゼ"), "teeze");
        assert_eq!(to_romaji("ライラック"), "rairakku");
        assert_eq!(to_romaji("きみ"), "kimi");
        assert_eq!(to_romaji("マッチ"), "matchi");
        assert_eq!(to_romaji("ヴァイオリン"), "vaiorin");
        assert_eq!(to_romaji("ディズニー"), "dizunii");
        assert_eq!(to_romaji("ウォーター"), "wootaa");
        assert_eq!(to_romaji("シンイチ"), "shin'ichi");
        assert_eq!(to_romaji("コンヤ"), "kon'ya");
        assert_eq!(to_romaji("さんぽ"), "sanpo");
        assert_eq!(to_romaji("ABC"), "ABC");
    }
}
