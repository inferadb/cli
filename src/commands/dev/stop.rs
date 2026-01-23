//! Stop/uninstall command for dev cluster.
//!
//! Handles pausing containers and destroying the cluster.

use std::{fs, process::Command};

use teapot::{
    output::{info as print_info, success as print_success},
    style::{Color, RESET},
};

use super::{
    commands::{run_command, run_command_optional},
    constants::{CLUSTER_NAME, KUBE_CONTEXT, REGISTRY_NAME, TAILSCALE_DEVICE_PREFIX},
    docker::{
        cluster_exists, docker_container_exists, get_cluster_containers, get_dev_docker_images,
        get_expected_cluster_containers, is_container_paused, registry_exists, remove_image,
    },
    output::{
        StepOutcome, confirm_prompt, format_dot_leader, print_destroy_skipped, print_hint,
        print_styled_header, run_destroy_step,
    },
    paths::{
        get_config_dir, get_data_dir, get_deploy_dir, get_state_dir, get_tailscale_creds_file,
    },
    tailscale::load_tailscale_credentials,
};
use crate::{
    client::Context,
    error::{Error, Result},
    tui::UninstallInfo,
};

// ============================================================================
// Public API
// ============================================================================

/// Run dev stop - pause or destroy the cluster.
pub async fn stop(
    ctx: &Context,
    destroy: bool,
    yes: bool,
    with_credentials: bool,
    interactive: bool,
) -> Result<()> {
    if destroy {
        if interactive && crate::tui::is_interactive(ctx) {
            return uninstall_interactive(with_credentials);
        }
        uninstall_with_spinners(yes, with_credentials);
        return Ok(());
    }

    // Pause containers
    stop_with_spinners();
    Ok(())
}

// ============================================================================
// Pause Mode
// ============================================================================

/// Stop with inline spinners (pause containers).
fn stop_with_spinners() {
    print_styled_header("Pausing InferaDB Development Cluster");
    println!();

    let mut any_paused = false;

    let expected_containers = get_expected_cluster_containers();

    for container in &expected_containers {
        any_paused |= pause_container_with_spinner(container);
    }

    let actual_containers = get_cluster_containers();
    for container in &actual_containers {
        if expected_containers.contains(container) {
            continue;
        }
        any_paused |= pause_container_with_spinner(container);
    }

    any_paused |= pause_container_with_spinner(REGISTRY_NAME);

    println!();
    if any_paused {
        print_success("Cluster paused successfully!");
    } else {
        println!("Nothing to pause.");
    }
    println!();
    print_hint("Run 'inferadb dev start' to resume the cluster");
    print_hint("Run 'inferadb dev stop --destroy' to tear down the cluster");
}

/// Pause a single container, showing spinner and returning whether work was done.
fn pause_container_with_spinner(container: &str) -> bool {
    use crate::tui::start_spinner;

    let display_name = container.strip_prefix(&format!("{CLUSTER_NAME}-")).unwrap_or(container);
    let in_progress = format!("Pausing {display_name}");
    let completed = format!("Paused {display_name}");
    let mut spin = start_spinner(&in_progress);

    if !docker_container_exists(container) {
        spin.stop();
        print_destroy_skipped(&completed);
        return false;
    }

    if is_container_paused(container) {
        spin.stop();
        print_destroy_skipped(&completed);
        return false;
    }

    match run_command("docker", &["pause", container]) {
        Ok(_) => {
            spin.success(&format_dot_leader(&completed, "OK"));
            true
        },
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("already paused")
                || err_str.contains("No such container")
                || err_str.contains("not found")
            {
                spin.stop();
                print_destroy_skipped(&completed);
            } else {
                spin.failure(&err_str);
            }
            false
        },
    }
}

// ============================================================================
// Uninstall Info
// ============================================================================

