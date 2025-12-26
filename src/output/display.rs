//! Styled terminal display utilities for the InferaDB CLI.
//!
//! Provides consistent, structured output with TRON-style aesthetics:
//! - Terminal-width aware layouts
//! - Box-drawing character tables
//! - ANSI color support with TTY detection
//! - Responsive design for narrow terminals

use std::io::IsTerminal;

use terminal_size::{terminal_size, Width};
use unicode_width::UnicodeWidthStr;

/// ANSI color codes for TRON aesthetic
pub mod colors {
    /// Reset all formatting
    pub const RESET: &str = "\x1b[0m";
    /// Bold text
    pub const BOLD: &str = "\x1b[1m";
    /// Dimmed text
    pub const DIM: &str = "\x1b[2m";
    /// Cyan text (borders, structure)
    pub const CYAN: &str = "\x1b[36m";
    /// Bright cyan text (highlights)
    pub const BRIGHT_CYAN: &str = "\x1b[96m";
    /// Green text (success, values)
    pub const GREEN: &str = "\x1b[32m";
    /// Yellow text (warnings)
    pub const YELLOW: &str = "\x1b[33m";
    /// Red text (errors)
    pub const RED: &str = "\x1b[31m";
    /// Blue text
    pub const BLUE: &str = "\x1b[34m";
    /// Magenta text
    pub const MAGENTA: &str = "\x1b[35m";
}

/// ASCII art for "INFERADB" in FIGlet-style block letters
const ASCII_ART: &[&str] = &[
    "██╗███╗   ██╗███████╗███████╗██████╗  █████╗ ██████╗ ██████╗ ",
    "██║████╗  ██║██╔════╝██╔════╝██╔══██╗██╔══██╗██╔══██╗██╔══██╗",
    "██║██╔██╗ ██║█████╗  █████╗  ██████╔╝███████║██║  ██║██████╔╝",
    "██║██║╚██╗██║██╔══╝  ██╔══╝  ██╔══██╗██╔══██║██║  ██║██╔══██╗",
    "██║██║ ╚████║██║     ███████╗██║  ██║██║  ██║██████╔╝██████╔╝",
    "╚═╝╚═╝  ╚═══╝╚═╝     ╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝ ╚═════╝ ",
];

/// Width of the full ASCII art (in characters)
const ASCII_ART_WIDTH: usize = 61;

/// Minimum terminal width for full ASCII art display
const MIN_WIDTH_FOR_FULL_ART: usize = 80;

/// Minimum terminal width for table display
const MIN_WIDTH_FOR_TABLE: usize = 50;

/// Get terminal width, defaulting to 80 if detection fails
pub fn get_terminal_width() -> usize {
    terminal_size()
        .map(|(Width(w), _)| w as usize)
        .unwrap_or(80)
}

/// Check if stdout is a terminal (TTY)
pub fn is_terminal() -> bool {
    std::io::stdout().is_terminal()
}

/// Style variant for display entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntryStyle {
    /// Normal display (green text)
    #[default]
    Normal,
    /// Success indicator (green with checkmark)
    Success,
    /// Warning display (yellow text)
    Warning,
    /// Error display (red text)
    Error,
    /// Dimmed/disabled display
    Dimmed,
    /// Sensitive value (masked with ********)
    Sensitive,
    /// Separator line (renders as horizontal divider)
    Separator,
}

/// A single entry for display in a status table
#[derive(Debug, Clone)]
pub struct DisplayEntry {
    /// Category/group name (e.g., "Cluster", "Network")
    pub category: &'static str,
    /// Property name (left column)
    pub name: String,
    /// Value (right column)
    pub value: String,
    /// Display style
    pub style: EntryStyle,
}

