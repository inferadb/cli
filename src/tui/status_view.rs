//! Interactive status view for dev cluster.
//!
//! A full-screen TUI for viewing cluster status with tabs for
//! URLs, Services, Nodes, and Pods.

use std::sync::Arc;
use std::time::Duration;

use ferment::components::{BadgeVariant, Column, StatusBadge, Tab, TabBar, Table};
use ferment::runtime::Sub;
use ferment::style::Color;
use ferment::terminal::{Event, KeyCode};
use ferment::util::measure_text;
use ferment::{Cmd, Model};

/// Data returned by a refresh callback.
#[derive(Clone)]
pub struct RefreshResult {
    /// Cluster status.
    pub cluster_status: ClusterStatus,
    /// URLs tab data.
    pub urls: TabData,
    /// Services tab data.
    pub services: TabData,
    /// Nodes tab data.
    pub nodes: TabData,
    /// Pods tab data.
    pub pods: TabData,
}

/// Type alias for the refresh callback function.
pub type RefreshFn = Arc<dyn Fn() -> RefreshResult + Send + Sync>;

/// The active tab in the status view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusTab {
    /// URLs tab showing cluster endpoints.
    #[default]
    Urls,
    /// Services tab showing Kubernetes services.
    Services,
    /// Nodes tab showing cluster nodes.
    Nodes,
    /// Pods tab showing running pods.
    Pods,
}

impl StatusTab {
    /// Get the tab ID.
    pub fn id(&self) -> &'static str {
        match self {
            StatusTab::Urls => "urls",
            StatusTab::Services => "services",
            StatusTab::Nodes => "nodes",
            StatusTab::Pods => "pods",
        }
    }

    /// Get the tab key hint.
    pub fn key(&self) -> char {
        match self {
            StatusTab::Urls => 'u',
            StatusTab::Services => 's',
            StatusTab::Nodes => 'n',
            StatusTab::Pods => 'p',
        }
    }

    /// Get the tab display name.
    pub fn label(&self) -> &'static str {
        match self {
            StatusTab::Urls => "urls",
            StatusTab::Services => "services",
            StatusTab::Nodes => "nodes",
            StatusTab::Pods => "pods",
        }
    }

    /// All tabs in order.
    pub fn all() -> &'static [StatusTab] {
        &[
            StatusTab::Urls,
            StatusTab::Services,
            StatusTab::Nodes,
            StatusTab::Pods,
        ]
    }

    /// Create from ID string.
    pub fn from_id(id: &str) -> Option<StatusTab> {
        match id {
            "urls" => Some(StatusTab::Urls),
            "services" => Some(StatusTab::Services),
            "nodes" => Some(StatusTab::Nodes),
            "pods" => Some(StatusTab::Pods),
            _ => None,
        }
    }
}

/// A row of data for display in a table.
#[derive(Debug, Clone)]
pub struct TableRow {
    /// The cell values for this row.
    pub cells: Vec<String>,
}

impl TableRow {
    /// Create a new table row with the given cells.
    pub fn new(cells: Vec<String>) -> Self {
        Self { cells }
    }
}

/// Data for a tab's table.
#[derive(Debug, Clone, Default)]
pub struct TabData {
    /// Column headers.
    pub headers: Vec<String>,
    /// Data rows.
    pub rows: Vec<TableRow>,
}

impl TabData {
    /// Create new tab data with headers and rows.
    pub fn new(headers: Vec<String>, rows: Vec<TableRow>) -> Self {
        Self { headers, rows }
    }
}

/// Cluster online status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClusterStatus {
    /// Status is unknown or cannot be determined.
    #[default]
    Unknown,
    /// Cluster is online and healthy.
    Online,
    /// Cluster is offline or unreachable.
    Offline,
    /// Cluster is paused or suspended.
    Paused,
}

impl ClusterStatus {
    /// Get a StatusBadge for this status.
    pub fn badge(&self) -> StatusBadge {
        match self {
            ClusterStatus::Online => StatusBadge::new("Online").variant(BadgeVariant::Success),
            ClusterStatus::Offline => StatusBadge::new("Offline").variant(BadgeVariant::Error),
            ClusterStatus::Paused => StatusBadge::new("Paused").variant(BadgeVariant::Warning),
            ClusterStatus::Unknown => StatusBadge::new("Unknown").variant(BadgeVariant::Neutral),
        }
    }
}

/// Environment readiness status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EnvironmentStatus {
    /// Status is being checked.
    #[default]
    Checking,
    /// Environment is ready.
    Ready,
    /// Environment is not ready.
    NotReady,
}

