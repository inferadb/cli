//! Interactive stop view for dev cluster.
//!
//! A full-screen TUI showing progress with animated spinners
//! when stopping (pausing) or destroying the development cluster.

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

/// Message type for dev stop view.
#[derive(Debug, Clone)]
pub enum DevStopViewMsg {
    /// Advance spinner animation and poll for worker results.
    Tick,
    /// Start the stop process.
    Start,
    /// Close error modal.
    CloseModal,
    /// User pressed 'q' to quit/cancel.
    Quit,
    /// Terminal resize.
    Resize(u16, u16),
}

/// Result message from a worker thread.
type WorkerResult = (usize, StepResult);

/// The dev stop view state.
pub struct DevStopView {
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
    /// Whether stop was cancelled.
    was_cancelled: bool,
}

impl DevStopView {
    /// Create a new stop view with the given steps.
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
            subtitle: "Stop".to_string(),
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

    /// Check if stop was cancelled by user.
    pub fn was_cancelled(&self) -> bool {
        self.was_cancelled
    }

    /// Check if stop completed successfully.
    pub fn is_success(&self) -> bool {
        self.task_list.is_all_complete() && !self.task_list.has_failure()
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
}

impl Model for DevStopView {
    type Message = DevStopViewMsg;

    fn init(&self) -> Option<Cmd<Self::Message>> {
        // Schedule start after a brief delay to allow initial render
        Some(Cmd::tick(Duration::from_millis(100), |_| {
            DevStopViewMsg::Start
        }))
    }

    fn update(&mut self, msg: Self::Message) -> Option<Cmd<Self::Message>> {
        match msg {
            DevStopViewMsg::Tick => {
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
                            DevStopViewMsg::Tick
                        }));
                    }
                }
                None
            }
            DevStopViewMsg::Start => {
                if !self.started && !self.executors.is_empty() {
                    self.started = true;
                    self.current_step = 0;
                    self.task_list.start_task(0);
                    self.executing = true;
                    self.result_receiver = Some(self.spawn_step_worker(0));
                    // Return a tick command to ensure polling starts immediately
                    return Some(Cmd::tick(Duration::from_millis(80), |_| {
                        DevStopViewMsg::Tick
                    }));
                }
                None
            }
            DevStopViewMsg::CloseModal => {
                self.error_modal = None;
                None
            }
            DevStopViewMsg::Quit => {
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
            DevStopViewMsg::Resize(w, h) => {
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

        // Footer (hidden hints when modal is showing)
        if self.error_modal.is_some() {
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

        // Overlay error modal if showing
        if self.error_modal.is_some() {
            self.render_error_modal(&output)
        } else {
            output
        }
    }

    fn handle_event(&self, event: Event) -> Option<Self::Message> {
        match event {
            Event::Key(key) => {
                // If modal is showing, only modal keys work
                if self.error_modal.is_some() {
                    match key.code {
                        KeyCode::Esc => Some(DevStopViewMsg::CloseModal),
                        _ => None,
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => Some(DevStopViewMsg::Quit),
                        _ => None,
                    }
                }
            }
            Event::Resize { width, height } => Some(DevStopViewMsg::Resize(width, height)),
            _ => None,
        }
    }

    fn subscriptions(&self) -> Sub<Self::Message> {
        // Keep ticking while executing
        if self.executing || self.task_list.is_running() {
            Sub::interval("stop-tick", Duration::from_millis(80), || {
                DevStopViewMsg::Tick
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
    fn test_stop_view_creation() {
        let view = DevStopView::new(vec![]);
        assert!(!view.should_quit());
    }

    #[test]
    fn test_stop_view_with_steps() {
        let steps = vec![InstallStep::new("Test step")];
        let view = DevStopView::new(steps);
        assert!(!view.is_success());
    }
}