impl DisplayEntry {
    /// Create a new entry with default (normal) style
    pub fn new(
        category: &'static str,
        name: impl Into<String>,
        value: impl ToString,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            value: value.to_string(),
            style: EntryStyle::Normal,
        }
    }

    /// Create a success entry (green with checkmark)
    pub fn success(
        category: &'static str,
        name: impl Into<String>,
        value: impl ToString,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            value: value.to_string(),
            style: EntryStyle::Success,
        }
    }

    /// Create a warning entry (yellow)
    pub fn warning(
        category: &'static str,
        name: impl Into<String>,
        value: impl ToString,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            value: value.to_string(),
            style: EntryStyle::Warning,
        }
    }

    /// Create an error entry (red)
    pub fn error(
        category: &'static str,
        name: impl Into<String>,
        value: impl ToString,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            value: value.to_string(),
            style: EntryStyle::Error,
        }
    }

    /// Create a dimmed entry
    pub fn dimmed(
        category: &'static str,
        name: impl Into<String>,
        value: impl ToString,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            value: value.to_string(),
            style: EntryStyle::Dimmed,
        }
    }

    /// Create a sensitive entry (value will be masked)
    pub fn sensitive(
        category: &'static str,
        name: impl Into<String>,
        _value: impl ToString,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            value: "********".to_string(),
            style: EntryStyle::Sensitive,
        }
    }

    /// Create a separator (horizontal divider)
    pub fn separator(category: &'static str) -> Self {
        Self {
            category,
            name: String::new(),
            value: String::new(),
            style: EntryStyle::Separator,
        }
    }

    /// Convert to success style
    pub fn as_success(mut self) -> Self {
        self.style = EntryStyle::Success;
        self
    }

    /// Convert to warning style
    pub fn as_warning(mut self) -> Self {
        self.style = EntryStyle::Warning;
        self
    }

    /// Convert to error style
    pub fn as_error(mut self) -> Self {
        self.style = EntryStyle::Error;
        self
    }
}

/// Builder for creating styled status displays
pub struct StyledDisplay {
    title: Option<String>,
    subtitle: Option<String>,
    entries: Vec<DisplayEntry>,
    use_ansi: bool,
    show_banner: bool,
}

impl StyledDisplay {
    /// Create a new styled display builder
    pub fn new() -> Self {
        Self {
            title: None,
            subtitle: None,
            entries: Vec::new(),
            use_ansi: is_terminal(),
            show_banner: false,
        }
    }

    /// Set the title (shown above the table)
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the subtitle (shown below the title)
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Show the ASCII banner
    pub fn with_banner(mut self) -> Self {
        self.show_banner = true;
        self
    }

    /// Override ANSI color detection
    pub fn with_ansi(mut self, use_ansi: bool) -> Self {
        self.use_ansi = use_ansi;
        self
    }

