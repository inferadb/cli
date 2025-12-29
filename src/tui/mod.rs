//! Terminal UI helpers using Ferment.
//!
//! This module provides CLI-friendly wrappers around Ferment components
//! for common operations like spinners, progress bars, and confirmations.
//!
//! ## View Pattern
//!
//! Full-screen TUI views implement the [`ferment::Model`] trait directly:
//!
//! - [`DevStatusView`] - Cluster status with tabs and auto-refresh
//! - [`DevDoctorView`] - Environment health checks
//! - [`DevInstallView`] - Step-by-step installation progress
//! - [`DevUninstallView`] - Uninstall with confirmation modal
//! - [`DevStartView`] - Start cluster with Tailscale setup modals
//! - [`DevStopView`] - Stop/pause cluster with progress

mod confirm;
pub mod doctor_view;
mod form;
pub mod install_view;
mod progress;
mod spinner;
pub mod start_view;
pub mod status_view;
pub mod stop_view;
pub mod uninstall_view;

pub use confirm::{confirm, confirm_danger, confirm_with_options, ConfirmOptions, ConfirmResult};
pub use doctor_view::{CheckResult, DevDoctorView, DevDoctorViewMsg};
pub use form::{is_accessible, run_form};
pub use install_view::{DevInstallView, DevInstallViewMsg, InstallStep, StepExecutor, StepResult};
pub use progress::{multi_progress, progress, MultiProgressBar, ProgressBar};
pub use spinner::{spin, spin_result, start as start_spinner, SpinnerHandle};
pub use start_view::{DevStartView, DevStartViewMsg, StartPhase};
pub use status_view::{
    ClusterStatus, DevStatusView, DevStatusViewMsg, EnvironmentStatus, RefreshFn, RefreshResult,
    StatusTab, TabData, TableRow,
};
pub use stop_view::{DevStopView, DevStopViewMsg};
pub use uninstall_view::{DevUninstallView, DevUninstallViewMsg, UninstallInfo};

use crate::client::Context;

/// Check if we should use interactive TUI features.
///
/// Returns false if:
/// - Running in CI
/// - Output is not a TTY
/// - Quiet mode is enabled
pub fn is_interactive(ctx: &Context) -> bool {
    !ctx.output.is_quiet() && ferment::output::is_tty() && !ferment::output::is_ci()
}
