//! The languages of the interface. The texts live in `locales/*.yml` (rust-i18n, one file per
//! part of the app with every language side by side) and are read with `t!("key")`; a text
//! missing in a language falls back to English.
//!
//! The language is the one picked on the appearance screen, or the system's (`LC_ALL`,
//! `LC_MESSAGES`, `LANG`) when none is picked. YouTube Music is asked for its own texts (the
//! titles in Explore, the names of mixes) in the same language.

/// A language the interface is translated into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    /// The locale code of `locales/*.yml`.
    pub code: &'static str,
    /// The language's name in itself, as the picker shows it.
    pub name: &'static str,
    /// YouTube's code for the same language, for the texts YouTube Music sends.
    youtube: &'static str,
}

/// The languages offered: the ten most spoken in the world (counting second-language
/// speakers), Traditional Chinese, and the languages of three of the largest music markets.
pub const LANGUAGES: [Language; 14] = [
    lang("en", "English", "en"),
    lang("zh-CN", "简体中文", "zh-CN"),
    lang("zh-TW", "繁體中文", "zh-TW"),
    lang("hi", "हिन्दी", "hi"),
    lang("es", "Español", "es-419"),
    lang("ar", "العربية", "ar"),
    lang("fr", "Français", "fr"),
    lang("bn", "বাংলা", "bn"),
    lang("pt-BR", "Português (Brasil)", "pt"),
    lang("ru", "Русский", "ru"),
    lang("id", "Bahasa Indonesia", "id"),
    lang("de", "Deutsch", "de"),
    lang("ja", "日本語", "ja"),
    lang("ko", "한국어", "ko"),
];

const fn lang(code: &'static str, name: &'static str, youtube: &'static str) -> Language {
    Language {
        code,
        name,
        youtube,
    }
}

/// The language of a POSIX locale like "zh_CN.UTF-8", "pt_PT" or "de_AT@euro", if it is one
/// of `LANGUAGES`: Chinese by script region, Portuguese of any country as Brazilian, any other
/// by the language alone.
pub fn from_posix(locale: &str) -> Option<&'static Language> {
    let locale = locale.split(['.', '@']).next()?.replace('_', "-");
    let (language, region) = match locale.split_once('-') {
        Some((language, region)) => (language.to_lowercase(), region.to_uppercase()),
        None => (locale.to_lowercase(), String::new()),
    };
    let code = match (language.as_str(), region.as_str()) {
        ("zh", "TW" | "HK" | "MO") => "zh-TW",
        ("zh", _) => "zh-CN",
        ("pt", _) => "pt-BR",
        (language, _) => language,
    };
    LANGUAGES.iter().find(|candidate| candidate.code == code)
}

/// The system's language, when it is one of `LANGUAGES`.
pub fn system() -> Option<&'static Language> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .and_then(|locale| from_posix(&locale))
}

/// The language to show for the user's `choice` (a code from `LANGUAGES`): the choice, else
/// the system's, else English.
pub fn resolve(choice: Option<&str>) -> &'static Language {
    choice
        .and_then(|code| LANGUAGES.iter().find(|language| language.code == code))
        .or_else(system)
        .unwrap_or(&LANGUAGES[0])
}

/// Shows the interface in the language for `choice`. Tests keep English, so their texts do
/// not depend on the machine.
pub fn apply(choice: Option<&str>) {
    #[cfg(not(test))]
    rust_i18n::set_locale(resolve(choice).code);
    #[cfg(test)]
    let _ = choice;
}

/// YouTube's language for the interface's: what YouTube Music sends its own texts in.
pub fn youtube_language() -> rustypipe::param::Language {
    let current = rust_i18n::locale();
    let code = LANGUAGES
        .iter()
        .find(|language| language.code == &*current)
        .map_or("en", |language| language.youtube);
    code.parse().unwrap_or(rustypipe::param::Language::En)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_posix_locales() {
        let code = |locale: &str| from_posix(locale).map(|language| language.code);
        assert_eq!(code("zh_CN.UTF-8"), Some("zh-CN"));
        assert_eq!(code("zh_TW.UTF-8"), Some("zh-TW"));
        assert_eq!(code("zh_HK"), Some("zh-TW"));
        assert_eq!(code("pt_PT.UTF-8"), Some("pt-BR"));
        assert_eq!(code("es_AR.UTF-8"), Some("es"));
        assert_eq!(code("de_AT@euro"), Some("de"));
        assert_eq!(code("C.UTF-8"), None);
        assert_eq!(code("fi_FI"), None);
    }

    #[test]
    fn a_choice_wins_over_the_system() {
        assert_eq!(resolve(Some("ja")).code, "ja");
        assert_eq!(resolve(Some("xx")).code, resolve(None).code);
    }

    #[test]
    fn every_language_has_a_youtube_code() {
        for language in LANGUAGES {
            assert!(
                language
                    .youtube
                    .parse::<rustypipe::param::Language>()
                    .is_ok(),
                "{}",
                language.code
            );
        }
    }

    /// The `%{name}` placeholders of a text.
    fn placeholders(text: &str) -> Vec<&str> {
        let mut found: Vec<&str> = text
            .split("%{")
            .skip(1)
            .filter_map(|rest| rest.split('}').next())
            .collect();
        found.sort_unstable();
        found
    }

    #[test]
    fn every_text_exists_in_every_language_with_the_same_placeholders() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
        let mut checked = 0;
        for file in std::fs::read_dir(&dir).expect("locales") {
            let path = file.expect("file").path();
            let yaml = std::fs::read_to_string(&path).expect("read");
            let texts: std::collections::BTreeMap<String, serde_yaml::Value> =
                serde_yaml::from_str(&yaml).expect("yaml");
            for (key, translations) in texts.iter().filter(|(key, _)| *key != "_version") {
                let english = translations["en"].as_str().unwrap_or_else(|| {
                    panic!("{}: {key} has no English", path.display());
                });
                for language in LANGUAGES {
                    let text = translations[language.code].as_str().unwrap_or_else(|| {
                        panic!("{}: {key} has no {}", path.display(), language.code)
                    });
                    assert!(
                        !text.trim().is_empty(),
                        "{key} is empty in {}",
                        language.code
                    );
                    assert_eq!(
                        placeholders(text),
                        placeholders(english),
                        "{key} in {}",
                        language.code
                    );
                }
                checked += 1;
            }
        }
        assert!(checked > 0, "no texts in {}", dir.display());
    }
}
