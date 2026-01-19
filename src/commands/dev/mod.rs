//! Local development cluster commands.
//!
//! Manages a local Talos Kubernetes cluster for `InferaDB` development,
//! including Ledger, the engine, control plane, and dashboard.
//!
//! # Module Structure
//!
//! - `commands` - Shell command wrappers
//! - `constants` - Cluster configuration constants
//! - `docker` - Docker container operations
//! - `doctor` - Environment checking
//! - `kubernetes` - Kubernetes/kubectl abstractions
//! - `output` - Output formatting utilities
//! - `paths` - Path helpers
//! - `reset` - Reset command implementation
//! - `start` - Start command implementation
//! - `status` - Status command implementation
//! - `stop` - Stop command implementation
//! - `tailscale` - Tailscale credential handling

// Submodules
pub mod commands;
pub mod constants;
pub mod docker;
pub mod doctor;
pub mod kubernetes;
pub mod output;
pub mod paths;
mod reset;
mod start;
mod status;
mod stop;
pub mod tailscale;

// Re-export public items from submodules for convenience
use std::process::Command;

use commands::run_command_streaming;
pub use constants::*;
use docker::cluster_exists;
pub use doctor::doctor;

use crate::{
    client::Context,
    error::{Error, Result},
};

// ============================================================================
// Public Command Functions
// ============================================================================

/// Run dev start - create or resume local development cluster.
pub async fn start(
    ctx: &Context,
    skip_build: bool,
    interactive: bool,
    tailscale_client: Option<String>,
    tailscale_secret: Option<String>,
    force: bool,
    commit: Option<&str>,
) -> Result<()> {
    start::start(ctx, skip_build, interactive, tailscale_client, tailscale_secret, force, commit)
        .await
}

/// Run dev stop - pause or destroy the cluster.
pub async fn stop(
    ctx: &Context,
    destroy: bool,
    yes: bool,
    with_credentials: bool,
    interactive: bool,
) -> Result<()> {
    stop::stop(ctx, destroy, yes, with_credentials, interactive).await
}

/// Run dev status - show cluster status.
pub async fn dev_status(ctx: &Context, interactive: bool) -> Result<()> {
    status::dev_status(ctx, interactive).await
}

/// Run dev reset - reset cluster data.
pub async fn reset(ctx: &Context, yes: bool) -> Result<()> {
    reset::reset(ctx, yes).await
}

/// Run dev logs - view logs.
pub async fn logs(_ctx: &Context, follow: bool, service: Option<&str>, tail: u32) -> Result<()> {
    if !cluster_exists() {
        return Err(Error::Other(
            "Cluster is not running. Start with 'inferadb dev start'.".to_string(),
        ));
    }

    let mut args = vec!["logs", "-n", "inferadb"];

    let (selector, service_name) = match service {
        Some("engine") => ("app.kubernetes.io/name=inferadb-engine", "engine"),
        Some("control") => ("app.kubernetes.io/name=inferadb-control", "control"),
        Some("dashboard") => ("app.kubernetes.io/name=inferadb-dashboard", "dashboard"),
        Some("ledger") => ("app.kubernetes.io/name=inferadb-ledger", "ledger"),
        Some(s) => (s, s),
        None => ("app.kubernetes.io/part-of=inferadb", "all"),
    };

    args.extend(["-l", selector]);

    if follow {
        args.push("-f");
    }

    let tail_str = tail.to_string();
    args.extend(["--tail", &tail_str]);

    println!("Streaming logs for {service_name} pods...\n");
    run_command_streaming("kubectl", &args, &[])?;
    Ok(())
}

/// Run dev dashboard - open dashboard in browser.
pub async fn dashboard(_ctx: &Context) -> Result<()> {
    use commands::run_command_optional;

    // Get dashboard URL from ingress
    let hostname = run_command_optional(
        "kubectl",
        &[
            "get",
            "ingress",
            "dev-inferadb-dashboard-tailscale",
            "-n",
            "inferadb",
            "-o",
            "jsonpath={.status.loadBalancer.ingress[0].hostname}",
        ],
    );

    match hostname {
        Some(h) if !h.is_empty() => {
            let url = format!("https://{}", h.trim());
            println!("Opening dashboard: {url}");
            #[cfg(target_os = "macos")]
            {
                let _ = Command::new("open").arg(&url).spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = Command::new("xdg-open").arg(&url).spawn();
            }
            #[cfg(target_os = "windows")]
            {
                let _ = Command::new("cmd").args(["/C", "start", &url]).spawn();
            }
            Ok(())
        },
        _ => Err(Error::Other("Dashboard ingress not found. Is the cluster running?".to_string())),
    }
}