    /// Add a single entry
    pub fn entry(mut self, entry: DisplayEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Add multiple entries
    pub fn entries(mut self, entries: impl IntoIterator<Item = DisplayEntry>) -> Self {
        self.entries.extend(entries);
        self
    }

    /// Display everything
    pub fn display(&self) {
        if self.show_banner {
            self.print_banner();
        }

        if let Some(title) = &self.title {
            self.print_title(title, self.subtitle.as_deref());
        }

        if !self.entries.is_empty() {
            self.print_entries();
        }
    }

    fn print_banner(&self) {
        let width = get_terminal_width();

        if width >= MIN_WIDTH_FOR_FULL_ART {
            self.print_full_banner(width);
        } else {
            self.print_compact_banner(width);
        }
    }

    fn print_full_banner(&self, terminal_width: usize) {
        let (reset, bold, bright_cyan) = if self.use_ansi {
            (colors::RESET, colors::BOLD, colors::BRIGHT_CYAN)
        } else {
            ("", "", "")
        };

        let art_left_pad = terminal_width.saturating_sub(ASCII_ART_WIDTH) / 2;
        let art_indent = " ".repeat(art_left_pad);

        println!();
        for line in ASCII_ART {
            println!("{art_indent}{bold}{bright_cyan}{line}{reset}");
        }
        println!();
    }

    fn print_compact_banner(&self, terminal_width: usize) {
        let (reset, bold, bright_cyan) = if self.use_ansi {
            (colors::RESET, colors::BOLD, colors::BRIGHT_CYAN)
        } else {
            ("", "", "")
        };

        println!();
        let title = "▀▀▀ INFERADB ▀▀▀";
        let left_pad = terminal_width.saturating_sub(title.len()) / 2;
        println!(
            "{pad}{bold}{bright_cyan}{title}{reset}",
            pad = " ".repeat(left_pad)
        );
        println!();
    }

    fn print_title(&self, title: &str, subtitle: Option<&str>) {
        let (reset, bold, dim, cyan) = if self.use_ansi {
            (colors::RESET, colors::BOLD, colors::DIM, colors::CYAN)
        } else {
            ("", "", "", "")
        };

        println!("{bold}{cyan}# {title}{reset}");
        if let Some(sub) = subtitle {
            println!("{dim}  {sub}{reset}");
        }
        println!();
    }

    fn print_entries(&self) {
        let terminal_width = get_terminal_width();

        // Group entries by category
        let mut categories: Vec<(&str, Vec<&DisplayEntry>)> = Vec::new();
        for entry in &self.entries {
            if let Some((_, entries)) = categories
                .iter_mut()
                .find(|(cat, _)| *cat == entry.category)
            {
                entries.push(entry);
            } else {
                categories.push((entry.category, vec![entry]));
            }
        }

        if terminal_width >= MIN_WIDTH_FOR_TABLE {
            self.print_tables(&categories, terminal_width);
        } else {
            self.print_simple(&categories);
        }
    }

    fn print_tables(&self, categories: &[(&str, Vec<&DisplayEntry>)], terminal_width: usize) {
        let (reset, dim, cyan) = if self.use_ansi {
            (colors::RESET, colors::DIM, colors::CYAN)
        } else {
            ("", "", "")
        };

        for (category, entries) in categories {
            // Category header
            println!("{dim}{category}{reset}");

            // Calculate column widths
            let max_name_len = entries.iter().map(|e| e.name.len()).max().unwrap_or(0);
            let name_col_width = max_name_len;

            // Value column gets remaining space
            let value_col_width = terminal_width
                .saturating_sub(3) // 3 border characters
                .saturating_sub(4) // 4 padding spaces
                .saturating_sub(name_col_width)
                .max(10);

            // Top border
            println!(
                "{cyan}╔{name_border}╦{val_border}╗{reset}",
                name_border = "═".repeat(name_col_width + 2),
                val_border = "═".repeat(value_col_width + 2)
            );

            // Rows
            for entry in entries {
                if entry.style == EntryStyle::Separator {
                    println!(
                        "{cyan}╠{name_border}╬{val_border}╣{reset}",
                        name_border = "═".repeat(name_col_width + 2),
                        val_border = "═".repeat(value_col_width + 2)
                    );
                    continue;
                }

                let (display_value, value_display_len) =
                    self.format_value(&entry.value, entry.style, value_col_width);

                let value_padding = value_col_width.saturating_sub(value_display_len);

                println!(
                    "{cyan}║{reset} {name:<name_width$} {cyan}║{reset} {val}{padding} {cyan}║{reset}",
                    name = entry.name,
                    name_width = name_col_width,
                    val = display_value,
                    padding = " ".repeat(value_padding)
                );
            }

            // Bottom border
            println!(
                "{cyan}╚{name_border}╩{val_border}╝{reset}",
                name_border = "═".repeat(name_col_width + 2),
                val_border = "═".repeat(value_col_width + 2)
            );

            println!();
        }
    }

    fn format_value(&self, value: &str, style: EntryStyle, max_width: usize) -> (String, usize) {
        let (green, yellow, red, dim, reset) = if self.use_ansi {
            (
                colors::GREEN,
                colors::YELLOW,
                colors::RED,
                colors::DIM,
                colors::RESET,
            )
        } else {
            ("", "", "", "", "")
        };

        let (color, prefix) = match style {
            EntryStyle::Normal => (green, ""),
            EntryStyle::Success => (green, "✓ "),
            EntryStyle::Warning => (yellow, ""),
            EntryStyle::Error => (red, "✗ "),
            EntryStyle::Dimmed => (dim, "○ "),
            EntryStyle::Sensitive => (yellow, ""),
            EntryStyle::Separator => unreachable!(),
        };

        let full_value = format!("{}{}", prefix, value);
        let display_width = full_value.width();

        if display_width > max_width {
            // Truncate with ellipsis
            let mut truncated = String::from(prefix);
            let mut width = prefix.len();
            for c in value.chars() {
                let char_width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                if width + char_width > max_width.saturating_sub(3) {
                    break;
                }
                truncated.push(c);
                width += char_width;
            }
            (format!("{color}{}...{reset}", truncated), max_width)
        } else {
            (format!("{color}{}{reset}", full_value), display_width)
        }
    }

    fn print_simple(&self, categories: &[(&str, Vec<&DisplayEntry>)]) {
        let (reset, dim, green, yellow, red) = if self.use_ansi {
            (
                colors::RESET,
                colors::DIM,
                colors::GREEN,
                colors::YELLOW,
                colors::RED,
            )
        } else {
            ("", "", "", "", "")
        };

        for (category, entries) in categories {
            println!("{dim}{category}{reset}");
            for entry in entries {
                if entry.style == EntryStyle::Separator {
                    println!("{dim}  ────{reset}");
                    continue;
                }

                let (color, prefix) = match entry.style {
                    EntryStyle::Normal => (green, ""),
                    EntryStyle::Success => (green, "✓ "),
                    EntryStyle::Warning => (yellow, ""),
                    EntryStyle::Error => (red, "✗ "),
                    EntryStyle::Dimmed => (dim, "○ "),
                    EntryStyle::Sensitive => (yellow, ""),
                    EntryStyle::Separator => unreachable!(),
                };

                println!(
                    "  {}: {color}{prefix}{}{reset}",
                    entry.name, entry.value
                );
            }
            println!();
        }
    }
}

impl Default for StyledDisplay {
    fn default() -> Self {
        Self::new()
    }
}

/// Print a section header with underline
pub fn print_header(title: &str) {
    let use_ansi = is_terminal();
    let (reset, bold, dim, cyan) = if use_ansi {
        (colors::RESET, colors::BOLD, colors::DIM, colors::CYAN)
    } else {
        ("", "", "", "")
    };

    println!("{bold}{cyan}{title}{reset}");
    println!("{dim}{}{reset}", "─".repeat(title.width()));
    println!();
}

/// Print a phase/step indicator
pub fn print_phase(phase: &str) {
    let use_ansi = is_terminal();
    let (reset, dim) = if use_ansi {
        (colors::RESET, colors::DIM)
    } else {
        ("", "")
    };

    println!("{dim}━━━ {phase} ━━━{reset}");
}

/// Print a success message
pub fn print_success(message: &str) {
    let use_ansi = is_terminal();
    let (reset, green) = if use_ansi {
        (colors::RESET, colors::GREEN)
    } else {
        ("", "")
    };

    println!("{green}✓{reset} {message}");
}

/// Print a warning message
pub fn print_warning(message: &str) {
    let use_ansi = is_terminal();
    let (reset, yellow) = if use_ansi {
        (colors::RESET, colors::YELLOW)
    } else {
        ("", "")
    };

    println!("{yellow}⚠{reset} {message}");
}

/// Print an error message
pub fn print_error(message: &str) {
    let use_ansi = is_terminal();
    let (reset, red) = if use_ansi {
        (colors::RESET, colors::RED)
    } else {
        ("", "")
    };

    eprintln!("{red}✗{reset} {message}");
}

/// Print an info/skipped message
pub fn print_info(message: &str) {
    let use_ansi = is_terminal();
    let (reset, dim) = if use_ansi {
        (colors::RESET, colors::DIM)
    } else {
        ("", "")
    };

    println!("{dim}○{reset} {message}");
}

/// Print a simple key-value pair
pub fn print_kv(key: &str, value: &str) {
    let use_ansi = is_terminal();
    let (reset, dim, green) = if use_ansi {
        (colors::RESET, colors::DIM, colors::GREEN)
    } else {
        ("", "", "")
    };

    println!("{dim}{key}:{reset} {green}{value}{reset}");
}

/// Print a boxed message (for important notices)
pub fn print_boxed(title: &str, lines: &[&str]) {
    let use_ansi = is_terminal();
    let (reset, bold, yellow) = if use_ansi {
        (colors::RESET, colors::BOLD, colors::YELLOW)
    } else {
        ("", "", "")
    };

    let terminal_width = get_terminal_width();
    let max_line_len = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let content_width = terminal_width
        .saturating_sub(4)
        .max(max_line_len)
        .max(title.len());

    let title_left_pad = content_width.saturating_sub(title.len()) / 2;
    let title_right_pad = content_width.saturating_sub(title_left_pad + title.len());

    println!();
    println!(
        "{yellow}╔{border}╗{reset}",
        border = "═".repeat(content_width + 2)
    );
    println!(
        "{yellow}║{reset} {left}{bold}{title}{reset}{right} {yellow}║{reset}",
        left = " ".repeat(title_left_pad),
        right = " ".repeat(title_right_pad)
    );
    println!(
        "{yellow}╠{border}╣{reset}",
        border = "═".repeat(content_width + 2)
    );

    for line in lines {
        let padding = content_width.saturating_sub(line.len());
        println!(
            "{yellow}║{reset} {line}{padding} {yellow}║{reset}",
            padding = " ".repeat(padding)
        );
    }

    println!(
        "{yellow}╚{border}╝{reset}",
        border = "═".repeat(content_width + 2)
    );
    println!();
}

/// A progress box for multi-step operations like installation.
///
/// Prints output incrementally as steps are completed:
/// ```text
/// ╔═ Title ═══════════════════════════════════════════════════════════════════════╗
/// ╟ Step name                                                                     ║
/// ║ ✓ Success message                                                             ║
/// ║ ○ Info message                                                                ║
/// ║                                                                               ║
/// ╟ Another step                                                                  ║
/// ║ ✓ Done                                                                        ║
/// ╚═══════════════════════════════════════════════════════════════════════════════╝
/// ```
pub struct ProgressBox {
    width: usize,
    use_ansi: bool,
}

impl ProgressBox {
    /// Start a new progress box with the given title.
    /// Prints the top border immediately.
    pub fn new(title: &str) -> Self {
        let use_ansi = is_terminal();
        let width = get_terminal_width().saturating_sub(2); // Account for box edges

        let (reset, bold, cyan) = if use_ansi {
            (colors::RESET, colors::BOLD, colors::CYAN)
        } else {
            ("", "", "")
        };

        // Top border with embedded title: ╔═ Title ═══...═══╗
        let title_with_padding = format!(" {} ", title);
        let title_display_width = title_with_padding.width();
        let remaining = width.saturating_sub(title_display_width + 1); // +1 for the ═ before title

        println!(
            "{cyan}╔═{reset}{bold}{title}{reset}{cyan}{border}╗{reset}",
            title = title_with_padding,
            border = "═".repeat(remaining)
        );

        Self { width, use_ansi }
    }

