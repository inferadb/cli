//! Doctor view for environment health checks.
//!
//! A full-screen TUI for displaying development cluster diagnostics
//! including dependency checks, service status, and configuration validation.

use teapot::{
    Cmd, Model,
    components::{Column, FooterHints, Table, TitleBar},
    style::{Color, RESET},
    terminal::{Event, KeyCode},
    util::{ScrollState, measure_text},
};

use super::status_view::EnvironmentStatus;

/// A single check result for display.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Category (e.g., "Dependencies", "Services", "Configuration").
    pub category: String,
    /// Component name (e.g., "Docker", "kubectl").
    pub component: String,
    /// Status message (e.g., "✓ v1.2.3", "✗ NOT FOUND").
    pub status: String,
}

impl CheckResult {
    /// Create a new check result.
    pub fn new(
        category: impl Into<String>,
        component: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self { category: category.into(), component: component.into(), status: status.into() }
    }

    /// Create a success result.
    pub fn success(
        category: impl Into<String>,
        component: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(category, component, format!("✓ {}", detail.into()))
    }

    /// Create a failure result.
    pub fn failure(
        category: impl Into<String>,
        component: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self::new(category, component, format!("✗ {}", hint.into()))
    }

    /// Create an optional/warning result.
    pub fn optional(
        category: impl Into<String>,
        component: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(category, component, format!("○ {}", detail.into()))
    }
}

/// Message type for the doctor view.
#[derive(Clone)]
pub enum DevDoctorViewMsg {
    /// Move selection up.
    SelectPrev,
    /// Move selection down.
    SelectNext,
    /// Scroll left (horizontal).
    ScrollLeft,
    /// Scroll right (horizontal).
    ScrollRight,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Quit the view.
    Quit,
    /// Resize the view.
    Resize {
        /// New width.
        width: usize,
        /// New height.
        height: usize,
    },
}

/// Doctor view for environment health checks.
pub struct DevDoctorView {
    /// Terminal width.
    width: usize,
    /// Terminal height.
    height: usize,
    /// Title text.
    title: String,
    /// Subtitle text.
    subtitle: String,
    /// Overall environment status.
    status: EnvironmentStatus,
    /// Check results.
    results: Vec<CheckResult>,
    /// Table columns.
    columns: Vec<Column>,
    /// Table rows.
    rows: Vec<Vec<String>>,
    /// Scroll state for table navigation.
    scroll: ScrollState,
    /// Footer hints.
    footer_hints: Vec<(&'static str, &'static str)>,
}

impl DevDoctorView {
    /// Create a new doctor view.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            title: "InferaDB Development Cluster".to_string(),
            subtitle: "Doctor".to_string(),
            status: EnvironmentStatus::Checking,
            results: Vec::new(),
            columns: vec![
                Column::new("Category"),
                Column::new("Component"),
                Column::new("Status").grow(),
            ],
            rows: Vec::new(),
            scroll: ScrollState::new(),
            footer_hints: vec![("↑/↓", "select"), ("q", "quit")],
        }
    }

    /// Set the overall environment status.
    pub fn with_status(mut self, status: EnvironmentStatus) -> Self {
        self.status = status;
        self
    }

    /// Set the check results.
    pub fn with_results(mut self, results: Vec<CheckResult>) -> Self {
        self.results = results;
        self.sync_rows();
        self
    }

    /// Add a single check result.
    pub fn add_result(&mut self, result: CheckResult) {
        self.results.push(result);
        self.sync_rows();
    }

    /// Get the overall status.
    pub fn status(&self) -> EnvironmentStatus {
        self.status
    }

    /// Check if environment is ready.
    pub fn is_ready(&self) -> bool {
        matches!(self.status, EnvironmentStatus::Ready)
    }

    /// Sync results to table rows.
    fn sync_rows(&mut self) {
        self.rows = self
            .results
            .iter()
            .map(|r| vec![r.category.clone(), r.component.clone(), r.status.clone()])
            .collect();
    }

    /// Get visible rows for the table.
    fn visible_rows(&self) -> usize {
        // title(1) + blank(1) + status(1) + sep(1) + header(1) + sep(1) + footer(1) = 7
        self.height.saturating_sub(7)
    }

    /// Clamp scroll positions.
    fn clamp_scroll(&mut self) {
        self.scroll.clamp(self.rows.len(), self.visible_rows());
    }

    /// Build the table component.
    fn build_table(&self) -> Table {
        Table::new()
            .columns(self.columns.clone())
            .rows(self.rows.clone())
            .height(self.visible_rows())
            .width(self.width)
            .with_h_scroll_offset(self.scroll.h_offset())
            .show_borders(false)
            .header_color(Color::Default)
            .selected_row_color(Color::Cyan)
            .with_cursor_row(self.scroll.selected())
            .with_offset(self.scroll.offset())
    }

    /// Get the content width of the table.
    fn table_content_width(&self) -> usize {
        self.build_table().content_width()
    }

    /// Maximum horizontal scroll offset.
    fn max_h_scroll(&self) -> usize {
        self.table_content_width().saturating_sub(self.width)
    }

    /// Check if horizontal scrolling is possible.
    fn can_scroll_horizontal(&self) -> bool {
        self.table_content_width() > self.width
    }

    /// Render the title bar with dimmed slashes.
    fn render_title_bar(&self) -> String {
        if self.title.is_empty() {
            return String::new();
        }

        let mut bar = TitleBar::new(&self.title).width(self.width);
        if !self.subtitle.is_empty() {
            bar = bar.subtitle(&self.subtitle);
        }
        bar.render()
    }

    /// Render the status line.
    fn render_status_line(&self) -> String {
        let status_line = self.status.badge().render();
        // Right-align status with 1 char padding on right
        let status_len = measure_text(&status_line);
        let padding = self.width.saturating_sub(status_len + 1);
        format!("{}{} ", " ".repeat(padding), status_line)
    }

    /// Render the footer with right-aligned hints.
    fn render_footer(&self) -> String {
        let show_left = self.scroll.h_offset() > 0;
        let show_right =
            self.can_scroll_horizontal() && self.scroll.h_offset() < self.max_h_scroll();

        FooterHints::new()
            .hints(self.footer_hints.iter().map(|(k, d)| (*k, *d)).collect())
            .width(self.width)
            .scroll_left(show_left)
            .scroll_right(show_right)
            .render()
    }
}

