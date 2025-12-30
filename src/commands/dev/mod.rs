//! Local development cluster commands.
//!
//! Manages a local Talos Kubernetes cluster for InferaDB development,
//! including FoundationDB, the engine, control plane, and dashboard.
//!
//! # Module Structure
//!
//! - `commands` - Shell command wrappers
//! - `constants` - Cluster configuration constants
//! - `docker` - Docker container operations
//! - `doctor` - Environment checking
//! - `output` - Output formatting utilities
//! - `paths` - Path helpers
//! - `tailscale` - Tailscale credential handling

// Submodules
pub mod commands;
pub mod constants;
pub mod docker;
pub mod doctor;
pub mod output;
pub mod paths;
pub mod tailscale;

// Re-export public items from submodules for convenience
pub use constants::*;
pub use doctor::doctor;
pub use output::{
    format_dot_leader, format_reset_dot_leader, print_colored_prefix_dot_leader, print_hint,
    print_phase_header, print_prefixed_dot_leader, print_section_header, print_styled_header,
    run_destroy_step, run_step, run_step_with_result, StartStep, StepOutcome,
};

// Re-export commonly used items
use commands::{
    command_exists, parse_kubectl_apply_line, run_command, run_command_optional,
    run_command_streaming,
};
use docker::{
    are_containers_paused, cluster_exists, docker_container_exists, get_cluster_containers,
    get_container_ip, get_dev_docker_images,
};
use paths::{
    get_config_dir, get_control_dir, get_dashboard_dir, get_data_dir, get_deploy_dir,
    get_engine_dir, get_state_dir, get_tailscale_creds_file,
};
use tailscale::{
    get_tailnet_info, get_tailscale_credentials, load_tailscale_credentials,
    save_tailscale_credentials,
};

// Standard library imports
use std::fs;
use std::io::{self, Write as IoWrite};
use std::process::{Command, Stdio};
use std::time::Duration;

// Crate imports
use crate::client::Context;
use crate::error::{Error, Result};
use crate::tui::{ClusterStatus, RefreshResult, TabData, UninstallInfo};
use ferment::output::{info as print_info, success as print_success};
use ferment::style::Color;

// ============================================================================
// Reset Helpers
// ============================================================================

/// Get FoundationDB clusters for reset dry run.
/// Returns: Vec<(name, process_count, version)>
fn get_fdb_clusters_for_reset() -> Vec<(String, String, String)> {
    let output = run_command_optional(
        "kubectl",
        &["get", "foundationdbcluster", "-n", "inferadb", "-o", "json"],
    );

    let mut clusters = Vec::new();
    if let Some(json_str) = output {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(items) = json.get("items").and_then(|i| i.as_array()) {
                for item in items {
                    let name = item
                        .pointer("/metadata/name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    // Sum up all process counts
                    let process_counts = item.pointer("/spec/processCounts");
                    let total_processes: i64 = if let Some(counts) = process_counts {
                        counts
                            .as_object()
                            .map(|obj| obj.values().filter_map(|v| v.as_i64()).sum())
                            .unwrap_or(0)
                    } else {
                        0
                    };
                    let processes = if total_processes > 0 {
                        format!("{} processes", total_processes)
                    } else {
                        "unknown".to_string()
                    };

                    let version = item
                        .pointer("/status/runningVersion")
                        .and_then(|v| v.as_str())
                        .map(|v| format!("v{}", v))
                        .unwrap_or_else(|| "unknown".to_string());

                    clusters.push((name, processes, version));
                }
            }
        }
    }

    clusters
}

/// Get deployments for reset dry run.
/// Returns: Vec<(name, replicas, image_tag)>
fn get_deployments_for_reset() -> Vec<(String, String, String)> {
    let output = run_command_optional(
        "kubectl",
        &[
            "get",
            "deployments",
            "-n",
            "inferadb",
            "-l",
            "app.kubernetes.io/name in (inferadb-engine,inferadb-control,inferadb-dashboard)",
            "-o",
            "json",
        ],
    );

    let mut deployments = Vec::new();
    if let Some(json_str) = output {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(items) = json.get("items").and_then(|i| i.as_array()) {
                for item in items {
                    let name = item
                        .pointer("/metadata/name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let replicas = item
                        .pointer("/spec/replicas")
                        .and_then(|v| v.as_i64())
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "1".to_string());

                    // Get image and extract just the tag
                    let image = item
                        .pointer("/spec/template/spec/containers/0/image")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    let image_tag = if let Some(tag_pos) = image.rfind(':') {
                        &image[tag_pos + 1..]
                    } else if let Some(slash_pos) = image.rfind('/') {
                        &image[slash_pos + 1..]
                    } else {
                        image
                    };

                    deployments.push((name, replicas, image_tag.to_string()));
                }
            }
        }
    }

    deployments
}

/// Get PVCs for reset dry run.
/// Returns: Vec<(name, size, status)>
fn get_pvcs_for_reset() -> Vec<(String, String, String)> {
    let output = run_command_optional("kubectl", &["get", "pvc", "-n", "inferadb", "-o", "json"]);

    let mut pvcs = Vec::new();
    if let Some(json_str) = output {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(items) = json.get("items").and_then(|i| i.as_array()) {
                for item in items {
                    let name = item
                        .pointer("/metadata/name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let size = item
                        .pointer("/spec/resources/requests/storage")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let status = item
                        .pointer("/status/phase")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    pvcs.push((name, size, status));
                }
            }
        }
    }

    pvcs
}

// ============================================================================
// Cluster Status
// ============================================================================

/// Get current cluster status.
fn get_cluster_status() -> ClusterStatus {
    // Check if containers exist
    if !cluster_exists() {
        return ClusterStatus::Offline;
    }

    // Check if paused
    if are_containers_paused() {
        return ClusterStatus::Paused;
    }

    // Check if kubectl can reach the cluster
    if run_command_optional("kubectl", &["cluster-info"]).is_some() {
        ClusterStatus::Online
    } else {
        ClusterStatus::Unknown
    }
}

// ============================================================================
// Uninstall Helpers
// ============================================================================

