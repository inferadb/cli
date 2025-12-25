//! Table formatting for CLI output.
//!
//! Provides aligned, human-readable table output with support for:
//! - Column headers
//! - Auto-sizing columns based on content
//! - Terminal width detection
//! - Truncation for long values

use unicode_width::UnicodeWidthStr;

/// A simple table formatter.
#[derive(Debug, Default)]
pub struct TableFormatter {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    min_widths: Vec<usize>,
}

impl TableFormatter {
    /// Create a new table formatter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the column headers.
    pub fn headers<I, S>(&mut self, headers: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.headers = headers.into_iter().map(|s| s.into()).collect();
        self
    }

    /// Add a row to the table.
    pub fn row<I, S>(&mut self, cells: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.rows
            .push(cells.into_iter().map(|s| s.into()).collect());
        self
    }

    /// Set minimum column widths.
    pub fn min_widths(&mut self, widths: Vec<usize>) -> &mut Self {
        self.min_widths = widths;
        self
    }

    /// Calculate column widths based on content.
    fn calculate_widths(&self) -> Vec<usize> {
        let num_cols = self
            .headers
            .len()
            .max(self.rows.first().map(|r| r.len()).unwrap_or(0));

        let mut widths = vec![0usize; num_cols];

        // Consider header widths
        for (i, header) in self.headers.iter().enumerate() {
            widths[i] = widths[i].max(UnicodeWidthStr::width(header.as_str()));
        }

        // Consider row content widths
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(UnicodeWidthStr::width(cell.as_str()));
                }
            }
        }

        // Apply minimum widths
        for (i, min) in self.min_widths.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(*min);
            }
        }

        widths
    }

    /// Get the terminal width.
    fn terminal_width() -> usize {
        terminal_size::terminal_size()
            .map(|(w, _)| w.0 as usize)
            .unwrap_or(80)
    }

    /// Print the table to stdout.
    pub fn print(&self) {
        let widths = self.calculate_widths();
        let _term_width = Self::terminal_width();

        // Print headers
        if !self.headers.is_empty() {
            self.print_row(&self.headers, &widths, true);
        }

        // Print rows
        for row in &self.rows {
            self.print_row(row, &widths, false);
        }
    }

    /// Print a single row.
    fn print_row(&self, cells: &[String], widths: &[usize], is_header: bool) {
        let mut parts = Vec::new();

        for (i, cell) in cells.iter().enumerate() {
            let width = widths.get(i).copied().unwrap_or(0);
            let cell_width = UnicodeWidthStr::width(cell.as_str());

            if cell_width <= width {
                // Pad the cell
                let padding = width - cell_width;
                parts.push(format!("{}{}", cell, " ".repeat(padding)));
            } else {
                // Truncate the cell
                parts.push(truncate(cell, width));
            }
        }

        let line = parts.join("  ");

        if is_header {
            println!("{}", line);
            // Print separator
            let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
            println!("{}", sep.join("  "));
        } else {
            println!("{}", line);
        }
    }

    /// Render the table as a string.
    pub fn render(&self) -> String {
        let widths = self.calculate_widths();
        let mut output = String::new();

        // Print headers
        if !self.headers.is_empty() {
            output.push_str(&self.row_to_string(&self.headers, &widths));
            output.push('\n');
            // Separator
            let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
            output.push_str(&sep.join("  "));
            output.push('\n');
        }

        // Print rows
        for row in &self.rows {
            output.push_str(&self.row_to_string(row, &widths));
            output.push('\n');
        }

        output
    }

    fn row_to_string(&self, cells: &[String], widths: &[usize]) -> String {
        let parts: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let width = widths.get(i).copied().unwrap_or(0);
                let cell_width = UnicodeWidthStr::width(cell.as_str());

                if cell_width <= width {
                    let padding = width - cell_width;
                    format!("{}{}", cell, " ".repeat(padding))
                } else {
                    truncate(cell, width)
                }
            })
            .collect();

        parts.join("  ")
    }
}

/// Truncate a string to fit within a given width.
fn truncate(s: &str, max_width: usize) -> String {
    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let mut width = 0;
    let mut chars = String::new();

    for c in s.chars() {
        let char_width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if width + char_width + 3 > max_width {
            chars.push_str("...");
            break;
        }
        chars.push(c);
        width += char_width;
    }

    // Pad to exact width if needed
    let current_width = UnicodeWidthStr::width(chars.as_str());
    if current_width < max_width {
        chars.push_str(&" ".repeat(max_width - current_width));
    }

    chars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_basic() {
        let mut table = TableFormatter::new();
        table.headers(["Name", "Value"]);
        table.row(["foo", "bar"]);
        table.row(["baz", "qux"]);

        let output = table.render();
        assert!(output.contains("Name"));
        assert!(output.contains("foo"));
        assert!(output.contains("bar"));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("hi", 10).trim(), "hi");
    }
}
