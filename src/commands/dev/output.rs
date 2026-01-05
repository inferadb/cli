//! Output formatting utilities for dev commands.
//!
//! Provides consistent formatting for step output, dot leaders, and status messages.

use teapot::style::RESET;

use super::constants::STEP_LINE_WIDTH;
use crate::{
    error::{Error, Result},
    tui::start_spinner,
};

// ============================================================================
// Color Constants (using Teapot's Color for consistency)
// ============================================================================

// Static color strings cached for performance (Color::*.to_ansi_fg() allocates)
const DIM_ANSI: &str = "\x1b[90m"; // Color::BrightBlack
const GREEN_ANSI: &str = "\x1b[32m"; // Color::Green
const YELLOW_ANSI: &str = "\x1b[33m"; // Color::Yellow
const RED_ANSI: &str = "\x1b[31m"; // Color::Red

/// Get ANSI escape code for dim/gray text.
#[inline]
const fn dim() -> &'static str {
    DIM_ANSI
}

/// Get ANSI escape code for green text.
#[inline]
const fn green() -> &'static str {
    GREEN_ANSI
}

/// Get ANSI escape code for yellow text.
#[inline]
const fn yellow() -> &'static str {
    YELLOW_ANSI
}

/// Get ANSI escape code for red text.
#[inline]
const fn red() -> &'static str {
    RED_ANSI
}

/// Get ANSI reset code.
#[inline]
const fn reset() -> &'static str {
    RESET
}

// ============================================================================
// Step Types
// ============================================================================

/// A step in the start process with in-progress and completed text variants.
pub struct StartStep {
    /// Text shown while the step is running (e.g., "Cloning deployment repository")
    pub in_progress: String,
    /// Text shown when the step completes (e.g., "Cloned deployment repository")
    pub completed: String,
    /// Whether to show dot leaders to status on the right
    pub dot_leader: bool,
}

impl StartStep {
    /// Create a step with dot leaders to status (OK or SKIPPED).
    pub fn with_ok(in_progress: &str, completed: &str) -> Self {
        Self {
            in_progress: in_progress.to_string(),
            completed: completed.to_string(),
            dot_leader: true,
        }
    }
}

/// Result of running a start step.
#[derive(Debug, Clone)]
pub enum StepOutcome {
    /// Step completed successfully.
    Success,
    /// Step was skipped.
    Skipped,
    /// Step failed with error.
    Failed(String),
}

/// Convert `Result<Option<String>, String>` to `StepOutcome`.
///
/// - `Ok(Some(_))` -> `StepOutcome::Skipped`
/// - `Ok(None)` -> `StepOutcome::Success`
/// - `Err(e)` -> `StepOutcome::Failed(e)`
impl From<std::result::Result<Option<String>, String>> for StepOutcome {
    fn from(result: std::result::Result<Option<String>, String>) -> Self {
        match result {
            Ok(Some(_)) => Self::Skipped,
            Ok(None) => Self::Success,
            Err(e) => Self::Failed(e),
        }
    }
}

// ============================================================================
// Dot Leader Configuration
// ============================================================================

/// Configuration for formatting a dot leader line.
#[derive(Default)]
pub struct DotLeaderConfig<'a> {
    /// Optional prefix symbol (e.g., "✓", "○", "✗")
    pub prefix: Option<&'a str>,
    /// Color for the prefix
    pub prefix_color: Option<&'a str>,
    /// Main text content
    pub text: &'a str,
    /// Status text (e.g., "OK", "SKIPPED")
    pub status: &'a str,
    /// Whether to auto-color status based on value
    pub auto_color_status: bool,
}

impl<'a> DotLeaderConfig<'a> {
    /// Create a simple dot leader with just text and status.
    pub fn simple(text: &'a str, status: &'a str) -> Self {
        Self { text, status, auto_color_status: true, ..Default::default() }
    }

    /// Create a dot leader with a prefix.
    pub fn with_prefix(prefix: &'a str, text: &'a str, status: &'a str) -> Self {
        Self { prefix: Some(prefix), text, status, auto_color_status: true, ..Default::default() }
    }

    /// Create a dot leader with a colored prefix.
    pub const fn with_colored_prefix(
        prefix: &'a str,
        prefix_color: &'a str,
        text: &'a str,
        status: &'a str,
    ) -> Self {
        Self {
            prefix: Some(prefix),
            prefix_color: Some(prefix_color),
            text,
            status,
            auto_color_status: true,
        }
    }
}