/// Gather information about what will be uninstalled.
fn gather_uninstall_info() -> UninstallInfo {
    let deploy_dir = get_deploy_dir();
    let creds_file = get_tailscale_creds_file();
    let config_dir = get_config_dir();
    let data_dir = get_data_dir();
    let state_dir = get_state_dir();

    let has_cluster = cluster_exists();
    let cluster_status = if has_cluster {
        Some(
            if are_containers_paused() {
                "paused"
            } else {
                "running"
            }
            .to_string(),
        )
    } else {
        None
    };

    let has_registry = docker::registry_exists();
    let has_deploy_dir = deploy_dir.exists();
    let has_state_dir = state_dir.exists();
    let has_creds_file = creds_file.exists();

    let dev_images = get_dev_docker_images();
    let dev_image_count = dev_images.len();

    let has_kube_context =
        run_command_optional("kubectl", &["config", "get-contexts", "-o", "name"])
            .map(|o| o.lines().any(|l| l == KUBE_CONTEXT))
            .unwrap_or(false);
    let has_talos_context = run_command_optional("talosctl", &["config", "contexts"])
        .map(|o| o.contains(CLUSTER_NAME))
        .unwrap_or(false);

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

/// Clean up stale kubectl/talosctl contexts.
fn cleanup_stale_contexts() {
    // Clean kubectl context
    let _ = run_command_optional("kubectl", &["config", "delete-context", KUBE_CONTEXT]);
    let _ = run_command_optional("kubectl", &["config", "delete-cluster", CLUSTER_NAME]);
    let _ = run_command_optional(
        "kubectl",
        &["config", "delete-user", &format!("admin@{}", CLUSTER_NAME)],
    );

    // Clean talosctl context
    let _ = run_command_optional(
        "talosctl",
        &["config", "remove", CLUSTER_NAME, "--noconfirm"],
    );
}

// ============================================================================
// Uninstall Step Functions
// ============================================================================

/// Step: Destroy Talos cluster and clean up Tailscale devices.
fn step_destroy_cluster() -> std::result::Result<StepOutcome, String> {
    if !cluster_exists() {
        return Ok(StepOutcome::Skipped(String::new()));
    }

    // Clean up Tailscale devices first
    cleanup_tailscale_devices().map_err(|e| e.to_string())?;

    // Destroy cluster
    run_command("talosctl", &["cluster", "destroy", "--name", CLUSTER_NAME])
        .map_err(|e| e.to_string())?;

    Ok(StepOutcome::Success)
}

/// Step: Remove local Docker registry.
fn step_remove_registry() -> std::result::Result<StepOutcome, String> {
    if !docker::registry_exists() {
        return Ok(StepOutcome::Skipped(String::new()));
    }

    let _ = run_command_optional("docker", &["stop", REGISTRY_NAME]);
    let _ = run_command_optional("docker", &["rm", "-f", REGISTRY_NAME]);

    Ok(StepOutcome::Success)
}

/// Step: Clean up kubectl/talosctl contexts.
fn step_cleanup_contexts() -> std::result::Result<StepOutcome, String> {
    let has_talos = run_command_optional("talosctl", &["config", "contexts"])
        .map(|o| o.contains(CLUSTER_NAME))
        .unwrap_or(false);

    let has_kube = run_command_optional("kubectl", &["config", "get-contexts", "-o", "name"])
        .map(|o| o.lines().any(|l| l == KUBE_CONTEXT))
        .unwrap_or(false);

    if !has_talos && !has_kube {
        return Ok(StepOutcome::Skipped(String::new()));
    }

    cleanup_stale_contexts();
    Ok(StepOutcome::Success)
}

/// Step: Remove Docker images.
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

    Ok(Some(format!(
        "Removed {} of {} images",
        removed,
        dev_images.len()
    )))
}

/// Step: Remove state directory.
fn step_remove_state_dir() -> std::result::Result<StepOutcome, String> {
    let state_dir = get_state_dir();
    if !state_dir.exists() {
        return Ok(StepOutcome::Skipped(String::new()));
    }

    fs::remove_dir_all(&state_dir)
        .map_err(|e| format!("Failed to remove {}: {}", state_dir.display(), e))?;

    Ok(StepOutcome::Success)
}

/// Step: Remove Tailscale credentials.
fn step_remove_tailscale_creds() -> std::result::Result<StepOutcome, String> {
    let creds_file = get_tailscale_creds_file();
    if !creds_file.exists() {
        return Ok(StepOutcome::Skipped(String::new()));
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
    let (client_id, client_secret) = match load_tailscale_credentials() {
        Some(creds) => creds,
        None => return Ok(()),
    };

    // Get OAuth token
    let output = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            "-d",
            &format!(
                "client_id={}&client_secret={}",
                client_id, client_secret
            ),
            "https://api.tailscale.com/api/v2/oauth/token",
        ])
        .output()
        .map_err(|e| Error::Other(e.to_string()))?;

    let response = String::from_utf8_lossy(&output.stdout);
    let token: Option<String> = serde_json::from_str::<serde_json::Value>(&response)
        .ok()
        .and_then(|v| v.get("access_token").and_then(|t| t.as_str()).map(String::from));

    let token = match token {
        Some(t) => t,
        None => return Ok(()),
    };

    // List devices
    let output = Command::new("curl")
        .args([
            "-s",
            "-H",
            &format!("Authorization: Bearer {}", token),
            "https://api.tailscale.com/api/v2/tailnet/-/devices",
        ])
        .output()
        .map_err(|e| Error::Other(e.to_string()))?;

    let response = String::from_utf8_lossy(&output.stdout);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
        if let Some(devices) = json.get("devices").and_then(|d| d.as_array()) {
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
                            &format!("Authorization: Bearer {}", token),
                            &format!("https://api.tailscale.com/api/v2/device/{}", id),
                        ])
                        .output();
                }
            }
        }
    }

    Ok(())
}

// ============================================================================
// Install Step Functions
// ============================================================================

/// Clone a git repository to a target directory.
fn clone_repo(
    repo_url: &str,
    target_dir: &std::path::Path,
    force: bool,
    commit: Option<&str>,
) -> std::result::Result<Option<String>, String> {
    if target_dir.exists() {
        if force {
            fs::remove_dir_all(target_dir)
                .map_err(|e| format!("Failed to remove {}: {}", target_dir.display(), e))?;
        } else {
            return Ok(Some("already cloned".to_string()));
        }
    }

    if let Some(parent) = target_dir.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    let clone_ok = if commit.is_some() {
        run_command_optional(
            "git",
            &[
                "clone",
                "--recurse-submodules",
                "--quiet",
                repo_url,
                target_dir.to_str().unwrap(),
            ],
        )
        .is_some()
    } else {
        run_command_optional(
            "git",
            &[
                "clone",
                "--depth",
                "1",
                "--recurse-submodules",
                "--shallow-submodules",
                "--quiet",
                repo_url,
                target_dir.to_str().unwrap(),
            ],
        )
        .is_some()
    };

    if !clone_ok {
        return Err(format!("Failed to clone {}", repo_url));
    }

    if let Some(ref_spec) = commit {
        if run_command_optional(
            "git",
            &["-C", target_dir.to_str().unwrap(), "checkout", ref_spec],
        )
        .is_none()
        {
            return Err(format!("Failed to checkout '{}'", ref_spec));
        }
        let _ = run_command_optional(
            "git",
            &[
                "-C",
                target_dir.to_str().unwrap(),
                "submodule",
                "update",
                "--init",
                "--recursive",
                "--quiet",
            ],
        );
    }

    Ok(None)
}

/// Step: Clone the deployment repository.
fn step_clone_repo(
    deploy_dir: &std::path::Path,
    force: bool,
    commit: Option<&str>,
) -> std::result::Result<Option<String>, String> {
    clone_repo(DEPLOY_REPO_URL, deploy_dir, force, commit)
}

/// Step: Clone a component repository.
fn step_clone_component(
    name: &str,
    repo_url: &str,
    target_dir: &std::path::Path,
    force: bool,
) -> std::result::Result<StepOutcome, String> {
    match clone_repo(repo_url, target_dir, force, None) {
        Ok(Some(msg)) => Ok(StepOutcome::Skipped(msg)),
        Ok(None) => Ok(StepOutcome::Success),
        Err(e) => Err(format!("Failed to clone {}: {}", name, e)),
    }
}

/// Step: Create the configuration directory.
fn step_create_config_dir() -> std::result::Result<Option<String>, String> {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".config"))
        .join("inferadb");

    if config_dir.exists() {
        return Ok(Some("already exists".to_string()));
    }

    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    Ok(None)
}

