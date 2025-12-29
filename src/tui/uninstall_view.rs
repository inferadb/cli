//! Interactive uninstall view for dev cluster removal.
//!
//! A full-screen TUI showing a confirmation modal before uninstalling,
//! then progress with animated spinners, task completion status, and error modals.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use ferment::components::{Modal, ModalBorder, TaskList};
use ferment::runtime::Sub;
use ferment::style::Color;
use ferment::terminal::{Event, KeyCode};
use ferment::{Cmd, Model};

use super::install_view::{InstallStep, StepExecutor, StepResult};

// Re-export constants for use by UninstallInfo
const CLUSTER_NAME: &str = "inferadb-dev";
const REGISTRY_NAME: &str = "inferadb-registry";

/// Information about what will be uninstalled.
#[derive(Debug, Clone)]
pub struct UninstallInfo {
    /// Whether cluster exists.
    pub has_cluster: bool,
    /// Cluster status (running/paused).
    pub cluster_status: Option<String>,
    /// Whether registry exists.
    pub has_registry: bool,
    /// Deploy directory path.
    pub deploy_dir: PathBuf,
    /// Whether deploy directory exists.
    pub has_deploy_dir: bool,
    /// Data directory path.
    pub data_dir: PathBuf,
    /// State directory path.
    pub state_dir: PathBuf,
    /// Whether state directory exists.
    pub has_state_dir: bool,
    /// Config directory path.
    pub config_dir: PathBuf,
    /// Credentials file path.
    pub creds_file: PathBuf,
    /// Whether credentials file exists.
    pub has_creds_file: bool,
    /// Number of dev Docker images.
    pub dev_image_count: usize,
    /// Whether kubectl context exists.
    pub has_kube_context: bool,
    /// Whether talos context exists.
    pub has_talos_context: bool,
}

impl UninstallInfo {
    /// Get description lines for what will be removed.
    pub fn removal_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        if self.has_cluster {
            let status = self.cluster_status.as_deref().unwrap_or("unknown");
            lines.push(format!("Talos cluster '{}' ({})", CLUSTER_NAME, status));
        }

        if self.has_registry {
            lines.push(format!("Local Docker registry '{}'", REGISTRY_NAME));
        }

        if self.has_deploy_dir {
            lines.push(format!("Deploy repository: {}", self.deploy_dir.display()));
        }

        if self.has_state_dir {
            lines.push(format!("State directory: {}", self.state_dir.display()));
        }

        if self.dev_image_count > 0 {
            lines.push(format!(
                "{} Docker image(s) (inferadb-*, ghcr.io/siderolabs/*)",
                self.dev_image_count
            ));
        }

        if self.has_kube_context || self.has_talos_context {
            lines.push("Kubernetes/Talos configuration contexts".to_string());
        }

        lines
    }

    /// Check if there's anything to uninstall.
    pub fn has_anything(&self) -> bool {
        self.has_cluster
            || self.has_registry
            || self.has_deploy_dir
            || self.has_state_dir
            || self.dev_image_count > 0
            || self.has_kube_context
            || self.has_talos_context
    }
}

/// Message type for uninstall view.
#[derive(Debug, Clone)]
pub enum DevUninstallViewMsg {
    /// Advance spinner animation and poll for worker results.
    Tick,
    /// User confirmed uninstall.
    Confirm,
    /// User declined/cancelled uninstall.
    Cancel,
    /// Run a specific step.
    RunStep(usize),
    /// A step completed with result.
    StepCompleted(usize, StepResult),
    /// Close error modal.
    CloseModal,
    /// User pressed 'q' to quit/cancel.
    Quit,
    /// Terminal resize.
    Resize(u16, u16),
}

/// Phase of the uninstall process.
#[derive(Debug, Clone, PartialEq)]
enum Phase {
    /// Showing confirmation modal.
    Confirming,
    /// Running uninstall steps.
    Running,
    /// Completed (success or failure).
    Completed,
}

/// Result message from a worker thread.
type WorkerResult = (usize, StepResult);

/// The uninstall view state.
pub struct DevUninstallView {
    /// Title for the view.
    title: String,
    /// Subtitle for the view.
    subtitle: String,
    /// Current phase.
    phase: Phase,
    /// Information about what will be uninstalled.
    info: UninstallInfo,
    /// Whether to also remove credentials.
    with_credentials: bool,
    /// The task list component.
    task_list: TaskList,
    /// Step executors.
    executors: Vec<Option<StepExecutor>>,
    /// Current step index being processed.
    current_step: usize,
    /// Whether a step is currently executing in a worker thread.
    executing: bool,
    /// Receiver for worker thread results.
    result_receiver: Option<Receiver<WorkerResult>>,
    /// Terminal width.
    width: u16,
    /// Terminal height.
    height: u16,
    /// Error modal content (if showing).
    error_modal: Option<(String, String)>,
    /// Whether the view should quit.
    should_quit: bool,
    /// Whether uninstall was cancelled.
    was_cancelled: bool,
}

