//! Interactive start view for dev cluster.
//!
//! A full-screen TUI for starting the development cluster with:
//! - Setup instructions modal for Tailscale configuration
//! - Credentials input modal
//! - Step-by-step progress with animated spinners

use std::{sync::Arc, time::Duration};

use teapot::{
    Cmd, Model,
    components::{FooterHints, Modal, ModalBorder, TaskList, TextInput, TextInputMsg, TitleBar},
    runtime::Sub,
    style::{Color, RESET, UNDERLINE},
    terminal::{Event, KeyCode},
    util::WorkerHandle,
};

use super::install_view::{InstallStep, StepExecutor, StepResult};

/// Create a clickable terminal hyperlink using OSC 8 escape sequences.
///
/// Most modern terminals support this (iTerm2, Kitty, WezTerm, VS Code, etc.).
/// Terminals that don't support it will just show the underlined text without the link.
fn hyperlink(url: &str, text: &str) -> String {
    // OSC 8 for hyperlink + underline styling to indicate clickability
    format!("\x1b]8;;{}\x07{}{}{}\x1b]8;;\x07", url, UNDERLINE, text, RESET)
}

/// Phase of the start view.
#[derive(Debug, Clone, PartialEq)]
pub enum StartPhase {
    /// Checking prerequisites.
    CheckingPrereqs,
    /// Showing Tailscale setup instructions.
    SetupInstructions,
    /// Inputting Tailscale credentials.
    CredentialsInput,
    /// Running cluster creation steps.
    Running,
    /// Completed successfully.
    Completed,
    /// Failed with error.
    Failed,
}

/// Message type for start view.
#[derive(Debug, Clone)]
pub enum DevStartViewMsg {
    /// Advance spinner animation and poll for worker results.
    Tick,
    /// Prerequisites check completed.
    PrereqsChecked(Result<(), String>),
    /// Credentials were found, skip setup.
    CredentialsFound(String, String),
    /// Credentials not found, show setup.
    CredentialsNotFound,
    /// User pressed Enter to continue from setup instructions.
    ContinueToCredentials,
    /// User pressed Esc to go back to setup instructions.
    BackToInstructions,
    /// Text input message for client ID field.
    ClientIdInput(TextInputMsg),
    /// Text input message for client secret field.
    ClientSecretInput(TextInputMsg),
    /// Switch focus between input fields.
    SwitchFocus,
    /// Submit credentials.
    SubmitCredentials,
    /// Credentials validation result.
    CredentialsValidated(Result<(), String>),
    /// Start running the steps.
    StartRunning,
    /// Close error modal.
    CloseModal,
    /// User pressed 'q' to quit/cancel.
    Quit,
    /// Terminal resize.
    Resize(u16, u16),
}

/// Result message from a worker thread.
type WorkerResult = (usize, StepResult);

/// The start view state.
#[allow(clippy::type_complexity)]
pub struct DevStartView {
    /// Title for the view.
    title: String,
    /// Current phase.
    phase: StartPhase,
    /// Whether to skip image builds.
    skip_build: bool,
    /// The task list component.
    task_list: TaskList,
    /// Step executors (populated when running).
    executors: Vec<Option<StepExecutor>>,
    /// Current step index being processed.
    current_step: usize,
    /// Worker handle for step execution.
    worker: Option<WorkerHandle<WorkerResult>>,
    /// Terminal width.
    width: u16,
    /// Terminal height.
    height: u16,
    /// Error modal content (if showing).
    error_modal: Option<(String, String)>,
    /// Whether the view should quit.
    should_quit: bool,
    /// Whether start was cancelled.
    was_cancelled: bool,
    /// Tailscale client ID input.
    client_id_input: TextInput,
    /// Tailscale client secret input.
    client_secret_input: TextInput,
    /// Which credential field is focused (0 = client_id, 1 = client_secret).
    focused_field: usize,
    /// Saved Tailscale credentials.
    tailscale_credentials: Option<(String, String)>,
    /// Step builder function (to be called when credentials are ready).
    step_builder: Option<Arc<dyn Fn(String, String, bool) -> Vec<InstallStep> + Send + Sync>>,
    /// Prereq checker function.
    prereq_checker: Option<Arc<dyn Fn() -> Result<(), String> + Send + Sync>>,
    /// Credentials loader function.
    credentials_loader: Option<Arc<dyn Fn() -> Option<(String, String)> + Send + Sync>>,
    /// Credentials saver function.
    credentials_saver: Option<Arc<dyn Fn(&str, &str) -> Result<(), String> + Send + Sync>>,
}