/// Step: Set up Helm repositories.
fn step_setup_helm() -> std::result::Result<Option<String>, String> {
    if !command_exists("helm") {
        return Ok(Some("Helm not installed, skipping".to_string()));
    }

    // Check if tailscale repo already exists
    let repo_exists = run_command_optional("helm", &["repo", "list", "-o", "json"])
        .map(|output| output.contains("tailscale"))
        .unwrap_or(false);

    if !repo_exists {
        if run_command_optional(
            "helm",
            &[
                "repo",
                "add",
                "tailscale",
                "https://pkgs.tailscale.com/helmcharts",
            ],
        )
        .is_none()
        {
            let _ = run_command_optional("helm", &["repo", "update", "tailscale"]);
        }
    }

    let _ = run_command_optional("helm", &["repo", "update"]);

    Ok(Some("Helm repositories configured".to_string()))
}

// ============================================================================
// Public Command Functions
// ============================================================================

/// Run dev start - create or resume local development cluster.
pub async fn start(
    _ctx: &Context,
    skip_build: bool,
    interactive: bool,
    tailscale_client: Option<String>,
    tailscale_secret: Option<String>,
    force: bool,
    commit: Option<&str>,
) -> Result<()> {
    // Save CLI-provided credentials if both are present
    if let (Some(client_id), Some(client_secret)) = (&tailscale_client, &tailscale_secret) {
        if !client_id.is_empty() && !client_secret.is_empty() {
            save_tailscale_credentials(client_id, client_secret)?;
        }
    }

    // For new cluster creation, use interactive mode if requested
    if interactive {
        return start_interactive(skip_build, force, commit);
    }

    // Non-interactive mode
    start_with_streaming(skip_build, force, commit)
}

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
        return uninstall_with_spinners(yes, with_credentials);
    }

    // Pause containers
    stop_with_spinners()
}

/// Run dev status - show cluster status.
pub async fn dev_status(ctx: &Context, interactive: bool) -> Result<()> {
    if interactive && crate::tui::is_interactive(ctx) {
        return status_interactive();
    }
    status_with_spinners()
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
        Some("fdb") | Some("foundationdb") => ("app.kubernetes.io/name=fdb", "fdb"),
        Some(s) => (s, s),
        None => ("app.kubernetes.io/part-of=inferadb", "all"),
    };

    args.extend(["-l", selector]);

    if follow {
        args.push("-f");
    }

    let tail_str = tail.to_string();
    args.extend(["--tail", &tail_str]);

    println!("Streaming logs for {} pods...\n", service_name);
    run_command_streaming("kubectl", &args, &[])?;
    Ok(())
}

/// Run dev dashboard - open dashboard in browser.
pub async fn dashboard(_ctx: &Context) -> Result<()> {
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
            println!("Opening dashboard: {}", url);
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
        }
        _ => Err(Error::Other(
            "Dashboard ingress not found. Is the cluster running?".to_string(),
        )),
    }
}

/// Run dev reset - reset cluster data.
pub async fn reset(_ctx: &Context, yes: bool) -> Result<()> {
    reset_with_spinners(yes)
}

// ============================================================================
// Implementation Details
// ============================================================================

// NOTE: The full implementations are being incrementally migrated.
// For now, these functions delegate to placeholder implementations.
// The original dev.rs file remains as a reference during migration.

/// Check if a specific container is paused.
fn is_container_paused(container: &str) -> bool {
    run_command_optional(
        "docker",
        &[
            "inspect",
            "-f",
            "{{.State.Paused}}",
            container,
        ],
    )
    .map(|output| output.trim() == "true")
    .unwrap_or(false)
}

/// Show final success output with URLs and hints.
fn show_final_success(tailnet_suffix: Option<&str>) {
    let green = Color::Green.to_ansi_fg();
    let reset = "\x1b[0m";

    println!();
    println!("{}✓{} Development cluster ready", green, reset);
    println!();

    if let Some(suffix) = tailnet_suffix {
        println!("  API: https://inferadb-api.{}", suffix);
        println!("  Dashboard: https://inferadb-dashboard.{}", suffix);
    } else {
        println!("  API: https://inferadb-api.<your-tailnet>.ts.net");
        println!("  Dashboard: https://inferadb-dashboard.<your-tailnet>.ts.net");
    }

    println!();
    print_hint("Run 'inferadb dev status' for cluster details");
    print_hint("Run 'inferadb dev stop' to pause or destroy the cluster");
}

/// Pause a single container, showing spinner and returning whether work was done.
fn pause_container(container: &str) -> bool {
    use crate::tui::start_spinner;
    use output::print_destroy_skipped;

    let display_name = container
        .strip_prefix(&format!("{}-", CLUSTER_NAME))
        .unwrap_or(container);
    let in_progress = format!("Pausing {}", display_name);
    let completed = format!("Paused {}", display_name);
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
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("already paused")
                || err_str.contains("No such container")
                || err_str.contains("not found")
            {
                spin.stop();
                print_destroy_skipped(&completed);
                false
            } else {
                spin.failure(&err_str);
                false
            }
        }
    }
}

/// Stop with inline spinners.
fn stop_with_spinners() -> Result<()> {
    print_styled_header("Pausing InferaDB Development Cluster");
    println!();

    let mut any_paused = false;

    let expected_containers = vec![
        format!("{}-controlplane-1", CLUSTER_NAME),
        format!("{}-worker-1", CLUSTER_NAME),
    ];

    for container in &expected_containers {
        any_paused |= pause_container(container);
    }

    let actual_containers = get_cluster_containers();
    for container in &actual_containers {
        if expected_containers.contains(container) {
            continue;
        }
        any_paused |= pause_container(container);
    }

    any_paused |= pause_container(REGISTRY_NAME);

    println!();
    if any_paused {
        print_success("Cluster paused successfully!");
    } else {
        println!("Nothing to pause.");
    }
    println!();
    print_hint("Run 'inferadb dev start' to resume the cluster");
    print_hint("Run 'inferadb dev stop --destroy' to tear down the cluster");

    Ok(())
}

/// Status with inline spinners.
fn status_with_spinners() -> Result<()> {
    let green = Color::Green.to_ansi_fg();
    let yellow = Color::Yellow.to_ansi_fg();
    let red = Color::Red.to_ansi_fg();
    let reset = "\x1b[0m";

    print_styled_header("InferaDB Development Cluster Status");
    println!();

    let cluster_status = get_cluster_status();
    match cluster_status {
        ClusterStatus::Offline => {
            let prefix = format!("{}✗{}", red, reset);
            let status = format!("{}NOT RUNNING{}", red, reset);
            print_colored_prefix_dot_leader(&prefix, 1, "Cluster", &status);
            println!();
            print_hint(TIP_START_CLUSTER);
            return Ok(());
        }
        ClusterStatus::Paused => {
            let prefix = format!("{}⚠{}", yellow, reset);
            let status = format!("{}STOPPED{}", yellow, reset);
            print_colored_prefix_dot_leader(&prefix, 1, "Cluster", &status);
            println!();
            print_hint(TIP_RESUME_CLUSTER);
            return Ok(());
        }
        ClusterStatus::Online => {
            let prefix = format!("{}✓{}", green, reset);
            print_colored_prefix_dot_leader(&prefix, 1, "Cluster", "RUNNING");
        }
        ClusterStatus::Unknown => {
            let prefix = format!("{}○{}", yellow, reset);
            print_colored_prefix_dot_leader(&prefix, 1, "Cluster", "UNKNOWN");
        }
    }

    if let Some(current) = run_command_optional("kubectl", &["config", "current-context"]) {
        let context = current.trim();
        print_prefixed_dot_leader("○", "kubectl context", context);
    } else {
        print_prefixed_dot_leader("○", "kubectl context", "NOT CONFIGURED");
    }

    print_section_header("Nodes");
    print_nodes_status();

    print_section_header("Pods");
    print_pods_status();

    print_section_header("URLs");
    print_urls_status();

    Ok(())
}

