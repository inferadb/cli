//! Reset command for dev cluster.
//!
//! Deletes and redeploys InferaDB applications with fresh data.

use std::time::Duration;

use crate::client::Context;
use crate::error::{Error, Result};
use crate::tui::start_spinner;
use ferment::style::Color;

use super::commands::{parse_kubectl_apply_line, run_command, run_command_optional};
use super::constants::{INFERADB_DEPLOYMENTS, INFERADB_NAMESPACE, RESOURCE_TERMINATE_DELAY_SECS};
use super::docker::docker_container_exists;
use super::kubernetes::{get_fdb_clusters, get_inferadb_deployments, get_pvcs};
use super::output::{
    confirm_warning, format_dot_leader, format_reset_dot_leader, print_prefixed_dot_leader,
    print_section_header, print_styled_header,
};
use super::paths::get_deploy_dir;
use super::CLUSTER_NAME;

// ============================================================================
// Public API
// ============================================================================

/// Run dev reset - reset cluster data.
pub async fn reset(_ctx: &Context, yes: bool) -> Result<()> {
    reset_with_spinners(yes)
}

// ============================================================================
// Implementation
// ============================================================================

/// Reset with spinners.
fn reset_with_spinners(yes: bool) -> Result<()> {
    if !docker_container_exists(CLUSTER_NAME) {
        return Err(Error::Other(
            "Cluster is not running. Start with 'inferadb dev start'.".to_string(),
        ));
    }

    let deploy_dir = get_deploy_dir();
    let can_redeploy = deploy_dir.exists();

    if !yes {
        show_reset_preview(can_redeploy)?;
    }

    perform_reset(can_redeploy, &deploy_dir)
}

/// Show what will be reset and prompt for confirmation.
fn show_reset_preview(can_redeploy: bool) -> Result<()> {
    let fdb_clusters = get_fdb_clusters();
    let deployments = get_inferadb_deployments();
    let pvcs = get_pvcs();

    print_styled_header("Reset InferaDB Development Cluster");
    print_section_header("Resources to be deleted");

    if fdb_clusters.is_empty() {
        print_prefixed_dot_leader("○", "FoundationDB Cluster", "none found");
    } else {
        for (name, processes, version) in &fdb_clusters {
            let detail = format!("{} ({}, {})", name, processes, version);
            print_prefixed_dot_leader("○", "FoundationDB Cluster", &detail);
        }
    }

    if deployments.is_empty() {
        print_prefixed_dot_leader("○", "Deployment", "none found");
    } else {
        for (name, replicas, image) in &deployments {
            let short_name = name
                .strip_prefix("dev-inferadb-")
                .or_else(|| name.strip_prefix("inferadb-"))
                .unwrap_or(name);
            let detail = format!("{} replica(s), {}", replicas, image);
            print_prefixed_dot_leader("○", &format!("Deployment: {}", short_name), &detail);
        }
    }

    if pvcs.is_empty() {
        print_prefixed_dot_leader("○", "Persistent Volume", "none found");
    } else {
        for (name, size, status) in &pvcs {
            let short_name = if name.starts_with("dev-inferadb-fdb-") {
                let suffix = name.strip_prefix("dev-inferadb-fdb-").unwrap_or(name);
                if let Some(pos) = suffix.find('-') {
                    format!("fdb-{}", &suffix[..pos])
                } else {
                    format!("fdb-{}", suffix)
                }
            } else {
                name.clone()
            };
            let detail = format!("{}, {}", size, status);
            print_prefixed_dot_leader("○", &format!("Volume: {}", short_name), &detail);
        }
    }

    print_section_header("Actions after deletion");

    if can_redeploy {
        print_prefixed_dot_leader("○", "Redeploy from", "deploy/flux/apps/dev");
        print_prefixed_dot_leader("○", "FoundationDB", "new cluster with empty data");
        print_prefixed_dot_leader("○", "InferaDB Engine", "fresh deployment");
        print_prefixed_dot_leader("○", "InferaDB Control", "fresh deployment");
        print_prefixed_dot_leader("○", "InferaDB Dashboard", "fresh deployment");
    } else {
        print_prefixed_dot_leader(
            "!",
            "Deploy directory",
            "not found - manual redeploy required",
        );
    }

    println!();
    match confirm_warning("This action cannot be undone") {
        Ok(true) => {
            println!();
            Ok(())
        }
        Ok(false) => {
            println!("Aborted.");
            Err(Error::Other("User cancelled".to_string()))
        }
        Err(e) => Err(Error::Other(e.to_string())),
    }
}

