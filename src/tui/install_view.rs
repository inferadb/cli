//! Interactive install view for dev cluster setup.
//!
//! A full-screen TUI showing installation progress with animated spinners,
//! task completion status, and error modals.
//!
//! This module provides a thin wrapper around Teapot's `TaskProgressView`.

use std::sync::Arc;

// Re-export StepResult from Teapot for convenience
pub use teapot::components::StepResult;
use teapot::{
    components::{Phase, TaskProgressMsg, TaskProgressView, TaskStep},
    runtime::{Cmd, Model, Sub},
    terminal::Event,
};

// ============================================================================
// Type Aliases
// ============================================================================

/// Type alias for step executor function.
/// Returns Ok(detail) on success, or `Err(error_message)` on failure.
pub type StepExecutor = Arc<dyn Fn() -> Result<Option<String>, String> + Send + Sync>;

/// Message type for install view.
/// This is a type alias to the underlying `TaskProgressMsg`.
pub type DevInstallViewMsg = TaskProgressMsg;

/// Installation step definition.
/// This is a compatibility wrapper around Teapot's `TaskStep`.
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
        Self { name: name.into(), executor: None }
    }

    /// Create a step with an executor function.
    pub fn with_executor<F>(name: impl Into<String>, executor: F) -> Self
    where
        F: Fn() -> Result<Option<String>, String> + Send + Sync + 'static,
    {
        Self { name: name.into(), executor: Some(Arc::new(executor)) }
    }
}

impl From<InstallStep> for TaskStep {
    fn from(step: InstallStep) -> Self {
        if let Some(ex) = step.executor {
            Self::with_executor(step.name, move || ex())
        } else {
            Self::new(step.name)
        }
    }
}

// ============================================================================
// View Implementation
// ============================================================================

/// The install view state.
pub struct DevInstallView {
    inner: TaskProgressView,
}

impl DevInstallView {
    /// Create a new install view with the given steps.
    pub fn new(steps: Vec<InstallStep>) -> Self {
        let task_steps: Vec<TaskStep> = steps.into_iter().map(std::convert::Into::into).collect();

        let inner = TaskProgressView::builder(task_steps)
            .title("InferaDB Development Cluster")
            .subtitle("Install")
            .auto_start()
            .external_control()
            .build();

        Self { inner }
    }

    /// Set custom title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.inner = TaskProgressView::builder(vec![])
            .title(title)
            .subtitle("Install")
            .auto_start()
            .external_control()
            .build();
        self
    }

    /// Set custom subtitle.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.inner = TaskProgressView::builder(vec![])
            .title("InferaDB Development Cluster")
            .subtitle(subtitle)
            .auto_start()
            .external_control()
            .build();
        self
    }

    /// Check if the view should quit.
    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.inner.should_quit()
    }

    /// Check if install was cancelled by user.
    #[must_use]
    pub fn was_cancelled(&self) -> bool {
        self.inner.was_cancelled()
    }

    /// Check if install completed successfully.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.inner.is_success()
    }

    /// Check if there was a failure.
    #[must_use]
    pub fn has_failure(&self) -> bool {
        self.inner.has_failure()
    }

    /// Get the current phase.
    #[must_use]
    pub fn phase(&self) -> &Phase {
        self.inner.phase()
    }

    /// Start a task by index.
    pub fn start_task(&mut self, index: usize) {
        self.inner.start_task(index);
    }

    /// Complete a task with result.
    pub fn complete_task(&mut self, index: usize, result: StepResult) {
        self.inner.complete_task(index, result);
    }

    /// Check if a task is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.inner.is_running()
    }

    /// Check if all tasks are complete.
    #[must_use]
    pub fn is_all_complete(&self) -> bool {
        self.inner.is_all_complete()
    }

    /// Get current task index.
    #[must_use]
    pub fn current_task_index(&self) -> Option<usize> {
        self.inner.current_task_index()
    }
}

impl Model for DevInstallView {
    type Message = DevInstallViewMsg;

    fn init(&self) -> Option<Cmd<Self::Message>> {
        self.inner.init()
    }

    fn update(&mut self, msg: Self::Message) -> Option<Cmd<Self::Message>> {
        self.inner.update(msg)
    }

    fn view(&self) -> String {
        self.inner.view()
    }

    fn handle_event(&self, event: Event) -> Option<Self::Message> {
        self.inner.handle_event(event)
    }

    fn subscriptions(&self) -> Sub<Self::Message> {
        self.inner.subscriptions()
    }
}

// ============================================================================
// Tests
// ============================================================================

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

        let view = DevInstallView::new(steps);
        assert!(!view.is_all_complete());
        assert!(!view.is_running());
    }

    #[test]
    fn test_task_lifecycle() {
        let steps = vec![InstallStep::new("Step 1"), InstallStep::new("Step 2")];

        let mut view = DevInstallView::new(steps);

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
        let mut view = DevInstallView::new(steps);

        view.start_task(0);
        view.complete_task(0, StepResult::Failure("Something went wrong".to_string()));

        assert!(view.has_failure());
    }
}