/// Print formatted node status.
fn print_nodes_status() {
    let output = run_command_optional("kubectl", &["get", "nodes", "-o", "json"]);

    let green = Color::Green.to_ansi_fg();
    let red = Color::Red.to_ansi_fg();
    let reset = "\x1b[0m";

    if let Some(output) = output {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output) {
            if let Some(items) = json["items"].as_array() {
                for node in items {
                    let name = node["metadata"]["name"].as_str().unwrap_or("");
                    let labels = &node["metadata"]["labels"];
                    let is_control_plane = labels
                        .get("node-role.kubernetes.io/control-plane")
                        .is_some();

                    let ready = node["status"]["conditions"]
                        .as_array()
                        .and_then(|conditions| conditions.iter().find(|c| c["type"] == "Ready"))
                        .map(|c| c["status"] == "True")
                        .unwrap_or(false);

                    let role = if is_control_plane {
                        "control-plane"
                    } else {
                        "worker"
                    };
                    let status = if ready {
                        format!("{}Ready{} ({})", green, reset, role)
                    } else {
                        format!("{}NotReady{} ({})", red, reset, role)
                    };

                    let display_name = name.strip_prefix("inferadb-dev-").unwrap_or(name);
                    print_prefixed_dot_leader(" ", display_name, &status);
                }
            }
        }
    }
}

/// Print formatted pod status.
fn print_pods_status() {
    let green = Color::Green.to_ansi_fg();
    let yellow = Color::Yellow.to_ansi_fg();
    let red = Color::Red.to_ansi_fg();
    let reset = "\x1b[0m";

    let inferadb_pods = run_command_optional(
        "kubectl",
        &["get", "pods", "-n", "inferadb", "-o", "jsonpath={range .items[*]}{.metadata.name}|{.status.phase}|{.status.containerStatuses[*].ready}{\"\\n\"}{end}"],
    );

    let fdb_pods = run_command_optional(
        "kubectl",
        &["get", "pods", "-n", "fdb-system", "-o", "jsonpath={range .items[*]}{.metadata.name}|{.status.phase}|{.status.containerStatuses[*].ready}{\"\\n\"}{end}"],
    );

    let format_pod = |line: &str| -> Option<(String, String)> {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 2 {
            let name = parts[0];
            let phase = parts[1];
            let ready_statuses = parts.get(2).unwrap_or(&"");

            if phase == "Succeeded" || phase == "Completed" {
                return None;
            }

            let ready_count = ready_statuses
                .split_whitespace()
                .filter(|s| *s == "true")
                .count();
            let total_count = ready_statuses.split_whitespace().count().max(1);

            let status = match phase {
                "Running" => format!("{}{}/{} Running{}", green, ready_count, total_count, reset),
                "Pending" => format!("{}{}/{} Pending{}", yellow, ready_count, total_count, reset),
                _ => format!("{}{}/{} {}{}", red, ready_count, total_count, phase, reset),
            };

            let display_name = if name.starts_with("controller-manager-") {
                "fdb-operator".to_string()
            } else {
                let base = name
                    .strip_prefix("dev-inferadb-")
                    .or_else(|| name.strip_prefix("inferadb-"))
                    .unwrap_or(name);
                base.split('-').take_while(|s| !s.chars().all(|c| c.is_ascii_digit()) && s.len() < 9).collect::<Vec<_>>().join("-")
            };

            if display_name.is_empty() {
                return None;
            }

            Some((display_name, status))
        } else {
            None
        }
    };

    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Some(output) = inferadb_pods {
        for line in output.lines() {
            if let Some((name, status)) = format_pod(line) {
                if seen_names.insert(name.clone()) {
                    print_prefixed_dot_leader(" ", &name, &status);
                }
            }
        }
    }

    if let Some(output) = fdb_pods {
        for line in output.lines() {
            if let Some((name, status)) = format_pod(line) {
                if seen_names.insert(name.clone()) {
                    print_prefixed_dot_leader(" ", &name, &status);
                }
            }
        }
    }
}

/// Print formatted URLs.
fn print_urls_status() {
    let output = run_command_optional(
        "kubectl",
        &["get", "ingress", "-n", "inferadb", "-o", "jsonpath={range .items[*]}{.metadata.name}|{.status.loadBalancer.ingress[0].hostname}{\"\\n\"}{end}"],
    );

    if let Some(output) = output {
        for line in output.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 2 {
                let name = parts[0];
                let hostname = parts[1];

                if hostname.is_empty() {
                    continue;
                }

                let label = match name {
                    "dev-inferadb-dashboard-tailscale" => "Dashboard",
                    "dev-inferadb-api-tailscale" => "API",
                    "dev-inferadb-mailpit-tailscale" => "Mailpit",
                    _ => name,
                };

                let url = format!("https://{}", hostname);
                print_prefixed_dot_leader(" ", label, &url);
            }
        }
    }
}

/// Status interactive TUI mode.
fn status_interactive() -> Result<()> {
    use crate::tui::DevStatusView;
    use ferment::output::{terminal_height, terminal_width};
    use ferment::runtime::{Program, ProgramOptions};

    let width = terminal_width();
    let height = terminal_height();

    let initial_data = fetch_status_data();

    let view = DevStatusView::new(width, height)
        .with_refresh(fetch_status_data)
        .with_status(initial_data.cluster_status)
        .with_urls(initial_data.urls)
        .with_services(initial_data.services)
        .with_nodes(initial_data.nodes)
        .with_pods(initial_data.pods);

    Program::new(view)
        .with_options(ProgramOptions::fullscreen())
        .run()
        .map_err(|e| Error::Other(e.to_string()))?;

    Ok(())
}

/// Fetch all status data for the status view.
fn fetch_status_data() -> RefreshResult {
    let cluster_status = get_cluster_status();
    let urls = TabData::default();
    let services = TabData::default();
    let nodes = TabData::default();
    let pods = TabData::default();

    RefreshResult {
        cluster_status,
        urls,
        services,
        nodes,
        pods,
    }
}

