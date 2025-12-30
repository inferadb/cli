//! Output formatting utilities for dev commands.
//!
//! Provides consistent formatting for step output, dot leaders, and status messages.

use crate::error::{Error, Result};
use crate::tui::start_spinner;
use ferment::style::Color;

use super::constants::STEP_LINE_WIDTH;

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
#[allow(dead_code)]
pub enum StepOutcome {
    /// Step completed successfully.
    Success,
    /// Step completed with a custom message.
    SuccessMsg(String),
    /// Step was skipped (with reason).
    Skipped(String),
    /// Step failed with error.
    Failed(String),
}

/// Print a phase header.
pub fn print_phase_header(title: &str) {
    println!("\n  {} ...\n", title);
}

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

/// Format a line with dot leaders to a status suffix.
///
/// Format: `{text} {dots} {status}` where total width is `STEP_LINE_WIDTH`.
/// The dots are dimmed for visual distinction. Status may contain ANSI codes.
pub fn format_dot_leader(text: &str, status: &str) -> String {
    let dim = Color::BrightBlack.to_ansi_fg();
    let green = Color::Green.to_ansi_fg();
    let reset = "\x1b[0m";

    // Color the status based on value
    let status_colored = match status.to_uppercase().as_str() {
        "OK" | "CREATED" | "CONFIGURED" => format!("{}{}{}", green, status, reset),
        "SKIPPED" | "UNCHANGED" => format!("{}{}{}", dim, status, reset),
        _ => status.to_string(),
    };

    // Calculate dots needed: total width - text length - visible status length - 2 spaces
    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(text.len())
        .saturating_sub(visible_len(status))
        .saturating_sub(2);
    let dots = ".".repeat(dots_len);

    format!("{} {}{}{} {}", text, dim, dots, reset, status_colored)
}

/// Format a dot leader line with colored prefix and status for reset output.
pub fn format_reset_dot_leader(prefix: &str, text: &str, status: &str) -> String {
    let dim = Color::BrightBlack.to_ansi_fg();
    let green = Color::Green.to_ansi_fg();
    let reset = "\x1b[0m";

    // Color the prefix
    let prefix_colored = if prefix == "✓" {
        format!("{}{}{}", green, prefix, reset)
    } else {
        format!("{}{}{}", dim, prefix, reset)
    };

    // Color the status based on value
    let status_upper = status.to_uppercase();
    let status_colored = match status_upper.as_str() {
        "OK" | "CREATED" | "CONFIGURED" => format!("{}{}{}", green, status_upper, reset),
        "SKIPPED" | "UNCHANGED" => format!("{}{}{}", dim, status_upper, reset),
        _ => status_upper,
    };

    // Calculate dots needed
    let prefix_len = 1; // visible prefix length (✓ or ○)
    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(prefix_len)
        .saturating_sub(1) // space after prefix
        .saturating_sub(text.len())
        .saturating_sub(visible_len(status))
        .saturating_sub(2); // spaces around dots
    let dots = ".".repeat(dots_len);

    format!(
        "{} {} {}{}{} {}",
        prefix_colored, text, dim, dots, reset, status_colored
    )
}

/// Print a line with a dimmed prefix symbol, dot leaders, and status.
///
/// Format: `{prefix} {text} {dots} {status}` where total width is `STEP_LINE_WIDTH`.
/// Status may contain ANSI codes which are handled correctly.
pub fn print_prefixed_dot_leader(prefix: &str, text: &str, status: &str) {
    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";

    // Calculate dots needed: total width - prefix - text - visible status - spaces
    let prefix_len = prefix.chars().count(); // Use char count for Unicode
    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(prefix_len)
        .saturating_sub(1) // space after prefix
        .saturating_sub(text.len())
        .saturating_sub(visible_len(status))
        .saturating_sub(2); // spaces around dots
    let dots = ".".repeat(dots_len);

    println!(
        "{}{}{} {} {}{}{} {}",
        dim, prefix, reset, text, dim, dots, reset, status
    );
}

/// Print a line with a colored prefix symbol, dot leaders, and status.
///
/// Format: `{prefix} {text} {dots} {status}` where total width is `STEP_LINE_WIDTH`.
/// The `prefix_formatted` should include ANSI codes, `prefix_width` is the visible character count.
/// Status may contain ANSI codes which are handled correctly.
pub fn print_colored_prefix_dot_leader(
    prefix_formatted: &str,
    prefix_width: usize,
    text: &str,
    status: &str,
) {
    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";

    // Calculate dots needed: total width - prefix - text - visible status - spaces
    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(prefix_width)
        .saturating_sub(1) // space after prefix
        .saturating_sub(text.len())
        .saturating_sub(visible_len(status))
        .saturating_sub(2); // spaces around dots
    let dots = ".".repeat(dots_len);

    println!(
        "{} {} {}{}{} {}",
        prefix_formatted, text, dim, dots, reset, status
    );
}

