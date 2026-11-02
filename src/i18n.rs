use std::collections::HashMap;
use std::sync::{LazyLock, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Slovak,
}

impl Language {
    pub fn from_preferences(preferences: &[impl AsRef<str>]) -> Self {
        preferences
            .iter()
            .find_map(|value| {
                match value
                    .as_ref()
                    .split(['-', '_'])
                    .next()?
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "en" => Some(Self::English),
                    "sk" => Some(Self::Slovak),
                    _ => None,
                }
            })
            .unwrap_or(Self::English)
    }
}

struct Locale {
    language: Language,
    decimal: String,
}
static LOCALE: OnceLock<Locale> = OnceLock::new();
static ENGLISH: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../locales/en.json"))
        .expect("validated translation catalogue")
});

pub fn initialize(override_language: Option<Language>) {
    let (languages, decimal) = crate::native::locale_preferences();
    let _ = LOCALE.set(Locale {
        language: override_language.unwrap_or_else(|| Language::from_preferences(&languages)),
        decimal,
    });
}

pub fn language() -> Language {
    LOCALE
        .get()
        .map(|locale| locale.language)
        .unwrap_or(Language::Slovak)
}

pub fn translate(source: &str, language: Language) -> &str {
    if language == Language::English {
        ENGLISH.get(source).map(String::as_str).unwrap_or(source)
    } else {
        source
    }
}

pub fn t(source: &str) -> &str {
    translate(source, language())
}

pub fn message(source: &str, positional: &[String], named: &[(&str, String)]) -> String {
    interpolate(t(source), positional, named)
}

fn interpolate(template: &str, positional: &[String], named: &[(&str, String)]) -> String {
    let mut result = String::with_capacity(template.len() + 32);
    let mut rest = template;
    let mut position = 0;
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}').map(|end| start + end) else {
            result.push_str(&rest[start..]);
            return result;
        };
        let key = &rest[start + 1..end];
        let value = if key.is_empty() {
            let value = positional.get(position);
            position += 1;
            value
        } else {
            named
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value)
        };
        if let Some(value) = value {
            result.push_str(value);
        } else {
            result.push_str(&rest[start..=end]);
        }
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    result
}

pub fn decimal(value: String) -> String {
    let separator = LOCALE
        .get()
        .map(|locale| locale.decimal.as_str())
        .unwrap_or(".");
    if separator == "." {
        value
    } else {
        value.replace('.', separator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_order_wins_over_region_and_unknown_languages_fall_back_to_english() {
        assert_eq!(
            Language::from_preferences(&["en-SK", "sk-SK"]),
            Language::English
        );
        assert_eq!(
            Language::from_preferences(&["sk_SK", "en-US"]),
            Language::Slovak
        );
        assert_eq!(
            Language::from_preferences(&["de-DE", "sk"]),
            Language::Slovak
        );
        assert_eq!(Language::from_preferences(&["fr-FR"]), Language::English);
    }
    #[test]
    fn translations_preserve_named_and_positional_values() {
        let template = translate("Vyžaduje macOS {version}", Language::English);
        assert_eq!(
            interpolate(template, &[], &[("version", "28".into())]),
            "Requires macOS 28"
        );
        let template = translate("Nainštalovať {}", Language::English);
        assert_eq!(
            interpolate(template, &["sample".into()], &[]),
            "Install sample"
        );
    }
    #[test]
    fn all_translation_templates_keep_their_placeholders() {
        fn placeholders(value: &str) -> Vec<&str> {
            let mut result = Vec::new();
            let mut rest = value;
            while let Some(start) = rest.find('{') {
                let end = rest[start..].find('}').unwrap() + start;
                result.push(&rest[start + 1..end]);
                rest = &rest[end + 1..];
            }
            result.sort();
            result
        }
        for (source, english) in ENGLISH.iter() {
            assert_eq!(placeholders(source), placeholders(english), "{source}");
            assert!(!english.is_empty());
        }
    }
}