/// Gather information about what will be uninstalled.
pub fn gather_uninstall_info() -> UninstallInfo {
    use super::docker::are_containers_paused;

    let deploy_dir = get_deploy_dir();
    let creds_file = get_tailscale_creds_file();
    let config_dir = get_config_dir();
    let data_dir = get_data_dir();
    let state_dir = get_state_dir();

    let has_cluster = cluster_exists();
    let cluster_status = if has_cluster {
        Some(if are_containers_paused() { "paused" } else { "running" }.to_string())
    } else {
        None
    };

    let has_registry = registry_exists();
    let has_deploy_dir = deploy_dir.exists();
    let has_state_dir = state_dir.exists();
    let has_creds_file = creds_file.exists();

    let dev_images = get_dev_docker_images();
    let dev_image_count = dev_images.len();

    let has_kube_context =
        run_command_optional("kubectl", &["config", "get-contexts", "-o", "name"])
            .is_some_and(|o| o.lines().any(|l| l == KUBE_CONTEXT));
    let has_talos_context = run_command_optional("talosctl", &["config", "contexts"])
        .is_some_and(|o| o.contains(CLUSTER_NAME));

    UninstallInfo {
        has_cluster,
        cluster_status,
        has_registry,
        deploy_dir,
        has_deploy_dir,
        data_dir,
        state_dir,
        has_state_dir,
        config_dir,
        creds_file,
        has_creds_file,
        dev_image_count,
        has_kube_context,
        has_talos_context,
    }
}

// ============================================================================
// Uninstall Steps
// ============================================================================

/// Step: Destroy Talos cluster and clean up Tailscale devices.
fn step_destroy_cluster() -> std::result::Result<StepOutcome, String> {
    if !cluster_exists() {
        return Ok(StepOutcome::Skipped);
    }

    // Clean up Tailscale devices first
    cleanup_tailscale_devices().map_err(|e| e.to_string())?;

    // Destroy cluster
    run_command("talosctl", &["cluster", "destroy", "--name", CLUSTER_NAME])
        .map_err(|e| e.to_string())?;

    Ok(StepOutcome::Success)
}

/// Step: Remove local Docker registry.
#[allow(clippy::unnecessary_wraps)]
fn step_remove_registry() -> std::result::Result<StepOutcome, String> {
    if !registry_exists() {
        return Ok(StepOutcome::Skipped);
    }

    let _ = run_command_optional("docker", &["stop", REGISTRY_NAME]);
    let _ = run_command_optional("docker", &["rm", "-f", REGISTRY_NAME]);

    Ok(StepOutcome::Success)
}

/// Step: Clean up kubectl/talosctl contexts.
#[allow(clippy::unnecessary_wraps)]
fn step_cleanup_contexts() -> std::result::Result<StepOutcome, String> {
    let has_talos = run_command_optional("talosctl", &["config", "contexts"])
        .is_some_and(|o| o.contains(CLUSTER_NAME));

    let has_kube = run_command_optional("kubectl", &["config", "get-contexts", "-o", "name"])
        .is_some_and(|o| o.lines().any(|l| l == KUBE_CONTEXT));

    if !has_talos && !has_kube {
        return Ok(StepOutcome::Skipped);
    }

    cleanup_stale_contexts();
    Ok(StepOutcome::Success)
}

/// Clean up stale kubectl/talosctl contexts.
fn cleanup_stale_contexts() {
    // Clean kubectl context
    let _ = run_command_optional("kubectl", &["config", "delete-context", KUBE_CONTEXT]);
    let _ = run_command_optional("kubectl", &["config", "delete-cluster", CLUSTER_NAME]);
    let _ = run_command_optional(
        "kubectl",
        &["config", "delete-user", &format!("admin@{CLUSTER_NAME}")],
    );

    // Clean talosctl context
    let _ = run_command_optional("talosctl", &["config", "remove", CLUSTER_NAME, "--noconfirm"]);
}

/// Step: Remove Docker images.
#[allow(clippy::unnecessary_wraps)]
fn step_remove_docker_images() -> std::result::Result<Option<String>, String> {
    let dev_images = get_dev_docker_images();
    if dev_images.is_empty() {
        return Ok(Some("No images to remove".to_string()));
    }

    let mut removed = 0;
    for image in &dev_images {
        if run_command_optional("docker", &["rmi", "-f", image]).is_some() {
            removed += 1;
        }
    }

    Ok(Some(format!("Removed {} of {} images", removed, dev_images.len())))
}

