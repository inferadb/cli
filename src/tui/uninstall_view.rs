//! Interactive uninstall view for dev cluster removal.
//!
//! A full-screen TUI showing a confirmation modal before uninstalling,
//! then progress with animated spinners, task completion status, and error modals.
//!
//! This module provides a thin wrapper around Ferment's `TaskProgressView`.

use std::any::Any;
use std::path::PathBuf;

use ferment::components::{
    ConfirmationConfig, Phase, StepResult as FermentStepResult, TaskProgressMsg, TaskProgressView,
    TaskStep,
};
use ferment::runtime::{Cmd, Model, Sub};
use ferment::style::Color;
use ferment::terminal::Event;

use super::install_view::{InstallStep, StepResult};

// ============================================================================
// Constants
// ============================================================================

const CLUSTER_NAME: &str = "inferadb-dev";
const REGISTRY_NAME: &str = "inferadb-registry";

// ============================================================================
// Uninstall Info
// ============================================================================

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

// ============================================================================
// Context for Confirmation Modal
// ============================================================================

/// Context passed to the confirmation modal.
#[derive(Clone)]
struct UninstallContext {
    info: UninstallInfo,
    with_credentials: bool,
}

// ============================================================================
// Message Type
// ============================================================================

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

impl From<DevUninstallViewMsg> for TaskProgressMsg {
    fn from(msg: DevUninstallViewMsg) -> Self {
        match msg {
            DevUninstallViewMsg::Tick => TaskProgressMsg::Tick,
            DevUninstallViewMsg::Confirm => TaskProgressMsg::Confirm,
            DevUninstallViewMsg::Cancel => TaskProgressMsg::Cancel,
            DevUninstallViewMsg::RunStep(idx) => TaskProgressMsg::RunStep(idx),
            DevUninstallViewMsg::StepCompleted(idx, result) => {
                TaskProgressMsg::StepCompleted(idx, result.into())
            }
            DevUninstallViewMsg::CloseModal => TaskProgressMsg::CloseModal,
            DevUninstallViewMsg::Quit => TaskProgressMsg::Quit,
            DevUninstallViewMsg::Resize(w, h) => TaskProgressMsg::Resize(w, h),
        }
    }
}

impl From<TaskProgressMsg> for DevUninstallViewMsg {
    fn from(msg: TaskProgressMsg) -> Self {
        match msg {
            TaskProgressMsg::Tick => DevUninstallViewMsg::Tick,
            TaskProgressMsg::Start => DevUninstallViewMsg::Confirm,
            TaskProgressMsg::Confirm => DevUninstallViewMsg::Confirm,
            TaskProgressMsg::Cancel => DevUninstallViewMsg::Cancel,
            TaskProgressMsg::RunStep(idx) => DevUninstallViewMsg::RunStep(idx),
            TaskProgressMsg::StepCompleted(idx, result) => DevUninstallViewMsg::StepCompleted(
                idx,
                match result {
                    FermentStepResult::Success(d) => StepResult::Success(d),
                    FermentStepResult::Skipped(r) => StepResult::Skipped(r),
                    FermentStepResult::Failure(e) => StepResult::Failure(e),
                },
            ),
            TaskProgressMsg::StartTask(_) => DevUninstallViewMsg::Tick,
            TaskProgressMsg::CompleteTask(idx, result) => DevUninstallViewMsg::StepCompleted(
                idx,
                match result {
                    FermentStepResult::Success(d) => StepResult::Success(d),
                    FermentStepResult::Skipped(r) => StepResult::Skipped(r),
                    FermentStepResult::Failure(e) => StepResult::Failure(e),
                },
            ),
            TaskProgressMsg::CloseModal => DevUninstallViewMsg::CloseModal,
            TaskProgressMsg::Quit => DevUninstallViewMsg::Quit,
            TaskProgressMsg::Resize(w, h) => DevUninstallViewMsg::Resize(w, h),
        }
    }
}

// ============================================================================
// View Implementation
// ============================================================================

/// The uninstall view state.
pub struct DevUninstallView {
    inner: TaskProgressView,
}

impl DevUninstallView {
    /// Create a new uninstall view with the given steps and info.
    pub fn new(steps: Vec<InstallStep>, info: UninstallInfo, with_credentials: bool) -> Self {
        let task_steps: Vec<TaskStep> = steps.into_iter().map(|s| s.into()).collect();

        let context = UninstallContext {
            info: info.clone(),
            with_credentials,
        };

        let inner = TaskProgressView::with_context(task_steps, context)
            .title("InferaDB Development Cluster")
            .subtitle("Uninstall")
            .with_confirmation(ConfirmationConfig {
                title: "Confirm Uninstall".to_string(),
                title_color: Color::Yellow,
                border_color: Color::Yellow,
                content_fn: Box::new(|ctx: &dyn Any| {
                    if let Some(uctx) = ctx.downcast_ref::<UninstallContext>() {
                        let mut lines = uctx.info.removal_lines();
                        if uctx.with_credentials && uctx.info.has_creds_file {
                            lines.push("Tailscale credentials".to_string());
                        }
                        lines
                    } else {
                        vec!["Unknown items".to_string()]
                    }
                }),
            })
            .hints_confirming(vec![("y", "confirm"), ("n", "cancel")])
            .hints_running(vec![("q", "cancel")])
            .hints_completed(vec![("q", "quit")])
            .build();

        Self { inner }
    }

    /// Set custom title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.inner = TaskProgressView::builder(vec![])
            .title(title)
            .subtitle("Uninstall")
            .build();
        self
    }

    /// Set custom subtitle.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.inner = TaskProgressView::builder(vec![])
            .title("InferaDB Development Cluster")
            .subtitle(subtitle)
            .build();
        self
    }

    /// Check if the view should quit.
    pub fn should_quit(&self) -> bool {
        self.inner.should_quit()
    }

    /// Check if uninstall was cancelled by user.
    pub fn was_cancelled(&self) -> bool {
        self.inner.was_cancelled()
    }

    /// Check if uninstall completed successfully.
    pub fn is_success(&self) -> bool {
        self.inner.is_success()
    }

    /// Check if there was a failure.
    pub fn has_failure(&self) -> bool {
        self.inner.has_failure()
    }

    /// Get the current phase.
    pub fn phase(&self) -> &Phase {
        self.inner.phase()
    }
}

impl Model for DevUninstallView {
    type Message = DevUninstallViewMsg;

    fn init(&self) -> Option<Cmd<Self::Message>> {
        self.inner
            .init()
            .map(|cmd| cmd.map(DevUninstallViewMsg::from))
    }

    fn update(&mut self, msg: Self::Message) -> Option<Cmd<Self::Message>> {
        let inner_msg: TaskProgressMsg = msg.into();
        self.inner
            .update(inner_msg)
            .map(|cmd| cmd.map(DevUninstallViewMsg::from))
    }

    fn view(&self) -> String {
        self.inner.view()
    }

    fn handle_event(&self, event: Event) -> Option<Self::Message> {
        self.inner
            .handle_event(event)
            .map(DevUninstallViewMsg::from)
    }

    fn subscriptions(&self) -> Sub<Self::Message> {
        self.inner.subscriptions().map(DevUninstallViewMsg::from)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uninstall_view_creation() {
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
