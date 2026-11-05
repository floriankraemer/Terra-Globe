//! Translation catalogues.
//!
//! The web app's nested i18next resources are flattened to dotted keys by
//! `tools/parity/extract-i18n.mjs`, so lookup is a single map access and the
//! key-parity test compares exact key sets per locale.

use std::collections::BTreeMap;

pub const DEFAULT_LOCALE: &str = "en";

/// Every bundled catalogue, compiled into the binary. Missing a locale here is
/// caught by `available_locales_match_the_bundled_catalogues`.
const CATALOGUES: &[(&str, &str)] = &[
    ("de", include_str!("../i18n/de.json")),
    ("en", include_str!("../i18n/en.json")),
    ("es", include_str!("../i18n/es.json")),
    ("fi", include_str!("../i18n/fi.json")),
    ("fr", include_str!("../i18n/fr.json")),
    ("hi", include_str!("../i18n/hi.json")),
    ("ja", include_str!("../i18n/ja.json")),
    ("no", include_str!("../i18n/no.json")),
    ("pl", include_str!("../i18n/pl.json")),
    ("sv", include_str!("../i18n/sv.json")),
    ("uk", include_str!("../i18n/uk.json")),
    ("zh", include_str!("../i18n/zh.json")),
];

#[derive(Debug, thiserror::Error)]
pub enum I18nError {
    #[error("unknown locale: {0}")]
    UnknownLocale(String),
    #[error("catalogue for {0} is invalid: {1}")]
    InvalidCatalogue(String, String),
}

#[derive(Debug, Clone)]
pub struct Catalogue {
    locale: String,
    entries: BTreeMap<String, String>,
}

impl Catalogue {
    pub fn load(locale: &str) -> Result<Self, I18nError> {
        let raw = CATALOGUES
            .iter()
            .find(|(code, _)| *code == locale)
            .map(|(_, raw)| *raw)
            .ok_or_else(|| I18nError::UnknownLocale(locale.to_string()))?;
        let entries: BTreeMap<String, String> = serde_json::from_str(raw)
            .map_err(|error| I18nError::InvalidCatalogue(locale.to_string(), error.to_string()))?;
        Ok(Self {
            locale: locale.to_string(),
            entries,
        })
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Active locale plus the English fallback, mirroring i18next's `fallbackLng`.
#[derive(Debug, Clone)]
pub struct I18n {
    active: Catalogue,
    fallback: Catalogue,
}

impl Default for I18n {
    fn default() -> Self {
        let fallback = Catalogue::load(DEFAULT_LOCALE).expect("bundled English catalogue");
        Self {
            active: fallback.clone(),
            fallback,
        }
    }
}

impl I18n {
    /// Unknown locales fall back to English rather than failing startup.
    pub fn new(locale: &str) -> Self {
        let fallback = Catalogue::load(DEFAULT_LOCALE).expect("bundled English catalogue");
        let active = Catalogue::load(locale).unwrap_or_else(|_| fallback.clone());
        Self { active, fallback }
    }

    pub fn locale(&self) -> &str {
        self.active.locale()
    }

    pub fn set_locale(&mut self, locale: &str) {
        if let Ok(catalogue) = Catalogue::load(locale) {
            self.active = catalogue;
        }
    }

    /// Translated text, or the key itself when both catalogues lack it. Returning
    /// the key makes a missing translation visible instead of rendering nothing.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.active
            .get(key)
            .or_else(|| self.fallback.get(key))
            .unwrap_or(key)
    }

    /// Pluralised text. Only the categories this app's locales actually use are
    /// implemented (Polish additionally needs `few`/`many`); anything else falls
    /// back to `_other`, then to the base key.
    pub fn t_plural<'a>(&'a self, key: &'a str, count: i64) -> &'a str {
        let category = plural_category(self.locale(), count);
        let candidates = [
            format!("{key}_{category}"),
            format!("{key}_other"),
            key.to_string(),
        ];
        for candidate in &candidates {
            if let Some(text) = self.active.get(candidate) {
                return text;
            }
        }
        for candidate in &candidates {
            if let Some(text) = self.fallback.get(candidate) {
                return text;
            }
        }
        key
    }

    /// Translated text with `{{name}}` placeholders substituted.
    pub fn t_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut text = self.t(key).to_string();
        for (name, value) in args {
            text = text.replace(&format!("{{{{{name}}}}}"), value);
        }
        text
    }
}