/// Step: Remove state directory.
fn step_remove_state_dir() -> std::result::Result<StepOutcome, String> {
    let state_dir = get_state_dir();
    if !state_dir.exists() {
        return Ok(StepOutcome::Skipped);
    }

    fs::remove_dir_all(&state_dir)
        .map_err(|e| format!("Failed to remove {}: {}", state_dir.display(), e))?;

    Ok(StepOutcome::Success)
}

/// Step: Remove Tailscale credentials.
fn step_remove_tailscale_creds() -> std::result::Result<StepOutcome, String> {
    let creds_file = get_tailscale_creds_file();
    if !creds_file.exists() {
        return Ok(StepOutcome::Skipped);
    }
    fs::remove_file(&creds_file)
        .map_err(|e| format!("Failed to remove {}: {}", creds_file.display(), e))?;
    Ok(StepOutcome::Success)
}

// ============================================================================
// Tailscale Cleanup
// ============================================================================

/// Clean up Tailscale devices via API.
fn cleanup_tailscale_devices() -> Result<()> {
    let Some((client_id, client_secret)) = load_tailscale_credentials() else {
        return Ok(());
    };

    // Get OAuth token
    let output = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            "-d",
            &format!("client_id={client_id}&client_secret={client_secret}"),
            "https://api.tailscale.com/api/v2/oauth/token",
        ])
        .output()
        .map_err(|e| Error::Other(e.to_string()))?;

    let response = String::from_utf8_lossy(&output.stdout);
    let token: Option<String> = serde_json::from_str::<serde_json::Value>(&response)
        .ok()
        .and_then(|v| v.get("access_token").and_then(|t| t.as_str()).map(String::from));

    let Some(token) = token else {
        return Ok(());
    };

    // List devices
    let output = Command::new("curl")
        .args([
            "-s",
            "-H",
            &format!("Authorization: Bearer {token}"),
            "https://api.tailscale.com/api/v2/tailnet/-/devices",
        ])
        .output()
        .map_err(|e| Error::Other(e.to_string()))?;

    let response = String::from_utf8_lossy(&output.stdout);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response)
        && let Some(devices) = json.get("devices").and_then(|d| d.as_array())
    {
        for device in devices {
            let name = device.get("hostname").and_then(|n| n.as_str()).unwrap_or("");
            let id = device.get("id").and_then(|i| i.as_str()).unwrap_or("");

            if name.starts_with(TAILSCALE_DEVICE_PREFIX) && !id.is_empty() {
                let _ = Command::new("curl")
                    .args([
                        "-s",
                        "-X",
                        "DELETE",
                        "-H",
                        &format!("Authorization: Bearer {token}"),
                        &format!("https://api.tailscale.com/api/v2/device/{id}"),
                    ])
                    .output();
            }
        }
    }

    Ok(())
}

// ============================================================================
// Streaming Mode
// ============================================================================