impl DevStartView {
    /// Create a new start view.
    pub fn new(skip_build: bool) -> Self {
        let (width, height) = teapot::terminal::size().unwrap_or((80, 24));

        Self {
            title: "InferaDB Development Cluster".to_string(),
            phase: StartPhase::CheckingPrereqs,
            skip_build,
            task_list: TaskList::new(),
            executors: Vec::new(),
            current_step: 0,
            worker: None,
            width,
            height,
            error_modal: None,
            should_quit: false,
            was_cancelled: false,
            client_id_input: TextInput::new()
                .placeholder("Enter Client ID...")
                .prompt("Client ID: "),
            client_secret_input: TextInput::new()
                .placeholder("Enter Client Secret...")
                .prompt("Secret:    ")
                .hidden(true),
            focused_field: 0,
            tailscale_credentials: None,
            step_builder: None,
            prereq_checker: None,
            credentials_loader: None,
            credentials_saver: None,
        }
    }

    /// Set the step builder function.
    pub fn with_step_builder<F>(mut self, builder: F) -> Self
    where
        F: Fn(String, String, bool) -> Vec<InstallStep> + Send + Sync + 'static,
    {
        self.step_builder = Some(Arc::new(builder));
        self
    }

    /// Set the prerequisites checker function.
    pub fn with_prereq_checker<F>(mut self, checker: F) -> Self
    where
        F: Fn() -> Result<(), String> + Send + Sync + 'static,
    {
        self.prereq_checker = Some(Arc::new(checker));
        self
    }

    /// Set the credentials loader function.
    pub fn with_credentials_loader<F>(mut self, loader: F) -> Self
    where
        F: Fn() -> Option<(String, String)> + Send + Sync + 'static,
    {
        self.credentials_loader = Some(Arc::new(loader));
        self
    }

    /// Set the credentials saver function.
    pub fn with_credentials_saver<F>(mut self, saver: F) -> Self
    where
        F: Fn(&str, &str) -> Result<(), String> + Send + Sync + 'static,
    {
        self.credentials_saver = Some(Arc::new(saver));
        self
    }

    /// Check if the view should quit.
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Check if start was cancelled by user.
    pub fn was_cancelled(&self) -> bool {
        self.was_cancelled
    }

    /// Check if start completed successfully.
    pub fn is_success(&self) -> bool {
        self.phase == StartPhase::Completed
    }

    /// Check if there was a failure.
    pub fn has_failure(&self) -> bool {
        self.phase == StartPhase::Failed || self.task_list.has_failure()
    }

    /// Render the title bar with dimmed slashes.
    fn render_title_bar(&self) -> String {
        let subtitle = match self.phase {
            StartPhase::CheckingPrereqs => "Checking Prerequisites",
            StartPhase::SetupInstructions => "Tailscale Setup",
            StartPhase::CredentialsInput => "Tailscale Credentials",
            StartPhase::Running => "Starting",
            StartPhase::Completed => "Complete",
            StartPhase::Failed => "Failed",
        };

        TitleBar::new(&self.title).subtitle(subtitle).width(self.width as usize).render()
    }

    /// Render the footer with right-aligned hints.
    fn render_footer(&self) -> String {
        let hints: Vec<(&str, &str)> = match self.phase {
            StartPhase::SetupInstructions => vec![("enter", "continue"), ("q", "cancel")],
            StartPhase::CredentialsInput => {
                vec![("tab", "switch"), ("enter", "submit"), ("q", "cancel")]
            },
            StartPhase::Running => vec![("q", "cancel")],
            StartPhase::Completed | StartPhase::Failed => vec![("q", "quit")],
            _ => vec![("q", "cancel")],
        };

        FooterHints::new().hints(hints).width(self.width as usize).with_separator().render()
    }