impl DevUninstallView {
    /// Create a new uninstall view with the given steps and info.
    pub fn new(steps: Vec<InstallStep>, info: UninstallInfo, with_credentials: bool) -> Self {
        let mut task_list = TaskList::new();
        let mut executors = Vec::new();

        for step in steps {
            task_list = task_list.add_task(&step.name);
            executors.push(step.executor);
        }

        // Get terminal size
        let (width, height) = ferment::terminal::size().unwrap_or((80, 24));

        Self {
            title: "InferaDB Development Cluster".to_string(),
            subtitle: "Uninstall".to_string(),
            phase: Phase::Confirming,
            info,
            with_credentials,
            task_list,
            executors,
            current_step: 0,
            executing: false,
            result_receiver: None,
            width,
            height,
            error_modal: None,
            should_quit: false,
            was_cancelled: false,
        }
    }

    /// Set custom title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set custom subtitle.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    /// Check if the view should quit.
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Check if uninstall was cancelled by user.
    pub fn was_cancelled(&self) -> bool {
        self.was_cancelled
    }

    /// Check if uninstall completed successfully.
    pub fn is_success(&self) -> bool {
        self.phase == Phase::Completed
            && self.task_list.is_all_complete()
            && !self.task_list.has_failure()
    }

    /// Check if there was a failure.
    pub fn has_failure(&self) -> bool {
        self.task_list.has_failure()
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
            let remaining = (self.width as usize).saturating_sub(prefix_len);
            let fill = format!("{}{}{}", dim, "/".repeat(remaining), reset);
            format!("{}{}", prefix, fill)
        } else {
            // With subtitle: "//  Title  /////...  Subtitle  //"
            let prefix_len = 2 + 2 + self.title.len() + 2;
            let suffix_len = 2 + self.subtitle.len() + 2 + 2;
            let fill_count = (self.width as usize).saturating_sub(prefix_len + suffix_len);
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

        let separator = format!("{}{}{}", dim, "─".repeat(self.width as usize), reset);

        let hint_text = match self.phase {
            Phase::Confirming => "y confirm  n cancel",
            Phase::Running => "q cancel",
            Phase::Completed => "q quit",
        };

        // Split into styled parts
        let styled_hint = hint_text
            .split("  ")
            .map(|part| {
                if let Some((key, desc)) = part.split_once(' ') {
                    format!("{}{}{} {}{}", reset, key, dim, desc, reset)
                } else {
                    part.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("  ");

        let plain_len = hint_text.len();
        let padding = (self.width as usize).saturating_sub(plain_len);

        format!(
            "{}\r\n{}{}{}",
            separator,
            " ".repeat(padding),
            styled_hint,
            reset
        )
    }

    /// Spawn a worker thread to execute a step.
    fn spawn_step_worker(&self, index: usize) -> Receiver<WorkerResult> {
        let (tx, rx) = mpsc::channel();

        if let Some(Some(executor)) = self.executors.get(index) {
            let executor = Arc::clone(executor);
            thread::spawn(move || {
                let result = executor();
                let step_result = match result {
                    Ok(detail) => StepResult::Success(detail),
                    Err(error) => StepResult::Failure(error),
                };
                let _ = tx.send((index, step_result));
            });
        } else {
            // No executor - auto-succeed
            thread::spawn(move || {
                let _ = tx.send((index, StepResult::Success(None)));
            });
        }

        rx
    }

    /// Poll for worker result.
    fn poll_worker_result(&mut self) -> Option<WorkerResult> {
        if let Some(ref rx) = self.result_receiver {
            match rx.try_recv() {
                Ok(result) => {
                    self.executing = false;
                    self.result_receiver = None;
                    Some(result)
                }
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    self.executing = false;
                    self.result_receiver = None;
                    None
                }
            }
        } else {
            None
        }
    }

    /// Render the confirmation modal.
    fn render_confirm_modal(&self, background: &str) -> String {
        let mut removal_lines = self.info.removal_lines();
        if self.with_credentials && self.info.has_creds_file {
            removal_lines.push("Tailscale credentials".to_string());
        }
        let modal_width = 60.min(self.width as usize - 4);
        let modal_height = (removal_lines.len() + 8).min(self.height as usize - 4);

        // Build content
        let mut content_lines = vec!["This will remove:".to_string(), String::new()];
        for line in &removal_lines {
            content_lines.push(format!("  • {}", line));
        }
        content_lines.push(String::new());
        content_lines.push("Are you sure you want to continue?".to_string());

        let modal = Modal::new(modal_width, modal_height)
            .border(ModalBorder::Rounded)
            .border_color(Color::Yellow)
            .title("Confirm Uninstall")
            .title_color(Color::Yellow)
            .content(content_lines.join("\n"))
            .footer_hints(vec![("y", "confirm"), ("n", "cancel")]);

        modal.render_overlay(self.width as usize, self.height as usize, background)
    }

    /// Render the error modal.
    fn render_error_modal(&self, background: &str) -> String {
        if let Some((task_name, error_msg)) = &self.error_modal {
            let modal_width = 60.min(self.width as usize - 4);
            let modal_height = 10.min(self.height as usize - 4);

            let modal = Modal::new(modal_width, modal_height)
                .border(ModalBorder::Rounded)
                .border_color(Color::Red)
                .title("Error")
                .title_color(Color::Red)
                .content(format!("Failed: {}\n\n{}", task_name, error_msg))
                .footer_hint("esc", "close");

            modal.render_overlay(self.width as usize, self.height as usize, background)
        } else {
            background.to_string()
        }
    }

    /// Start the uninstall process.
    fn start_uninstall(&mut self) -> Option<Cmd<DevUninstallViewMsg>> {
        if !self.executors.is_empty() {
            self.phase = Phase::Running;
            self.current_step = 0;
            self.task_list.start_task(0);
            self.executing = true;
            self.result_receiver = Some(self.spawn_step_worker(0));
            // Return a tick command to ensure polling starts immediately
            return Some(Cmd::tick(Duration::from_millis(80), |_| {
                DevUninstallViewMsg::Tick
            }));
        }
        None
    }
}

impl Model for DevUninstallView {
    type Message = DevUninstallViewMsg;

    fn init(&self) -> Option<Cmd<Self::Message>> {
        // No auto-start - wait for user confirmation
        None
    }

    fn update(&mut self, msg: Self::Message) -> Option<Cmd<Self::Message>> {
        match msg {
            DevUninstallViewMsg::Tick => {
                // Forward tick to task list for spinner animation
                self.task_list
                    .update(ferment::components::TaskListMsg::Tick);

                // Poll for worker result if we're executing
                if let Some((index, result)) = self.poll_worker_result() {
                    // Process the result
                    match &result {
                        StepResult::Success(detail) => {
                            self.task_list.complete_task(index, detail.clone());
                        }
                        StepResult::Skipped(reason) => {
                            self.task_list.skip_task(index, Some(reason.clone()));
                        }
                        StepResult::Failure(error) => {
                            self.task_list.fail_task(index, Some(error.clone()));
                            if let Some(task) = self.task_list.get(index) {
                                self.error_modal = Some((task.name.clone(), error.clone()));
                            }
                            self.phase = Phase::Completed;
                            return None; // Stop on failure
                        }
                    }

                    // Start next step if there is one
                    let next_step = index + 1;
                    if next_step < self.executors.len() {
                        self.current_step = next_step;
                        self.task_list.start_task(next_step);
                        self.executing = true;
                        self.result_receiver = Some(self.spawn_step_worker(next_step));
                        return Some(Cmd::tick(Duration::from_millis(80), |_| {
                            DevUninstallViewMsg::Tick
                        }));
                    } else {
                        // All steps complete
                        self.phase = Phase::Completed;
                    }
                }
                None
            }
            DevUninstallViewMsg::Confirm => {
                if self.phase == Phase::Confirming {
                    return self.start_uninstall();
                }
                None
            }
            DevUninstallViewMsg::Cancel => {
                self.was_cancelled = true;
                self.should_quit = true;
                Some(Cmd::quit())
            }
            DevUninstallViewMsg::RunStep(index) => {
                if index < self.executors.len() && !self.executing {
                    self.task_list.start_task(index);
                    self.executing = true;
                    self.result_receiver = Some(self.spawn_step_worker(index));
                }
                None
            }
            DevUninstallViewMsg::StepCompleted(index, result) => {
                match &result {
                    StepResult::Success(detail) => {
                        self.task_list.complete_task(index, detail.clone());
                    }
                    StepResult::Skipped(reason) => {
                        self.task_list.skip_task(index, Some(reason.clone()));
                    }
                    StepResult::Failure(error) => {
                        self.task_list.fail_task(index, Some(error.clone()));
                        if let Some(task) = self.task_list.get(index) {
                            self.error_modal = Some((task.name.clone(), error.clone()));
                        }
                    }
                }
                None
            }
            DevUninstallViewMsg::CloseModal => {
                self.error_modal = None;
                None
            }
            DevUninstallViewMsg::Quit => {
                if self.error_modal.is_some() {
                    // Close modal first
                    self.error_modal = None;
                    None
                } else {
                    self.should_quit = true;
                    if self.phase != Phase::Completed {
                        self.was_cancelled = true;
                    }
                    Some(Cmd::quit())
                }
            }
            DevUninstallViewMsg::Resize(w, h) => {
                self.width = w;
                self.height = h;
                None
            }
        }
    }

    fn view(&self) -> String {
        let mut output = String::new();

        // Title bar
        output.push_str(&self.render_title_bar());
        output.push_str("\r\n\r\n");

        // Task list (even during confirmation, shows pending tasks)
        output.push_str(&self.task_list.render());

        // Calculate remaining space for padding
        let title_lines = 2; // title + blank line
        let task_lines = self.task_list.line_count();
        let footer_lines = 2; // separator + hint
        let content_lines = title_lines + task_lines;
        let available = self.height as usize;

        if available > content_lines + footer_lines {
            let padding = available - content_lines - footer_lines;
            for _ in 0..padding {
                output.push_str("\r\n");
            }
        }

        // Footer (hidden hints when modal is showing)
        let has_modal = matches!(self.phase, Phase::Confirming) || self.error_modal.is_some();
        if has_modal {
            // Just render separator and empty hint line
            let dim = Color::BrightBlack.to_ansi_fg();
            let reset = "\x1b[0m";
            output.push_str(&format!(
                "{}{}{}\r\n{}",
                dim,
                "─".repeat(self.width as usize),
                reset,
                " ".repeat(self.width as usize)
            ));
        } else {
            output.push_str(&self.render_footer());
        }

        // Overlay modal based on phase
        match self.phase {
            Phase::Confirming => self.render_confirm_modal(&output),
            _ => {
                if self.error_modal.is_some() {
                    self.render_error_modal(&output)
                } else {
                    output
                }
            }
        }
    }

    fn handle_event(&self, event: Event) -> Option<Self::Message> {
        match event {
            Event::Key(key) => {
                // If error modal is showing, only modal keys work
                if self.error_modal.is_some() {
                    match key.code {
                        KeyCode::Esc => Some(DevUninstallViewMsg::CloseModal),
                        _ => None,
                    }
                } else {
                    match self.phase {
                        // Confirming phase has its own modal-like keys
                        Phase::Confirming => match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                Some(DevUninstallViewMsg::Confirm)
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                Some(DevUninstallViewMsg::Cancel)
                            }
                            KeyCode::Char('q') => Some(DevUninstallViewMsg::Cancel),
                            _ => None,
                        },
                        Phase::Running | Phase::Completed => match key.code {
                            KeyCode::Char('q') => Some(DevUninstallViewMsg::Quit),
                            _ => None,
                        },
                    }
                }
            }
            Event::Resize { width, height } => Some(DevUninstallViewMsg::Resize(width, height)),
            _ => None,
        }
    }