impl EnvironmentStatus {
    /// Get the display text for this status.
    pub fn text(&self) -> &'static str {
        match self {
            EnvironmentStatus::Checking => "Checking...",
            EnvironmentStatus::Ready => "Ready",
            EnvironmentStatus::NotReady => "Not Ready",
        }
    }

    /// Get the color for this status.
    pub fn color(&self) -> Color {
        match self {
            EnvironmentStatus::Checking => Color::Yellow,
            EnvironmentStatus::Ready => Color::Green,
            EnvironmentStatus::NotReady => Color::Red,
        }
    }

    /// Get the status indicator icon.
    pub fn indicator(&self) -> &'static str {
        match self {
            EnvironmentStatus::Checking => "◌",
            EnvironmentStatus::Ready => "✓",
            EnvironmentStatus::NotReady => "✗",
        }
    }
}

/// Message type for the status view.
#[derive(Clone)]
pub enum StatusViewMsg {
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
    /// Switch to a tab by ID.
    SwitchTab(String),
    /// Switch to next tab.
    NextTab,
    /// Switch to previous tab.
    PrevTab,
    /// Cycle through sort options (column and direction).
    CycleSort,
    /// Quit the view.
    Quit,
    /// Resize the view.
    Resize {
        /// New width.
        width: usize,
        /// New height.
        height: usize,
    },
    /// Tick (triggers data refresh).
    Tick,
    /// Data has been refreshed.
    RefreshData(RefreshResult),
}

/// Interactive status view state.
pub struct StatusView {
    /// Terminal width.
    width: usize,
    /// Terminal height.
    height: usize,
    /// Title text.
    title: String,
    /// Subtitle text.
    subtitle: String,
    /// Tab bar.
    tab_bar: TabBar,
    /// Current tab (tracked separately for data switching).
    current_tab: StatusTab,
    /// Cluster status.
    pub cluster_status: ClusterStatus,
    /// Data for URLs tab.
    pub urls_data: TabData,
    /// Data for services tab.
    pub services_data: TabData,
    /// Data for nodes tab.
    pub nodes_data: TabData,
    /// Data for pods tab.
    pub pods_data: TabData,
    /// Current table columns.
    columns: Vec<Column>,
    /// Current table rows.
    rows: Vec<Vec<String>>,
    /// Selected row index.
    selected_row: usize,
    /// Vertical scroll offset.
    scroll_offset: usize,
    /// Horizontal scroll offset.
    h_scroll_offset: usize,
    /// Column index to sort by.
    sort_column: usize,
    /// Sort direction: true = ascending, false = descending.
    sort_ascending: bool,
    /// Footer hints.
    footer_hints: Vec<(&'static str, &'static str)>,
    /// Optional refresh callback for auto-refresh.
    refresh_fn: Option<RefreshFn>,
    /// Refresh interval in seconds (default: 5).
    refresh_interval_secs: u64,
}

impl StatusView {
    /// Create a new status view.
    pub fn new(width: usize, height: usize) -> Self {
        let tabs: Vec<Tab> = StatusTab::all()
            .iter()
            .map(|t| Tab::new(t.id(), t.label()).key(t.key()))
            .collect();

        let tab_bar = TabBar::new()
            .tabs(tabs)
            .active_color(Color::Cyan)
            .inactive_color(Color::BrightBlack)
            .key_color(Color::Cyan);

        Self {
            width,
            height,
            title: "InferaDB Development Cluster".to_string(),
            subtitle: "Status".to_string(),
            tab_bar,
            current_tab: StatusTab::Urls,
            cluster_status: ClusterStatus::Unknown,
            urls_data: TabData::default(),
            services_data: TabData::default(),
            nodes_data: TabData::default(),
            pods_data: TabData::default(),
            columns: Vec::new(),
            rows: Vec::new(),
            selected_row: 0,
            scroll_offset: 0,
            h_scroll_offset: 0,
            sort_column: 0,
            sort_ascending: true,
            footer_hints: vec![
                ("tab/⇧tab", "tabs"),
                ("S", "sort"),
                ("↑/↓", "select"),
                ("q", "quit"),
            ],
            refresh_fn: None,
            refresh_interval_secs: 5,
        }
    }

    /// Set the refresh callback for auto-refresh.
    pub fn with_refresh<F>(mut self, f: F) -> Self
    where
        F: Fn() -> RefreshResult + Send + Sync + 'static,
    {
        self.refresh_fn = Some(Arc::new(f));
        self
    }

    /// Set the refresh interval in seconds (default: 5).
    pub fn with_refresh_interval(mut self, secs: u64) -> Self {
        self.refresh_interval_secs = secs;
        self
    }