    /// Render the setup instructions content.
    fn render_setup_instructions(&self) -> String {
        let modal_width = 70.min(self.width as usize - 4);
        let modal_height = 20.min(self.height as usize - 4);

        // URLs with clickable hyperlinks
        let dns_url = "https://login.tailscale.com/admin/dns";
        let tags_url = "https://login.tailscale.com/admin/acls/tags";
        let oauth_url = "https://login.tailscale.com/admin/settings/oauth";

        let content = format!(
            r#"Tailscale OAuth credentials are required for the Kubernetes operator.

Step 1: Enable HTTPS on your tailnet (one-time setup)
  Go to: {}
  Scroll to 'HTTPS Certificates' and click 'Enable HTTPS'

Step 2: Create tags (one-time setup)
  Go to: {}
  Create tag 'k8s-operator' with yourself as owner
  Create tag 'k8s' with 'tag:k8s-operator' as owner

Step 3: Create OAuth client
  Go to: {}
  Click 'Generate OAuth client'
  Add scopes:
    - Devices > Core: Read & Write, tag: k8s-operator
    - Keys > Auth Keys: Read & Write, tag: k8s-operator
  Click 'Generate client' and copy the credentials"#,
            hyperlink(dns_url, dns_url),
            hyperlink(tags_url, tags_url),
            hyperlink(oauth_url, oauth_url)
        );

        let modal = Modal::new(modal_width, modal_height)
            .border(ModalBorder::Rounded)
            .border_color(Color::Cyan)
            .title("Tailscale Setup")
            .title_color(Color::Cyan)
            .content(content)
            .footer_hint("enter", "continue");

        // Create empty background
        let mut background = String::new();
        background.push_str(&self.render_title_bar());
        background.push_str("\r\n");
        for _ in 0..(self.height as usize - 3) {
            background.push_str("\r\n");
        }
        background.push_str(&self.render_footer());

        modal.render_overlay(self.width as usize, self.height as usize, &background)
    }

    /// Render the credentials input content.
    fn render_credentials_input(&self) -> String {
        let modal_width = 60.min(self.width as usize - 4);
        let modal_height = 10.min(self.height as usize - 4);

        // Build content with both inputs
        let mut content = String::new();
        content.push_str("Enter your Tailscale OAuth credentials:\n\n");

        // Client ID input
        let mut id_input = self.client_id_input.clone();
        id_input.set_focused(self.focused_field == 0);
        content.push_str(&id_input.view());
        content.push_str("\n\n");

        // Client Secret input
        let mut secret_input = self.client_secret_input.clone();
        secret_input.set_focused(self.focused_field == 1);
        content.push_str(&secret_input.view());

        let modal = Modal::new(modal_width, modal_height)
            .border(ModalBorder::Rounded)
            .border_color(Color::Cyan)
            .title("Tailscale Credentials")
            .title_color(Color::Cyan)
            .content(content)
            .footer_hints(vec![("tab", "switch"), ("enter", "submit"), ("esc", "back")]);

        // Create empty background
        let mut background = String::new();
        background.push_str(&self.render_title_bar());
        background.push_str("\r\n");
        for _ in 0..(self.height as usize - 3) {
            background.push_str("\r\n");
        }
        background.push_str(&self.render_footer());

        modal.render_overlay(self.width as usize, self.height as usize, &background)
    }

    /// Spawn a worker thread to execute a step.
    fn spawn_step_worker(&self, index: usize) -> WorkerHandle<WorkerResult> {
        if let Some(Some(executor)) = self.executors.get(index) {
            let executor = Arc::clone(executor);
            WorkerHandle::spawn(move || {
                let result = executor();
                let step_result = match result {
                    Ok(detail) => StepResult::Success(detail),
                    Err(error) => StepResult::Failure(error),
                };
                (index, step_result)
            })
        } else {
            WorkerHandle::spawn(move || (index, StepResult::Success(None)))
        }
    }

