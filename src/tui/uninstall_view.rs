//! Interactive uninstall view for dev cluster removal.
//!
//! A full-screen TUI showing a confirmation modal before uninstalling,
//! then progress with animated spinners, task completion status, and error modals.
//!
//! This module provides a thin wrapper around Teapot's `TaskProgressView`.

use std::{any::Any, path::PathBuf};

use bon::Builder;
use teapot::{
    components::{ConfirmationConfig, Phase, TaskProgressMsg, TaskProgressView, TaskStep},
    runtime::{Cmd, Model, Sub},
    style::Color,
    terminal::Event,
};

use super::install_view::InstallStep;

// ============================================================================
// Constants
// ============================================================================

const CLUSTER_NAME: &str = "inferadb-dev";
const REGISTRY_NAME: &str = "inferadb-registry";

// ============================================================================
// Uninstall Info
// ============================================================================

/// Information about what will be uninstalled.
#[derive(Debug, Clone, Builder)]
pub struct UninstallInfo {
    /// Whether cluster exists.
    #[builder(default)]
    pub has_cluster: bool,
    /// Cluster status (running/paused).
    pub cluster_status: Option<String>,
    /// Whether registry exists.
    #[builder(default)]
    pub has_registry: bool,
    /// Deploy directory path.
    #[builder(into)]
    pub deploy_dir: PathBuf,
    /// Whether deploy directory exists.
    #[builder(default)]
    pub has_deploy_dir: bool,
    /// Data directory path.
    #[builder(into)]
    pub data_dir: PathBuf,
    /// State directory path.
    #[builder(into)]
    pub state_dir: PathBuf,
    /// Whether state directory exists.
    #[builder(default)]
    pub has_state_dir: bool,
    /// Config directory path.
    #[builder(into)]
    pub config_dir: PathBuf,
    /// Credentials file path.
    #[builder(into)]
    pub creds_file: PathBuf,
    /// Whether credentials file exists.
    #[builder(default)]
    pub has_creds_file: bool,
    /// Number of dev Docker images.
    #[builder(default)]
    pub dev_image_count: usize,
    /// Whether kubectl context exists.
    #[builder(default)]
    pub has_kube_context: bool,
    /// Whether talos context exists.
    #[builder(default)]
    pub has_talos_context: bool,
}

impl UninstallInfo {
    /// Get description lines for what will be removed.
    #[must_use]
    pub fn removal_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        if self.has_cluster {
            let status = self.cluster_status.as_deref().unwrap_or("unknown");
            lines.push(format!("Talos cluster '{CLUSTER_NAME}' ({status})"));
        }

        if self.has_registry {
            lines.push(format!("Local Docker registry '{REGISTRY_NAME}'"));
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
    #[must_use]
    pub const fn has_anything(&self) -> bool {
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
// Type Alias
// ============================================================================

/// Message type for uninstall view.
/// This is a type alias to the underlying `TaskProgressMsg`.
pub type DevUninstallViewMsg = TaskProgressMsg;

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
        let task_steps: Vec<TaskStep> = steps.into_iter().map(std::convert::Into::into).collect();

        let context = UninstallContext { info, with_credentials };

        let inner = TaskProgressView::builder()
            .steps(task_steps)
            .title("InferaDB Development Cluster")
            .subtitle("Uninstall")
            .confirmation(ConfirmationConfig {
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
            .context(Box::new(context) as Box<dyn Any + Send + Sync>)
            .hints_confirming(vec![
                ("y".to_string(), "confirm".to_string()),
                ("n".to_string(), "cancel".to_string()),
            ])
            .hints_running(vec![("q".to_string(), "cancel".to_string())])
            .hints_completed(vec![("q".to_string(), "quit".to_string())])
            .build();

        Self { inner }
    }

    /// Set custom title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.inner =
            TaskProgressView::builder().steps(vec![]).title(title).subtitle("Uninstall").build();
        self
    }

    /// Set custom subtitle.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.inner = TaskProgressView::builder()
            .steps(vec![])
            .title("InferaDB Development Cluster")
            .subtitle(subtitle)
            .build();
        self
    }

    /// Check if the view should quit.
    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.inner.should_quit()
    }

    /// Check if uninstall was cancelled by user.
    #[must_use]
    pub fn was_cancelled(&self) -> bool {
        self.inner.was_cancelled()
    }

    /// Check if uninstall completed successfully.
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

impl Model for DevUninstallView {
    type Message = DevUninstallViewMsg;

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
    fn test_uninstall_view_creation() {
        let info = UninstallInfo::builder()
            .has_cluster(true)
            .cluster_status("running".to_string())
            .has_registry(true)
            .deploy_dir("/test/deploy")
            .has_deploy_dir(true)
            .data_dir("/test/data")
            .state_dir("/test/state")
            .has_state_dir(true)
            .config_dir("/test/config")
            .creds_file("/test/creds")
            .has_creds_file(true)
            .dev_image_count(5)
            .has_kube_context(true)
            .has_talos_context(true)
            .build();

        let steps = vec![
            InstallStep::builder().name("Step 1").build(),
            InstallStep::builder().name("Step 2").build(),
        ];

        let view = DevUninstallView::new(steps, info, false);
        assert!(!view.is_success());
        assert!(!view.was_cancelled());
    }

    #[test]
    fn test_uninstall_info_removal_lines() {
        let info = UninstallInfo::builder()
            .has_cluster(true)
            .cluster_status("running".to_string())
            .deploy_dir("/test/deploy")
            .has_deploy_dir(true)
            .data_dir("/test/data")
            .state_dir("/test/state")
            .config_dir("/test/config")
            .creds_file("/test/creds")
            .build();

        let lines = info.removal_lines();
        assert!(lines.iter().any(|l| l.contains("Talos cluster")));
        assert!(lines.iter().any(|l| l.contains("Deploy repository")));
        assert!(!lines.iter().any(|l| l.contains("registry")));
    }
}
