//! Output formatting for the CLI.
//!
//! Provides format selection (table/json/yaml/jsonl) and integrates with Teapot
//! for table rendering. For message output (success, error, warning, info),
//! use `teapot::output` directly.

use std::io::IsTerminal;

use serde::Serialize;
use teapot::{
    components::{Column, Table},
    output as toutput,
};

use crate::error::Result;

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
                "Unknown output format '{s}'. Use: table, json, yaml, jsonl"
            ))),
        }
    }
}

/// Trait for types that can be displayed as table rows.
pub trait Displayable {
    /// Display as a table row.
    fn table_row(&self) -> Vec<String>;

    /// Get column headers for table display.
    fn table_headers() -> Vec<&'static str>;
}

/// Output writer that handles format selection.
///
/// For message output (success, error, warning, info), use `teapot::output` directly.
pub struct Output {
    /// The output format.
    pub format: OutputFormat,
    /// Whether color is enabled.
    pub color: bool,
    /// Whether quiet mode is enabled.
    pub quiet: bool,
}

impl Output {
    /// Create a new output writer.
    #[must_use]
    pub const fn new(format: OutputFormat, color: bool, quiet: bool) -> Self {
        Self { format, color, quiet }
    }

    /// Create an output writer from CLI options.
    pub fn from_cli(format: &str, color: &str, quiet: bool) -> Result<Self> {
        let format = OutputFormat::parse(format)?;

        let color = match color {
            "always" => true,
            "never" => false,
            _ => std::io::stdout().is_terminal(),
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
            },
        }
    }

    /// Output a list of items as a table.
    pub fn table<T: Displayable + Serialize>(&self, items: &[T]) -> Result<()> {
        match self.format {
            OutputFormat::Table => {
                let columns: Vec<Column> =
                    T::table_headers().into_iter().map(Column::new).collect();

                let rows: Vec<Vec<String>> = items.iter().map(Displayable::table_row).collect();

                let table =
                    Table::new().columns(columns).rows(rows).show_borders(false).focused(false);

                let output = table.render();
                if self.color {
                    println!("{output}");
                } else {
                    println!("{}", toutput::strip_ansi(&output));
                }
                Ok(())
            },
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&items)?;
                println!("{json}");
                Ok(())
            },
            OutputFormat::Yaml => {
                let yaml = serde_yaml::to_string(&items)?;
                print!("{yaml}");
                Ok(())
            },
            OutputFormat::JsonLines => {
                for item in items {
                    self.jsonl(item)?;
                }
                Ok(())
            },
        }
    }

    /// Output a single item with table format.
    pub fn item<T: Displayable + Serialize + Clone>(&self, item: &T) -> Result<()> {
        match self.format {
            OutputFormat::Table => {
                let columns: Vec<Column> =
                    T::table_headers().into_iter().map(Column::new).collect();

                let table = Table::new()
                    .columns(columns)
                    .rows(vec![item.table_row()])
                    .show_borders(false)
                    .focused(false);

                let output = table.render();
                if self.color {
                    println!("{output}");
                } else {
                    println!("{}", toutput::strip_ansi(&output));
                }
                Ok(())
            },
            OutputFormat::Json => self.json(item),
            OutputFormat::Yaml => self.yaml(item),
            OutputFormat::JsonLines => self.jsonl(item),
        }
    }

    /// Output raw JSON.
    fn json<T: Serialize + ?Sized>(&self, value: &T) -> Result<()> {
        let json = serde_json::to_string_pretty(value)?;
        println!("{json}");
        Ok(())
    }

    /// Output YAML.
    fn yaml<T: Serialize + ?Sized>(&self, value: &T) -> Result<()> {
        let yaml = serde_yaml::to_string(value)?;
        print!("{yaml}");
        Ok(())
    }

    /// Output JSON Lines (one object per line).
    fn jsonl<T: Serialize>(&self, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        println!("{json}");
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Message output methods - thin wrappers around teapot::output
    // These respect the quiet flag before calling Teapot.
    // -------------------------------------------------------------------------

    /// Print an info message (respects quiet mode).
    /// Wraps `teapot::output::info`.
    pub fn info(&self, message: &str) {
        if !self.quiet {
            if self.color {
                toutput::info(message);
            } else {
                eprintln!("- {message}");
            }
        }
    }

    /// Print a success message (respects quiet mode).
    /// Wraps `teapot::output::success`.
    pub fn success(&self, message: &str) {
        if !self.quiet {
            if self.color {
                toutput::success(message);
            } else {
                eprintln!("+ {message}");
            }
        }
    }

    /// Print a warning message (respects quiet mode).
    /// Wraps `teapot::output::warning`.
    pub fn warn(&self, message: &str) {
        if !self.quiet {
            if self.color {
                toutput::warning(message);
            } else {
                eprintln!("! {message}");
            }
        }
    }

    /// Print an error message (always shown, even in quiet mode).
    /// Wraps `teapot::output::error`.
    pub fn error(&self, message: &str) {
        if self.color {
            toutput::error(message);
        } else {
            eprintln!("x {message}");
        }
    }

    // -------------------------------------------------------------------------
    // Accessor methods for compatibility
    // -------------------------------------------------------------------------

    /// Check if output is in quiet mode.
    #[must_use]
    pub const fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Get the current output format.
    #[must_use]
    pub const fn format(&self) -> OutputFormat {
        self.format
    }
}

impl Default for Output {
    fn default() -> Self {
        Self::new(OutputFormat::Table, true, false)
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
        assert_eq!(OutputFormat::parse("jsonl").unwrap(), OutputFormat::JsonLines);
        assert!(OutputFormat::parse("invalid").is_err());
    }
}