/// Minimal CLDR plural categories for the bundled locales.
fn plural_category(locale: &str, count: i64) -> &'static str {
    let n = count.unsigned_abs();
    match locale {
        "zh" | "ja" => "other",
        "fr" => {
            if n <= 1 {
                "one"
            } else {
                "other"
            }
        }
        "pl" => {
            if n == 1 {
                "one"
            } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
                "few"
            } else {
                "many"
            }
        }
        "uk" => {
            if n % 10 == 1 && n % 100 != 11 {
                "one"
            } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
                "few"
            } else {
                "many"
            }
        }
        _ => {
            if n == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

pub fn available_locales() -> Vec<&'static str> {
    CATALOGUES.iter().map(|(code, _)| *code).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_key(key: &str) -> &str {
        for suffix in ["_zero", "_one", "_two", "_few", "_many", "_other"] {
            if let Some(base) = key.strip_suffix(suffix) {
                return base;
            }
        }
        key
    }

    #[test]
    fn every_catalogue_has_the_same_keys_as_english() {
        let english = Catalogue::load(DEFAULT_LOCALE).expect("english");
        let english_keys: Vec<&str> = english.keys().collect();
        let english_bases: std::collections::BTreeSet<&str> =
            english_keys.iter().map(|key| base_key(key)).collect();

        for locale in available_locales() {
            let catalogue = Catalogue::load(locale).expect("catalogue");
            // Every English key must exist, so no locale silently falls back to
            // English for a string that was supposed to be translated.
            for key in &english_keys {
                assert!(
                    catalogue.get(key).is_some(),
                    "locale {locale} is missing {key}"
                );
            }
            // Plural categories differ per language (Polish has few/many), so the
            // base key sets are what must match.
            let bases: std::collections::BTreeSet<&str> = catalogue.keys().map(base_key).collect();
            assert_eq!(bases, english_bases, "locale {locale} has different keys");
        }
    }

    #[test]
    fn plural_selection_follows_locale_rules() {
        let english = I18n::new("en");
        assert_eq!(
            english.t_plural("importExport.importedFolders", 1),
            english.t("importExport.importedFolders_one")
        );
        assert_eq!(
            english.t_plural("importExport.importedFolders", 5),
            english.t("importExport.importedFolders_other")
        );

        let polish = I18n::new("pl");
        assert_eq!(
            polish.t_plural("importExport.importedFolders", 1),
            polish.t("importExport.importedFolders_one")
        );
        assert_eq!(
            polish.t_plural("importExport.importedFolders", 3),
            polish.t("importExport.importedFolders_few")
        );
        assert_eq!(
            polish.t_plural("importExport.importedFolders", 12),
            polish.t("importExport.importedFolders_many")
        );

        // Chinese has no plural distinction beyond `_other`.
        let chinese = I18n::new("zh");
        assert_eq!(
            chinese.t_plural("importExport.importedFolders", 1),
            chinese.t("importExport.importedFolders_one")
        );
        assert_eq!(
            chinese.t_plural("importExport.importedFolders", 9),
            chinese.t("importExport.importedFolders_other")
        );
    }

    #[test]
    fn bundle_matches_the_generated_manifest() {
        let manifest: Vec<String> =
            serde_json::from_str(include_str!("../i18n/locales.json")).expect("manifest");
        let mut bundled: Vec<String> = available_locales().iter().map(|s| s.to_string()).collect();
        bundled.sort();
        assert_eq!(bundled, manifest);
    }

    #[test]
    fn translates_and_interpolates() {
        let i18n = I18n::new("de");
        assert_eq!(i18n.locale(), "de");
        assert_eq!(i18n.t("common.cancel"), "Abbrechen");
        let text = i18n.t_args("about.title", &[("name", "Terra Globe")]);
        assert!(text.contains("Terra Globe"), "got {text}");
        assert!(
            !text.contains("{{"),
            "placeholders must be substituted: {text}"
        );
    }

    #[test]
    fn unknown_locales_and_keys_degrade_visibly() {
        let i18n = I18n::new("xx");
        assert_eq!(i18n.locale(), "en");
        assert_eq!(i18n.t("no.such.key"), "no.such.key");
    }

    #[test]
    fn switching_locale_changes_lookups() {
        let mut i18n = I18n::new("en");
        assert_eq!(i18n.t("common.cancel"), "Cancel");
        i18n.set_locale("fr");
        assert_eq!(i18n.t("common.cancel"), "Annuler");
        i18n.set_locale("xx");
        assert_eq!(i18n.locale(), "fr", "invalid locales are ignored");
    }
}