// ============================================================================
// String Utilities
// ============================================================================

/// Calculate the visible length of a string, stripping ANSI escape sequences.
pub fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else if c == '\x1b' {
            in_escape = true;
        } else {
            len += 1;
        }
    }
    len
}

/// Color a status string based on its value.
fn color_status(status: &str) -> String {
    match status.to_uppercase().as_str() {
        "OK" | "CREATED" | "CONFIGURED" | "RUNNING" | "READY" => {
            format!("{}{}{}", green(), status, reset())
        },
        "SKIPPED" | "UNCHANGED" => format!("{}{}{}", dim(), status, reset()),
        "FAILED" | "ERROR" | "NOT FOUND" | "NOT RUNNING" => {
            format!("{}{}{}", red(), status, reset())
        },
        "STOPPED" | "PAUSED" | "UNKNOWN" => format!("{}{}{}", yellow(), status, reset()),
        _ => status.to_string(),
    }
}

// ============================================================================
// Dot Leader Formatting (Unified)
// ============================================================================

/// Format a line with dot leaders using the configuration.
pub fn format_dot_leader_config(config: &DotLeaderConfig<'_>) -> String {
    let status_display = if config.auto_color_status {
        color_status(config.status)
    } else {
        config.status.to_string()
    };

    // Calculate prefix contribution
    let (prefix_str, prefix_visible_len) = match (config.prefix, config.prefix_color) {
        (Some(p), Some(color)) => (format!("{}{}{} ", color, p, reset()), p.chars().count() + 1),
        (Some(p), None) => (format!("{}{}{} ", dim(), p, reset()), p.chars().count() + 1),
        (None, _) => (String::new(), 0),
    };

    // Calculate dots needed
    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(prefix_visible_len)
        .saturating_sub(config.text.len())
        .saturating_sub(visible_len(config.status))
        .saturating_sub(2); // spaces around dots
    let dots = ".".repeat(dots_len);

    format!("{}{} {}{}{} {}", prefix_str, config.text, dim(), dots, reset(), status_display)
}

/// Format a line with dot leaders to a status suffix.
///
/// Format: `{text} {dots} {status}` where total width is `STEP_LINE_WIDTH`.
/// The dots are dimmed for visual distinction. Status may contain ANSI codes.
pub fn format_dot_leader(text: &str, status: &str) -> String {
    format_dot_leader_config(&DotLeaderConfig::simple(text, status))
}

/// Format a dot leader line with colored prefix and status for reset output.
pub fn format_reset_dot_leader(prefix: &str, text: &str, status: &str) -> String {
    let prefix_color = if prefix == "✓" { green() } else { dim() };
    format_dot_leader_config(&DotLeaderConfig::with_colored_prefix(
        prefix,
        prefix_color,
        text,
        &status.to_uppercase(),
    ))
}

/// Print a line with a dimmed prefix symbol, dot leaders, and status.
pub fn print_prefixed_dot_leader(prefix: &str, text: &str, status: &str) {
    println!("{}", format_dot_leader_config(&DotLeaderConfig::with_prefix(prefix, text, status)));
}

/// Print a line with a colored prefix symbol, dot leaders, and status.
///
/// The `prefix_formatted` should include ANSI codes, `prefix_width` is the visible character count.
pub fn print_colored_prefix_dot_leader(
    prefix_formatted: &str,
    prefix_width: usize,
    text: &str,
    status: &str,
) {
    let status_display = color_status(status);

    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(prefix_width)
        .saturating_sub(1) // space after prefix
        .saturating_sub(text.len())
        .saturating_sub(visible_len(status))
        .saturating_sub(2);
    let dots = ".".repeat(dots_len);

    println!("{} {} {}{}{} {}", prefix_formatted, text, dim(), dots, reset(), status_display);
}

// ============================================================================
// Headers and Hints
// ============================================================================

/// Print a phase header.
pub fn print_phase_header(title: &str) {
    println!("\n  {title} ...\n");
}

/// Print a styled header for major sections.
pub fn print_styled_header(title: &str) {
    println!("\n  {title}");
}

/// Print a section header for subsections.
pub fn print_section_header(title: &str) {
    println!("\n  {}{}{}", dim(), title, reset());
}

