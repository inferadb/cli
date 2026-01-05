//! Locale detection and resolution.

/// List of supported locales.
pub const SUPPORTED_LOCALES: &[&str] = &["en-US"];

/// Detect the user's preferred locale from environment variables.
///
/// Checks in order:
/// 1. `INFERADB_LOCALE` - explicit CLI locale override
/// 2. `LC_ALL` - POSIX locale override
/// 3. `LANG` - default POSIX locale
///
/// Returns "en-US" as the default if no locale is detected.
pub fn detect_locale() -> String {
    // Check INFERADB_LOCALE first (explicit override)
    if let Ok(locale) = std::env::var("INFERADB_LOCALE") {
        if !locale.is_empty() {
            return normalize_locale(&locale);
        }
    }

    // Check LC_ALL
    if let Ok(locale) = std::env::var("LC_ALL") {
        if !locale.is_empty() && locale != "C" && locale != "POSIX" {
            return normalize_locale(&locale);
        }
    }

    // Check LANG
    if let Ok(locale) = std::env::var("LANG") {
        if !locale.is_empty() && locale != "C" && locale != "POSIX" {
            return normalize_locale(&locale);
        }
    }

    // Default
    "en-US".to_string()
}

/// Normalize a locale string to BCP 47 format.
///
/// Examples:
/// - "en_US.UTF-8" -> "en-US"
/// - "`en_US`" -> "en-US"
/// - "en" -> "en"
pub fn normalize_locale(locale: &str) -> String {
    // Remove encoding suffix (e.g., ".UTF-8")
    let locale = locale.split('.').next().unwrap_or(locale);

    // Replace underscore with hyphen
    locale.replace('_', "-")
}

/// Resolve a locale to a supported locale.
///
/// Falls back to en-US for unsupported locales.
pub fn resolve_locale(locale: &str) -> &'static str {
    let normalized = normalize_locale(locale);

    // Currently only en-US is supported, add more locales/language fallbacks here
    // For now, all locales resolve to en-US
    let _ = normalized; // Acknowledge normalized is intentionally unused for now
    "en-US"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_locale() {
        assert_eq!(normalize_locale("en_US.UTF-8"), "en-US");
        assert_eq!(normalize_locale("en_US"), "en-US");
        assert_eq!(normalize_locale("en-US"), "en-US");
        assert_eq!(normalize_locale("en"), "en");
        assert_eq!(normalize_locale("de_DE.UTF-8"), "de-DE");
    }

    #[test]
    fn test_resolve_locale() {
        assert_eq!(resolve_locale("en-US"), "en-US");
        assert_eq!(resolve_locale("en_US.UTF-8"), "en-US");
        assert_eq!(resolve_locale("en"), "en-US");
        assert_eq!(resolve_locale("de-DE"), "en-US"); // Fallback
        assert_eq!(resolve_locale("unknown"), "en-US");
    }
}