/// Uninstall with spinners.
fn uninstall_with_spinners(yes: bool, with_credentials: bool) -> Result<()> {
    use output::print_destroy_skipped;

    print_styled_header("Destroying InferaDB Development Cluster");

    let info = gather_uninstall_info();
    let initially_had_something = info.has_anything();

    if initially_had_something {
        println!();
        println!("This will destroy:");
        println!();
        for line in info.removal_lines() {
            println!("  • {}", line);
        }
        if with_credentials && info.has_creds_file {
            println!("  • Tailscale credentials");
        }
        println!();

        if !yes {
            print!("Are you sure you want to continue? [y/N] ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim().to_lowercase();

            if input != "y" && input != "yes" {
                println!("Aborted.");
                return Ok(());
            }
            println!();
        }
    } else {
        println!();
    }

    let mut did_work = false;

    did_work |= run_destroy_step("Removing registry", "Removed registry", step_remove_registry);
    did_work |= run_destroy_step(
        "Destroying cluster",
        "Destroyed cluster",
        step_destroy_cluster,
    );
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
                &format!("Removing image {}", display_name),
                &format!("Removed image {}", display_name),
                move || {
                    if run_command_optional("docker", &["rmi", "-f", &image_clone]).is_some() {
                        Ok(StepOutcome::Success)
                    } else {
                        Ok(StepOutcome::Skipped(String::new()))
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
        let reset = "\x1b[0m";
        println!("{}Cluster destroyed successfully.{}", green, reset);

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

    Ok(())
}

/// Uninstall interactive TUI mode.
fn uninstall_interactive(with_credentials: bool) -> Result<()> {
    use crate::tui::{DevUninstallView, InstallStep};
    use ferment::runtime::{Program, ProgramOptions};

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
                StepOutcome::Skipped(_) => Some("Skipped".to_string()),
                _ => None,
            })
        }));
    }

    if info.has_cluster {
        steps.push(InstallStep::with_executor("Destroying cluster", || {
            step_destroy_cluster().map(|r| match r {
                StepOutcome::Success => Some("Destroyed".to_string()),
                StepOutcome::Skipped(_) => Some("Skipped".to_string()),
                _ => None,
            })
        }));
    }

    if info.has_kube_context || info.has_talos_context {
        steps.push(InstallStep::with_executor("Cleaning contexts", || {
            step_cleanup_contexts().map(|r| match r {
                StepOutcome::Success => Some("Cleaned".to_string()),
                StepOutcome::Skipped(_) => Some("Skipped".to_string()),
                _ => None,
            })
        }));
    }

    if info.dev_image_count > 0 {
        steps.push(InstallStep::with_executor(
            "Removing Docker images",
            step_remove_docker_images,
        ));
    }

    if info.has_state_dir {
        steps.push(InstallStep::with_executor("Removing state directory", || {
            step_remove_state_dir().map(|r| match r {
                StepOutcome::Success => Some("Removed".to_string()),
                StepOutcome::Skipped(_) => Some("Skipped".to_string()),
                _ => None,
            })
        }));
    }

    if with_credentials && info.has_creds_file {
        steps.push(InstallStep::with_executor(
            "Removing Tailscale credentials",
            || {
                step_remove_tailscale_creds().map(|r| match r {
                    StepOutcome::Success => Some("Removed".to_string()),
                    StepOutcome::Skipped(_) => Some("Skipped".to_string()),
                    _ => None,
                })
            },
        ));
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

/// Start interactive TUI mode.
fn start_interactive(skip_build: bool, force: bool, commit: Option<&str>) -> Result<()> {
    use crate::tui::DevStartView;
    use ferment::runtime::{Program, ProgramOptions};

    let deploy_dir = get_deploy_dir();
    let commit_owned = commit.map(|s| s.to_string());

    let view = DevStartView::new(skip_build)
        .with_prereq_checker({
            let deploy_dir = deploy_dir.clone();
            move || {
                // Check prerequisites
                for cmd in &["docker", "talosctl", "kubectl", "helm"] {
                    if !command_exists(cmd) {
                        return Err(format!(
                            "{} is not installed. Run 'inferadb dev doctor' for setup instructions.",
                            cmd
                        ));
                    }
                }

                // Check Docker is running
                if run_command_optional("docker", &["info"]).is_none() {
                    return Err(
                        "Docker daemon is not running. Please start Docker first.".to_string(),
                    );
                }

                // Ensure deploy repo exists
                if !deploy_dir.exists() && !force {
                    return Err(
                        "Deploy repository not found. It will be cloned during setup.".to_string(),
                    );
                }

                Ok(())
            }
        })
        .with_credentials_loader(load_tailscale_credentials)
        .with_credentials_saver(|id, secret| {
            save_tailscale_credentials(id, secret).map_err(|e| e.to_string())
        })
        .with_step_builder({
            let deploy_dir = deploy_dir.clone();
            let commit = commit_owned.clone();
            move |client_id, client_secret, skip_build| {
                build_start_steps(
                    client_id,
                    client_secret,
                    skip_build,
                    force,
                    commit.as_deref(),
                    &deploy_dir,
                )
            }
        });

    let result = Program::new(view)
        .with_options(ProgramOptions::fullscreen())
        .run();

    match result {
        Ok(view) if view.is_success() => {
            // Show success message - try to get tailnet info
            let tailnet_suffix = get_tailnet_info();
            show_final_success(tailnet_suffix.as_deref());
            Ok(())
        }
        Ok(view) if view.was_cancelled() => Err(Error::Other("Cancelled".to_string())),
        Ok(_) => Err(Error::Other("Start failed".to_string())),
        Err(e) => Err(Error::Other(e.to_string())),
    }
}

/// Build the steps for starting a new cluster (includes install steps).
fn build_start_steps(
    _client_id: String,
    _client_secret: String,
    _skip_build: bool,
    force: bool,
    commit: Option<&str>,
    deploy_dir: &std::path::Path,
) -> Vec<crate::tui::InstallStep> {
    use crate::tui::InstallStep;

    let deploy_dir_owned = deploy_dir.to_path_buf();
    let commit_owned = commit.map(|s| s.to_string());
    let is_paused = docker_container_exists(CLUSTER_NAME) && are_containers_paused();

    let mut steps = Vec::new();

    // Phase 0: Resume paused cluster if needed
    if is_paused {
        // Add a step for each cluster container
        let containers = get_cluster_containers();
        for container in containers {
            let container_name = container.clone();
            steps.push(InstallStep::with_executor(
                format!("Resuming {}", container),
                move || {
                    run_command("docker", &["unpause", &container_name])
                        .map(|_| None)
                        .or_else(|e| {
                            if e.to_string().contains("not paused") {
                                Ok(None)
                            } else {
                                Err(e.to_string())
                            }
                        })
                },
            ));
        }

        // Resume registry
        if docker_container_exists(REGISTRY_NAME) {
            steps.push(InstallStep::with_executor(
                format!("Resuming {}", REGISTRY_NAME),
                || {
                    let _ = run_command_optional("docker", &["unpause", REGISTRY_NAME]);
                    Ok(None)
                },
            ));
        }

        steps.push(InstallStep::with_executor(
            "Waiting for containers to stabilize",
            || {
                std::thread::sleep(Duration::from_secs(3));
                Ok(Some("ready".to_string()))
            },
        ));
    }

    // Phase 1: Conditioning environment
    steps.push(InstallStep::with_executor("Cloning deployment repository", {
        let deploy_dir = deploy_dir_owned.clone();
        let commit = commit_owned.clone();
        move || step_clone_repo(&deploy_dir, force, commit.as_deref())
    }));
    steps.push(InstallStep::with_executor(
        "Creating configuration directory",
        step_create_config_dir,
    ));
    steps.push(InstallStep::with_executor(
        "Setting up Helm repositories",
        step_setup_helm,
    ));

    // Phase 2: Setting up cluster
    steps.push(InstallStep::with_executor("Cleaning stale contexts", || {
        let _ = run_command_optional("talosctl", &["config", "context", ""]);
        if let Some(contexts) = run_command_optional("talosctl", &["config", "contexts"]) {
            for line in contexts.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[1].starts_with(CLUSTER_NAME) {
                    let _ = run_command_optional(
                        "talosctl",
                        &["config", "remove", parts[1], "--noconfirm"],
                    );
                }
            }
        }
        Ok(Some("Cleaned".to_string()))
    }));
    steps.push(InstallStep::with_executor("Creating Talos cluster", || {
        match run_command(
            "talosctl",
            &[
                "cluster",
                "create",
                "--name",
                CLUSTER_NAME,
                "--workers",
                "1",
                "--controlplanes",
                "1",
                "--provisioner",
                "docker",
                "--kubernetes-version",
                "1.32.0",
                "--wait-timeout",
                "10m",
            ],
        ) {
            Ok(_) => Ok(Some("Created".to_string())),
            Err(e) => Err(e.to_string()),
        }
    }));
    steps.push(InstallStep::with_executor("Setting kubectl context", || {
        match run_command("kubectl", &["config", "use-context", KUBE_CONTEXT]) {
            Ok(_) => Ok(Some("Set".to_string())),
            Err(e) => Err(e.to_string()),
        }
    }));
    steps.push(InstallStep::with_executor(
        "Verifying cluster is ready",
        || match run_command("kubectl", &["get", "nodes"]) {
            Ok(_) => Ok(Some("Verified".to_string())),
            Err(e) => Err(e.to_string()),
        },
    ));

    steps
}