    /// Calculate padding needed to reach the right border
    fn calc_padding(&self, content: &str) -> String {
        let content_width = content.width();
        let padding = self.width.saturating_sub(content_width);
        " ".repeat(padding)
    }

    /// Print a step header (e.g., "Clone deployment repository")
    pub fn step(&self, name: &str) {
        let (reset, cyan) = if self.use_ansi {
            (colors::RESET, colors::CYAN)
        } else {
            ("", "")
        };

        let content = format!(" {}", name);
        let pad = self.calc_padding(&content);

        println!("{cyan}╟{reset}{content}{pad}{cyan}║{reset}");
    }

    /// Print a success message with checkmark
    pub fn success(&self, message: &str) {
        let (reset, cyan, green) = if self.use_ansi {
            (colors::RESET, colors::CYAN, colors::GREEN)
        } else {
            ("", "", "")
        };

        let content = format!(" ✓ {}", message);
        let pad = self.calc_padding(&content);

        println!("{cyan}║{reset} {green}✓{reset} {message}{pad}{cyan}║{reset}");
    }

    /// Print an info/neutral message
    pub fn info(&self, message: &str) {
        let (reset, cyan, dim) = if self.use_ansi {
            (colors::RESET, colors::CYAN, colors::DIM)
        } else {
            ("", "", "")
        };

        let content = format!(" ○ {}", message);
        let pad = self.calc_padding(&content);

        println!("{cyan}║{reset} {dim}○{reset} {message}{pad}{cyan}║{reset}");
    }

