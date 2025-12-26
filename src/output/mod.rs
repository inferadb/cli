//! Output formatting for the CLI.
//!
//! Supports multiple output formats:
//! - `table` - Human-readable table (default)
//! - `json` - Structured JSON
//! - `yaml` - YAML format
//! - `jsonl` - JSON Lines (one object per line)
//!
//! Also provides styled terminal display utilities via the `display` module.

mod display;
mod table;

pub use display::{
    colors, get_terminal_width, is_terminal, print_boxed, print_error, print_header, print_info,
    print_kv, print_phase, print_success, print_warning, DisplayEntry, EntryStyle, ProgressBox,
    StyledDisplay,
};
pub use table::TableFormatter;

use crate::error::Result;
use serde::Serialize;
use std::io::{self, Write};

/// Output format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Human-readable table format.
    #[default]
    Table,
    /// JSON format.
    Json,
    /// YAML format.
    Yaml,
    /// JSON Lines format (one object per line).
    JsonLines,
}

impl OutputFormat {
    /// Parse an output format from a string.
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "table" => Ok(Self::Table),
            "json" => Ok(Self::Json),
            "yaml" => Ok(Self::Yaml),
            "jsonl" | "jsonlines" => Ok(Self::JsonLines),
            _ => Err(crate::error::Error::invalid_arg(format!(
                "Unknown output format '{}'. Use: table, json, yaml, jsonl",
                s
            ))),
        }
    }
}

/// Trait for types that can be displayed in the CLI.
pub trait Displayable {
    /// Display as a table row.
    fn table_row(&self) -> Vec<String>;

    /// Get column headers for table display.
    fn table_headers() -> Vec<&'static str>;
}

/// Output writer that handles format selection and terminal capabilities.
pub struct Output {
    format: OutputFormat,
    color: bool,
    quiet: bool,
}

impl Output {
    /// Create a new output writer.
    pub fn new(format: OutputFormat, color: bool, quiet: bool) -> Self {
        Self {
            format,
            color,
            quiet,
        }
    }

    /// Create an output writer from CLI options.
    pub fn from_cli(format: &str, color: &str, quiet: bool) -> Result<Self> {
        let format = OutputFormat::parse(format)?;

        let color = match color {
            "always" => true,
            "never" => false,
            _ => atty::is(atty::Stream::Stdout),
        };

        Ok(Self::new(format, color, quiet))
    }

    /// Output a single serializable value.
    pub fn value<T: Serialize>(&self, value: &T) -> Result<()> {
        match self.format {
            OutputFormat::Json => self.json(value),
            OutputFormat::Yaml => self.yaml(value),
            OutputFormat::JsonLines => self.jsonl(value),
            OutputFormat::Table => {
                // For single values in table mode, fall back to JSON
                self.json(value)
            }
        }
    }

    /// Output a list of items as a table.
    pub fn table<T: Displayable + Serialize>(&self, items: &[T]) -> Result<()> {
        match self.format {
            OutputFormat::Table => {
                let mut formatter = TableFormatter::new();
                formatter.headers(T::table_headers());
                for item in items {
                    formatter.row(item.table_row());
                }
                formatter.print();
                Ok(())
            }
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&items)?;
                println!("{}", json);
                Ok(())
            }
            OutputFormat::Yaml => {
                let yaml = serde_yaml::to_string(&items)?;
                print!("{}", yaml);
                Ok(())
            }
            OutputFormat::JsonLines => {
                for item in items {
                    self.jsonl(item)?;
                }
                Ok(())
            }
        }
    }

    /// Output a single item with table format.
    pub fn item<T: Displayable + Serialize + Clone>(&self, item: &T) -> Result<()> {
        match self.format {
            OutputFormat::Table => {
                let mut formatter = TableFormatter::new();
                formatter.headers(T::table_headers());
                formatter.row(item.table_row());
                formatter.print();
                Ok(())
            }
            OutputFormat::Json => self.json(item),
            OutputFormat::Yaml => self.yaml(item),
            OutputFormat::JsonLines => self.jsonl(item),
        }
    }

    /// Output raw JSON.
    fn json<T: Serialize + ?Sized>(&self, value: &T) -> Result<()> {
        let json = serde_json::to_string_pretty(value)?;
        println!("{}", json);
        Ok(())
    }

    /// Output YAML.
    fn yaml<T: Serialize + ?Sized>(&self, value: &T) -> Result<()> {
        let yaml = serde_yaml::to_string(value)?;
        print!("{}", yaml);
        Ok(())
    }

    /// Output JSON Lines (one object per line).
    fn jsonl<T: Serialize>(&self, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        println!("{}", json);
        Ok(())
    }

    /// Print a message to stderr (info, warnings, progress).
    pub fn info(&self, message: &str) {
        if !self.quiet {
            eprintln!("{}", message);
        }
    }

    /// Print a success message.
    pub fn success(&self, message: &str) {
        if !self.quiet {
            if self.color {
                eprintln!("\x1b[32m✓\x1b[0m {}", message);
            } else {
                eprintln!("✓ {}", message);
            }
        }
    }

    /// Print a warning message.
    pub fn warn(&self, message: &str) {
        if !self.quiet {
            if self.color {
                eprintln!("\x1b[33m⚠\x1b[0m {}", message);
            } else {
                eprintln!("⚠ {}", message);
            }
        }
    }

    /// Print an error message.
    pub fn error(&self, message: &str) {
        if self.color {
            eprintln!("\x1b[31m✗\x1b[0m {}", message);
        } else {
            eprintln!("✗ {}", message);
        }
    }

    /// Print raw text to stdout.
    pub fn raw(&self, text: &str) {
        print!("{}", text);
        let _ = io::stdout().flush();
    }

    /// Print a line to stdout.
    pub fn line(&self, text: &str) {
        println!("{}", text);
    }

    /// Check if output is in quiet mode.
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Check if color is enabled.
    pub fn has_color(&self) -> bool {
        self.color
    }

    /// Get the current output format.
    pub fn format(&self) -> OutputFormat {
        self.format
    }
}

impl Default for Output {
    fn default() -> Self {
        Self::new(OutputFormat::Table, true, false)
    }
}

// atty crate functionality (inline to avoid extra dependency)
mod atty {
    #[allow(dead_code)]
    pub enum Stream {
        Stdout,
        Stderr,
    }

    pub fn is(stream: Stream) -> bool {
        use std::os::unix::io::AsRawFd;
        let fd = match stream {
            Stream::Stdout => std::io::stdout().as_raw_fd(),
            Stream::Stderr => std::io::stderr().as_raw_fd(),
        };
        unsafe { libc::isatty(fd) != 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_parse() {
        assert_eq!(OutputFormat::parse("table").unwrap(), OutputFormat::Table);
        assert_eq!(OutputFormat::parse("json").unwrap(), OutputFormat::Json);
        assert_eq!(OutputFormat::parse("yaml").unwrap(), OutputFormat::Yaml);
        assert_eq!(
            OutputFormat::parse("jsonl").unwrap(),
            OutputFormat::JsonLines
        );
        assert!(OutputFormat::parse("invalid").is_err());
    }
}