    /// Poll for worker result.
    fn poll_worker_result(&mut self) -> Option<WorkerResult> {
        if let Some(ref handle) = self.worker {
            if let Some(result) = handle.try_recv() {
                self.worker = None;
                return Some(result);
            }
        }
        None
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

    /// Initialize steps from builder.
    fn initialize_steps(&mut self) {
        if let (Some(builder), Some((client_id, client_secret))) =
            (&self.step_builder, &self.tailscale_credentials)
        {
            let steps = builder(client_id.clone(), client_secret.clone(), self.skip_build);

            self.task_list = TaskList::new();
            self.executors.clear();

            for step in steps {
                self.task_list = std::mem::take(&mut self.task_list).add_task(&step.name);
                self.executors.push(step.executor);
            }
        }
    }
}

impl Model for DevStartView {
    type Message = DevStartViewMsg;

    fn init(&self) -> Option<Cmd<Self::Message>> {
        // Start by checking prerequisites
        Some(Cmd::tick(Duration::from_millis(100), |_| DevStartViewMsg::Tick))
    }

    fn update(&mut self, msg: Self::Message) -> Option<Cmd<Self::Message>> {
        match msg {
            DevStartViewMsg::Tick => {
                match self.phase {
                    StartPhase::CheckingPrereqs => {
                        // Check prerequisites
                        if let Some(checker) = &self.prereq_checker {
                            let checker = Arc::clone(checker);
                            match checker() {
                                Ok(()) => {
                                    // Check for existing credentials
                                    if let Some(loader) = &self.credentials_loader {
                                        if let Some((id, secret)) = loader() {
                                            self.tailscale_credentials = Some((id, secret));
                                            self.phase = StartPhase::Running;
                                            self.initialize_steps();
                                            return Some(Cmd::tick(
                                                Duration::from_millis(100),
                                                |_| DevStartViewMsg::StartRunning,
                                            ));
                                        }
                                    }
                                    // No credentials, show setup
                                    self.phase = StartPhase::SetupInstructions;
                                },
                                Err(e) => {
                                    self.error_modal = Some(("Prerequisites".to_string(), e));
                                    self.phase = StartPhase::Failed;
                                },
                            }
                        } else {
                            // No prereq checker, skip to credentials check
                            if let Some(loader) = &self.credentials_loader {
                                if let Some((id, secret)) = loader() {
                                    self.tailscale_credentials = Some((id, secret));
                                    self.phase = StartPhase::Running;
                                    self.initialize_steps();
                                    return Some(Cmd::tick(Duration::from_millis(100), |_| {
                                        DevStartViewMsg::StartRunning
                                    }));
                                }
                            }
                            self.phase = StartPhase::SetupInstructions;
                        }
                    },
                    StartPhase::Running => {
                        // Forward tick to task list
                        self.task_list.update(teapot::components::TaskListMsg::Tick);

                        // Poll for worker result
                        if let Some((index, result)) = self.poll_worker_result() {
                            match &result {
                                StepResult::Success(detail) => {
                                    self.task_list.complete_task(index, detail.clone());
                                },
                                StepResult::Skipped(reason) => {
                                    self.task_list.skip_task(index, Some(reason.clone()));
                                },
                                StepResult::Failure(error) => {
                                    self.task_list.fail_task(index, Some(error.clone()));
                                    if let Some(task) = self.task_list.get(index) {
                                        self.error_modal = Some((task.name.clone(), error.clone()));
                                    }
                                    self.phase = StartPhase::Failed;
                                    return None;
                                },
                            }

                            // Start next step
                            let next_step = index + 1;
                            if next_step < self.executors.len() {
                                self.current_step = next_step;
                                self.task_list.start_task(next_step);
                                self.worker = Some(self.spawn_step_worker(next_step));
                            } else {
                                self.phase = StartPhase::Completed;
                            }
                        }
                    },
                    _ => {},
                }
                None
            },

            DevStartViewMsg::PrereqsChecked(result) => {
                match result {
                    Ok(()) => {
                        // Check for credentials
                        if let Some(loader) = &self.credentials_loader {
                            if let Some((id, secret)) = loader() {
                                self.tailscale_credentials = Some((id, secret));
                                self.phase = StartPhase::Running;
                                self.initialize_steps();
                                return Some(Cmd::tick(Duration::from_millis(100), |_| {
                                    DevStartViewMsg::StartRunning
                                }));
                            }
                        }
                        self.phase = StartPhase::SetupInstructions;
                    },
                    Err(e) => {
                        self.error_modal = Some(("Prerequisites".to_string(), e));
                        self.phase = StartPhase::Failed;
                    },
                }
                None
            },

            DevStartViewMsg::CredentialsFound(id, secret) => {
                self.tailscale_credentials = Some((id, secret));
                self.phase = StartPhase::Running;
                self.initialize_steps();
                Some(Cmd::tick(Duration::from_millis(100), |_| DevStartViewMsg::StartRunning))
            },

            DevStartViewMsg::CredentialsNotFound => {
                self.phase = StartPhase::SetupInstructions;
                None
            },

            DevStartViewMsg::ContinueToCredentials => {
                self.phase = StartPhase::CredentialsInput;
                self.focused_field = 0;
                self.client_id_input.set_focused(true);
                self.client_secret_input.set_focused(false);
                None
            },

            DevStartViewMsg::BackToInstructions => {
                self.phase = StartPhase::SetupInstructions;
                self.client_id_input.set_focused(false);
                self.client_secret_input.set_focused(false);
                None
            },

            DevStartViewMsg::ClientIdInput(input_msg) => {
                self.client_id_input.update(input_msg);
                None
            },

            DevStartViewMsg::ClientSecretInput(input_msg) => {
                self.client_secret_input.update(input_msg);
                None
            },

            DevStartViewMsg::SwitchFocus => {
                self.focused_field = (self.focused_field + 1) % 2;
                self.client_id_input.set_focused(self.focused_field == 0);
                self.client_secret_input.set_focused(self.focused_field == 1);
                None
            },

            DevStartViewMsg::SubmitCredentials => {
                let client_id = self.client_id_input.get_value().trim().to_string();
                let client_secret = self.client_secret_input.get_value().trim().to_string();

                if client_id.is_empty() {
                    self.client_id_input.set_error("Client ID is required");
                    self.focused_field = 0;
                    self.client_id_input.set_focused(true);
                    self.client_secret_input.set_focused(false);
                    return None;
                }

                if client_secret.is_empty() {
                    self.client_secret_input.set_error("Client Secret is required");
                    self.focused_field = 1;
                    self.client_id_input.set_focused(false);
                    self.client_secret_input.set_focused(true);
                    return None;
                }

                // Save credentials
                if let Some(saver) = &self.credentials_saver {
                    if let Err(e) = saver(&client_id, &client_secret) {
                        self.error_modal = Some(("Save Credentials".to_string(), e));
                        return None;
                    }
                }

                self.tailscale_credentials = Some((client_id, client_secret));
                self.phase = StartPhase::Running;
                self.initialize_steps();
                Some(Cmd::tick(Duration::from_millis(100), |_| DevStartViewMsg::StartRunning))
            },

            DevStartViewMsg::CredentialsValidated(result) => match result {
                Ok(()) => {
                    self.phase = StartPhase::Running;
                    self.initialize_steps();
                    Some(Cmd::tick(Duration::from_millis(100), |_| DevStartViewMsg::StartRunning))
                },
                Err(e) => {
                    self.error_modal = Some(("Credentials".to_string(), e));
                    None
                },
            },

            DevStartViewMsg::StartRunning => {
                if !self.executors.is_empty() {
                    self.current_step = 0;
                    self.task_list.start_task(0);
                    self.worker = Some(self.spawn_step_worker(0));
                    return Some(Cmd::tick(Duration::from_millis(80), |_| DevStartViewMsg::Tick));
                }
                None
            },

            DevStartViewMsg::CloseModal => {
                self.error_modal = None;
                None
            },

            DevStartViewMsg::Quit => {
                if self.error_modal.is_some() {
                    self.error_modal = None;
                    None
                } else {
                    self.should_quit = true;
                    if self.phase != StartPhase::Completed {
                        self.was_cancelled = true;
                    }
                    Some(Cmd::quit())
                }
            },

            DevStartViewMsg::Resize(w, h) => {
                self.width = w;
                self.height = h;
                None
            },
        }
    }

