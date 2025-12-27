//! Interactive install view for dev cluster setup.
//!
//! A full-screen TUI showing installation progress with animated spinners,
//! task completion status, and error modals.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use ferment::components::{Modal, ModalBorder, TaskList};
use ferment::runtime::Sub;
use ferment::style::Color;
use ferment::terminal::{Event, KeyCode};
use ferment::{Cmd, Model};

/// Type alias for step executor function.
/// Returns Ok(detail) on success, or Err(error_message) on failure.
pub type StepExecutor = Arc<dyn Fn() -> Result<Option<String>, String> + Send + Sync>;

/// Installation step definition.
#[derive(Clone)]
pub struct InstallStep {
    /// Step name displayed to user.
    pub name: String,
    /// Optional executor function for this step.
    pub executor: Option<StepExecutor>,
}

impl std::fmt::Debug for InstallStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallStep")
            .field("name", &self.name)
            .field("executor", &self.executor.is_some())
            .finish()
    }
}

impl InstallStep {
    /// Create a new installation step.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            executor: None,
        }
    }

    /// Create a step with an executor function.
    pub fn with_executor<F>(name: impl Into<String>, executor: F) -> Self
    where
        F: Fn() -> Result<Option<String>, String> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            executor: Some(Arc::new(executor)),
        }
    }
}

/// Result of running an installation step.
#[derive(Debug, Clone)]
pub enum StepResult {
    /// Step completed successfully.
    Success(Option<String>),
    /// Step was skipped (with reason).
    Skipped(String),
    /// Step failed with error.
    Failure(String),
}

/// Message type for install view.
#[derive(Debug, Clone)]
pub enum InstallViewMsg {
    /// Advance spinner animation and poll for worker results.
    Tick,
    /// Start the installation process.
    Start,
    /// Run a specific step.
    RunStep(usize),
    /// A step completed with result.
    StepCompleted(usize, StepResult),
    /// Start a task (for manual control).
    StartTask(usize),
    /// Complete a task with result (for manual control).
    CompleteTask(usize, StepResult),
    /// User pressed 'q' to quit/cancel.
    Quit,
    /// Close error modal.
    CloseModal,
    /// Terminal resize.
    Resize(u16, u16),
}

/// Result message from a worker thread.
type WorkerResult = (usize, StepResult);

/// The install view state.
pub struct InstallView {
    /// Title for the view.
    title: String,
    /// Subtitle for the view.
    subtitle: String,
    /// The task list component.
    task_list: TaskList,
    /// Step executors.
    executors: Vec<Option<StepExecutor>>,
    /// Current step index being processed.
    current_step: usize,
    /// Whether we've started running.
    started: bool,
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
    /// Whether install was cancelled.
    was_cancelled: bool,
}

impl InstallView {
    /// Create a new install view with the given steps.
    pub fn new(steps: Vec<InstallStep>) -> Self {
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
            subtitle: "Install".to_string(),
            task_list,
            executors,
            current_step: 0,
            started: false,
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

    /// Check if install was cancelled by user.
    pub fn was_cancelled(&self) -> bool {
        self.was_cancelled
    }

    /// Check if install completed successfully.
    pub fn is_success(&self) -> bool {
        self.task_list.is_all_complete() && !self.task_list.has_failure()
    }

    /// Check if there was a failure.
    pub fn has_failure(&self) -> bool {
        self.task_list.has_failure()
    }

    /// Start a task by index.
    pub fn start_task(&mut self, index: usize) {
        self.task_list.start_task(index);
    }

    /// Complete a task with result.
    pub fn complete_task(&mut self, index: usize, result: StepResult) {
        match result {
            StepResult::Success(detail) => {
                self.task_list.complete_task(index, detail);
            }
            StepResult::Skipped(reason) => {
                self.task_list.skip_task(index, Some(reason));
            }
            StepResult::Failure(error) => {
                self.task_list.fail_task(index, Some(error.clone()));
                // Show error modal
                if let Some(task) = self.task_list.get(index) {
                    self.error_modal = Some((task.name.clone(), error));
                }
            }
        }
    }

    /// Check if a task is currently running.
    pub fn is_running(&self) -> bool {
        self.task_list.is_running()
    }

    /// Check if all tasks are complete.
    pub fn is_all_complete(&self) -> bool {
        self.task_list.is_all_complete()
    }

    /// Get current task index.
    pub fn current_task_index(&self) -> Option<usize> {
        self.task_list.current_task_index()
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

        // Build styled hints: shortcuts in default color, descriptions dimmed
        let hint_text = if self.task_list.is_all_complete() || self.has_failure() {
            "q quit"
        } else {
            "q cancel"
        };

        // Split into key and description
        let (key, desc) = hint_text.split_once(' ').unwrap_or((hint_text, ""));
        let styled_hint = format!("{}{}{} {}{}", reset, key, dim, desc, reset);
        let plain_len = hint_text.len();

        // Right-align the hints (with padding on left)
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
    /// Returns a receiver to poll for the result.
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
                    // Worker crashed or finished without sending
                    self.executing = false;
                    self.result_receiver = None;
                    None
                }
            }
        } else {
            None
        }
    }

    /// Render the error modal.
    fn render_modal(&self, background: &str) -> String {
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
}

