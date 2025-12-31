//! Form utilities for multi-field input.
//!
//! Provides a wrapper around Teapot's Forms for CLI use.
//!
//! # Example
//!
//! ```rust,ignore
//! use inferadb_cli::tui;
//! use teapot::forms::{Form, Group, InputField};
//!
//! let form = Form::new()
//!     .title("Setup Wizard")
//!     .group(
//!         Group::new()
//!             .field(InputField::new("name").title("Your name").build())
//!             .field(InputField::new("email").title("Email").build())
//!     );
//!
//! if let Some(results) = tui::run_form(form)? {
//!     let name = results.get_string("name").unwrap_or("");
//!     println!("Hello, {}!", name);
//! }
//! ```

use crate::error::Result;
use teapot::forms::{Form, FormResults};
use teapot::output::{is_ci, is_tty};

/// Run a form and return its results.
///
/// In interactive mode, shows the form with formatted prompts.
/// In non-interactive mode (CI, piped input), shows plain text prompts.
///
/// Returns `None` if the form was cancelled.
pub fn run_form(mut form: Form) -> Result<Option<FormResults>> {
    // Always use accessible mode for CLI - it's the most compatible
    // and still provides a good experience
    form.run_accessible()
        .map_err(|e| crate::error::Error::other(e.to_string()))
}

/// Check if forms should use accessible (plain text) mode.
pub fn is_accessible() -> bool {
    std::env::var("ACCESSIBLE").is_ok() || !is_tty() || is_ci()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_accessible_in_test() {
        // In tests, we're typically not in a TTY, so accessible mode should be true
        // unless explicitly set otherwise
        // Just ensure it doesn't panic and returns a valid bool
        let _result = is_accessible();
    }
}