/// Perform the actual reset operation.
fn perform_reset(can_redeploy: bool, deploy_dir: &std::path::Path) -> Result<()> {
    print_styled_header("Resetting InferaDB Development Cluster");
    println!();

    // Delete FDB cluster
    {
        let spin = start_spinner("Deleting FoundationDB Cluster");
        let _ = run_command_optional(
            "kubectl",
            &[
                "delete",
                "foundationdbcluster",
                "--all",
                "-n",
                INFERADB_NAMESPACE,
            ],
        );
        spin.success(&format_dot_leader("Deleted FoundationDB Cluster", "OK"));
    }

    // Delete InferaDB deployments
    {
        let spin = start_spinner("Deleting InferaDB Deployments");
        for deploy in INFERADB_DEPLOYMENTS {
            let _ = run_command_optional(
                "kubectl",
                &["delete", "deployment", deploy, "-n", INFERADB_NAMESPACE],
            );
        }
        spin.success(&format_dot_leader("Deleted InferaDB Deployments", "OK"));
    }

    // Delete PVCs
    {
        let spin = start_spinner("Deleting Persistent Volumes");
        let _ = run_command_optional(
            "kubectl",
            &["delete", "pvc", "--all", "-n", INFERADB_NAMESPACE],
        );
        spin.success(&format_dot_leader("Deleted Persistent Volumes", "OK"));
    }

    // Wait for resources to terminate
    {
        let spin = start_spinner("Waiting for resources to terminate");
        std::thread::sleep(Duration::from_secs(RESOURCE_TERMINATE_DELAY_SECS));
        spin.success(&format_dot_leader("Resources terminated", "OK"));
    }

    // Redeploy if possible
    if can_redeploy {
        redeploy_applications(deploy_dir)?;
    }

    let green = Color::Green.to_ansi_fg();
    let dim = Color::BrightBlack.to_ansi_fg();
    let reset_color = "\x1b[0m";
    println!();
    println!("{}✓ Cluster reset complete.{}", green, reset_color);
    println!(
        "{}  Applications may take a few minutes to become available.{}",
        dim, reset_color
    );

    Ok(())
}

/// Redeploy applications from the deploy directory.
fn redeploy_applications(deploy_dir: &std::path::Path) -> Result<()> {
    print_section_header("Redeploying Applications");

    let spin = start_spinner("Applying Kubernetes manifests");
    let apply_output = run_command(
        "kubectl",
        &[
            "apply",
            "-k",
            deploy_dir.join("flux/apps/dev").to_str().unwrap(),
        ],
    );
    spin.clear();

    if let Ok(output) = apply_output {
        for line in output.lines() {
            if let Some((resource, status)) = parse_kubectl_apply_line(line) {
                let prefix = if status == "created" || status == "configured" {
                    "✓"
                } else {
                    "○"
                };
                println!("  {}", format_reset_dot_leader(prefix, &resource, &status));
            }
        }
    }

    println!();

    // Wait for FDB cluster
    {
        let spin = start_spinner("Waiting for FoundationDB cluster");
        let mut ready = false;
        for _ in 0..150 {
            if let Some(output) = run_command_optional(
                "kubectl",
                &[
                    "get",
                    "foundationdbcluster",
                    "dev-inferadb-fdb",
                    "-n",
                    INFERADB_NAMESPACE,
                    "-o",
                    "jsonpath={.status.health.available}",
                ],
            ) {
                if output.trim() == "true" {
                    ready = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        if ready {
            spin.success(&format_dot_leader("FoundationDB cluster ready", "OK"));
        } else {
            spin.success(&format_dot_leader(
                "FoundationDB cluster",
                "WAITING (may take a few minutes)",
            ));
        }
    }

    // Restart engine to pick up new cluster file
    {
        let spin = start_spinner("Restarting engine to pick up new cluster file");
        let _ = run_command_optional(
            "kubectl",
            &[
                "rollout",
                "restart",
                "deployment/dev-inferadb-engine",
                "-n",
                INFERADB_NAMESPACE,
            ],
        );
        spin.success(&format_dot_leader("Engine deployment restarted", "OK"));
    }

    Ok(())
}