/// Start with streaming output.
#[allow(clippy::too_many_lines)]
fn start_with_streaming(skip_build: bool, force: bool, commit: Option<&str>) -> Result<()> {
    let deploy_dir = get_deploy_dir();

    print_styled_header("Starting InferaDB Development Cluster");

    // Phase 0: Resume paused cluster if needed
    if docker_container_exists(CLUSTER_NAME) && are_containers_paused() {
        print_phase_header("Resuming paused cluster");

        let containers = get_cluster_containers();
        for container in &containers {
            let container_name = container.clone();
            let in_progress = format!("Resuming {}", container);
            let completed = format!("Resumed {}", container);
            run_step(&StartStep::with_ok(&in_progress, &completed), || {
                if !is_container_paused(&container_name) {
                    return Ok(StepOutcome::Skipped(String::new()));
                }
                run_command("docker", &["unpause", &container_name])
                    .map(|_| StepOutcome::Success)
                    .or_else(|e| {
                        if e.to_string().contains("not paused") {
                            Ok(StepOutcome::Skipped(String::new()))
                        } else {
                            Err(e.to_string())
                        }
                    })
            })?;
        }

        if docker_container_exists(REGISTRY_NAME) {
            run_step(
                &StartStep::with_ok(
                    &format!("Resuming {}", REGISTRY_NAME),
                    &format!("Resumed {}", REGISTRY_NAME),
                ),
                || {
                    if !is_container_paused(REGISTRY_NAME) {
                        return Ok(StepOutcome::Skipped(String::new()));
                    }
                    run_command("docker", &["unpause", REGISTRY_NAME])
                        .map(|_| StepOutcome::Success)
                        .or_else(|e| {
                            if e.to_string().contains("not paused") {
                                Ok(StepOutcome::Skipped(String::new()))
                            } else {
                                Err(e.to_string())
                            }
                        })
                },
            )?;
        }

        run_step(
            &StartStep::with_ok(
                "Waiting for containers to stabilize",
                "Containers stabilized",
            ),
            || {
                std::thread::sleep(Duration::from_secs(3));
                Ok(StepOutcome::Success)
            },
        )?;

        // Cluster resumed, show success
        show_final_success(get_tailnet_info().as_deref());
        return Ok(());
    }

    // Phase 1: Conditioning environment
    print_phase_header("Conditioning environment");

    run_step(
        &StartStep::with_ok(
            "Cloning deployment repository",
            "Cloned deployment repository",
        ),
        || match step_clone_repo(&deploy_dir, force, commit) {
            Ok(Some(_)) => Ok(StepOutcome::Skipped(String::new())),
            Ok(None) => Ok(StepOutcome::Success),
            Err(e) => Err(e),
        },
    )?;

    let engine_dir = get_engine_dir();
    run_step(
        &StartStep::with_ok("Cloning engine repository", "Cloned engine repository"),
        || step_clone_component("engine", ENGINE_REPO_URL, &engine_dir, force),
    )?;

    let control_dir = get_control_dir();
    run_step(
        &StartStep::with_ok("Cloning control repository", "Cloned control repository"),
        || step_clone_component("control", CONTROL_REPO_URL, &control_dir, force),
    )?;

    let dashboard_dir = get_dashboard_dir();
    run_step(
        &StartStep::with_ok(
            "Cloning dashboard repository",
            "Cloned dashboard repository",
        ),
        || step_clone_component("dashboard", DASHBOARD_REPO_URL, &dashboard_dir, force),
    )?;

    run_step(
        &StartStep::with_ok(
            "Creating configuration directory",
            "Created configuration directory",
        ),
        || match step_create_config_dir() {
            Ok(Some(_)) => Ok(StepOutcome::Skipped(String::new())),
            Ok(None) => Ok(StepOutcome::Success),
            Err(e) => Err(e),
        },
    )?;

    run_step(
        &StartStep::with_ok(
            "Setting up Tailscale Helm repository",
            "Set up Tailscale Helm repository",
        ),
        || {
            if let Some(repos) = run_command_optional("helm", &["repo", "list", "-o", "json"]) {
                if repos.contains("\"tailscale\"") || repos.contains("\"name\":\"tailscale\"") {
                    return Ok(StepOutcome::Skipped(String::new()));
                }
            }
            run_command(
                "helm",
                &[
                    "repo",
                    "add",
                    "tailscale",
                    "https://pkgs.tailscale.com/helmcharts",
                ],
            )
            .map(|_| StepOutcome::Success)
            .map_err(|e| e.to_string())
        },
    )?;

    run_step(
        &StartStep::with_ok("Updating Helm repositories", "Updated Helm repositories"),
        || {
            run_command("helm", &["repo", "update"])
                .map(|_| StepOutcome::Success)
                .map_err(|e| e.to_string())
        },
    )?;

    run_step(
        &StartStep::with_ok(
            "Pulling Docker registry image",
            "Pulled Docker registry image",
        ),
        || {
            run_command("docker", &["pull", "registry:2"])
                .map(|_| StepOutcome::Success)
                .map_err(|e| e.to_string())
        },
    )?;

    // Phase 2: Setting up cluster
    print_phase_header("Setting up cluster");

    run_step(
        &StartStep::with_ok("Checking prerequisites", "Checked prerequisites"),
        || {
            for cmd in &["docker", "talosctl", "kubectl", "helm"] {
                if !command_exists(cmd) {
                    return Err(format!(
                        "{} is not installed. Run 'inferadb dev doctor' for setup instructions.",
                        cmd
                    ));
                }
            }
            if run_command_optional("docker", &["info"]).is_none() {
                return Err("Docker daemon is not running. Please start Docker first.".to_string());
            }
            Ok(StepOutcome::Success)
        },
    )?;

    let (ts_client_id, ts_client_secret) = get_tailscale_credentials()?;
    let cluster_already_exists = docker_container_exists(CLUSTER_NAME);

    run_step(
        &StartStep::with_ok("Cleaning stale contexts", "Cleaned stale contexts"),
        || {
            if cluster_already_exists {
                return Ok(StepOutcome::Skipped(String::new()));
            }
            cleanup_stale_contexts();
            Ok(StepOutcome::Success)
        },
    )?;

    run_step(
        &StartStep::with_ok("Provisioning Talos cluster", "Provisioned Talos cluster"),
        || {
            if cluster_already_exists {
                if run_command_optional("kubectl", &["--context", KUBE_CONTEXT, "get", "nodes"])
                    .is_some()
                {
                    return Ok(StepOutcome::Skipped(String::new()));
                }
                return Err("Cluster containers exist but kubectl context is broken. Run 'inferadb dev stop --destroy' and try again.".to_string());
            }
            run_command(
                "talosctl",
                &[
                    "cluster",
                    "create",
                    "--name",
                    CLUSTER_NAME,
                    "--workers",
                    "1",
                    "--controlplanes",
                    "1",
                    "--provisioner",
                    "docker",
                    "--kubernetes-version",
                    "1.32.0",
                    "--wait-timeout",
                    "10m",
                ],
            )
            .map(|_| StepOutcome::Success)
            .map_err(|e| e.to_string())
        },
    )?;

    run_step(
        &StartStep::with_ok("Setting kubectl context", "Set kubectl context"),
        || {
            if let Some(current) = run_command_optional("kubectl", &["config", "current-context"]) {
                if current.trim() == KUBE_CONTEXT {
                    return Ok(StepOutcome::Skipped(String::new()));
                }
            }
            run_command("kubectl", &["config", "use-context", KUBE_CONTEXT])
                .map(|_| StepOutcome::Success)
                .map_err(|e| e.to_string())
        },
    )?;

    run_step(
        &StartStep::with_ok("Verifying cluster is ready", "Verified cluster is ready"),
        || {
            run_command("kubectl", &["get", "nodes"])
                .map(|_| StepOutcome::Success)
                .map_err(|e| e.to_string())
        },
    )?;

    let registry_ip = setup_container_registry()?;

    if !skip_build {
        run_step(
            &StartStep::with_ok(
                "Building and pushing container images",
                "Built and pushed container images",
            ),
            || build_and_push_images(&registry_ip),
        )?;
    }

    run_step(
        &StartStep::with_ok(
            "Setting up Kubernetes resources",
            "Set up Kubernetes resources",
        ),
        || setup_kubernetes_resources(&registry_ip),
    )?;

    run_step(
        &StartStep::with_ok(
            "Installing Tailscale operator",
            "Installed Tailscale operator",
        ),
        || install_tailscale_operator(&ts_client_id, &ts_client_secret),
    )?;

    run_step(
        &StartStep::with_ok(
            "Installing FoundationDB operator",
            "Installed FoundationDB operator",
        ),
        install_fdb_operator,
    )?;

    let tailnet_suffix = run_step_with_result(
        &StartStep::with_ok("Deploying InferaDB", "Deployed InferaDB"),
        || deploy_inferadb(&deploy_dir, &registry_ip),
    )?;

    show_final_success(tailnet_suffix.as_deref());
    Ok(())
}

