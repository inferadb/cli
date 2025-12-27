//! Internationalization (i18n) support for the InferaDB CLI.
//!
//! Uses Project Fluent for localization, embedding `.ftl` translation files
//! in the binary for zero-runtime-dependency locale loading.
//!
//! # Usage
//!
//! ```rust,ignore
//! use inferadb_cli::i18n;
//! use inferadb_cli::t;
//!
//! // Initialize once at startup
//! i18n::init("en-US");
//!
//! // Get translations
//! let msg = t!("msg-login-success");
//! let msg = t!("error-profile-not-found", "name" => "prod");
//! ```

mod bundle;
mod locales;

pub use bundle::I18n;
pub use locales::SUPPORTED_LOCALES;

use std::sync::OnceLock;

/// Global i18n instance, initialized once at startup.
static I18N: OnceLock<I18n> = OnceLock::new();

/// Initialize the i18n system with the given locale.
///
/// This should be called once at startup. If the requested locale
/// is not available, falls back to en-US.
///
/// Returns `true` if the requested locale was supported, `false` if
/// a fallback was used.
///
/// # Panics
///
/// Panics if called more than once.
pub fn init(locale: &str) -> bool {
    let supported = is_supported(locale);
    let resolved = locales::resolve_locale(locale);
    let i18n = I18n::new(resolved);
    I18N.set(i18n).expect("i18n::init() called more than once");
    supported
}

/// Check if a locale is supported.
pub fn is_supported(locale: &str) -> bool {
    let normalized = locales::normalize_locale(locale);
    SUPPORTED_LOCALES.contains(&normalized.as_str())
        || SUPPORTED_LOCALES
            .iter()
            .any(|supported| supported.starts_with(normalized.split('-').next().unwrap_or("")))
}

/// Initialize with auto-detected locale from environment.
///
/// Checks `INFERADB_LOCALE`, then `LC_ALL`, then `LANG`.
pub fn init_auto() -> bool {
    let locale = locales::detect_locale();
    init(&locale)
}

/// Get the global i18n instance.
///
/// # Panics
///
/// Panics if `init()` has not been called.
pub fn get() -> &'static I18n {
    I18N.get()
        .expect("i18n not initialized - call i18n::init() first")
}

/// Try to get the global i18n instance without panicking.
pub fn try_get() -> Option<&'static I18n> {
    I18N.get()
}

/// Get a translation by key.
///
/// Prefer using the `t!()` macro for ergonomic access.
pub fn translate(key: &str) -> String {
    get().translate(key, None)
}

/// Get a translation with arguments.
///
/// Prefer using the `t!()` macro for ergonomic access.
pub fn translate_with_args(key: &str, args: &[(&str, &str)]) -> String {
    get().translate(key, Some(args))
}

/// Translation macro for convenient access to localized strings.
///
/// # Examples
///
/// ```rust,ignore
/// // Simple translation
/// let msg = t!("msg-login-success");
///
/// // Translation with arguments
/// let msg = t!("error-profile-not-found", "name" => "prod");
/// ```
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::translate($key)
    };
    ($key:expr, $($k:expr => $v:expr),+ $(,)?) => {{
        let args: &[(&str, &str)] = &[$(($k, $v)),+];
        $crate::i18n::translate_with_args($key, args)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Tests need to be careful about the global OnceLock.
    // Each test that calls init() must be in a separate process,
    // or we use try_get() to check if already initialized.

    #[test]
    fn test_locale_detection() {
        let locale = locales::detect_locale();
        // Should always return something valid
        assert!(!locale.is_empty());
    }

    #[test]
    fn test_locale_resolution() {
        assert_eq!(locales::resolve_locale("en-US"), "en-US");
        assert_eq!(locales::resolve_locale("en"), "en-US");
        assert_eq!(locales::resolve_locale("unknown"), "en-US");
    }
}