    /// Set the cluster status.
    pub fn with_status(mut self, status: ClusterStatus) -> Self {
        self.cluster_status = status;
        self
    }

    /// Set URLs data.
    pub fn with_urls(mut self, data: TabData) -> Self {
        self.urls_data = data;
        self.sync_current_tab_data();
        self
    }

    /// Set services data.
    pub fn with_services(mut self, data: TabData) -> Self {
        self.services_data = data;
        self.sync_current_tab_data();
        self
    }

    /// Set nodes data.
    pub fn with_nodes(mut self, data: TabData) -> Self {
        self.nodes_data = data;
        self.sync_current_tab_data();
        self
    }

    /// Set pods data.
    pub fn with_pods(mut self, data: TabData) -> Self {
        self.pods_data = data;
        self.sync_current_tab_data();
        self
    }

    /// Sync internal columns/rows with current tab's data.
    fn sync_current_tab_data(&mut self) {
        // Get reference to data based on current tab
        let (headers, rows) = match self.current_tab {
            StatusTab::Urls => (&self.urls_data.headers, &self.urls_data.rows),
            StatusTab::Services => (&self.services_data.headers, &self.services_data.rows),
            StatusTab::Nodes => (&self.nodes_data.headers, &self.nodes_data.rows),
            StatusTab::Pods => (&self.pods_data.headers, &self.pods_data.rows),
        };

        // Clamp sort_column to valid range
        let num_cols = headers.len();
        if num_cols > 0 && self.sort_column >= num_cols {
            self.sort_column = 0;
        }

        // Create columns with sort indicator on sorted column
        self.columns = headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let header_text = if i == self.sort_column && num_cols > 0 {
                    let arrow = if self.sort_ascending { "▲" } else { "▼" };
                    format!("{} {}", h, arrow)
                } else {
                    h.clone()
                };
                let col = Column::new(&header_text);
                if i == 0 {
                    col.grow()
                } else {
                    col
                }
            })
            .collect();

        // Clone and sort rows
        let mut sorted_rows: Vec<Vec<String>> = rows.iter().map(|r| r.cells.clone()).collect();
        if num_cols > 0 {
            let col = self.sort_column;
            sorted_rows.sort_by(|a, b| {
                let a_val = a.get(col).map(|s| s.as_str()).unwrap_or("");
                let b_val = b.get(col).map(|s| s.as_str()).unwrap_or("");
                let cmp = a_val.cmp(b_val);
                if self.sort_ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }
        self.rows = sorted_rows;
    }

    /// Handle tab switch: update current_tab and sync data.
    fn handle_tab_switch(&mut self) {
        if let Some(new_tab) = StatusTab::from_id(self.tab_bar.selected_id()) {
            if new_tab != self.current_tab {
                self.current_tab = new_tab;
                self.selected_row = 0;
                self.scroll_offset = 0;
                self.h_scroll_offset = 0;
                // Reset sort to first column ascending when switching tabs
                self.sort_column = 0;
                self.sort_ascending = true;
                self.sync_current_tab_data();
            }
        }
    }

    /// Cycle through sort options: col1 asc → col1 desc → col2 asc → col2 desc → ... → wrap
    fn cycle_sort(&mut self) {
        let num_cols = self.columns.len();
        if num_cols == 0 {
            return;
        }

        if self.sort_ascending {
            // Currently ascending, switch to descending
            self.sort_ascending = false;
        } else {
            // Currently descending, move to next column (ascending)
            self.sort_ascending = true;
            self.sort_column = (self.sort_column + 1) % num_cols;
        }
        self.sync_current_tab_data();
    }

    /// Get visible rows for the table.
    fn visible_rows(&self) -> usize {
        // title(1) + blank(1) + tabs/status(1) + sep(1) + header(1) + sep(1) + footer(1) = 7
        self.height.saturating_sub(7)
    }

    /// Clamp scroll positions.
    fn clamp_scroll(&mut self) {
        let row_count = self.rows.len();
        let visible = self.visible_rows();
        let max_scroll = row_count.saturating_sub(visible);
        self.scroll_offset = self.scroll_offset.min(max_scroll);
        self.selected_row = self.selected_row.min(row_count.saturating_sub(1));
    }

    /// Build the table component.
    fn build_table(&self) -> Table {
        Table::new()
            .columns(self.columns.clone())
            .rows(self.rows.clone())
            .height(self.visible_rows())
            .width(self.width)
            .with_h_scroll_offset(self.h_scroll_offset)
            .show_borders(false)
            .header_color(Color::Default)
            .selected_row_color(Color::Cyan)
            .with_cursor_row(self.selected_row)
            .with_offset(self.scroll_offset)
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

        let reset = "\x1b[0m";
        let dim = Color::BrightBlack.to_ansi_fg();

        if self.subtitle.is_empty() {
            // No subtitle: "// Title //////..."
            let prefix = format!("{}//{}  {}  ", dim, reset, self.title);
            let prefix_len = 2 + 2 + self.title.len() + 2;
            let remaining = self.width.saturating_sub(prefix_len);
            let fill = format!("{}{}{}", dim, "/".repeat(remaining), reset);
            format!("{}{}\r\n\r\n", prefix, fill)
        } else {
            // With subtitle: "//  Title  /////...  Subtitle  //"
            let prefix_len = 2 + 2 + self.title.len() + 2;
            let suffix_len = 2 + self.subtitle.len() + 2 + 2;
            let fill_count = self.width.saturating_sub(prefix_len + suffix_len);
            let fill = "/".repeat(fill_count);
            format!(
                "{}//{}  {}  {}{}{}  {}  {}//{}",
                dim, reset, self.title, dim, fill, reset, self.subtitle, dim, reset
            )
        }
    }

    /// Render the footer with right-aligned hints.
    fn render_footer(&self) -> String {
        let reset = "\x1b[0m";
        let dim = Color::BrightBlack.to_ansi_fg();

        // Build styled hints
        let mut styled_hints = String::new();
        let mut plain_len = 0;

        for (i, (shortcut, desc)) in self.footer_hints.iter().enumerate() {
            if i > 0 {
                styled_hints.push_str("  ");
                plain_len += 2;
            }
            styled_hints.push_str(reset);
            styled_hints.push_str(shortcut);
            plain_len += measure_text(shortcut);
            styled_hints.push_str(&dim);
            styled_hints.push(' ');
            styled_hints.push_str(desc);
            plain_len += 1 + desc.len();
        }
        styled_hints.push_str(reset);

        // Scroll indicators
        let show_left = self.h_scroll_offset > 0;
        let show_right = self.can_scroll_horizontal() && self.h_scroll_offset < self.max_h_scroll();
        let left_indicator = if show_left { "◀ " } else { "  " };
        let right_indicator = if show_right { " ▶" } else { "  " };
        let indicators_len = 4;
        let padding = self.width.saturating_sub(plain_len + indicators_len);

        format!(
            "{}{}{}{}{}{}",
            dim,
            left_indicator,
            " ".repeat(padding),
            styled_hints,
            right_indicator,
            reset
        )
    }
}