    fn view(&self) -> String {
        match self.phase {
            StartPhase::SetupInstructions => self.render_setup_instructions(),
            StartPhase::CredentialsInput => {
                let view = self.render_credentials_input();
                if self.error_modal.is_some() { self.render_error_modal(&view) } else { view }
            },
            StartPhase::CheckingPrereqs
            | StartPhase::Running
            | StartPhase::Completed
            | StartPhase::Failed => {
                let mut output = String::new();

                // Title bar
                output.push_str(&self.render_title_bar());
                output.push_str("\r\n\r\n");

                // Task list or status
                if self.phase == StartPhase::CheckingPrereqs {
                    output.push_str("  Checking prerequisites...\r\n");
                } else {
                    output.push_str(&self.task_list.render());
                }

                // Padding
                let title_lines = 2;
                let task_lines = if self.phase == StartPhase::CheckingPrereqs {
                    1
                } else {
                    self.task_list.line_count()
                };
                let footer_lines = 2;
                let content_lines = title_lines + task_lines;
                let available = self.height as usize;

                if available > content_lines + footer_lines {
                    let padding = available - content_lines - footer_lines;
                    for _ in 0..padding {
                        output.push_str("\r\n");
                    }
                }

                // Footer
                if self.error_modal.is_some() {
                    let dim = Color::BrightBlack.to_ansi_fg();
                    let reset = RESET;
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

                // Error modal overlay
                if self.error_modal.is_some() { self.render_error_modal(&output) } else { output }
            },
        }
    }

    fn handle_event(&self, event: Event) -> Option<Self::Message> {
        match event {
            Event::Key(ref key) => {
                // Handle error modal first
                if self.error_modal.is_some() {
                    return match key.code {
                        KeyCode::Esc | KeyCode::Enter => Some(DevStartViewMsg::CloseModal),
                        _ => None,
                    };
                }

                match self.phase {
                    StartPhase::SetupInstructions => match key.code {
                        KeyCode::Enter => Some(DevStartViewMsg::ContinueToCredentials),
                        KeyCode::Char('q') => Some(DevStartViewMsg::Quit),
                        _ => None,
                    },
                    StartPhase::CredentialsInput => match key.code {
                        KeyCode::Tab => Some(DevStartViewMsg::SwitchFocus),
                        KeyCode::Enter => Some(DevStartViewMsg::SubmitCredentials),
                        KeyCode::Char('q') if key.modifiers.is_empty() => None, // Allow typing 'q'
                        KeyCode::Esc => Some(DevStartViewMsg::BackToInstructions),
                        _ => {
                            // Forward to focused input
                            let input = if self.focused_field == 0 {
                                &self.client_id_input
                            } else {
                                &self.client_secret_input
                            };
                            input.handle_event(event).map(|msg| {
                                if self.focused_field == 0 {
                                    DevStartViewMsg::ClientIdInput(msg)
                                } else {
                                    DevStartViewMsg::ClientSecretInput(msg)
                                }
                            })
                        },
                    },
                    StartPhase::Running | StartPhase::Completed | StartPhase::Failed => {
                        match key.code {
                            KeyCode::Char('q') => Some(DevStartViewMsg::Quit),
                            _ => None,
                        }
                    },
                    StartPhase::CheckingPrereqs => match key.code {
                        KeyCode::Char('q') => Some(DevStartViewMsg::Quit),
                        _ => None,
                    },
                }
            },
            Event::Resize { width, height } => Some(DevStartViewMsg::Resize(width, height)),
            _ => None,
        }
    }

    fn subscriptions(&self) -> Sub<Self::Message> {
        match self.phase {
            StartPhase::CheckingPrereqs | StartPhase::Running => {
                Sub::interval("start-tick", Duration::from_millis(80), || DevStartViewMsg::Tick)
            },
            _ => Sub::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_view_creation() {
        let view = DevStartView::new(false);
        assert!(!view.should_quit());
        assert_eq!(view.phase, StartPhase::CheckingPrereqs);
    }

    #[test]
    fn test_start_view_with_skip_build() {
        let view = DevStartView::new(true);
        assert!(view.skip_build);
    }
}