impl Model for InstallView {
    type Message = InstallViewMsg;

    fn init(&self) -> Option<Cmd<Self::Message>> {
        // Schedule start after a brief delay to allow initial render
        Some(Cmd::tick(Duration::from_millis(100), |_| {
            InstallViewMsg::Start
        }))
    }

    fn update(&mut self, msg: Self::Message) -> Option<Cmd<Self::Message>> {
        match msg {
            InstallViewMsg::Tick => {
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
                        // Continue ticking for the next step
                        return Some(Cmd::tick(Duration::from_millis(80), |_| {
                            InstallViewMsg::Tick
                        }));
                    }
                }
                None
            }
            InstallViewMsg::Start => {
                if !self.started && !self.executors.is_empty() {
                    self.started = true;
                    self.current_step = 0;
                    self.task_list.start_task(0);
                    self.executing = true;
                    self.result_receiver = Some(self.spawn_step_worker(0));
                    // Return a tick command to ensure polling starts immediately
                    return Some(Cmd::tick(Duration::from_millis(80), |_| {
                        InstallViewMsg::Tick
                    }));
                }
                None
            }
            InstallViewMsg::RunStep(index) => {
                if index < self.executors.len() && !self.executing {
                    self.task_list.start_task(index);
                    self.executing = true;
                    self.result_receiver = Some(self.spawn_step_worker(index));
                }
                None
            }
            InstallViewMsg::StepCompleted(index, result) => {
                // Manual completion (for external control)
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
            InstallViewMsg::StartTask(index) => {
                self.start_task(index);
                None
            }
            InstallViewMsg::CompleteTask(index, result) => {
                self.complete_task(index, result);
                None
            }
            InstallViewMsg::Quit => {
                if self.error_modal.is_some() {
                    // Close modal first
                    self.error_modal = None;
                    None
                } else {
                    self.should_quit = true;
                    if !self.task_list.is_all_complete() {
                        self.was_cancelled = true;
                    }
                    Some(Cmd::quit())
                }
            }
            InstallViewMsg::CloseModal => {
                self.error_modal = None;
                None
            }
            InstallViewMsg::Resize(w, h) => {
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

        // Task list
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

        // Footer
        output.push_str(&self.render_footer());

        // Overlay modal if showing
        if self.error_modal.is_some() {
            self.render_modal(&output)
        } else {
            output
        }
    }

    fn handle_event(&self, event: Event) -> Option<Self::Message> {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Char('q') => Some(InstallViewMsg::Quit),
                KeyCode::Esc => {
                    if self.error_modal.is_some() {
                        Some(InstallViewMsg::CloseModal)
                    } else {
                        Some(InstallViewMsg::Quit)
                    }
                }
                _ => None,
            },
            Event::Resize { width, height } => Some(InstallViewMsg::Resize(width, height)),
            _ => None,
        }
    }

    fn subscriptions(&self) -> Sub<Self::Message> {
        // Keep ticking while executing to:
        // 1. Animate the spinner
        // 2. Poll for worker thread results
        if self.executing || self.task_list.is_running() {
            Sub::interval("install-spinner", Duration::from_millis(80), || {
                InstallViewMsg::Tick
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
    fn test_install_view_creation() {
        let steps = vec![
            InstallStep::new("Clone repository"),
            InstallStep::new("Install dependencies"),
            InstallStep::new("Build project"),
        ];

        let view = InstallView::new(steps);
        assert!(!view.is_all_complete());
        assert!(!view.is_running());
    }

    #[test]
    fn test_task_lifecycle() {
        let steps = vec![InstallStep::new("Step 1"), InstallStep::new("Step 2")];

        let mut view = InstallView::new(steps);

        // Start first task
        view.start_task(0);
        assert!(view.is_running());
        assert_eq!(view.current_task_index(), Some(0));

        // Complete first task
        view.complete_task(0, StepResult::Success(Some("/path".to_string())));
        assert!(!view.is_running());
        assert!(!view.is_all_complete());

        // Complete second task
        view.start_task(1);
        view.complete_task(1, StepResult::Success(None));
        assert!(view.is_all_complete());
        assert!(view.is_success());
    }

    #[test]
    fn test_failure_shows_modal() {
        let steps = vec![InstallStep::new("Failing step")];
        let mut view = InstallView::new(steps);

        view.start_task(0);
        view.complete_task(0, StepResult::Failure("Something went wrong".to_string()));

        assert!(view.has_failure());
        assert!(view.error_modal.is_some());
    }
}