impl Model for StatusView {
    type Message = StatusViewMsg;

    fn init(&self) -> Option<Cmd<Self::Message>> {
        None
    }

    fn update(&mut self, msg: Self::Message) -> Option<Cmd<Self::Message>> {
        match msg {
            StatusViewMsg::SelectPrev => {
                if self.selected_row > 0 {
                    self.selected_row -= 1;
                    if self.selected_row < self.scroll_offset {
                        self.scroll_offset = self.selected_row;
                    }
                }
            }
            StatusViewMsg::SelectNext => {
                if self.selected_row < self.rows.len().saturating_sub(1) {
                    self.selected_row += 1;
                    let visible = self.visible_rows();
                    if self.selected_row >= self.scroll_offset + visible {
                        self.scroll_offset = self.selected_row.saturating_sub(visible - 1);
                    }
                }
            }
            StatusViewMsg::ScrollLeft => {
                self.h_scroll_offset = self.h_scroll_offset.saturating_sub(4);
            }
            StatusViewMsg::ScrollRight => {
                let max = self.max_h_scroll();
                if self.h_scroll_offset + 4 <= max {
                    self.h_scroll_offset += 4;
                } else {
                    self.h_scroll_offset = max;
                }
            }
            StatusViewMsg::PageUp => {
                let page_size = self.visible_rows().saturating_sub(1);
                self.selected_row = self.selected_row.saturating_sub(page_size);
                if self.selected_row < self.scroll_offset {
                    self.scroll_offset = self.selected_row;
                }
            }
            StatusViewMsg::PageDown => {
                let page_size = self.visible_rows().saturating_sub(1);
                self.selected_row =
                    (self.selected_row + page_size).min(self.rows.len().saturating_sub(1));
                let visible = self.visible_rows();
                if self.selected_row >= self.scroll_offset + visible {
                    self.scroll_offset = self.selected_row.saturating_sub(visible - 1);
                }
            }
            StatusViewMsg::SwitchTab(id) => {
                self.tab_bar.set_selected(&id);
                self.handle_tab_switch();
            }
            StatusViewMsg::NextTab => {
                self.tab_bar.update(ferment::components::TabBarMsg::Next);
                self.handle_tab_switch();
            }
            StatusViewMsg::PrevTab => {
                self.tab_bar
                    .update(ferment::components::TabBarMsg::Previous);
                self.handle_tab_switch();
            }
            StatusViewMsg::CycleSort => {
                self.cycle_sort();
            }
            StatusViewMsg::Quit => {
                return Some(Cmd::quit());
            }
            StatusViewMsg::Resize { width, height } => {
                self.width = width;
                self.height = height;
            }
            StatusViewMsg::Tick => {
                // Trigger data refresh if callback is available
                if let Some(ref refresh_fn) = self.refresh_fn {
                    let f = Arc::clone(refresh_fn);
                    return Some(Cmd::perform(move || StatusViewMsg::RefreshData(f())));
                }
            }
            StatusViewMsg::RefreshData(result) => {
                // Apply the refreshed data
                self.cluster_status = result.cluster_status;
                self.urls_data = result.urls;
                self.services_data = result.services;
                self.nodes_data = result.nodes;
                self.pods_data = result.pods;
                self.sync_current_tab_data();
            }
        }
        self.clamp_scroll();
        None
    }