/// Print a hint line with a dimmed circle prefix.
///
/// Format: `○ {text}` where the circle is dimmed.
pub fn print_hint(text: &str) {
    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";

    println!("{}○{} {}", dim, reset, text);
}

/// Print a skipped destroy step in dot-leader format.
///
/// Outputs: `○ {text} ....... SKIPPED` (dimmed)
pub fn print_destroy_skipped(text: &str) {
    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";

    let prefix = "○";
    let prefix_width = 1;
    let status = "SKIPPED";
    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(prefix_width)
        .saturating_sub(1) // space after prefix
        .saturating_sub(text.len())
        .saturating_sub(status.len())
        .saturating_sub(2); // spaces around dots
    let dots = ".".repeat(dots_len);

    println!(
        "{}{}{} {} {}{}{} {}{}{}",
        dim, prefix, reset, text, dim, dots, reset, dim, status, reset
    );
}

/// Run a destroy step with spinner, then show dot-leader format on completion.
///
/// Shows `{in_progress}...` spinner while running, then outputs:
/// - `✓ {completed} ....... OK` on success (green checkmark and OK)
/// - `○ {completed} ....... SKIPPED` if nothing to do (dimmed)
/// - `✗ {completed} ....... FAILED` on failure
///
/// Returns whether work was done (for tracking if anything was destroyed).
pub fn run_destroy_step<F>(in_progress: &str, completed: &str, executor: F) -> bool
where
    F: FnOnce() -> std::result::Result<StepOutcome, String>,
{
    let mut spin = start_spinner(in_progress);

    match executor() {
        Ok(StepOutcome::Success) | Ok(StepOutcome::SuccessMsg(_)) => {
            spin.stop();
            let green = Color::Green.to_ansi_fg();
            let dim = Color::BrightBlack.to_ansi_fg();
            let reset = "\x1b[0m";

            let checkmark = "✓";
            let prefix_width = 1;
            let status = format!("{}OK{}", green, reset);
            let dots_len = STEP_LINE_WIDTH
                .saturating_sub(prefix_width)
                .saturating_sub(1)
                .saturating_sub(completed.len())
                .saturating_sub(2)
                .saturating_sub(2);
            let dots = ".".repeat(dots_len);

            println!(
                "{}{}{} {} {}{}{} {}",
                green, checkmark, reset, completed, dim, dots, reset, status
            );
            true
        }
        Ok(StepOutcome::Skipped(_)) => {
            spin.stop();
            print_destroy_skipped(completed);
            false
        }
        Ok(StepOutcome::Failed(e)) | Err(e) => {
            spin.failure(&e);
            false
        }
    }
}

/// Run a step with spinner and format output according to the new design.
pub fn run_step<F>(step: &StartStep, executor: F) -> Result<()>
where
    F: FnOnce() -> std::result::Result<StepOutcome, String>,
{
    let spin = start_spinner(step.in_progress.to_string());

    match executor() {
        Ok(outcome) => {
            let (success_text, is_skipped) = match &outcome {
                StepOutcome::Success => (step.completed.to_string(), false),
                StepOutcome::SuccessMsg(msg) => (msg.clone(), false),
                StepOutcome::Skipped(_) => (step.completed.to_string(), true),
                StepOutcome::Failed(err) => {
                    spin.failure(err);
                    return Err(Error::Other(err.clone()));
                }
            };

            // Format output with optional dot leaders
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
        }
        Err(e) => {
            spin.failure(&e);
            Err(Error::Other(e))
        }
    }
}

/// Run a step with spinner that returns a value on success.
pub fn run_step_with_result<F, T>(step: &StartStep, executor: F) -> Result<T>
where
    F: FnOnce() -> std::result::Result<(StepOutcome, T), String>,
{
    let spin = start_spinner(step.in_progress.to_string());

    match executor() {
        Ok((outcome, value)) => {
            let (success_text, is_skipped) = match &outcome {
                StepOutcome::Success => (step.completed.to_string(), false),
                StepOutcome::SuccessMsg(msg) => (msg.clone(), false),
                StepOutcome::Skipped(_) => (step.completed.to_string(), true),
                StepOutcome::Failed(err) => {
                    spin.failure(err);
                    return Err(Error::Other(err.clone()));
                }
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
        }
        Err(e) => {
            spin.failure(&e);
            Err(Error::Other(e))
        }
    }
}

/// Print a styled header for major sections.
pub fn print_styled_header(title: &str) {
    println!("\n  {}", title);
}

/// Print a section header for subsections.
pub fn print_section_header(title: &str) {
    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";
    println!("\n  {}{}{}", dim, title, reset);
}