/// Reset with spinners.
fn reset_with_spinners(yes: bool) -> Result<()> {
    use crate::tui::start_spinner;

    if !docker_container_exists(CLUSTER_NAME) {
        return Err(Error::Other(
            "Cluster is not running. Start with 'inferadb dev start'.".to_string(),
        ));
    }

    let deploy_dir = get_deploy_dir();
    let can_redeploy = deploy_dir.exists();

    if !yes {
        let fdb_clusters = get_fdb_clusters_for_reset();
        let deployments = get_deployments_for_reset();
        let pvcs = get_pvcs_for_reset();

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
        let yellow = Color::Yellow.to_ansi_fg();
        let reset_color = "\x1b[0m";
        print!(
            "{}This action cannot be undone.{} Continue? [y/N] ",
            yellow, reset_color
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
        println!();
    }

    print_styled_header("Resetting InferaDB Development Cluster");
    println!();

    {
        let spin = start_spinner("Deleting FoundationDB Cluster");
        let _ = run_command_optional(
            "kubectl",
            &["delete", "foundationdbcluster", "--all", "-n", "inferadb"],
        );
        spin.success(&format_dot_leader("Deleted FoundationDB Cluster", "OK"));
    }

    {
        let spin = start_spinner("Deleting InferaDB Deployments");
        for deploy in &[
            "dev-inferadb-engine",
            "dev-inferadb-control",
            "dev-inferadb-dashboard",
        ] {
            let _ = run_command_optional(
                "kubectl",
                &["delete", "deployment", deploy, "-n", "inferadb"],
            );
        }
        spin.success(&format_dot_leader("Deleted InferaDB Deployments", "OK"));
    }

    {
        let spin = start_spinner("Deleting Persistent Volumes");
        let _ = run_command_optional("kubectl", &["delete", "pvc", "--all", "-n", "inferadb"]);
        spin.success(&format_dot_leader("Deleted Persistent Volumes", "OK"));
    }

    {
        let spin = start_spinner("Waiting for resources to terminate");
        std::thread::sleep(Duration::from_secs(5));
        spin.success(&format_dot_leader("Resources terminated", "OK"));
    }

    if can_redeploy {
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
                        "inferadb",
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

        {
            let spin = start_spinner("Restarting engine to pick up new cluster file");
            let _ = run_command_optional(
                "kubectl",
                &[
                    "rollout",
                    "restart",
                    "deployment/dev-inferadb-engine",
                    "-n",
                    "inferadb",
                ],
            );
            spin.success(&format_dot_leader("Engine deployment restarted", "OK"));
        }
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

// ============================================================================
// Helper functions for start
// ============================================================================

/// Set up the container registry and return its IP.
fn setup_container_registry() -> Result<String> {
    run_step_with_result(
        &StartStep::with_ok("Setting up container registry", "Set up container registry"),
        || {
            let registry_existed = docker_container_exists(REGISTRY_NAME);

            if !registry_existed {
                let talos_network = run_command_optional(
                    "docker",
                    &[
                        "network",
                        "ls",
                        "--filter",
                        &format!("name={}", CLUSTER_NAME),
                        "--format",
                        "{{.Name}}",
                    ],
                )
                .and_then(|s| s.lines().next().map(|l| l.to_string()))
                .unwrap_or_else(|| CLUSTER_NAME.to_string());

                run_command(
                    "docker",
                    &[
                        "run",
                        "-d",
                        "--name",
                        REGISTRY_NAME,
                        "--network",
                        &talos_network,
                        "-p",
                        &format!("{}:5000", REGISTRY_PORT),
                        "--restart",
                        "always",
                        "registry:2",
                    ],
                )
                .map_err(|e| e.to_string())?;

                std::thread::sleep(Duration::from_secs(3));
            }

            let registry_ip = get_container_ip(REGISTRY_NAME)
                .ok_or_else(|| "Failed to get registry IP".to_string())?;

            let outcome = if registry_existed {
                StepOutcome::Skipped(String::new())
            } else {
                StepOutcome::Success
            };

            Ok((outcome, registry_ip))
        },
    )
}

/// Build and push container images.
fn build_and_push_images(_registry_ip: &str) -> std::result::Result<StepOutcome, String> {
    let components = [
        ("inferadb-engine", get_engine_dir()),
        ("inferadb-control", get_control_dir()),
        ("inferadb-dashboard", get_dashboard_dir()),
    ];

    let any_exists = components.iter().any(|(_, dir)| dir.exists());
    if !any_exists {
        return Ok(StepOutcome::Skipped("no component repos cloned".to_string()));
    }

    let mut built_count = 0;
    for (name, dir) in &components {
        let dockerfile = dir.join("Dockerfile");
        if dockerfile.exists() {
            run_command(
                "docker",
                &[
                    "build",
                    "-t",
                    &format!("{}:latest", name),
                    dir.to_str().unwrap(),
                ],
            )
            .map_err(|e| e.to_string())?;
            run_command(
                "docker",
                &[
                    "tag",
                    &format!("{}:latest", name),
                    &format!("localhost:{}/{}:latest", REGISTRY_PORT, name),
                ],
            )
            .map_err(|e| e.to_string())?;
            run_command(
                "docker",
                &[
                    "push",
                    &format!("localhost:{}/{}:latest", REGISTRY_PORT, name),
                ],
            )
            .map_err(|e| e.to_string())?;
            built_count += 1;
        }
    }

    if built_count == 0 {
        return Ok(StepOutcome::Skipped(String::new()));
    }

    Ok(StepOutcome::Success)
}

/// Set up Kubernetes resources.
#[allow(clippy::unnecessary_wraps)]
fn setup_kubernetes_resources(_registry_ip: &str) -> std::result::Result<StepOutcome, String> {
    for ns in &[
        "inferadb",
        "fdb-system",
        "local-path-storage",
        "tailscale-system",
    ] {
        let yaml = format!(
            "apiVersion: v1\nkind: Namespace\nmetadata:\n  name: {}\n",
            ns
        );
        apply_yaml(&yaml)?;
    }

    for ns in &[
        "fdb-system",
        "inferadb",
        "local-path-storage",
        "tailscale-system",
    ] {
        let _ = run_command_optional(
            "kubectl",
            &[
                "label",
                "namespace",
                ns,
                "pod-security.kubernetes.io/enforce=privileged",
                "--overwrite",
            ],
        );
    }

    run_command("kubectl", &["apply", "-f", "https://raw.githubusercontent.com/rancher/local-path-provisioner/v0.0.26/deploy/local-path-storage.yaml"])
        .map_err(|e| e.to_string())?;
    run_command("kubectl", &["patch", "storageclass", "local-path", "-p", r#"{"metadata": {"annotations":{"storageclass.kubernetes.io/is-default-class":"true"}}}"#])
        .map_err(|e| e.to_string())?;

    Ok(StepOutcome::Success)
}

/// Apply YAML to Kubernetes via stdin.
fn apply_yaml(yaml: &str) -> std::result::Result<(), String> {
    let mut child = Command::new("kubectl")
        .args(["apply", "-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(yaml.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    child.wait().map_err(|e| e.to_string())?;
    Ok(())
}

/// Install Tailscale operator.
fn install_tailscale_operator(
    client_id: &str,
    client_secret: &str,
) -> std::result::Result<StepOutcome, String> {
    run_command("helm", &["repo", "update", "tailscale"]).map_err(|e| e.to_string())?;
    run_command(
        "helm",
        &[
            "upgrade",
            "--install",
            "tailscale-operator",
            "tailscale/tailscale-operator",
            "--namespace",
            "tailscale-system",
            "--set",
            &format!("oauth.clientId={}", client_id),
            "--set",
            &format!("oauth.clientSecret={}", client_secret),
            "--set",
            "apiServerProxyConfig.mode=noauth",
            "--wait",
            "--timeout",
            "5m",
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(StepOutcome::Success)
}

/// Install FoundationDB operator.
fn install_fdb_operator() -> std::result::Result<StepOutcome, String> {
    let fdb_version = "v2.19.0";
    let fdb_url = format!(
        "https://raw.githubusercontent.com/FoundationDB/fdb-kubernetes-operator/{}/config",
        fdb_version
    );

    for crd in &[
        "crd/bases/apps.foundationdb.org_foundationdbclusters.yaml",
        "crd/bases/apps.foundationdb.org_foundationdbbackups.yaml",
        "crd/bases/apps.foundationdb.org_foundationdbrestores.yaml",
    ] {
        run_command("kubectl", &["apply", "-f", &format!("{}/{}", fdb_url, crd)])
            .map_err(|e| e.to_string())?;
    }

    run_command(
        "kubectl",
        &[
            "wait",
            "--for=condition=established",
            "--timeout=60s",
            "crd/foundationdbclusters.apps.foundationdb.org",
        ],
    )
    .map_err(|e| e.to_string())?;

    run_command(
        "kubectl",
        &[
            "apply",
            "-f",
            &format!("{}/rbac/cluster_role.yaml", fdb_url),
        ],
    )
    .map_err(|e| e.to_string())?;
    run_command(
        "kubectl",
        &[
            "apply",
            "-f",
            &format!("{}/rbac/role.yaml", fdb_url),
            "-n",
            "fdb-system",
        ],
    )
    .map_err(|e| e.to_string())?;

    let manager_yaml = run_command(
        "curl",
        &["-s", &format!("{}/deployment/manager.yaml", fdb_url)],
    )
    .map_err(|e| e.to_string())?;
    let yaml_with_sa_fix = manager_yaml.replace(
        "serviceAccountName: fdb-kubernetes-operator-controller-manager",
        "serviceAccountName: controller-manager",
    );

    let mut modified_lines = Vec::new();
    let mut in_watch_namespace_block = false;
    for line in yaml_with_sa_fix.lines() {
        if line.contains("WATCH_NAMESPACE") {
            in_watch_namespace_block = true;
            continue;
        }
        if in_watch_namespace_block {
            if line.contains("fieldPath:") {
                in_watch_namespace_block = false;
                continue;
            }
            continue;
        }
        modified_lines.push(line);
    }

    let mut child = Command::new("kubectl")
        .args(["apply", "-n", "fdb-system", "-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(modified_lines.join("\n").as_bytes())
            .map_err(|e| e.to_string())?;
    }
    child.wait().map_err(|e| e.to_string())?;

    for (name, role) in &[
        ("fdb-operator-manager-role-global", "manager-role"),
        (
            "fdb-operator-manager-clusterrolebinding",
            "manager-clusterrole",
        ),
    ] {
        let binding_yaml = format!(
            r#"apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: {}
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: {}
subjects:
- kind: ServiceAccount
  name: controller-manager
  namespace: fdb-system
"#,
            name, role
        );
        apply_yaml(&binding_yaml)?;
    }

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(300);
    loop {
        if start.elapsed() > timeout {
            return Err("FDB operator did not become ready within 5 minutes".to_string());
        }
        if run_command_optional(
            "kubectl",
            &[
                "wait",
                "--for=condition=available",
                "--timeout=1s",
                "deployment/controller-manager",
                "-n",
                "fdb-system",
            ],
        )
        .is_some()
        {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
    }

    Ok(StepOutcome::Success)
}

/// Deploy InferaDB applications and return tailnet suffix.
fn deploy_inferadb(
    deploy_dir: &std::path::Path,
    registry_ip: &str,
) -> std::result::Result<(StepOutcome, Option<String>), String> {
    let registry_patch = format!(
        r#"# Auto-generated by inferadb dev start
apiVersion: apps/v1
kind: Deployment
metadata:
  name: inferadb-engine
  namespace: inferadb
spec:
  template:
    spec:
      containers:
        - name: inferadb-engine
          image: {}:5000/inferadb-engine:latest
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: inferadb-control
  namespace: inferadb
spec:
  template:
    spec:
      containers:
        - name: inferadb-control
          image: {}:5000/inferadb-control:latest
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: inferadb-dashboard
  namespace: inferadb
spec:
  template:
    spec:
      containers:
        - name: inferadb-dashboard
          image: {}:5000/inferadb-dashboard:latest
"#,
        registry_ip, registry_ip, registry_ip
    );

    let patch_file = deploy_dir.join("flux/apps/dev/registry-patch.yaml");
    fs::write(&patch_file, &registry_patch).map_err(|e| e.to_string())?;

    run_command(
        "kubectl",
        &[
            "apply",
            "-k",
            deploy_dir.join("flux/apps/dev").to_str().unwrap(),
        ],
    )
    .map_err(|e| e.to_string())?;

    std::thread::sleep(Duration::from_secs(10));

    let tailnet_suffix = get_tailnet_info();

    Ok((StepOutcome::Success, tailnet_suffix))
}
