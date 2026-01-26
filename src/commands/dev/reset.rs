//! Reset command for dev cluster.
//!
//! Deletes and redeploys `InferaDB` applications with fresh data.

use std::time::Duration;

use teapot::style::{Color, RESET};

use super::{
    commands::{parse_kubectl_apply_line, run_command, run_command_optional},
    constants::{
        CLUSTER_NAME, INFERADB_DEPLOYMENTS, INFERADB_NAMESPACE, RESOURCE_TERMINATE_DELAY_SECS,
    },
    docker::docker_container_exists,
    kubernetes::{get_inferadb_deployments, get_pvcs},
    output::{
        confirm_warning, format_dot_leader, format_reset_dot_leader, print_prefixed_dot_leader,
        print_section_header, print_styled_header,
    },
    paths::get_deploy_dir,
};
use crate::{
    client::Context,
    error::{Error, Result},
    tui::start_spinner,
};

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

    perform_reset(can_redeploy, &deploy_dir);
    Ok(())
}

/// Show what will be reset and prompt for confirmation.
fn show_reset_preview(can_redeploy: bool) -> Result<()> {
    let deployments = get_inferadb_deployments();
    let pvcs = get_pvcs();

    print_styled_header("Reset InferaDB Development Cluster");
    print_section_header("Resources to be deleted");

    if deployments.is_empty() {
        print_prefixed_dot_leader("○", "Deployment", "none found");
    } else {
        for (name, replicas, image) in &deployments {
            let short_name = name
                .strip_prefix("dev-inferadb-")
                .or_else(|| name.strip_prefix("inferadb-"))
                .unwrap_or(name);
            let detail = format!("{replicas} replica(s), {image}");
            print_prefixed_dot_leader("○", &format!("Deployment: {short_name}"), &detail);
        }
    }

    if pvcs.is_empty() {
        print_prefixed_dot_leader("○", "Persistent Volume", "none found");
    } else {
        for (name, size, status) in &pvcs {
            let short_name = if name.starts_with("dev-inferadb-ledger-") {
                let suffix = name.strip_prefix("dev-inferadb-ledger-").unwrap_or(name);
                if let Some(pos) = suffix.find('-') {
                    format!("ledger-{}", &suffix[..pos])
                } else {
                    format!("ledger-{suffix}")
                }
            } else {
                name.clone()
            };
            let detail = format!("{size}, {status}");
            print_prefixed_dot_leader("○", &format!("Volume: {short_name}"), &detail);
        }
    }

    print_section_header("Actions after deletion");

    if can_redeploy {
        print_prefixed_dot_leader("○", "Redeploy from", "deploy/flux/apps/dev");
        print_prefixed_dot_leader("○", "Ledger", "new cluster with empty data");
        print_prefixed_dot_leader("○", "InferaDB Engine", "fresh deployment");
        print_prefixed_dot_leader("○", "InferaDB Control", "fresh deployment");
        print_prefixed_dot_leader("○", "InferaDB Dashboard", "fresh deployment");
    } else {
        print_prefixed_dot_leader("!", "Deploy directory", "not found - manual redeploy required");
    }

    println!();
    match confirm_warning("This action cannot be undone") {
        Ok(true) => {
            println!();
            Ok(())
        },
        Ok(false) => {
            println!("Aborted.");
            Err(Error::Other("User cancelled".to_string()))
        },
        Err(e) => Err(Error::Other(e.to_string())),
    }
}

/// Perform the actual reset operation.
fn perform_reset(can_redeploy: bool, deploy_dir: &std::path::Path) {
    print_styled_header("Resetting InferaDB Development Cluster");
    println!();

    // Delete Ledger StatefulSet
    {
        let spin = start_spinner("Deleting Ledger StatefulSet");
        let _ = run_command_optional(
            "kubectl",
            &["delete", "statefulset", "--all", "-n", INFERADB_NAMESPACE],
        );
        spin.success(&format_dot_leader("Deleted Ledger StatefulSet", "OK"));
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
        let _ =
            run_command_optional("kubectl", &["delete", "pvc", "--all", "-n", INFERADB_NAMESPACE]);
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
        redeploy_applications(deploy_dir);
    }

    let green = Color::Green.to_ansi_fg();
    let dim = Color::BrightBlack.to_ansi_fg();
    let reset_color = RESET;
    println!();
    println!("{green}✓ Cluster reset complete.{reset_color}");
    println!("{dim}  Applications may take a few minutes to become available.{reset_color}");
}

/// Redeploy applications from the deploy directory.
fn redeploy_applications(deploy_dir: &std::path::Path) {
    print_section_header("Redeploying Applications");

    let spin = start_spinner("Applying Kubernetes manifests");
    let kustomize_path = deploy_dir.join("flux/apps/dev");
    let apply_output = run_command("kubectl", &["apply", "-k", &kustomize_path.to_string_lossy()]);
    spin.clear();

    apply_output.map_or_else(
        |_| {},
        |output| {
            for line in output.lines() {
                if let Some((resource, status)) = parse_kubectl_apply_line(line) {
                    let prefix =
                        if status == "created" || status == "configured" { "✓" } else { "○" };
                    println!("  {}", format_reset_dot_leader(prefix, &resource, &status));
                }
            }
        },
    );

    println!();

    // Wait for Ledger cluster
    {
        let spin = start_spinner("Waiting for Ledger cluster");
        let mut ready = false;
        for _ in 0..150 {
            if let Some(output) = run_command_optional(
                "kubectl",
                &[
                    "get",
                    "statefulset",
                    "dev-inferadb-ledger",
                    "-n",
                    INFERADB_NAMESPACE,
                    "-o",
                    "jsonpath={.status.readyReplicas}",
                ],
            ) && output.trim() == "1"
            {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        if ready {
            spin.success(&format_dot_leader("Ledger cluster ready", "OK"));
        } else {
            spin.success(&format_dot_leader("Ledger cluster", "WAITING (may take a few minutes)"));
        }
    }

    // Restart engine to pick up new Ledger connection
    {
        let spin = start_spinner("Restarting engine to connect to Ledger");
        let _ = run_command_optional(
            "kubectl",
            &["rollout", "restart", "deployment/dev-inferadb-engine", "-n", INFERADB_NAMESPACE],
        );
        spin.success(&format_dot_leader("Engine deployment restarted", "OK"));
    }
}