/// Uninstall with spinners.
fn uninstall_with_spinners(yes: bool, with_credentials: bool) {
    print_styled_header("Destroying InferaDB Development Cluster");

    let info = gather_uninstall_info();
    let initially_had_something = info.has_anything();

    println!();
    if initially_had_something {
        println!("This will destroy:");
        println!();
        for line in info.removal_lines() {
            println!("  * {line}");
        }
        if with_credentials && info.has_creds_file {
            println!("  * Tailscale credentials");
        }
        println!();

        if !yes {
            if !matches!(confirm_prompt("Are you sure you want to continue?"), Ok(true)) {
                println!("Aborted.");
                return;
            }
            println!();
        }
    }

    let mut did_work = false;

    did_work |= run_destroy_step("Removing registry", "Removed registry", step_remove_registry);
    did_work |= run_destroy_step("Destroying cluster", "Destroyed cluster", step_destroy_cluster);
    did_work |= run_destroy_step("Cleaning contexts", "Cleaned contexts", step_cleanup_contexts);

    let dev_images = get_dev_docker_images();
    if dev_images.is_empty() {
        print_destroy_skipped("Remove images");
    } else {
        for image in &dev_images {
            let image_clone = image.clone();
            let display_name = image
                .strip_prefix("ghcr.io/siderolabs/")
                .or_else(|| image.strip_prefix("registry.k8s.io/"))
                .unwrap_or(image);
            did_work |= run_destroy_step(
                &format!("Removing image {display_name}"),
                &format!("Removed image {display_name}"),
                move || {
                    if remove_image(&image_clone) {
                        Ok(StepOutcome::Success)
                    } else {
                        Ok(StepOutcome::Skipped)
                    }
                },
            );
        }
    }

    did_work |= run_destroy_step(
        "Removing state directory",
        "Removed state directory",
        step_remove_state_dir,
    );

    if with_credentials {
        did_work |= run_destroy_step(
            "Removing Tailscale credentials",
            "Removed Tailscale credentials",
            step_remove_tailscale_creds,
        );
    }

    println!();
    if did_work {
        let green = Color::Green.to_ansi_fg();
        let reset = RESET;
        println!("{green}Cluster destroyed successfully.{reset}");

        if !with_credentials && info.has_creds_file {
            println!();
            print_info("Tailscale credentials were preserved for future dev clusters.");
            print_info("To also remove them: inferadb dev stop --destroy --with-credentials");
        }
    } else {
        println!("Nothing to destroy.");
    }

    println!();
    print_hint("Run 'inferadb dev start' to start the cluster");
}

// ============================================================================
// Interactive Mode
// ============================================================================

/// Uninstall interactive TUI mode.
fn uninstall_interactive(with_credentials: bool) -> Result<()> {
    use teapot::runtime::{Program, ProgramOptions};

    use crate::tui::{DevUninstallView, InstallStep};

    let info = gather_uninstall_info();

    if !info.has_anything() {
        println!("Nothing to destroy. The development cluster is not installed.");
        return Ok(());
    }

    let mut steps = Vec::new();

    if info.has_registry {
        steps.push(InstallStep::with_executor("Removing registry", || {
            step_remove_registry().map(|r| match r {
                StepOutcome::Success => Some("Removed".to_string()),
                StepOutcome::Skipped => Some("Skipped".to_string()),
                StepOutcome::Failed(_) => None,
            })
        }));
    }

    if info.has_cluster {
        steps.push(InstallStep::with_executor("Destroying cluster", || {
            step_destroy_cluster().map(|r| match r {
                StepOutcome::Success => Some("Destroyed".to_string()),
                StepOutcome::Skipped => Some("Skipped".to_string()),
                StepOutcome::Failed(_) => None,
            })
        }));
    }

    if info.has_kube_context || info.has_talos_context {
        steps.push(InstallStep::with_executor("Cleaning contexts", || {
            step_cleanup_contexts().map(|r| match r {
                StepOutcome::Success => Some("Cleaned".to_string()),
                StepOutcome::Skipped => Some("Skipped".to_string()),
                StepOutcome::Failed(_) => None,
            })
        }));
    }

    if info.dev_image_count > 0 {
        steps.push(InstallStep::with_executor("Removing Docker images", step_remove_docker_images));
    }

    if info.has_state_dir {
        steps.push(InstallStep::with_executor("Removing state directory", || {
            step_remove_state_dir().map(|r| match r {
                StepOutcome::Success => Some("Removed".to_string()),
                StepOutcome::Skipped => Some("Skipped".to_string()),
                StepOutcome::Failed(_) => None,
            })
        }));
    }

    if with_credentials && info.has_creds_file {
        steps.push(InstallStep::with_executor("Removing Tailscale credentials", || {
            step_remove_tailscale_creds().map(|r| match r {
                StepOutcome::Success => Some("Removed".to_string()),
                StepOutcome::Skipped => Some("Skipped".to_string()),
                StepOutcome::Failed(_) => None,
            })
        }));
    }

    let view = DevUninstallView::new(steps, info, with_credentials);

    let result = Program::new(view)
        .with_options(ProgramOptions::fullscreen())
        .run()
        .map_err(|e| Error::Other(e.to_string()))?;

    if result.was_cancelled() {
        println!("Uninstall cancelled.");
    }

    Ok(())
}