impl Model for DevDoctorView {
    type Message = DevDoctorViewMsg;

    fn init(&self) -> Option<Cmd<Self::Message>> {
        None
    }

    fn update(&mut self, msg: Self::Message) -> Option<Cmd<Self::Message>> {
        match msg {
            DevDoctorViewMsg::SelectPrev => {
                self.scroll.select_prev();
            },
            DevDoctorViewMsg::SelectNext => {
                self.scroll.select_next(self.rows.len(), self.visible_rows());
            },
            DevDoctorViewMsg::ScrollLeft => {
                self.scroll.scroll_left(4);
            },
            DevDoctorViewMsg::ScrollRight => {
                self.scroll.scroll_right(4, self.max_h_scroll());
            },
            DevDoctorViewMsg::PageUp => {
                self.scroll.page_up(self.visible_rows());
            },
            DevDoctorViewMsg::PageDown => {
                self.scroll.page_down(self.rows.len(), self.visible_rows());
            },
            DevDoctorViewMsg::Quit => {
                return Some(Cmd::quit());
            },
            DevDoctorViewMsg::Resize { width, height } => {
                self.width = width;
                self.height = height;
            },
        }
        self.clamp_scroll();
        None
    }

    fn view(&self) -> String {
        let mut output = String::new();
        let reset = RESET;
        let dim = Color::BrightBlack.to_ansi_fg();

        // Title bar
        output.push_str(&self.render_title_bar());
        output.push_str("\r\n\r\n");

        // Status line (right-aligned)
        output.push_str(&self.render_status_line());
        output.push_str("\r\n");

        // Separator
        output.push_str(&format!("{}{}{}\r\n", dim, "─".repeat(self.width), reset));

        // Table content
        let table = self.build_table();
        let table_output = table.render();
        let table_lines: Vec<&str> = table_output.lines().collect();
        let content_height = self.visible_rows() + 1; // +1 for header

        for i in 0..content_height {
            if let Some(line) = table_lines.get(i) {
                output.push_str(line);
            }
            output.push_str("\r\n");
        }

        // Padding to push footer to bottom
        let fixed_overhead = 7; // title + blank + status + sep + sep + footer
        let total_content_lines = fixed_overhead + content_height;
        let padding_needed = self.height.saturating_sub(total_content_lines);
        for _ in 0..padding_needed {
            output.push_str("\r\n");
        }

        // Footer separator
        output.push_str(&format!("{}{}{}\r\n", dim, "─".repeat(self.width), reset));

        // Footer hints
        output.push_str(&self.render_footer());

        output
    }

    fn handle_event(&self, event: Event) -> Option<Self::Message> {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Char('q') => Some(DevDoctorViewMsg::Quit),
                KeyCode::Up | KeyCode::Char('k') => Some(DevDoctorViewMsg::SelectPrev),
                KeyCode::Down | KeyCode::Char('j') => Some(DevDoctorViewMsg::SelectNext),
                KeyCode::Left | KeyCode::Char('h') => Some(DevDoctorViewMsg::ScrollLeft),
                KeyCode::Right | KeyCode::Char('l') => Some(DevDoctorViewMsg::ScrollRight),
                KeyCode::PageUp => Some(DevDoctorViewMsg::PageUp),
                KeyCode::PageDown => Some(DevDoctorViewMsg::PageDown),
                _ => None,
            },
            Event::Resize { width, height } => {
                Some(DevDoctorViewMsg::Resize { width: width as usize, height: height as usize })
            },
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doctor_view_creation() {
        let view = DevDoctorView::new(80, 24);
        assert_eq!(view.status, EnvironmentStatus::Checking);
        assert!(view.results.is_empty());
    }

    #[test]
    fn test_check_result_constructors() {
        let success = CheckResult::success("Deps", "Docker", "v24.0.0");
        assert!(success.status.starts_with("✓"));

        let failure = CheckResult::failure("Deps", "kubectl", "NOT FOUND");
        assert!(failure.status.starts_with("✗"));

        let optional = CheckResult::optional("Config", "Tailscale", "not configured");
        assert!(optional.status.starts_with("○"));
    }

    #[test]
    fn test_with_status() {
        let view = DevDoctorView::new(80, 24).with_status(EnvironmentStatus::Ready);
        assert!(view.is_ready());
    }

    #[test]
    fn test_with_results() {
        let results = vec![
            CheckResult::success("Deps", "Docker", "v24.0.0"),
            CheckResult::success("Deps", "kubectl", "v1.30.0"),
        ];
        let view = DevDoctorView::new(80, 24).with_results(results);
        assert_eq!(view.results.len(), 2);
    }
}
