//! Terminal UI helpers using Ferment.
//!
//! This module provides CLI-friendly wrappers around Ferment components
//! for common operations like spinners, progress bars, and confirmations.
//!
//! ## View Pattern
//!
//! Full-screen TUI views implement the [`ferment::Model`] trait directly:
//!
//! - [`StatusView`] - Cluster status with tabs and auto-refresh
//! - [`DoctorView`] - Environment health checks
//! - [`InstallView`] - Step-by-step installation progress

mod confirm;
pub mod doctor_view;
mod form;
pub mod install_view;
mod progress;
mod spinner;
pub mod status_view;

pub use confirm::{confirm, confirm_danger, confirm_with_options, ConfirmOptions, ConfirmResult};
pub use doctor_view::{CheckResult, DoctorView, DoctorViewMsg};
pub use form::{is_accessible, run_form};
pub use install_view::{InstallStep, InstallView, InstallViewMsg, StepExecutor, StepResult};
pub use progress::{multi_progress, progress, MultiProgressBar, ProgressBar};
pub use spinner::{spin, spin_result, start as start_spinner, SpinnerHandle};
pub use status_view::{
    ClusterStatus, EnvironmentStatus, RefreshFn, RefreshResult, StatusTab, StatusView,
    StatusViewMsg, TabData, TableRow,
};

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
