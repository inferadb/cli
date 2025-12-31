//! Interactive confirmation utilities.
//!
//! Provides a beautiful confirmation prompt using Teapot's Confirm component.
//!
//! # Example
//!
//! ```rust,ignore
//! use inferadb_cli::tui;
//!
//! if tui::confirm("Delete this resource?").await? {
//!     delete_resource().await?;
//! }
//! ```

use std::io::{self, BufRead, Write};

use teapot::components::Confirm as TeapotConfirm;
use teapot::output::{is_ci, is_tty};
use teapot::style::{Color, RESET};
use teapot::Model;

/// Result of a confirmation prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmResult {
    /// User confirmed (yes).
    Yes,
    /// User declined (no).
    No,
    /// User cancelled (Ctrl+C, Escape).
    Cancelled,
}

impl ConfirmResult {
    /// Check if confirmed.
    pub fn is_confirmed(&self) -> bool {
        matches!(self, ConfirmResult::Yes)
    }
}

/// Configuration for the confirm prompt.
#[derive(Debug, Clone)]
pub struct ConfirmOptions {
    /// Default value if user just presses enter.
    pub default: bool,
    /// Label for "yes" option.
    pub yes_label: String,
    /// Label for "no" option.
    pub no_label: String,
}

impl Default for ConfirmOptions {
    fn default() -> Self {
        Self {
            default: false,
            yes_label: "Yes".to_string(),
            no_label: "No".to_string(),
        }
    }
}

impl ConfirmOptions {
    /// Create a new confirm options with default value true.
    pub fn default_yes() -> Self {
        Self {
            default: true,
            ..Default::default()
        }
    }
}

/// Show a confirmation prompt and return the result.
///
/// In non-interactive mode, returns the default value.
/// In interactive mode, shows a pretty confirmation prompt.
///
/// # Example
///
/// ```rust,ignore
/// if tui::confirm("Delete this file?")? {
///     std::fs::remove_file(path)?;
/// }
/// ```
pub fn confirm(message: &str) -> crate::error::Result<bool> {
    confirm_with_options(message, &ConfirmOptions::default())
}

/// Show a confirmation prompt with custom options.
pub fn confirm_with_options(message: &str, options: &ConfirmOptions) -> crate::error::Result<bool> {
    // In non-interactive mode, use default
    if !is_tty() || is_ci() {
        teapot::output::info(&format!(
            "{} [{}] (non-interactive, using default)",
            message,
            if options.default { "Y" } else { "N" }
        ));
        return Ok(options.default);
    }

    // Create and display the confirmation prompt
    let confirm = TeapotConfirm::new(message)
        .default(options.default)
        .yes_label(&options.yes_label)
        .no_label(&options.no_label);

    // Display the prompt
    eprint!("{} ", confirm.view());
    io::stderr().flush()?;

    // Read user input
    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    // Parse the response
    let result = match input.as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => options.default, // Use default on empty input
        _ => options.default,  // Use default on unknown input
    };

    Ok(result)
}

/// Show a confirmation prompt with danger styling.
///
/// Used for destructive operations like deletion.
///
/// # Example
///
/// ```rust,ignore
/// if tui::confirm_danger("This will permanently delete all data. Continue?")? {
///     delete_all_data().await?;
/// }
/// ```
pub fn confirm_danger(message: &str) -> crate::error::Result<bool> {
    // In non-interactive mode, use default (no for danger)
    if !is_tty() || is_ci() {
        teapot::output::info(&format!("{} [N] (non-interactive, using default)", message));
        return Ok(false);
    }

    // Show warning prefix in danger mode
    eprint!("{}⚠{} ", Color::Red.to_ansi_fg(), RESET);

    let confirm = TeapotConfirm::new(message)
        .default(false)
        .yes_label("Yes, I'm sure")
        .no_label("Cancel")
        .selected_color(Color::Red);

    eprint!("{} ", confirm.view());
    io::stderr().flush()?;

    // Read user input
    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    // Parse the response - only explicit yes for danger
    let result = matches!(input.as_str(), "y" | "yes");

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_options_default() {
        let opts = ConfirmOptions::default();
        assert!(!opts.default);
        assert_eq!(opts.yes_label, "Yes");
        assert_eq!(opts.no_label, "No");
    }

    #[test]
    fn test_confirm_options_default_yes() {
        let opts = ConfirmOptions::default_yes();
        assert!(opts.default);
    }

    #[test]
    fn test_confirm_result_is_confirmed() {
        assert!(ConfirmResult::Yes.is_confirmed());
        assert!(!ConfirmResult::No.is_confirmed());
        assert!(!ConfirmResult::Cancelled.is_confirmed());
    }
}
