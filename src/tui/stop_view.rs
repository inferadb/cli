//! Interactive stop view for dev cluster.
//!
//! A full-screen TUI showing progress with animated spinners
//! when stopping (pausing) or destroying the development cluster.
//!
//! This module provides a thin wrapper around Teapot's `TaskProgressView`.

use teapot::{
    components::{Phase, TaskProgressMsg, TaskProgressView, TaskStep},
    runtime::{Cmd, Model, Sub},
    terminal::Event,
};

// Re-export for backward compatibility
pub use super::install_view::InstallStep;

/// Message type for dev stop view.
/// This is a type alias to the underlying `TaskProgressMsg`.
pub type DevStopViewMsg = TaskProgressMsg;

/// The dev stop view state.
///
/// This is a thin wrapper around `TaskProgressView` for the stop operation.
pub struct DevStopView {
    inner: TaskProgressView,
}

impl DevStopView {
    /// Create a new stop view with the given steps.
    #[must_use]
    pub fn new(steps: Vec<InstallStep>) -> Self {
        let task_steps: Vec<TaskStep> = steps
            .into_iter()
            .map(|s| {
                if let Some(ex) = s.executor {
                    TaskStep::with_executor(s.name, move || ex())
                } else {
                    TaskStep::new(s.name)
                }
            })
            .collect();

        let inner = TaskProgressView::builder(task_steps)
            .title("InferaDB Development Cluster")
            .subtitle("Stop")
            .auto_start()
            .build();

        Self { inner }
    }

    /// Set custom title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        // Rebuild with new title
        let inner =
            TaskProgressView::builder(vec![]).title(title).subtitle("Stop").auto_start().build();
        self.inner = inner;
        self
    }

    /// Set custom subtitle.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        // Rebuild with new subtitle
        let inner = TaskProgressView::builder(vec![])
            .title("InferaDB Development Cluster")
            .subtitle(subtitle)
            .auto_start()
            .build();
        self.inner = inner;
        self
    }

    /// Check if the view should quit.
    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.inner.should_quit()
    }

    /// Check if stop was cancelled by user.
    #[must_use]
    pub fn was_cancelled(&self) -> bool {
        self.inner.was_cancelled()
    }

    /// Check if stop completed successfully.
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
}

impl Model for DevStopView {
    type Message = DevStopViewMsg;

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
        let steps = vec![InstallStep::builder().name("Test step").build()];
        let view = DevStopView::new(steps);
        assert!(!view.is_success());
    }
}