/// Print a hint line with a dimmed circle prefix.
pub fn print_hint(text: &str) {
    println!("{}○{} {}", dim(), reset(), text);
}

/// Print a skipped destroy step in dot-leader format.
pub fn print_destroy_skipped(text: &str) {
    println!(
        "{}",
        format_dot_leader_config(&DotLeaderConfig::with_colored_prefix(
            "○",
            dim(),
            text,
            "SKIPPED"
        ))
    );
}

// ============================================================================
// Step Execution
// ============================================================================

/// Run a destroy step with spinner, then show dot-leader format on completion.
///
/// Returns whether work was done (for tracking if anything was destroyed).
pub fn run_destroy_step<F>(in_progress: &str, completed: &str, executor: F) -> bool
where
    F: FnOnce() -> std::result::Result<StepOutcome, String>,
{
    let mut spin = start_spinner(in_progress);

    match executor() {
        Ok(StepOutcome::Success) => {
            spin.stop();
            println!(
                "{}",
                format_dot_leader_config(&DotLeaderConfig::with_colored_prefix(
                    "✓",
                    green(),
                    completed,
                    "OK"
                ))
            );
            true
        },
        Ok(StepOutcome::Skipped) => {
            spin.stop();
            print_destroy_skipped(completed);
            false
        },
        Ok(StepOutcome::Failed(e)) | Err(e) => {
            spin.failure(&e);
            false
        },
    }
}

/// Run a step with spinner and format output according to the new design.
pub fn run_step<F>(step: &StartStep, executor: F) -> Result<()>
where
    F: FnOnce() -> std::result::Result<StepOutcome, String>,
{
    let spin = start_spinner(step.in_progress.clone());

    match executor() {
        Ok(outcome) => {
            let (success_text, is_skipped) = match &outcome {
                StepOutcome::Success => (step.completed.clone(), false),
                StepOutcome::Skipped => (step.completed.clone(), true),
                StepOutcome::Failed(err) => {
                    spin.failure(err);
                    return Err(Error::Other(err.clone()));
                },
            };

            if step.dot_leader {
                let status = if is_skipped { "SKIPPED" } else { "OK" };
                let formatted = format_dot_leader(&success_text, status);
                if is_skipped {
                    spin.info(&formatted);
                } else {
                    spin.success(&formatted);
                }
            } else if is_skipped {
                spin.info(&success_text);
            } else {
                spin.success(&success_text);
            }

            Ok(())
        },
        Err(e) => {
            spin.failure(&e);
            Err(Error::Other(e))
        },
    }
}

/// Run a step with spinner that returns a value on success.
pub fn run_step_with_result<F, T>(step: &StartStep, executor: F) -> Result<T>
where
    F: FnOnce() -> std::result::Result<(StepOutcome, T), String>,
{
    let spin = start_spinner(step.in_progress.clone());

    match executor() {
        Ok((outcome, value)) => {
            let (success_text, is_skipped) = match &outcome {
                StepOutcome::Success => (step.completed.clone(), false),
                StepOutcome::Skipped => (step.completed.clone(), true),
                StepOutcome::Failed(err) => {
                    spin.failure(err);
                    return Err(Error::Other(err.clone()));
                },
            };

            if step.dot_leader {
                let status = if is_skipped { "SKIPPED" } else { "OK" };
                let formatted = format_dot_leader(&success_text, status);
                if is_skipped {
                    spin.info(&formatted);
                } else {
                    spin.success(&formatted);
                }
            } else if is_skipped {
                spin.info(&success_text);
            } else {
                spin.success(&success_text);
            }

            Ok(value)
        },
        Err(e) => {
            spin.failure(&e);
            Err(Error::Other(e))
        },
    }
}

// ============================================================================
// Confirmation Prompts
// ============================================================================

/// Print a confirmation prompt and get user input.
///
/// Returns `true` if user confirms (y/yes), `false` otherwise.
pub fn confirm_prompt(message: &str) -> std::io::Result<bool> {
    use std::io::{self, Write};

    print!("{message} [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    Ok(input == "y" || input == "yes")
}

/// Print a warning confirmation prompt with yellow coloring.
pub fn confirm_warning(message: &str) -> std::io::Result<bool> {
    use std::io::{self, Write};

    print!("{}{}{}. Continue? [y/N] ", yellow(), message, reset());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    Ok(input == "y" || input == "yes")
}