    /// Print a warning message
    pub fn warning(&self, message: &str) {
        let (reset, cyan, yellow) = if self.use_ansi {
            (colors::RESET, colors::CYAN, colors::YELLOW)
        } else {
            ("", "", "")
        };

        let content = format!(" ⚠ {}", message);
        let pad = self.calc_padding(&content);

        println!("{cyan}║{reset} {yellow}⚠{reset} {message}{pad}{cyan}║{reset}");
    }

    /// Print an error message
    pub fn error(&self, message: &str) {
        let (reset, cyan, red) = if self.use_ansi {
            (colors::RESET, colors::CYAN, colors::RED)
        } else {
            ("", "", "")
        };

        let content = format!(" ✗ {}", message);
        let pad = self.calc_padding(&content);

        println!("{cyan}║{reset} {red}✗{reset} {message}{pad}{cyan}║{reset}");
    }

    /// Print a blank line within the box
    pub fn blank(&self) {
        let (reset, cyan) = if self.use_ansi {
            (colors::RESET, colors::CYAN)
        } else {
            ("", "")
        };

        println!("{cyan}║{pad}║{reset}", pad = " ".repeat(self.width));
    }

    /// Print arbitrary text within the box (indented)
    pub fn text(&self, message: &str) {
        let (reset, cyan) = if self.use_ansi {
            (colors::RESET, colors::CYAN)
        } else {
            ("", "")
        };

        let content = format!("   {}", message);
        let pad = self.calc_padding(&content);

        println!("{cyan}║{reset}{content}{pad}{cyan}║{reset}");
    }