    fn view(&self) -> String {
        let mut output = String::new();
        let reset = "\x1b[0m";
        let dim = Color::BrightBlack.to_ansi_fg();

        // Title bar
        output.push_str(&self.render_title_bar());
        output.push_str("\r\n\r\n");

        // Tab bar + Status line
        let tabs_rendered = self.tab_bar.render();
        let tabs_len = measure_text(&tabs_rendered);
        let status_str = self.cluster_status.badge().render();
        let status_len = measure_text(&status_str);
        // 1 char padding on left and right
        let padding = self.width.saturating_sub(tabs_len + status_len + 2);
        output.push(' ');
        output.push_str(&tabs_rendered);
        output.push_str(&" ".repeat(padding));
        output.push_str(&status_str);
        output.push_str(" \r\n");

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
        let fixed_overhead = 7; // title + blank + tabs + sep + sep + footer
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
                KeyCode::Char('q') | KeyCode::Esc => Some(StatusViewMsg::Quit),
                KeyCode::Up | KeyCode::Char('k') => Some(StatusViewMsg::SelectPrev),
                KeyCode::Down | KeyCode::Char('j') => Some(StatusViewMsg::SelectNext),
                KeyCode::Left | KeyCode::Char('h') => Some(StatusViewMsg::ScrollLeft),
                KeyCode::Right | KeyCode::Char('l') => Some(StatusViewMsg::ScrollRight),
                KeyCode::PageUp => Some(StatusViewMsg::PageUp),
                KeyCode::PageDown => Some(StatusViewMsg::PageDown),
                KeyCode::Tab => Some(StatusViewMsg::NextTab),
                KeyCode::BackTab => Some(StatusViewMsg::PrevTab),
                KeyCode::Char('S') => Some(StatusViewMsg::CycleSort),
                KeyCode::Char(c) => {
                    // Check if character matches any tab's key shortcut
                    if let Some(id) = self.tab_bar.tab_for_key(c) {
                        return Some(StatusViewMsg::SwitchTab(id.to_string()));
                    }
                    None
                }
                _ => None,
            },
            Event::Resize { width, height } => Some(StatusViewMsg::Resize {
                width: width as usize,
                height: height as usize,
            }),
            _ => None,
        }
    }

    fn subscriptions(&self) -> Sub<Self::Message> {
        if self.refresh_fn.is_some() {
            Sub::interval(
                "status-refresh",
                Duration::from_secs(self.refresh_interval_secs),
                || StatusViewMsg::Tick,
            )
        } else {
            Sub::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_view_creation() {
        let view = StatusView::new(80, 24);
        assert_eq!(view.current_tab, StatusTab::Urls);
        assert_eq!(view.cluster_status, ClusterStatus::Unknown);
    }

    #[test]
    fn test_tab_switching() {
        let mut view = StatusView::new(80, 24);
        view.update(StatusViewMsg::SwitchTab("services".to_string()));
        assert_eq!(view.current_tab, StatusTab::Services);
    }

    #[test]
    fn test_cluster_status_badge() {
        let online_badge = ClusterStatus::Online.badge();
        assert!(online_badge.render().contains("Online"));

        let offline_badge = ClusterStatus::Offline.badge();
        assert!(offline_badge.render().contains("Offline"));
    }
}
