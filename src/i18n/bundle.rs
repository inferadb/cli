//! FluentBundle creation and management.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// Embedded locale files.
const EN_US_FTL: &str = include_str!("locales/en-US.ftl");

/// The i18n system, holding a FluentBundle for translation lookups.
///
/// Uses the concurrent FluentBundle variant for thread-safety.
pub struct I18n {
    bundle: FluentBundle<FluentResource>,
    locale: LanguageIdentifier,
}

impl I18n {
    /// Create a new I18n instance for the given locale.
    ///
    /// Falls back to en-US if the locale is not available.
    pub fn new(locale: &str) -> Self {
        let locale: LanguageIdentifier =
            locale.parse().unwrap_or_else(|_| "en-US".parse().unwrap());

        let ftl_string = match locale.language.as_str() {
            "en" => EN_US_FTL,
            // Add more languages here as they become available
            _ => EN_US_FTL,
        };

        let resource =
            FluentResource::try_new(ftl_string.to_string()).expect("Failed to parse FTL resource");

        // Use concurrent bundle for thread-safety with OnceLock
        let mut bundle = FluentBundle::new_concurrent(vec![locale.clone()]);
        bundle
            .add_resource(resource)
            .expect("Failed to add FTL resource to bundle");

        Self { bundle, locale }
    }

    /// Get the current locale.
    pub fn locale(&self) -> &LanguageIdentifier {
        &self.locale
    }

    /// Translate a message by key.
    ///
    /// If the key is not found, returns the key itself as a fallback.
    pub fn translate(&self, key: &str, args: Option<&[(&str, &str)]>) -> String {
        let msg = match self.bundle.get_message(key) {
            Some(msg) => msg,
            None => {
                // Key not found - return key as fallback
                tracing::warn!(key = key, "Missing translation key");
                return key.to_string();
            }
        };

        let pattern = match msg.value() {
            Some(pattern) => pattern,
            None => return key.to_string(),
        };

        let mut errors = vec![];

        let result = if let Some(args) = args {
            let mut fluent_args = FluentArgs::new();
            for (k, v) in args {
                fluent_args.set(*k, FluentValue::from(*v));
            }
            self.bundle
                .format_pattern(pattern, Some(&fluent_args), &mut errors)
        } else {
            self.bundle.format_pattern(pattern, None, &mut errors)
        };

        if !errors.is_empty() {
            tracing::warn!(key = key, errors = ?errors, "Translation errors");
        }

        result.into_owned()
    }
}

impl std::fmt::Debug for I18n {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("I18n")
            .field("locale", &self.locale.to_string())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_creation() {
        let i18n = I18n::new("en-US");
        assert_eq!(i18n.locale().language.as_str(), "en");
    }

    #[test]
    fn test_fallback_locale() {
        let i18n = I18n::new("xx-XX");
        // Should fall back to en-US
        assert_eq!(i18n.locale().language.as_str(), "xx");
        // But the bundle should use en-US content
    }

    #[test]
    fn test_missing_key_fallback() {
        let i18n = I18n::new("en-US");
        let result = i18n.translate("nonexistent-key", None);
        assert_eq!(result, "nonexistent-key");
    }
}