    /// End the progress box by printing the bottom border
    pub fn end(&self) {
        let (reset, cyan) = if self.use_ansi {
            (colors::RESET, colors::CYAN)
        } else {
            ("", "")
        };

        println!("{cyan}╚{border}╝{reset}", border = "═".repeat(self.width));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_creation() {
        let entry = DisplayEntry::new("Test", "Key", "Value");
        assert_eq!(entry.category, "Test");
        assert_eq!(entry.name, "Key");
        assert_eq!(entry.value, "Value");
        assert_eq!(entry.style, EntryStyle::Normal);
    }

    #[test]
    fn test_entry_styles() {
        let success = DisplayEntry::success("Test", "Key", "Value");
        assert_eq!(success.style, EntryStyle::Success);

        let warning = DisplayEntry::warning("Test", "Key", "Value");
        assert_eq!(warning.style, EntryStyle::Warning);

        let error = DisplayEntry::error("Test", "Key", "Value");
        assert_eq!(error.style, EntryStyle::Error);

        let sensitive = DisplayEntry::sensitive("Test", "Key", "secret");
        assert_eq!(sensitive.style, EntryStyle::Sensitive);
        assert_eq!(sensitive.value, "********");
    }

    #[test]
    fn test_styled_display_builder() {
        let display = StyledDisplay::new()
            .title("Test Title")
            .subtitle("Test Subtitle")
            .with_ansi(false)
            .entry(DisplayEntry::new("Category", "Key", "Value"));

        assert_eq!(display.title, Some("Test Title".to_string()));
        assert_eq!(display.subtitle, Some("Test Subtitle".to_string()));
        assert!(!display.use_ansi);
        assert_eq!(display.entries.len(), 1);
    }

    #[test]
    fn test_terminal_width() {
        let width = get_terminal_width();
        assert!(width > 0);
    }

    #[test]
    fn test_ascii_art_dimensions() {
        for line in ASCII_ART {
            assert_eq!(
                line.chars().count(),
                ASCII_ART_WIDTH,
                "ASCII art line has inconsistent width"
            );
        }
    }
}
