//! Interactive install view for dev cluster setup.
//!
//! A full-screen TUI showing installation progress with animated spinners,
//! task completion status, and error modals.
//!
//! This module provides a thin wrapper around Teapot's `TaskProgressView`.

use std::sync::Arc;

// Import StepResult from Teapot for internal use
pub(crate) use teapot::components::StepResult;
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
#[derive(Clone, bon::Builder)]
pub struct InstallStep {
    /// Step name displayed to user.
    #[builder(into)]
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

        let inner = TaskProgressView::builder()
            .steps(task_steps)
            .title("InferaDB Development Cluster")
            .subtitle("Install")
            .auto_start(true)
            .external_control(true)
            .build();

        Self { inner }
    }

    /// Set custom title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.inner = TaskProgressView::builder()
            .steps(vec![])
            .title(title)
            .subtitle("Install")
            .auto_start(true)
            .external_control(true)
            .build();
        self
    }

    /// Set custom subtitle.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.inner = TaskProgressView::builder()
            .steps(vec![])
            .title("InferaDB Development Cluster")
            .subtitle(subtitle)
            .auto_start(true)
            .external_control(true)
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_install_view_creation() {
        let steps = vec![
            InstallStep::builder().name("Clone repository").build(),
            InstallStep::builder().name("Install dependencies").build(),
            InstallStep::builder().name("Build project").build(),
        ];

        let view = DevInstallView::new(steps);
        assert!(!view.is_all_complete());
        assert!(!view.is_running());
    }

    #[test]
    fn test_task_lifecycle() {
        let steps = vec![
            InstallStep::builder().name("Step 1").build(),
            InstallStep::builder().name("Step 2").build(),
        ];

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
        let steps = vec![InstallStep::builder().name("Failing step").build()];
        let mut view = DevInstallView::new(steps);

        view.start_task(0);
        view.complete_task(0, StepResult::Failure("Something went wrong".to_string()));

        assert!(view.has_failure());
    }

    #[test]
    fn test_builder_with_executor() {
        let executor_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let executor_called_clone = executor_called.clone();

        let step = InstallStep::builder()
            .name("Step with executor")
            .executor(std::sync::Arc::new(move || {
                executor_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(Some("done".to_string()))
            }))
            .build();

        // Verify the step was created with an executor
        assert!(step.executor.is_some());
        assert_eq!(step.name, "Step with executor");

        // Call the executor and verify it works
        let result = (step.executor.as_ref().unwrap())();
        assert!(result.is_ok());
        assert!(executor_called.load(std::sync::atomic::Ordering::SeqCst));
    }
}