    fn subscriptions(&self) -> Sub<Self::Message> {
        // Keep ticking while executing
        if self.executing || self.task_list.is_running() {
            Sub::interval("uninstall-spinner", Duration::from_millis(80), || {
                DevUninstallViewMsg::Tick
            })
        } else {
            Sub::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uninstall_view_creation() {
        use std::path::PathBuf;

        let info = UninstallInfo {
            has_cluster: true,
            cluster_status: Some("running".to_string()),
            has_registry: true,
            deploy_dir: PathBuf::from("/test/deploy"),
            has_deploy_dir: true,
            data_dir: PathBuf::from("/test/data"),
            state_dir: PathBuf::from("/test/state"),
            has_state_dir: true,
            config_dir: PathBuf::from("/test/config"),
            creds_file: PathBuf::from("/test/creds"),
            has_creds_file: true,
            dev_image_count: 5,
            has_kube_context: true,
            has_talos_context: true,
        };

        let steps = vec![InstallStep::new("Step 1"), InstallStep::new("Step 2")];

        let view = DevUninstallView::new(steps, info, false);
        assert!(!view.is_success());
        assert!(!view.was_cancelled());
    }

    #[test]
    fn test_uninstall_info_removal_lines() {
        use std::path::PathBuf;

        let info = UninstallInfo {
            has_cluster: true,
            cluster_status: Some("running".to_string()),
            has_registry: false,
            deploy_dir: PathBuf::from("/test/deploy"),
            has_deploy_dir: true,
            data_dir: PathBuf::from("/test/data"),
            state_dir: PathBuf::from("/test/state"),
            has_state_dir: false,
            config_dir: PathBuf::from("/test/config"),
            creds_file: PathBuf::from("/test/creds"),
            has_creds_file: false,
            dev_image_count: 0,
            has_kube_context: false,
            has_talos_context: false,
        };

        let lines = info.removal_lines();
        assert!(lines.iter().any(|l| l.contains("Talos cluster")));
        assert!(lines.iter().any(|l| l.contains("Deploy repository")));
        assert!(!lines.iter().any(|l| l.contains("registry")));
    }
}
