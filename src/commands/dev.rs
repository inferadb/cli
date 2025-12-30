//! Local development cluster commands.
//!
//! Manages a local Talos Kubernetes cluster for InferaDB development,
//! including FoundationDB, the engine, control plane, and dashboard.

use std::env;
use std::fs;
use std::io::{self, Write as IoWrite};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::client::Context;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::tui::{ClusterStatus, RefreshResult, TabData, TableRow};
use ferment::output::{error as print_error, info as print_info, success as print_success};

// Constants
const CLUSTER_NAME: &str = "inferadb-dev";
const KUBE_CONTEXT: &str = "admin@inferadb-dev";
const REGISTRY_NAME: &str = "inferadb-registry";
const REGISTRY_PORT: u16 = 5050;
/// Prefix for Tailscale devices created by dev environment ingress resources
const TAILSCALE_DEVICE_PREFIX: &str = "inferadb-dev-";

// Repository URLs
const DEPLOY_REPO_URL: &str = "https://github.com/inferadb/deploy.git";
const ENGINE_REPO_URL: &str = "https://github.com/inferadb/engine.git";
const CONTROL_REPO_URL: &str = "https://github.com/inferadb/control.git";
const DASHBOARD_REPO_URL: &str = "https://github.com/inferadb/dashboard.git";

// Tip messages
const TIP_START_CLUSTER: &str = "Run 'inferadb dev start' to start the cluster";
const TIP_RESUME_CLUSTER: &str = "Run 'inferadb dev start' to resume the cluster";

/// Target line width for step output (before terminal margin).
/// This ensures consistent alignment across all phases.
const STEP_LINE_WIDTH: usize = 120;

// ============================================================================
// Start step types for clean output formatting
// ============================================================================

/// A step in the start process with in-progress and completed text variants.
struct StartStep {
    /// Text shown while the step is running (e.g., "Cloning deployment repository")
    in_progress: String,
    /// Text shown when the step completes (e.g., "Cloned deployment repository")
    completed: String,
    /// Whether to show dot leaders to status on the right
    dot_leader: bool,
}

impl StartStep {
    /// Create a step with dot leaders to status (OK or SKIPPED).
    fn with_ok(in_progress: &str, completed: &str) -> Self {
        Self {
            in_progress: in_progress.to_string(),
            completed: completed.to_string(),
            dot_leader: true,
        }
    }
}

/// Result of running a start step.
#[allow(dead_code)]
enum StepOutcome {
    /// Step completed successfully.
    Success,
    /// Step completed with a custom message.
    SuccessMsg(String),
    /// Step was skipped (with reason).
    Skipped(String),
    /// Step failed with error.
    Failed(String),
}

/// Print a phase header.
fn print_phase_header(title: &str) {
    println!("\n  {} ...\n", title);
}

/// Calculate the visible length of a string, stripping ANSI escape sequences.
fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else if c == '\x1b' {
            in_escape = true;
        } else {
            len += 1;
        }
    }
    len
}

/// Format a line with dot leaders to a status suffix.
///
/// Format: `{text} {dots} {status}` where total width is `STEP_LINE_WIDTH`.
/// The dots are dimmed for visual distinction. Status may contain ANSI codes.
fn format_dot_leader(text: &str, status: &str) -> String {
    use ferment::style::Color;

    let dim = Color::BrightBlack.to_ansi_fg();
    let green = Color::Green.to_ansi_fg();
    let reset = "\x1b[0m";

    // Color the status based on value
    let status_colored = match status.to_uppercase().as_str() {
        "OK" | "CREATED" | "CONFIGURED" => format!("{}{}{}", green, status, reset),
        "SKIPPED" | "UNCHANGED" => format!("{}{}{}", dim, status, reset),
        _ => status.to_string(),
    };

    // Calculate dots needed: total width - text length - visible status length - 2 spaces
    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(text.len())
        .saturating_sub(visible_len(status))
        .saturating_sub(2);
    let dots = ".".repeat(dots_len);

    format!("{} {}{}{} {}", text, dim, dots, reset, status_colored)
}

/// Format a dot leader line with colored prefix and status for reset output.
fn format_reset_dot_leader(prefix: &str, text: &str, status: &str) -> String {
    use ferment::style::Color;

    let dim = Color::BrightBlack.to_ansi_fg();
    let green = Color::Green.to_ansi_fg();
    let reset = "\x1b[0m";

    // Color the prefix
    let prefix_colored = if prefix == "✓" {
        format!("{}{}{}", green, prefix, reset)
    } else {
        format!("{}{}{}", dim, prefix, reset)
    };

    // Color the status based on value
    let status_upper = status.to_uppercase();
    let status_colored = match status_upper.as_str() {
        "OK" | "CREATED" | "CONFIGURED" => format!("{}{}{}", green, status_upper, reset),
        "SKIPPED" | "UNCHANGED" => format!("{}{}{}", dim, status_upper, reset),
        _ => status_upper,
    };

    // Calculate dots needed
    let prefix_len = 1; // visible prefix length (✓ or ○)
    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(prefix_len)
        .saturating_sub(1) // space after prefix
        .saturating_sub(text.len())
        .saturating_sub(visible_len(status))
        .saturating_sub(2); // spaces around dots
    let dots = ".".repeat(dots_len);

    format!(
        "{} {} {}{}{} {}",
        prefix_colored, text, dim, dots, reset, status_colored
    )
}

/// Print a line with a dimmed prefix symbol, dot leaders, and status.
///
/// Format: `{prefix} {text} {dots} {status}` where total width is `STEP_LINE_WIDTH`.
/// Status may contain ANSI codes which are handled correctly.
fn print_prefixed_dot_leader(prefix: &str, text: &str, status: &str) {
    use ferment::style::Color;

    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";

    // Calculate dots needed: total width - prefix - text - visible status - spaces
    let prefix_len = prefix.chars().count(); // Use char count for Unicode
    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(prefix_len)
        .saturating_sub(1) // space after prefix
        .saturating_sub(text.len())
        .saturating_sub(visible_len(status))
        .saturating_sub(2); // spaces around dots
    let dots = ".".repeat(dots_len);

    println!(
        "{}{}{} {} {}{}{} {}",
        dim, prefix, reset, text, dim, dots, reset, status
    );
}

/// Print a line with a colored prefix symbol, dot leaders, and status.
///
/// Format: `{prefix} {text} {dots} {status}` where total width is `STEP_LINE_WIDTH`.
/// The `prefix_formatted` should include ANSI codes, `prefix_width` is the visible character count.
/// Status may contain ANSI codes which are handled correctly.
fn print_colored_prefix_dot_leader(
    prefix_formatted: &str,
    prefix_width: usize,
    text: &str,
    status: &str,
) {
    use ferment::style::Color;

    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";

    // Calculate dots needed: total width - prefix - text - visible status - spaces
    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(prefix_width)
        .saturating_sub(1) // space after prefix
        .saturating_sub(text.len())
        .saturating_sub(visible_len(status))
        .saturating_sub(2); // spaces around dots
    let dots = ".".repeat(dots_len);

    println!(
        "{} {} {}{}{} {}",
        prefix_formatted, text, dim, dots, reset, status
    );
}

/// Print a hint line with a dimmed circle prefix.
///
/// Format: `○ {text}` where the circle is dimmed.
fn print_hint(text: &str) {
    use ferment::style::Color;

    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";

    println!("{}○{} {}", dim, reset, text);
}

/// Result of a destroy step - either work was done or it was skipped.
enum DestroyStepResult {
    /// Work was performed
    Done,
    /// Nothing to do (already in desired state)
    Skipped,
}

/// Run a destroy step with spinner, then show dot-leader format on completion.
///
/// Shows `{in_progress}...` spinner while running, then outputs:
/// - `✓ {completed} ....... OK` on success (green checkmark and OK)
/// - `○ {completed} ....... SKIPPED` if nothing to do (dimmed)
/// - `✗ {completed} ....... FAILED` on failure
///
/// Returns whether work was done (for tracking if anything was destroyed).
fn run_destroy_step<F>(in_progress: &str, completed: &str, executor: F) -> bool
where
    F: FnOnce() -> std::result::Result<DestroyStepResult, String>,
{
    use crate::tui::start_spinner;
    use ferment::style::Color;

    let mut spin = start_spinner(in_progress);

    match executor() {
        Ok(DestroyStepResult::Done) => {
            spin.stop();
            let green = Color::Green.to_ansi_fg();
            let dim = Color::BrightBlack.to_ansi_fg();
            let reset = "\x1b[0m";

            let checkmark = "✓";
            let prefix_width = 1;
            let status = format!("{}OK{}", green, reset);
            let dots_len = STEP_LINE_WIDTH
                .saturating_sub(prefix_width)
                .saturating_sub(1)
                .saturating_sub(completed.len())
                .saturating_sub(2)
                .saturating_sub(2);
            let dots = ".".repeat(dots_len);

            println!(
                "{}{}{} {} {}{}{} {}",
                green, checkmark, reset, completed, dim, dots, reset, status
            );
            true
        }
        Ok(DestroyStepResult::Skipped) => {
            spin.stop();
            print_destroy_skipped(completed);
            false
        }
        Err(e) => {
            spin.failure(&e);
            false
        }
    }
}

/// Print a skipped destroy step in dot-leader format.
///
/// Outputs: `○ {text} ....... SKIPPED` (dimmed)
fn print_destroy_skipped(text: &str) {
    use ferment::style::Color;

    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";

    let prefix = "○";
    let prefix_width = 1;
    let status = "SKIPPED";
    let dots_len = STEP_LINE_WIDTH
        .saturating_sub(prefix_width)
        .saturating_sub(1) // space after prefix
        .saturating_sub(text.len())
        .saturating_sub(status.len())
        .saturating_sub(2); // spaces around dots
    let dots = ".".repeat(dots_len);

    println!(
        "{}{}{} {} {}{}{} {}{}{}",
        dim, prefix, reset, text, dim, dots, reset, dim, status, reset
    );
}

/// Run a step with spinner and format output according to the new design.
fn run_step<F>(step: &StartStep, executor: F) -> Result<()>
where
    F: FnOnce() -> std::result::Result<StepOutcome, String>,
{
    use crate::tui::start_spinner;

    let spin = start_spinner(step.in_progress.to_string());

    match executor() {
        Ok(outcome) => {
            let (success_text, is_skipped) = match &outcome {
                StepOutcome::Success => (step.completed.to_string(), false),
                StepOutcome::SuccessMsg(msg) => (msg.clone(), false),
                StepOutcome::Skipped(_) => (step.completed.to_string(), true),
                StepOutcome::Failed(err) => {
                    spin.failure(err);
                    return Err(Error::Other(err.clone()));
                }
            };

            // Format output with optional dot leaders
            if step.dot_leader {
                let status = if is_skipped { "SKIPPED" } else { "OK" };
                let formatted = format_dot_leader(&success_text, status);
                if is_skipped {
                    spin.info(&formatted);
                } else {
                    spin.success(&formatted);
                }
            } else if is_skipped {
                spin.info(&success_text);
            } else {
                spin.success(&success_text);
            }

            Ok(())
        }
        Err(e) => {
            spin.failure(&e);
            Err(Error::Other(e))
        }
    }
}

/// Get the deploy directory path (~/.local/share/inferadb/deploy)
fn get_deploy_dir() -> PathBuf {
    Config::data_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share/inferadb"))
        .join("deploy")
}

/// Get the engine directory path (~/.local/share/inferadb/engine)
fn get_engine_dir() -> PathBuf {
    Config::data_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share/inferadb"))
        .join("engine")
}

/// Get the control directory path (~/.local/share/inferadb/control)
fn get_control_dir() -> PathBuf {
    Config::data_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share/inferadb"))
        .join("control")
}

/// Get the dashboard directory path (~/.local/share/inferadb/dashboard)
fn get_dashboard_dir() -> PathBuf {
    Config::data_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share/inferadb"))
        .join("dashboard")
}

/// Get the Tailscale credentials file path
fn get_tailscale_creds_file() -> PathBuf {
    Config::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config/inferadb"))
        .join("tailscale-credentials")
}

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

/// Parse a kubectl apply output line into (resource, status).
/// Example: "deployment.apps/inferadb-control created" -> ("deployment.apps/inferadb-control", "created")
fn parse_kubectl_apply_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // kubectl apply output format: "<resource> <status>"
    // Status can be: created, configured, unchanged, deleted
    let parts: Vec<&str> = line.rsplitn(2, ' ').collect();
    if parts.len() == 2 {
        let status = parts[0].to_lowercase();
        let resource = parts[1].to_string();
        if matches!(
            status.as_str(),
            "created" | "configured" | "unchanged" | "deleted"
        ) {
            return Some((resource, status));
        }
    }
    None
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

/// Check if a command is available in PATH
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run a command and return its output
fn run_command(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| Error::Other(format!("Failed to run {}: {}", cmd, e)))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(Error::Other(format!("{} failed: {}", cmd, stderr.trim())))
    }
}

/// Run a command, returning Ok(output) on success or Ok(None) on failure
fn run_command_optional(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

/// Normalize a version string to "vX.Y.Z" format, including build number if present.
fn normalize_version(raw: &str) -> String {
    // Find where the version number starts (first digit, optionally preceded by 'v')
    let trimmed = raw.trim();
    let start = trimmed.find(|c: char| c.is_ascii_digit()).unwrap_or(0);

    // Check if there's a 'v' just before the digit
    let has_v = start > 0 && trimmed.chars().nth(start - 1) == Some('v');
    let version_start = if has_v { start - 1 } else { start };

    // Extract version starting from this point
    let version_part = &trimmed[version_start..];

    // Find end of version (stop at whitespace or end)
    let version = version_part
        .split_whitespace()
        .next()
        .unwrap_or(version_part)
        .trim_end_matches([',', ';', ':']);

    // Ensure it starts with 'v'
    if version.starts_with('v') {
        version.to_string()
    } else if version
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("v{}", version)
    } else {
        raw.to_string()
    }
}

/// Extract a meaningful version string from command output.
fn extract_version_string(output: &str, command: &str) -> String {
    // Handle command-specific output formats
    let raw = match command {
        "talosctl" => {
            // Output: "Client:\nTalos v1.11.5"
            output
                .lines()
                .find(|line| line.contains("Talos v") || line.contains("Tag:"))
                .map(|line| line.trim().trim_start_matches("Tag:").trim().to_string())
                .unwrap_or_else(|| "installed".to_string())
        }
        "kubectl" => {
            // Output: "Client Version: v1.34.1\nKustomize Version: v5.7.1"
            output
                .lines()
                .find(|line| line.starts_with("Client Version:"))
                .map(|line| {
                    line.trim_start_matches("Client Version:")
                        .trim()
                        .to_string()
                })
                .unwrap_or_else(|| "installed".to_string())
        }
        _ => {
            // Default: take first non-empty line
            output
                .lines()
                .find(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
                .unwrap_or_else(|| "installed".to_string())
        }
    };
    normalize_version(&raw)
}

/// Run a command with live output streaming
fn run_command_streaming(cmd: &str, args: &[&str], env_vars: &[(&str, &str)]) -> Result<()> {
    let mut command = Command::new(cmd);
    command
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    for (key, value) in env_vars {
        command.env(key, value);
    }

    let status = command
        .status()
        .map_err(|e| Error::Other(format!("Failed to run {}: {}", cmd, e)))?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::Other(format!(
            "{} exited with code {}",
            cmd,
            status.code().unwrap_or(-1)
        )))
    }
}

/// Check if a Docker container exists
fn docker_container_exists(name: &str) -> bool {
    run_command_optional(
        "docker",
        &[
            "ps",
            "-a",
            "--filter",
            &format!("name={}", name),
            "--format",
            "{{.Names}}",
        ],
    )
    .map(|output| output.lines().any(|line| line.contains(name)))
    .unwrap_or(false)
}

/// Get all Docker containers for the cluster
fn get_cluster_containers() -> Vec<String> {
    run_command_optional(
        "docker",
        &[
            "ps",
            "-a",
            "--filter",
            &format!("name={}", CLUSTER_NAME),
            "--format",
            "{{.Names}}",
        ],
    )
    .map(|output| {
        output
            .lines()
            .filter(|line| !line.is_empty())
            .map(String::from)
            .collect()
    })
    .unwrap_or_default()
}

/// Check if cluster containers are paused
fn are_containers_paused() -> bool {
    run_command_optional(
        "docker",
        &[
            "ps",
            "-a",
            "--filter",
            &format!("name={}", CLUSTER_NAME),
            "--filter",
            "status=paused",
            "--format",
            "{{.Names}}",
        ],
    )
    .map(|output| !output.trim().is_empty())
    .unwrap_or(false)
}

/// Get Docker container IP on a specific network
fn get_container_ip(container_name: &str) -> Option<String> {
    run_command_optional(
        "docker",
        &[
            "inspect",
            container_name,
            "--format",
            "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
        ],
    )
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

/// Dependency information for doctor command
struct Dependency {
    name: &'static str,
    command: &'static str,
    version_args: &'static [&'static str],
    required: bool,
    install_hint_mac: &'static str,
    install_hint_linux: &'static str,
    install_hint_windows: &'static str,
}

const DEPENDENCIES: &[Dependency] = &[
    Dependency {
        name: "Docker",
        command: "docker",
        version_args: &["--version"],
        required: true,
        install_hint_mac: "brew install --cask docker",
        install_hint_linux: "https://docs.docker.com/engine/install/",
        install_hint_windows: "winget install Docker.DockerDesktop",
    },
    Dependency {
        name: "talosctl",
        command: "talosctl",
        version_args: &["version", "--client", "--short"],
        required: true,
        install_hint_mac: "brew install siderolabs/tap/talosctl",
        install_hint_linux: "curl -sL https://talos.dev/install | sh",
        install_hint_windows: "scoop install talosctl",
    },
    Dependency {
        name: "kubectl",
        command: "kubectl",
        version_args: &["version", "--client"],
        required: true,
        install_hint_mac: "brew install kubectl",
        install_hint_linux: "https://kubernetes.io/docs/tasks/tools/",
        install_hint_windows: "winget install Kubernetes.kubectl",
    },
    Dependency {
        name: "Helm",
        command: "helm",
        version_args: &["version", "--short"],
        required: true,
        install_hint_mac: "brew install helm",
        install_hint_linux: "https://helm.sh/docs/intro/install/",
        install_hint_windows: "winget install Helm.Helm",
    },
    Dependency {
        name: "Tailscale",
        command: "tailscale",
        version_args: &["version"],
        required: false,
        install_hint_mac: "brew install tailscale",
        install_hint_linux: "https://tailscale.com/download/linux",
        install_hint_windows: "winget install Tailscale.Tailscale",
    },
    Dependency {
        name: "git",
        command: "git",
        version_args: &["--version"],
        required: true,
        install_hint_mac: "xcode-select --install",
        install_hint_linux: "apt install git",
        install_hint_windows: "winget install Git.Git",
    },
];

/// Get the appropriate install hint for the current platform
fn get_install_hint(dep: &Dependency) -> &'static str {
    if cfg!(target_os = "macos") {
        dep.install_hint_mac
    } else if cfg!(target_os = "windows") {
        dep.install_hint_windows
    } else {
        dep.install_hint_linux
    }
}

// ============================================================================
// Shared helper functions
// ============================================================================

/// Print a formatted header for non-interactive mode with dimmed record icon.
fn print_styled_header(title: &str) {
    use ferment::style::Color;
    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";
    println!("\n{}⏺{} {}", dim, reset, title);
}

// ============================================================================
// Doctor check functions (shared between interactive and non-interactive modes)
// ============================================================================

use crate::tui::{CheckResult, EnvironmentStatus};

/// Check a single dependency and return the result.
fn check_dependency(dep: &Dependency) -> (CheckResult, bool) {
    let exists = command_exists(dep.command);
    let version = if exists {
        run_command_optional(dep.command, dep.version_args)
            .map(|v| extract_version_string(&v, dep.command))
            .unwrap_or_else(|| "installed".to_string())
    } else {
        String::new()
    };

    if exists {
        (
            CheckResult::success("Dependencies", dep.name, version),
            true,
        )
    } else if dep.required {
        (
            CheckResult::failure(
                "Dependencies",
                dep.name,
                format!("NOT FOUND → {}", get_install_hint(dep)),
            ),
            false,
        )
    } else {
        (
            CheckResult::optional("Dependencies", dep.name, "not found (optional)"),
            true,
        )
    }
}

/// Check if Docker daemon is running.
fn check_docker_daemon() -> Option<(CheckResult, bool)> {
    if !command_exists("docker") {
        return None;
    }

    match run_command_optional("docker", &["info"]) {
        Some(_) => Some((
            CheckResult::success("Services", "Docker daemon", "RUNNING"),
            true,
        )),
        None => Some((
            CheckResult::failure(
                "Services",
                "Docker daemon",
                "not running → start Docker Desktop",
            ),
            false,
        )),
    }
}

/// Check Tailscale connection status.
fn check_tailscale_connection() -> Option<CheckResult> {
    if !command_exists("tailscale") {
        return None;
    }

    match run_command_optional("tailscale", &["status", "--json"]) {
        Some(output) => {
            if output.contains("\"BackendState\"") && output.contains("\"Running\"") {
                Some(CheckResult::success("Services", "Tailscale", "CONNECTED"))
            } else {
                Some(CheckResult::optional(
                    "Services",
                    "Tailscale",
                    "not connected → tailscale up",
                ))
            }
        }
        None => None,
    }
}

/// Check for cached Tailscale OAuth credentials.
fn check_tailscale_credentials() -> CheckResult {
    let creds_file = get_tailscale_creds_file();
    if creds_file.exists() {
        CheckResult::success("Configuration", "Tailscale OAuth", "CONFIGURED")
    } else {
        CheckResult::optional(
            "Configuration",
            "Tailscale OAuth",
            "will be prompted during dev start",
        )
    }
}

/// Extract the detail from a CheckResult status string.
fn extract_status_detail(status: &str) -> &str {
    status
        .trim_start_matches("✓ ")
        .trim_start_matches("✗ ")
        .trim_start_matches("○ ")
}

/// Format a check result with component name and dot leaders.
fn format_check_output(component: &str, detail: &str) -> String {
    format_dot_leader(component, detail)
}

/// Run all doctor checks and return results.
fn run_all_checks() -> (Vec<CheckResult>, EnvironmentStatus) {
    let mut all_required_ok = true;
    let mut results: Vec<CheckResult> = Vec::new();

    // Check dependencies
    for dep in DEPENDENCIES {
        let (result, ok) = check_dependency(dep);
        if !ok {
            all_required_ok = false;
        }
        results.push(result);
    }

    // Check Docker daemon
    if let Some((result, ok)) = check_docker_daemon() {
        if !ok {
            all_required_ok = false;
        }
        results.push(result);
    }

    // Check Tailscale connection (optional)
    if let Some(result) = check_tailscale_connection() {
        results.push(result);
    }

    // Check Tailscale credentials
    results.push(check_tailscale_credentials());

    let status = if all_required_ok {
        EnvironmentStatus::Ready
    } else {
        EnvironmentStatus::NotReady
    };

    (results, status)
}

/// Run dev doctor - check environment readiness
pub async fn doctor(ctx: &Context, interactive: bool) -> Result<()> {
    // Use full-screen TUI if explicitly requested and available
    if interactive && crate::tui::is_interactive(ctx) {
        return doctor_interactive();
    }

    // Default: Use spinners for each check
    doctor_with_spinners()
}

/// Run doctor with inline spinners for each check.
fn doctor_with_spinners() -> Result<()> {
    use crate::tui::start_spinner;

    print_styled_header("InferaDB Development Cluster Doctor");

    let mut all_required_ok = true;

    // Phase 1: Check dependencies
    print_phase_header("Checking dependencies");

    for dep in DEPENDENCIES {
        let spin = start_spinner(format!("Checking {}", dep.name));
        let (result, ok) = check_dependency(dep);
        if !ok {
            all_required_ok = false;
        }

        let detail = extract_status_detail(&result.status);
        let output = format_check_output(&result.component, detail);

        if result.status.starts_with('✓') {
            spin.success(&output);
        } else if result.status.starts_with('✗') {
            spin.failure(&output);
        } else if result.status.starts_with('○') {
            spin.warning(&output);
        } else {
            spin.success(&output);
        }
    }

    // Phase 2: Check environment
    print_phase_header("Checking environment");

    // Check Docker daemon
    if command_exists("docker") {
        let spin = start_spinner("Checking Docker daemon");
        if let Some((result, ok)) = check_docker_daemon() {
            if !ok {
                all_required_ok = false;
            }

            let detail = extract_status_detail(&result.status);
            let output = format_check_output(&result.component, detail);

            if result.status.starts_with('✓') {
                spin.success(&output);
            } else if result.status.starts_with('✗') {
                spin.failure(&output);
            } else {
                spin.success(&output);
            }
        } else {
            spin.success(&format_dot_leader("Docker daemon", "SKIPPED"));
        }
    }

    // Check Tailscale connection (optional)
    if command_exists("tailscale") {
        let spin = start_spinner("Checking Tailscale");
        if let Some(result) = check_tailscale_connection() {
            let detail = extract_status_detail(&result.status);
            let output = format_check_output(&result.component, detail);

            if result.status.starts_with('✓') {
                spin.success(&output);
            } else if result.status.starts_with('○') {
                spin.warning(&output);
            } else {
                spin.success(&output);
            }
        } else {
            spin.warning(&format_dot_leader("Tailscale", "UNKNOWN"));
        }
    }

    // Check Tailscale credentials
    {
        let spin = start_spinner("Checking Tailscale OAuth");
        let result = check_tailscale_credentials();
        let detail = extract_status_detail(&result.status);
        let output = format_check_output(&result.component, detail);

        if result.status.starts_with('✓') {
            spin.success(&output);
        } else if result.status.starts_with('○') {
            spin.warning(&output);
        } else {
            spin.success(&output);
        }
    }

    // Print overall status and hints
    println!();
    if all_required_ok {
        print_hint("Run 'inferadb dev start' to start the development cluster");
        Ok(())
    } else {
        print_error("Environment not ready - missing required dependencies");
        Err(Error::Other("Missing required dependencies".to_string()))
    }
}

/// Run doctor with full-screen TUI.
fn doctor_interactive() -> Result<()> {
    use crate::tui::DevDoctorView;
    use ferment::output::{terminal_height, terminal_width};
    use ferment::runtime::{Program, ProgramOptions};

    let width = terminal_width();
    let height = terminal_height();

    let (results, status) = run_all_checks();

    let view = DevDoctorView::new(width, height)
        .with_status(status)
        .with_results(results);

    let is_ready = view.is_ready();

    Program::new(view)
        .with_options(ProgramOptions::fullscreen())
        .run()
        .map_err(|e| Error::Other(e.to_string()))?;

    if is_ready {
        Ok(())
    } else {
        Err(Error::Other("Missing required dependencies".to_string()))
    }
}

// ============================================================================
// Install step functions (shared between interactive and non-interactive modes)
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
        // Full clone for specific commit (need full history)
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
        // Shallow clone with shallow submodules
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
        // Update submodules after checkout
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

/// Step: Clone a component repository (engine, control, or dashboard).
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
        // Directory already exists, skip
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
    let _ = run_command_optional("helm", &["repo", "update"]);

    Ok(Some("Helm repositories configured".to_string()))
}

/// Load cached Tailscale credentials
fn load_tailscale_credentials() -> Option<(String, String)> {
    let creds_file = get_tailscale_creds_file();
    if !creds_file.exists() {
        return None;
    }

    let content = fs::read_to_string(&creds_file).ok()?;
    let mut client_id = None;
    let mut client_secret = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("TAILSCALE_CLIENT_ID=") {
            client_id = Some(
                line.trim_start_matches("TAILSCALE_CLIENT_ID=")
                    .trim_matches('"')
                    .to_string(),
            );
        } else if line.starts_with("TAILSCALE_CLIENT_SECRET=") {
            client_secret = Some(
                line.trim_start_matches("TAILSCALE_CLIENT_SECRET=")
                    .trim_matches('"')
                    .to_string(),
            );
        }
    }

    match (client_id, client_secret) {
        (Some(id), Some(secret)) if !id.is_empty() && !secret.is_empty() => Some((id, secret)),
        _ => None,
    }
}

/// Save Tailscale credentials
fn save_tailscale_credentials(client_id: &str, client_secret: &str) -> Result<()> {
    let creds_file = get_tailscale_creds_file();
    if let Some(parent) = creds_file.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| Error::Other(format!("Failed to create directory: {}", e)))?;
    }

    let content = format!(
        "# Tailscale OAuth credentials for InferaDB development\n\
         # Generated by inferadb dev start\n\
         TAILSCALE_CLIENT_ID=\"{}\"\n\
         TAILSCALE_CLIENT_SECRET=\"{}\"\n",
        client_id, client_secret
    );

    fs::write(&creds_file, &content)
        .map_err(|e| Error::Other(format!("Failed to write credentials: {}", e)))?;

    // Set file permissions to 600 on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&creds_file)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&creds_file, perms)?;
    }

    println!("Credentials saved to {}", creds_file.display());
    Ok(())
}

/// Get Tailscale credentials from environment, cache, or prompt
fn get_tailscale_credentials() -> Result<(String, String)> {
    use ferment::forms::{Form, Group, InputField, NoteField};

    // Try environment variables first
    if let (Ok(id), Ok(secret)) = (
        env::var("TAILSCALE_CLIENT_ID"),
        env::var("TAILSCALE_CLIENT_SECRET"),
    ) {
        if !id.is_empty() && !secret.is_empty() {
            return Ok((id, secret));
        }
    }

    // Try cached credentials
    if let Some((id, secret)) = load_tailscale_credentials() {
        return Ok((id, secret));
    }

    // Build the credentials form with setup instructions
    let instructions = r#"Tailscale OAuth credentials are required for the Kubernetes operator.

Step 1: Enable HTTPS on your tailnet (one-time setup)
  Go to: https://login.tailscale.com/admin/dns
  Scroll to 'HTTPS Certificates' and click 'Enable HTTPS'

Step 2: Create tags (one-time setup)
  Go to: https://login.tailscale.com/admin/acls/tags
  Create tag 'k8s-operator' with yourself as owner
  Create tag 'k8s' with 'tag:k8s-operator' as owner

Step 3: Create OAuth client
  Go to: https://login.tailscale.com/admin/settings/oauth
  Click 'Generate OAuth client'
  Add scopes:
    - Devices → Core: Read & Write, tag: k8s-operator
    - Keys → Auth Keys: Read & Write, tag: k8s-operator
  Click 'Generate client' and copy the credentials"#;

    let form = Form::new().title("Tailscale Setup").group(
        Group::new()
            .field(NoteField::new(instructions).build())
            .field(
                InputField::new("client_id")
                    .title("Client ID")
                    .required()
                    .build(),
            )
            .field(
                InputField::new("client_secret")
                    .title("Client Secret")
                    .required()
                    .hidden()
                    .build(),
            ),
    );

    let results = crate::tui::run_form(form)?
        .ok_or_else(|| Error::Other("Credentials input cancelled".to_string()))?;

    let client_id = results
        .get_string("client_id")
        .ok_or_else(|| Error::Other("Client ID is required".to_string()))?
        .to_string();

    let client_secret = results
        .get_string("client_secret")
        .ok_or_else(|| Error::Other("Client Secret is required".to_string()))?
        .to_string();

    if client_id.is_empty() || client_secret.is_empty() {
        return Err(Error::Other(
            "Both Client ID and Client Secret are required".to_string(),
        ));
    }

    // Save credentials for future use
    save_tailscale_credentials(&client_id, &client_secret)?;

    Ok((client_id, client_secret))
}

/// Get tailnet domain from local Tailscale CLI
fn get_tailnet_info() -> Option<String> {
    let output = run_command_optional("tailscale", &["status", "--json"])?;

    // Extract DNS name from JSON (simple parsing)
    for line in output.lines() {
        if line.contains("\"DNSName\"") {
            // Extract domain from "DNSName": "hostname.tail27bf77.ts.net."
            if let Some(start) = line.find(".ts.net") {
                // Work backwards to find the tailnet part
                let before_ts = &line[..start];
                if let Some(dot_pos) = before_ts.rfind('.') {
                    let tailnet = &before_ts[dot_pos + 1..];
                    return Some(format!("{}.ts.net", tailnet));
                }
            }
        }
    }
    None
}

use crate::tui::UninstallInfo;

/// Gather information about what will be uninstalled.
fn gather_uninstall_info() -> UninstallInfo {
    let deploy_dir = get_deploy_dir();
    let creds_file = get_tailscale_creds_file();
    let config_dir = Config::config_dir().unwrap_or_else(|| PathBuf::from(".config/inferadb"));
    let data_dir = Config::data_dir().unwrap_or_else(|| PathBuf::from(".local/share/inferadb"));
    let state_dir = Config::state_dir().unwrap_or_else(|| PathBuf::from(".local/state/inferadb"));

    let has_cluster = docker_container_exists(CLUSTER_NAME);
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

    let has_registry = docker_container_exists(REGISTRY_NAME);
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

// ============================================================================
// Uninstall step functions (shared between interactive and non-interactive)
// ============================================================================

/// Step: Destroy Talos cluster and clean up Tailscale devices.
fn step_destroy_cluster() -> std::result::Result<DestroyStepResult, String> {
    if !docker_container_exists(CLUSTER_NAME) {
        return Ok(DestroyStepResult::Skipped);
    }

    // Clean up Tailscale devices first
    cleanup_tailscale_devices().map_err(|e| e.to_string())?;

    // Destroy cluster (use run_command to capture output, not stream it)
    run_command("talosctl", &["cluster", "destroy", "--name", CLUSTER_NAME])
        .map_err(|e| e.to_string())?;

    Ok(DestroyStepResult::Done)
}

/// Step: Remove local Docker registry.
fn step_remove_registry() -> std::result::Result<DestroyStepResult, String> {
    if !docker_container_exists(REGISTRY_NAME) {
        return Ok(DestroyStepResult::Skipped);
    }

    let _ = run_command_optional("docker", &["stop", REGISTRY_NAME]);
    let _ = run_command_optional("docker", &["rm", "-f", REGISTRY_NAME]);

    Ok(DestroyStepResult::Done)
}

/// Step: Clean up kubectl/talosctl contexts.
/// Returns Done if any contexts were found and cleaned, Skipped otherwise.
fn step_cleanup_contexts() -> std::result::Result<DestroyStepResult, String> {
    let has_talos = run_command_optional("talosctl", &["config", "contexts"])
        .map(|o| o.contains(CLUSTER_NAME))
        .unwrap_or(false);

    let has_kube = run_command_optional("kubectl", &["config", "get-contexts", "-o", "name"])
        .map(|o| o.lines().any(|l| l == KUBE_CONTEXT))
        .unwrap_or(false);

    if !has_talos && !has_kube {
        return Ok(DestroyStepResult::Skipped);
    }

    cleanup_stale_contexts();
    Ok(DestroyStepResult::Done)
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
fn step_remove_state_dir() -> std::result::Result<DestroyStepResult, String> {
    let state_dir = Config::state_dir().unwrap_or_else(|| PathBuf::from(".local/state/inferadb"));
    if !state_dir.exists() {
        return Ok(DestroyStepResult::Skipped);
    }

    fs::remove_dir_all(&state_dir)
        .map_err(|e| format!("Failed to remove {}: {}", state_dir.display(), e))?;

    Ok(DestroyStepResult::Done)
}

/// Step: Remove Tailscale credentials.
fn step_remove_tailscale_creds() -> std::result::Result<DestroyStepResult, String> {
    let creds_file = get_tailscale_creds_file();
    if !creds_file.exists() {
        return Ok(DestroyStepResult::Skipped);
    }
    fs::remove_file(&creds_file)
        .map_err(|e| format!("Failed to remove {}: {}", creds_file.display(), e))?;
    Ok(DestroyStepResult::Done)
}

// Uninstall/destroy functionality is accessed via `stop --destroy`.
// The functions below are called from stop() when the destroy flag is set.

/// Run uninstall with inline spinners for each step
///
/// This function is idempotent - each step checks its own preconditions and
/// reports whether work was done or skipped. Safe to re-run if interrupted.
fn uninstall_with_spinners(yes: bool, with_credentials: bool) -> Result<()> {
    use ferment::style::Color;

    print_styled_header("Destroying InferaDB Development Cluster");

    // Gather initial info for confirmation prompt (snapshot at start)
    let info = gather_uninstall_info();
    let initially_had_something = info.has_anything();

    // Only show confirmation if there appears to be something to destroy
    if initially_had_something {
        // Show what will be destroyed
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

        // Confirm unless --yes
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

    // Track whether any work was actually done
    let mut did_work = false;

    // Step 1: Remove registry (must be before cluster to avoid network conflicts)
    // Always call - the step checks if registry exists
    did_work |= run_destroy_step("Removing registry", "Removed registry", step_remove_registry);

    // Step 2: Destroy cluster
    // Always call - the step checks if cluster exists
    did_work |= run_destroy_step(
        "Destroying cluster",
        "Destroyed cluster",
        step_destroy_cluster,
    );

    // Step 3: Clean up contexts
    // Always call - the step checks if contexts exist
    did_work |= run_destroy_step("Cleaning contexts", "Cleaned contexts", step_cleanup_contexts);

    // Step 4: Remove Docker images (each image individually)
    // Re-check images at execution time (not from cached info)
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
                        Ok(DestroyStepResult::Done)
                    } else {
                        // Image might already be removed - treat as skipped not error
                        Ok(DestroyStepResult::Skipped)
                    }
                },
            );
        }
    }

    // Step 5: Remove state directory
    // Always call - the step checks if directory exists
    did_work |= run_destroy_step(
        "Removing state directory",
        "Removed state directory",
        step_remove_state_dir,
    );

    // Step 6: Remove Tailscale credentials (optional, only shown if requested)
    if with_credentials {
        // Always call - the step checks if credentials exist
        did_work |= run_destroy_step(
            "Removing Tailscale credentials",
            "Removed Tailscale credentials",
            step_remove_tailscale_creds,
        );
    }

    // Summary
    println!();
    if did_work {
        let green = Color::Green.to_ansi_fg();
        let reset = "\x1b[0m";
        println!("{}Cluster destroyed successfully.{}", green, reset);

        // Show hint about credentials if they weren't removed
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

/// Run interactive uninstall with TUI and modal confirmation
fn uninstall_interactive(with_credentials: bool) -> Result<()> {
    use crate::tui::{DevUninstallView, InstallStep};
    use ferment::runtime::{Program, ProgramOptions};

    // Gather what will be removed
    let info = gather_uninstall_info();

    if !info.has_anything() {
        println!("Nothing to destroy. The development cluster is not installed.");
        return Ok(());
    }

    // Build steps based on what's installed
    // Note: Registry must be removed before cluster to avoid network conflicts
    // Each step is wrapped to convert DestroyStepResult to Option<String>
    let mut steps = Vec::new();

    if info.has_registry {
        steps.push(InstallStep::with_executor("Removing registry", || {
            step_remove_registry().map(|r| match r {
                DestroyStepResult::Done => Some("Removed".to_string()),
                DestroyStepResult::Skipped => Some("Skipped".to_string()),
            })
        }));
    }

    if info.has_cluster {
        steps.push(InstallStep::with_executor("Destroying cluster", || {
            step_destroy_cluster().map(|r| match r {
                DestroyStepResult::Done => Some("Destroyed".to_string()),
                DestroyStepResult::Skipped => Some("Skipped".to_string()),
            })
        }));
    }

    if info.has_kube_context || info.has_talos_context {
        steps.push(InstallStep::with_executor("Cleaning contexts", || {
            step_cleanup_contexts().map(|r| match r {
                DestroyStepResult::Done => Some("Cleaned".to_string()),
                DestroyStepResult::Skipped => Some("Skipped".to_string()),
            })
        }));
    }

    if info.dev_image_count > 0 {
        steps.push(InstallStep::with_executor(
            "Removing Docker images",
            step_remove_docker_images,
        ));
    }

    // Note: Deploy repository is intentionally NOT removed

    if info.has_state_dir {
        steps.push(InstallStep::with_executor("Removing state directory", || {
            step_remove_state_dir().map(|r| match r {
                DestroyStepResult::Done => Some("Removed".to_string()),
                DestroyStepResult::Skipped => Some("Skipped".to_string()),
            })
        }));
    }

    if with_credentials && info.has_creds_file {
        steps.push(InstallStep::with_executor(
            "Removing Tailscale credentials",
            || {
                step_remove_tailscale_creds().map(|r| match r {
                    DestroyStepResult::Done => Some("Removed".to_string()),
                    DestroyStepResult::Skipped => Some("Skipped".to_string()),
                })
            },
        ));
    }

    let view = DevUninstallView::new(steps, info, with_credentials);

    let result = Program::new(view)
        .with_options(ProgramOptions::fullscreen())
        .run()
        .map_err(|e| Error::Other(e.to_string()))?;

    // Check if user cancelled
    if result.was_cancelled() {
        println!("Uninstall cancelled.");
    }

    Ok(())
}

/// Get list of dev-related Docker images
fn get_dev_docker_images() -> Vec<String> {
    let mut images = Vec::new();

    // Get inferadb-* images
    if let Some(output) = run_command_optional(
        "docker",
        &[
            "images",
            "--format",
            "{{.Repository}}:{{.Tag}}",
            "inferadb-*",
        ],
    ) {
        for line in output.lines() {
            if !line.is_empty() && !line.contains("<none>") {
                images.push(line.to_string());
            }
        }
    }

    // Get Talos-related images
    if let Some(output) = run_command_optional(
        "docker",
        &["images", "--format", "{{.Repository}}:{{.Tag}}"],
    ) {
        for line in output.lines() {
            if (line.contains("ghcr.io/siderolabs/") || line.contains("registry.k8s.io/"))
                && !line.contains("<none>")
            {
                images.push(line.to_string());
            }
        }
    }

    // Get local registry image
    if let Some(output) = run_command_optional(
        "docker",
        &["images", "--format", "{{.Repository}}:{{.Tag}}", "registry"],
    ) {
        for line in output.lines() {
            if !line.is_empty() && !line.contains("<none>") {
                images.push(line.to_string());
            }
        }
    }

    images
}

/// Run dev start - create or resume local development cluster
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

    // Non-interactive mode - continue with existing flow
    // This handles both fresh starts and resuming paused clusters
    start_with_streaming(skip_build, force, commit)
}

/// Start new cluster with interactive TUI (shows Tailscale setup modal)
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
                    return Err("Docker daemon is not running. Please start Docker first.".to_string());
                }

                // Ensure deploy repo exists
                if !deploy_dir.exists() && !force {
                    return Err("Deploy repository not found. It will be cloned during setup.".to_string());
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
                build_start_steps(client_id, client_secret, skip_build, force, commit.as_deref(), &deploy_dir)
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

/// Build the steps for starting a new cluster (includes install steps)
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
    steps.push(InstallStep::with_executor(
        "Cloning deployment repository",
        {
            let deploy_dir = deploy_dir_owned.clone();
            let commit = commit_owned.clone();
            move || step_clone_repo(&deploy_dir, force, commit.as_deref())
        },
    ));
    steps.push(InstallStep::with_executor(
        "Creating configuration directory",
        step_create_config_dir,
    ));
    steps.push(InstallStep::with_executor(
        "Setting up Helm repositories",
        step_setup_helm,
    ));

    // Phase 2: Setting up cluster
    steps.push(InstallStep::with_executor(
        "Cleaning stale contexts",
        || {
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
        },
    ));
    steps.push(InstallStep::with_executor(
        "Creating Talos cluster",
        || match run_command(
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
        },
    ));
    steps.push(InstallStep::with_executor(
        "Setting kubectl context",
        || match run_command("kubectl", &["config", "use-context", KUBE_CONTEXT]) {
            Ok(_) => Ok(Some("Set".to_string())),
            Err(e) => Err(e.to_string()),
        },
    ));
    steps.push(InstallStep::with_executor(
        "Verifying cluster is ready",
        || match run_command("kubectl", &["get", "nodes"]) {
            Ok(_) => Ok(Some("Verified".to_string())),
            Err(e) => Err(e.to_string()),
        },
    ));

    steps
}

/// Show final success output with URLs and hints.
fn show_final_success(tailnet_suffix: Option<&str>) {
    use ferment::style::Color;
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

/// Start new cluster with streaming output (non-interactive)
#[allow(clippy::too_many_lines)]
fn start_with_streaming(skip_build: bool, force: bool, commit: Option<&str>) -> Result<()> {
    let deploy_dir = get_deploy_dir();

    print_styled_header("Starting InferaDB Development Cluster");

    // ========================================================================
    // Phase 0: Resume paused cluster if needed
    // ========================================================================
    if docker_container_exists(CLUSTER_NAME) && are_containers_paused() {
        print_phase_header("Resuming paused cluster");

        // Unpause each cluster container individually
        let containers = get_cluster_containers();
        for container in &containers {
            let container_name = container.clone();
            let in_progress = format!("Resuming {}", container);
            let completed = format!("Resumed {}", container);
            run_step(&StartStep::with_ok(&in_progress, &completed), || {
                // Check if container is paused before trying to unpause
                if !is_container_paused(&container_name) {
                    return Ok(StepOutcome::Skipped(String::new()));
                }
                run_command("docker", &["unpause", &container_name])
                    .map(|_| StepOutcome::Success)
                    .or_else(|e| {
                        // Container wasn't paused
                        if e.to_string().contains("not paused") {
                            Ok(StepOutcome::Skipped(String::new()))
                        } else {
                            Err(e.to_string())
                        }
                    })
            })?;
        }

        // Unpause registry if it exists
        if docker_container_exists(REGISTRY_NAME) {
            let in_progress = format!("Resuming {}", REGISTRY_NAME);
            let completed = format!("Resumed {}", REGISTRY_NAME);
            run_step(&StartStep::with_ok(&in_progress, &completed), || {
                // Check if registry is paused before trying to unpause
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
            })?;
        }

        // Wait for containers to stabilize
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
    }

    // ========================================================================
    // Phase 1: Conditioning environment
    // ========================================================================
    print_phase_header("Conditioning environment");

    // Step: Clone deployment repository
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

    // Step: Clone engine repository
    let engine_dir = get_engine_dir();
    run_step(
        &StartStep::with_ok("Cloning engine repository", "Cloned engine repository"),
        || step_clone_component("engine", ENGINE_REPO_URL, &engine_dir, force),
    )?;

    // Step: Clone control repository
    let control_dir = get_control_dir();
    run_step(
        &StartStep::with_ok("Cloning control repository", "Cloned control repository"),
        || step_clone_component("control", CONTROL_REPO_URL, &control_dir, force),
    )?;

    // Step: Clone dashboard repository
    let dashboard_dir = get_dashboard_dir();
    run_step(
        &StartStep::with_ok(
            "Cloning dashboard repository",
            "Cloned dashboard repository",
        ),
        || step_clone_component("dashboard", DASHBOARD_REPO_URL, &dashboard_dir, force),
    )?;

    // Step: Create configuration directory
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

    // Step: Set up Tailscale Helm repository
    run_step(
        &StartStep::with_ok(
            "Setting up Tailscale Helm repository",
            "Set up Tailscale Helm repository",
        ),
        || {
            // Check if repo already exists
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

    // Step: Update Helm repositories
    run_step(
        &StartStep::with_ok("Updating Helm repositories", "Updated Helm repositories"),
        || {
            run_command("helm", &["repo", "update"])
                .map(|_| StepOutcome::Success)
                .map_err(|e| e.to_string())
        },
    )?;

    // Step: Pull Docker registry image
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

    // Step: Validate Flux development overlay
    let flux_dev_dir = deploy_dir.join("flux/apps/dev");
    run_step(
        &StartStep::with_ok(
            "Validating Flux development overlay",
            "Validated Flux development overlay",
        ),
        || {
            if flux_dev_dir.exists() {
                Ok(StepOutcome::Success)
            } else {
                Ok(StepOutcome::Skipped(String::new()))
            }
        },
    )?;

    // Step: Validate Flux base manifests
    let flux_base_dir = deploy_dir.join("flux/clusters/dev-local/flux-system");
    run_step(
        &StartStep::with_ok(
            "Validating Flux base manifests",
            "Validated Flux base manifests",
        ),
        || {
            if flux_base_dir.join("gotk-components.yaml").exists() {
                Ok(StepOutcome::Success)
            } else {
                Ok(StepOutcome::Skipped(String::new()))
            }
        },
    )?;

    // Step: Validate deployment configuration
    run_step(
        &StartStep::with_ok(
            "Validating deployment configuration",
            "Validated deployment configuration",
        ),
        || {
            if deploy_dir.join("flux").exists() {
                Ok(StepOutcome::Success)
            } else {
                Err("flux directory not found in deploy repo".to_string())
            }
        },
    )?;

    // ========================================================================
    // Phase 2: Setting up cluster
    // ========================================================================
    print_phase_header("Setting up cluster");

    // Step: Check prerequisites
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

    // Get Tailscale credentials (before continuing)
    let (ts_client_id, ts_client_secret) = get_tailscale_credentials()?;

    // Check if cluster already exists
    let cluster_exists = docker_container_exists(CLUSTER_NAME);

    // Step: Clean stale contexts (only if cluster doesn't exist)
    run_step(
        &StartStep::with_ok("Cleaning stale contexts", "Cleaned stale contexts"),
        || {
            if cluster_exists {
                return Ok(StepOutcome::Skipped(String::new()));
            }
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
            if let Some(contexts) =
                run_command_optional("kubectl", &["config", "get-contexts", "-o", "name"])
            {
                if contexts.lines().any(|l| l == KUBE_CONTEXT) {
                    let _ = run_command_optional(
                        "kubectl",
                        &["config", "delete-context", KUBE_CONTEXT],
                    );
                    let _ = run_command_optional(
                        "kubectl",
                        &["config", "delete-cluster", CLUSTER_NAME],
                    );
                    let _ =
                        run_command_optional("kubectl", &["config", "delete-user", KUBE_CONTEXT]);
                }
            }
            Ok(StepOutcome::Success)
        },
    )?;

    // Step: Create or verify Talos cluster
    run_step(
        &StartStep::with_ok("Provisioning Talos cluster", "Provisioned Talos cluster"),
        || {
            if cluster_exists {
                // Cluster exists - verify it's healthy
                if run_command_optional("kubectl", &["--context", KUBE_CONTEXT, "get", "nodes"])
                    .is_some()
                {
                    return Ok(StepOutcome::Skipped(String::new()));
                }
                // Context might be stale, try to set it up
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

    // Step: Set kubectl context
    run_step(
        &StartStep::with_ok("Setting kubectl context", "Set kubectl context"),
        || {
            // Check if already the current context
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

    // Step: Verify cluster is ready
    run_step(
        &StartStep::with_ok("Verifying cluster is ready", "Verified cluster is ready"),
        || {
            run_command("kubectl", &["get", "nodes"])
                .map(|_| StepOutcome::Success)
                .map_err(|e| e.to_string())
        },
    )?;

    // Step: Bootstrap Flux
    run_step(
        &StartStep::with_ok("Bootstrapping Flux", "Bootstrapped Flux"),
        || {
            let flux_dir = deploy_dir.join("flux/clusters/dev-local/flux-system");
            if !flux_dir.join("gotk-components.yaml").exists() {
                return Ok(StepOutcome::Skipped(String::new()));
            }
            run_command(
                "kubectl",
                &[
                    "apply",
                    "-f",
                    flux_dir.join("gotk-components.yaml").to_str().unwrap(),
                ],
            )
            .map_err(|e| e.to_string())?;
            run_command(
                "kubectl",
                &[
                    "apply",
                    "-f",
                    flux_dir.join("gotk-sync.yaml").to_str().unwrap(),
                ],
            )
            .map(|_| StepOutcome::Success)
            .map_err(|e| e.to_string())
        },
    )?;

    // Step: Set up container registry
    let registry_ip = setup_container_registry()?;

    // Step: Build and push container images
    if !skip_build {
        run_step(
            &StartStep::with_ok(
                "Building and pushing container images",
                "Built and pushed container images",
            ),
            || build_and_push_images(&registry_ip),
        )?;
    } else {
        run_step(
            &StartStep::with_ok(
                "Building and pushing container images",
                "Built and pushed container images",
            ),
            || Ok(StepOutcome::Skipped(String::new())),
        )?;
    }

    // Step: Set up Kubernetes resources
    run_step(
        &StartStep::with_ok(
            "Setting up Kubernetes resources",
            "Set up Kubernetes resources",
        ),
        || setup_kubernetes_resources(&registry_ip),
    )?;

    // Step: Install Tailscale operator
    run_step(
        &StartStep::with_ok(
            "Installing Tailscale operator",
            "Installed Tailscale operator",
        ),
        || install_tailscale_operator(&ts_client_id, &ts_client_secret),
    )?;

    // Step: Install FoundationDB operator
    run_step(
        &StartStep::with_ok(
            "Installing FoundationDB operator",
            "Installed FoundationDB operator",
        ),
        install_fdb_operator,
    )?;

    // Step: Deploy InferaDB
    let tailnet_suffix = run_step_with_result(
        &StartStep::with_ok("Deploying InferaDB", "Deployed InferaDB"),
        || deploy_inferadb(&deploy_dir, &registry_ip),
    )?;

    // Show final success
    show_final_success(tailnet_suffix.as_deref());

    Ok(())
}

/// Run a step and return a result value.
fn run_step_with_result<F, T>(step: &StartStep, executor: F) -> Result<T>
where
    F: FnOnce() -> std::result::Result<(StepOutcome, T), String>,
{
    use crate::tui::start_spinner;

    let spin = start_spinner(step.in_progress.to_string());

    match executor() {
        Ok((outcome, value)) => {
            let (success_text, is_skipped) = match &outcome {
                StepOutcome::Success => (step.completed.to_string(), false),
                StepOutcome::SuccessMsg(msg) => (msg.clone(), false),
                StepOutcome::Skipped(_) => (step.completed.to_string(), true),
                StepOutcome::Failed(err) => {
                    spin.failure(err);
                    return Err(Error::Other(err.clone()));
                }
            };

            if step.dot_leader {
                let status = if is_skipped { "SKIPPED" } else { "OK" };
                let formatted = format_dot_leader(&success_text, status);
                if is_skipped {
                    spin.info(&formatted);
                } else {
                    spin.success(&formatted);
                }
            } else if is_skipped {
                spin.info(&success_text);
            } else {
                spin.success(&success_text);
            }

            Ok(value)
        }
        Err(e) => {
            spin.failure(&e);
            Err(Error::Other(e))
        }
    }
}

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

            // Configure Talos nodes for insecure registry (validates and repairs if needed)
            let repaired_nodes = configure_talos_registry(&registry_ip)?;

            let outcome = if registry_existed {
                if repaired_nodes > 0 {
                    StepOutcome::SuccessMsg(format!("repaired {} node(s)", repaired_nodes))
                } else {
                    StepOutcome::Skipped(String::new())
                }
            } else {
                StepOutcome::Success
            };

            Ok((outcome, registry_ip))
        },
    )
}

/// Configure Talos nodes for insecure registry access.
///
/// Note: This configures containerd to use the local HTTP registry. The config
/// should NOT include TLS settings (which would cause containerd to try HTTPS)
/// and should NOT use overridePath (which would skip the /v2/ prefix).
///
/// This function is idempotent - it validates the current config and only
/// applies changes if needed. It also repairs any misconfiguration.
///
/// Returns the number of nodes that were repaired (0 if all configs were correct).
fn configure_talos_registry(registry_ip: &str) -> std::result::Result<usize, String> {
    let controlplane_ip = get_container_ip(&format!("{}-controlplane-1", CLUSTER_NAME));
    let worker_ip = get_container_ip(&format!("{}-worker-1", CLUSTER_NAME));
    let mut repaired_count = 0;

    for node_ip in [controlplane_ip, worker_ip].into_iter().flatten() {
        // Check if registry config needs repair
        if needs_registry_repair(&node_ip, registry_ip) {
            repair_registry_config(&node_ip, registry_ip)?;
            repaired_count += 1;
        }
    }

    Ok(repaired_count)
}

/// Check if the registry configuration on a node needs repair.
///
/// Returns true if:
/// - The machine config has TLS/skip_verify settings for the registry
/// - The machine config has overridePath setting for the registry
/// - There are duplicate endpoints in the machine config
/// - The registry endpoint is not configured at all
fn needs_registry_repair(node_ip: &str, registry_ip: &str) -> bool {
    // Get the machine config to check registry settings
    let full_output = match run_command_optional(
        "talosctl",
        &["--nodes", node_ip, "get", "machineconfig", "-o", "yaml"],
    ) {
        Some(output) => output,
        None => return true, // Can't read config, assume needs repair
    };

    // The output may contain multiple YAML documents (separated by ---).
    // We only need to check the first one (the actual config content).
    let config_output = full_output.split("\n---\n").next().unwrap_or(&full_output);

    let endpoint_pattern = format!("http://{}:5000", registry_ip);
    let registry_key = format!("{}:5000:", registry_ip);

    // Check if registry is configured at all
    if !config_output.contains(&registry_key) {
        return true; // Registry not configured
    }

    // Check for duplicate endpoints within the machine: section
    // (indicates accumulated patch operations)
    let count = config_output.matches(&endpoint_pattern).count();
    if count > 1 {
        return true; // Duplicate endpoints
    }

    // Check for problematic TLS settings in the registries section
    // Look for insecureSkipVerify or skip_verify in config: sections
    if config_output.contains("insecureSkipVerify") || config_output.contains("skip_verify") {
        // These could be in registries.config section - needs repair
        // Check if it's specifically in the registry mirror config
        if let Some(reg_section_start) = config_output.find(&registry_key) {
            // Check the 500 chars after the registry key for TLS settings
            let end = (reg_section_start + 500).min(config_output.len());
            let reg_section = &config_output[reg_section_start..end];
            if reg_section.contains("Skip") || reg_section.contains("skip_verify") {
                return true;
            }
        }
    }

    // Check for overridePath setting
    if config_output.contains("overridePath") {
        if let Some(reg_section_start) = config_output.find(&registry_key) {
            let end = (reg_section_start + 500).min(config_output.len());
            let reg_section = &config_output[reg_section_start..end];
            if reg_section.contains("overridePath") {
                return true;
            }
        }
    }

    // Config looks correct
    false
}

/// Configure the registry on a node using talosctl patch.
///
/// This applies a YAML patch to set the registry configuration, which is simpler
/// and more reliable than extracting and modifying the full machine config.
fn repair_registry_config(node_ip: &str, registry_ip: &str) -> std::result::Result<(), String> {
    // Create a YAML patch that sets the registry configuration
    let patch = format!(
        r#"machine:
  registries:
    mirrors:
      {}:5000:
        endpoints:
          - http://{}:5000"#,
        registry_ip, registry_ip
    );

    // Write patch to temp file
    let patch_file =
        std::env::temp_dir().join(format!("talos-patch-{}.yaml", node_ip.replace('.', "-")));
    fs::write(&patch_file, &patch).map_err(|e| e.to_string())?;

    // Apply the patch
    let result = run_command_optional(
        "talosctl",
        &[
            "--nodes",
            node_ip,
            "patch",
            "machineconfig",
            "--patch-file",
            patch_file.to_str().unwrap(),
            "--mode",
            "no-reboot",
        ],
    );

    fs::remove_file(&patch_file).ok();

    if result.is_some() {
        Ok(())
    } else {
        Err(format!("Failed to apply registry patch to {}", node_ip))
    }
}

/// Build and push container images.
fn build_and_push_images(_registry_ip: &str) -> std::result::Result<StepOutcome, String> {
    let components = [
        ("inferadb-engine", get_engine_dir()),
        ("inferadb-control", get_control_dir()),
        ("inferadb-dashboard", get_dashboard_dir()),
    ];

    // Check if at least one component directory exists
    let any_exists = components.iter().any(|(_, dir)| dir.exists());
    if !any_exists {
        return Ok(StepOutcome::Skipped(
            "no component repos cloned".to_string(),
        ));
    }

    let mut built_count = 0;
    for (name, dir) in &components {
        let dockerfile = dir.join("Dockerfile");
        if dockerfile.exists() {
            // For Rust projects, ensure Cargo.lock exists
            let cargo_toml = dir.join("Cargo.toml");
            let cargo_lock = dir.join("Cargo.lock");
            if cargo_toml.exists() && !cargo_lock.exists() {
                let _ = std::process::Command::new("cargo")
                    .args(["generate-lockfile"])
                    .current_dir(dir)
                    .output();
            }

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

/// Set up Kubernetes resources (namespaces, RBAC, etc.).
#[allow(clippy::unnecessary_wraps)]
fn setup_kubernetes_resources(_registry_ip: &str) -> std::result::Result<StepOutcome, String> {
    // Create namespaces
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

    // Label namespaces for privileged workloads
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
        let _ = run_command_optional(
            "kubectl",
            &[
                "label",
                "namespace",
                ns,
                "pod-security.kubernetes.io/warn=privileged",
                "--overwrite",
            ],
        );
    }

    // Install local-path-provisioner
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

    // Install CRDs
    for crd in &[
        "crd/bases/apps.foundationdb.org_foundationdbclusters.yaml",
        "crd/bases/apps.foundationdb.org_foundationdbbackups.yaml",
        "crd/bases/apps.foundationdb.org_foundationdbrestores.yaml",
    ] {
        run_command("kubectl", &["apply", "-f", &format!("{}/{}", fdb_url, crd)])
            .map_err(|e| e.to_string())?;
    }

    // Wait for CRDs
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

    // Install RBAC
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

    // Install operator deployment
    let manager_yaml = run_command(
        "curl",
        &["-s", &format!("{}/deployment/manager.yaml", fdb_url)],
    )
    .map_err(|e| e.to_string())?;
    let yaml_with_sa_fix = manager_yaml.replace(
        "serviceAccountName: fdb-kubernetes-operator-controller-manager",
        "serviceAccountName: controller-manager",
    );

    // Remove WATCH_NAMESPACE block
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

    // Create ClusterRoleBindings
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

    // Create FDB sidecar RBAC
    let sidecar_rbac = r#"apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: fdb-sidecar
  namespace: inferadb
rules:
- apiGroups: [""]
  resources: ["pods"]
  verbs: ["get", "list", "watch", "patch", "update"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: fdb-sidecar
  namespace: inferadb
subjects:
- kind: ServiceAccount
  name: default
  namespace: inferadb
roleRef:
  kind: Role
  name: fdb-sidecar
  apiGroup: rbac.authorization.k8s.io
"#;
    apply_yaml(sidecar_rbac)?;

    // Wait for FDB operator to be ready
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
    // Generate registry patch
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

    // Apply dev overlay
    run_command(
        "kubectl",
        &[
            "apply",
            "-k",
            deploy_dir.join("flux/apps/dev").to_str().unwrap(),
        ],
    )
    .map_err(|e| e.to_string())?;

    // Wait for ingress and get tailnet info
    std::thread::sleep(Duration::from_secs(10));

    let ingress_hostname = run_command_optional(
        "kubectl",
        &[
            "get",
            "ingress",
            "dev-inferadb-api-tailscale",
            "-n",
            "inferadb",
            "-o",
            "jsonpath={.status.loadBalancer.ingress[0].hostname}",
        ],
    );

    let tailnet_suffix = ingress_hostname
        .as_ref()
        .and_then(|h| h.strip_prefix("inferadb-dev-api."))
        .map(|s| s.to_string())
        .or_else(get_tailnet_info);

    Ok((StepOutcome::Success, tailnet_suffix))
}

/// Clean up Tailscale devices via API
fn cleanup_tailscale_devices() -> Result<()> {
    let (client_id, client_secret) = match load_tailscale_credentials() {
        Some(creds) => creds,
        None => {
            // No credentials available - skip silently (spinner will show result)
            return Ok(());
        }
    };

    // Get OAuth token
    let token_output = run_command_optional(
        "curl",
        &[
            "-s",
            "-X",
            "POST",
            "https://api.tailscale.com/api/v2/oauth/token",
            "-u",
            &format!("{}:{}", client_id, client_secret),
            "-d",
            "grant_type=client_credentials",
        ],
    );

    let access_token = token_output.as_ref().and_then(|s| {
        // Simple JSON extraction
        s.find("\"access_token\":\"").and_then(|start| {
            let rest = &s[start + 16..];
            rest.find('"').map(|end| rest[..end].to_string())
        })
    });

    let access_token = match access_token {
        Some(t) => t,
        None => {
            // Could not get token - skip silently
            return Ok(());
        }
    };

    // List devices
    let devices_output = run_command_optional(
        "curl",
        &[
            "-s",
            "-X",
            "GET",
            "https://api.tailscale.com/api/v2/tailnet/-/devices",
            "-H",
            &format!("Authorization: Bearer {}", access_token),
        ],
    );

    if devices_output.is_none() || !devices_output.as_ref().unwrap().contains("\"devices\"") {
        // No devices found or API unavailable - skip silently
        return Ok(());
    }

    let devices = devices_output.unwrap();

    // Find and delete all devices matching our prefix (inferadb-*) or tailscale-operator
    // Parse device entries from JSON response
    let mut search_pos = 0;
    while let Some(name_start) = devices[search_pos..].find("\"name\":\"") {
        let abs_name_start = search_pos + name_start + 8; // Skip past `"name":"`
        if let Some(name_end) = devices[abs_name_start..].find('"') {
            let device_name = &devices[abs_name_start..abs_name_start + name_end];

            // Check if this device should be cleaned up
            let should_delete = device_name.starts_with(TAILSCALE_DEVICE_PREFIX)
                || device_name.starts_with("tailscale-operator");

            if should_delete {
                // Look backwards from name to find the device ID
                let before = &devices[..search_pos + name_start];
                if let Some(id_start) = before.rfind("\"id\":\"") {
                    let id_rest = &before[id_start + 6..];
                    if let Some(id_end) = id_rest.find('"') {
                        let device_id = &id_rest[..id_end];

                        let _ = run_command_optional(
                            "curl",
                            &[
                                "-s",
                                "-X",
                                "DELETE",
                                &format!("https://api.tailscale.com/api/v2/device/{}", device_id),
                                "-H",
                                &format!("Authorization: Bearer {}", access_token),
                            ],
                        );
                    }
                }
            }

            search_pos = abs_name_start + name_end;
        } else {
            break;
        }
    }

    Ok(())
}

/// Run dev stop - pause or destroy local development cluster
///
/// By default, this pauses all cluster containers so they can be quickly resumed.
/// With `--destroy`, it completely removes the cluster and all related resources.
///
/// This function is idempotent - each container is checked individually and
/// shows OK/SKIPPED based on its current state.
pub async fn stop(
    ctx: &Context,
    destroy: bool,
    yes: bool,
    with_credentials: bool,
    interactive: bool,
) -> Result<()> {
    // If --destroy is passed, run the uninstall/destroy logic instead
    if destroy {
        if interactive && crate::tui::is_interactive(ctx) {
            return uninstall_interactive(with_credentials);
        }
        return uninstall_with_spinners(yes, with_credentials);
    }

    // Normal stop (pause) behavior - always show status for each component
    if interactive && crate::tui::is_interactive(ctx) {
        return stop_interactive();
    }

    stop_with_spinners()
}

/// Run stop with inline spinners for each step
///
/// This function is idempotent - handles already-paused and non-existent containers.
/// Always shows status for each expected component (controlplane, worker, registry).
fn stop_with_spinners() -> Result<()> {
    print_styled_header("Pausing InferaDB Development Cluster");
    println!();

    let mut any_paused = false;

    // Define expected containers (always show these, even if they don't exist)
    let expected_containers = vec![
        format!("{}-controlplane-1", CLUSTER_NAME),
        format!("{}-worker-1", CLUSTER_NAME),
    ];

    // Pause each expected container
    for container in &expected_containers {
        any_paused |= pause_container(container);
    }

    // Also pause any other cluster containers that might exist
    let actual_containers = get_cluster_containers();
    for container in &actual_containers {
        // Skip if already processed above
        if expected_containers.contains(container) {
            continue;
        }
        any_paused |= pause_container(container);
    }

    // Pause registry
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

/// Pause a single container, showing spinner and returning whether work was done.
///
/// Returns true if the container was paused (or was already paused).
/// Returns false if the container doesn't exist.
fn pause_container(container: &str) -> bool {
    use crate::tui::start_spinner;

    let display_name = container
        .strip_prefix(&format!("{}-", CLUSTER_NAME))
        .unwrap_or(container);
    let in_progress = format!("Pausing {}", display_name);
    let completed = format!("Paused {}", display_name);
    let mut spin = start_spinner(&in_progress);

    // Check if container exists
    if !docker_container_exists(container) {
        spin.stop();
        print_destroy_skipped(&completed);
        return false;
    }

    // Check if container is already paused
    if is_container_paused(container) {
        spin.stop();
        print_destroy_skipped(&completed);
        return false;
    }

    // Try to pause
    match run_command("docker", &["pause", container]) {
        Ok(_) => {
            spin.success(&format_dot_leader(&completed, "OK"));
            true
        }
        Err(e) => {
            let err_str = e.to_string();
            // Already paused or container not found - treat as skip
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

/// Check if a specific container is paused
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

/// Run stop in interactive TUI mode
fn stop_interactive() -> Result<()> {
    use crate::tui::{DevStopView, InstallStep};
    use ferment::runtime::{Program, ProgramOptions};

    let mut steps = Vec::new();

    // Add a step for each cluster container
    let containers = get_cluster_containers();
    for container in containers {
        let container_name = container.clone();
        steps.push(InstallStep::with_executor(
            format!("Pausing {}", container),
            move || {
                run_command("docker", &["pause", &container_name])
                    .map(|_| None)
                    .or_else(|e| {
                        if e.to_string().contains("already paused") {
                            Ok(None)
                        } else {
                            Err(e.to_string())
                        }
                    })
            },
        ));
    }

    // Pause registry
    if docker_container_exists(REGISTRY_NAME) {
        steps.push(InstallStep::with_executor(
            format!("Pausing {}", REGISTRY_NAME),
            || {
                let _ = run_command_optional("docker", &["pause", REGISTRY_NAME]);
                Ok(None)
            },
        ));
    }

    let view = DevStopView::new(steps).subtitle("Stop");

    Program::new(view)
        .with_options(ProgramOptions::fullscreen())
        .run()
        .map_err(|e| Error::Other(e.to_string()))?;

    Ok(())
}

// ============================================================================
// Helper functions
// ============================================================================

/// Clean up stale talosctl and kubectl contexts
fn cleanup_stale_contexts() {
    // Clean up talosctl contexts
    if let Some(contexts) = run_command_optional("talosctl", &["config", "contexts"]) {
        for line in contexts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1].starts_with(CLUSTER_NAME) {
                let _ = run_command_optional("talosctl", &["config", "context", ""]);
                let _ = run_command_optional(
                    "talosctl",
                    &["config", "remove", parts[1], "--noconfirm"],
                );
            }
        }
    }

    // Clean up kubectl context
    if let Some(contexts) =
        run_command_optional("kubectl", &["config", "get-contexts", "-o", "name"])
    {
        if contexts.lines().any(|l| l == KUBE_CONTEXT) {
            let _ = run_command_optional("kubectl", &["config", "delete-context", KUBE_CONTEXT]);
            let _ = run_command_optional("kubectl", &["config", "delete-cluster", CLUSTER_NAME]);
            let _ = run_command_optional("kubectl", &["config", "delete-user", KUBE_CONTEXT]);
        }
    }
}

// =============================================================================
// Status Helpers
// =============================================================================

/// Get the current cluster status.
fn get_cluster_status() -> ClusterStatus {
    if !docker_container_exists(CLUSTER_NAME) {
        ClusterStatus::Offline
    } else if are_containers_paused() {
        ClusterStatus::Paused
    } else {
        ClusterStatus::Online
    }
}

/// Format a column header to be human-friendly.
/// Converts "NAME" → "Name", "CLUSTER-IP" → "Cluster IP", "EXTERNAL-IP" → "External IP"
/// Preserves common acronyms like IP, CPU, OS, etc.
fn format_header(header: &str) -> String {
    // Common acronyms that should stay uppercase
    const ACRONYMS: &[&str] = &["IP", "CPU", "OS", "ID", "URL", "API", "FDB", "URI"];

    header
        .split('-')
        .map(|word| {
            let upper = word.to_uppercase();
            if ACRONYMS.contains(&upper.as_str()) {
                upper
            } else {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse kubectl get output into TabData.
fn parse_kubectl_output(output: &str) -> TabData {
    let lines: Vec<&str> = output.lines().collect();
    if lines.is_empty() {
        return TabData::default();
    }

    // First line is headers - format them to be human-friendly
    let headers: Vec<String> = lines[0].split_whitespace().map(format_header).collect();

    // Parse data rows
    let rows: Vec<TableRow> = lines
        .iter()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let cells: Vec<String> = line.split_whitespace().map(String::from).collect();
            TableRow::new(cells)
        })
        .collect();

    TabData::new(headers, rows)
}

/// Parse kubectl get ingress output into TabData with URLs.
fn parse_ingress_data(output: &str) -> TabData {
    let lines: Vec<&str> = output.lines().collect();
    if lines.is_empty() {
        return TabData::new(vec!["Name".to_string(), "URL".to_string()], vec![]);
    }

    // Find header indices - ADDRESS contains the actual hostname
    let headers_line = lines[0];
    let headers: Vec<&str> = headers_line.split_whitespace().collect();
    let name_idx = headers.iter().position(|h| *h == "NAME").unwrap_or(0);
    let address_idx = headers.iter().position(|h| *h == "ADDRESS").unwrap_or(3);

    let rows: Vec<TableRow> = lines
        .iter()
        .skip(1)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > address_idx {
                let name = parts.get(name_idx).unwrap_or(&"").to_string();
                let address = parts.get(address_idx).unwrap_or(&"").to_string();
                if !address.is_empty() && address != "<none>" {
                    Some(TableRow::new(vec![name, format!("https://{}", address)]))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    TabData::new(vec!["Name".to_string(), "URL".to_string()], rows)
}

/// Format memory size from Ki to human readable.
fn format_memory(ki_str: &str) -> String {
    if let Ok(ki) = ki_str.trim_end_matches("Ki").parse::<u64>() {
        let gi = ki as f64 / (1024.0 * 1024.0);
        format!("{:.1}Gi", gi)
    } else {
        ki_str.to_string()
    }
}

/// Fetch nodes with capacity information (CPU/memory).
fn fetch_nodes_with_capacity() -> TabData {
    // Get basic node info
    let nodes_output = match run_command_optional("kubectl", &["get", "nodes"]) {
        Some(out) => out,
        None => return TabData::default(),
    };

    // Get capacity info via jsonpath
    let capacity_output = run_command_optional(
        "kubectl",
        &[
            "get",
            "nodes",
            "-o",
            "jsonpath={range .items[*]}{.metadata.name}\\t{.status.capacity.cpu}\\t{.status.capacity.memory}\\n{end}",
        ],
    );

    // Build capacity map
    let mut capacity_map: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    if let Some(cap) = capacity_output {
        for line in cap.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let name = parts[0].to_string();
                let cpu = parts[1].to_string();
                let mem = format_memory(parts[2]);
                capacity_map.insert(name, (cpu, mem));
            }
        }
    }

    // Parse basic node output and add capacity columns
    let lines: Vec<&str> = nodes_output.lines().collect();
    if lines.is_empty() {
        return TabData::default();
    }

    // Build headers with CPU and Memory added - format to be human-friendly
    let mut headers: Vec<String> = lines[0].split_whitespace().map(format_header).collect();
    headers.push("CPU".to_string());
    headers.push("Memory".to_string());

    // Build rows with capacity info
    let rows: Vec<TableRow> = lines
        .iter()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut cells: Vec<String> = line.split_whitespace().map(String::from).collect();
            let name = cells.first().cloned().unwrap_or_default();
            if let Some((cpu, mem)) = capacity_map.get(&name) {
                cells.push(cpu.clone());
                cells.push(mem.clone());
            } else {
                cells.push("-".to_string());
                cells.push("-".to_string());
            }
            TableRow::new(cells)
        })
        .collect();

    TabData::new(headers, rows)
}

/// Fetch all status data for the status view.
fn fetch_status_data() -> RefreshResult {
    let cluster_status = get_cluster_status();

    // URLs from ingress
    let urls = run_command_optional("kubectl", &["get", "ingress", "-n", "inferadb"])
        .map(|out| parse_ingress_data(&out))
        .unwrap_or_default();

    // Services
    let services = run_command_optional(
        "kubectl",
        &["get", "services", "-n", "inferadb", "-o", "wide"],
    )
    .map(|out| parse_kubectl_output(&out))
    .unwrap_or_default();

    // Nodes with capacity info
    let nodes = fetch_nodes_with_capacity();

    // Pods (InferaDB + FDB) - parse separately and merge rows
    let inferadb_pods =
        run_command_optional("kubectl", &["get", "pods", "-n", "inferadb", "-o", "wide"])
            .map(|out| parse_kubectl_output(&out))
            .unwrap_or_default();
    let fdb_pods = run_command_optional(
        "kubectl",
        &["get", "pods", "-n", "fdb-system", "-o", "wide"],
    )
    .map(|out| parse_kubectl_output(&out))
    .unwrap_or_default();

    // Merge pods - use inferadb headers, combine rows from both
    let pods = TabData::new(
        if inferadb_pods.headers.is_empty() {
            fdb_pods.headers
        } else {
            inferadb_pods.headers
        },
        inferadb_pods
            .rows
            .into_iter()
            .chain(fdb_pods.rows)
            .collect(),
    );

    RefreshResult {
        cluster_status,
        urls,
        services,
        nodes,
        pods,
    }
}

/// Run dev status - show cluster status
pub async fn dev_status(ctx: &Context, interactive: bool) -> Result<()> {
    // Use full-screen TUI if explicitly requested and available
    if interactive && crate::tui::is_interactive(ctx) {
        return status_interactive();
    }

    // Default: Use spinners and streaming output
    status_with_spinners()
}

/// Run status with inline spinners for each section.
/// Print a section header with dimmed circle prefix.
fn print_section_header(title: &str) {
    use ferment::style::Color;
    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";
    println!("\n{}○{} {}\n", dim, reset, title);
}

fn status_with_spinners() -> Result<()> {
    use ferment::style::Color;

    print_styled_header("InferaDB Development Cluster Status");
    println!();

    let green = Color::Green.to_ansi_fg();
    let yellow = Color::Yellow.to_ansi_fg();
    let red = Color::Red.to_ansi_fg();
    let reset = "\x1b[0m";

    // Check cluster status
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

    // Check kubectl context
    if let Some(current) = run_command_optional("kubectl", &["config", "current-context"]) {
        let context = current.trim();
        print_prefixed_dot_leader("○", "kubectl context", context);
    } else {
        print_prefixed_dot_leader("○", "kubectl context", "NOT CONFIGURED");
    }

    // Nodes
    print_section_header("Nodes");
    print_nodes_status();

    // Pods (combined InferaDB + FDB)
    print_section_header("Pods");
    print_pods_status();

    // URLs
    print_section_header("URLs");
    print_urls_status();

    Ok(())
}

/// Print formatted node status.
fn print_nodes_status() {
    use ferment::style::Color;

    // Get node info as JSON for more reliable parsing
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

                    // Check Ready condition
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

                    // Use simpler name without cluster prefix
                    let display_name = name.strip_prefix("inferadb-dev-").unwrap_or(name);
                    print_prefixed_dot_leader(" ", display_name, &status);
                }
            }
        }
    }
}

/// Print formatted pod status.
fn print_pods_status() {
    use ferment::style::Color;

    let green = Color::Green.to_ansi_fg();
    let yellow = Color::Yellow.to_ansi_fg();
    let red = Color::Red.to_ansi_fg();
    let reset = "\x1b[0m";

    // Get InferaDB pods
    let inferadb_pods = run_command_optional(
        "kubectl",
        &["get", "pods", "-n", "inferadb", "-o", "jsonpath={range .items[*]}{.metadata.name}|{.status.phase}|{.status.containerStatuses[*].ready}{\"\\n\"}{end}"],
    );

    // Get FDB pods
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

            // Skip completed pods early
            if phase == "Succeeded" || phase == "Completed" {
                return None;
            }

            // Count ready containers
            let ready_count = ready_statuses
                .split_whitespace()
                .filter(|s| *s == "true")
                .count();
            let total_count = ready_statuses.split_whitespace().count().max(1);

            // Build status string
            let status = match phase {
                "Running" => format!("{}{}/{} Running{}", green, ready_count, total_count, reset),
                "Pending" => format!("{}{}/{} Pending{}", yellow, ready_count, total_count, reset),
                _ => format!("{}{}/{} {}{}", red, ready_count, total_count, phase, reset),
            };

            // Simplify pod names by removing hash/id suffixes
            let display_name = if name.starts_with("controller-manager-") {
                "fdb-operator".to_string()
            } else {
                // Strip dev-inferadb- prefix first (or inferadb- for backwards compat)
                let base = name
                    .strip_prefix("dev-inferadb-")
                    .or_else(|| name.strip_prefix("inferadb-"))
                    .unwrap_or(name);

                // Split into parts
                let segments: Vec<&str> = base.split('-').collect();
                let len = segments.len();

                // Detect K8s Deployment pod pattern: <name>-<replicaset-hash>-<pod-hash>
                // Replicaset hash: 9-10 alphanumeric chars with digits
                // Pod hash: 5 alphanumeric chars (may be all letters)
                if len >= 3 {
                    let last = segments[len - 1];
                    let second_last = segments[len - 2];
                    let is_pod_hash = last.len() == 5 && last.chars().all(|c| c.is_alphanumeric());
                    let is_rs_hash = second_last.len() >= 9
                        && second_last.len() <= 10
                        && second_last.chars().any(|c| c.is_ascii_digit())
                        && second_last.chars().all(|c| c.is_alphanumeric());

                    if is_pod_hash && is_rs_hash {
                        let name = segments[..len - 2].join("-");
                        if !name.is_empty() {
                            return Some((name, status));
                        }
                    }
                }

                // Identify suffix segments to strip:
                // - Numeric suffixes (FDB pod IDs like "583", "36529")
                // - Alphanumeric hashes with digits (Deployment hashes like "59b5db5b77")
                let mut meaningful_end = segments.len();
                for (i, seg) in segments.iter().enumerate().rev() {
                    let is_numeric = seg.chars().all(|c| c.is_ascii_digit());
                    let has_digit = seg.chars().any(|c| c.is_ascii_digit());
                    let is_hash =
                        seg.len() >= 4 && has_digit && seg.chars().all(|c| c.is_alphanumeric());
                    if is_numeric || is_hash {
                        meaningful_end = i;
                    } else {
                        break;
                    }
                }

                if meaningful_end == 0 {
                    base.to_string()
                } else {
                    segments[..meaningful_end].join("-")
                }
            };

            // Skip if display name is empty
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

                // Map ingress names to friendly labels
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

/// Run status in full-screen interactive TUI mode.
fn status_interactive() -> Result<()> {
    use crate::tui::DevStatusView;
    use ferment::output::{terminal_height, terminal_width};
    use ferment::runtime::{Program, ProgramOptions};

    let width = terminal_width();
    let height = terminal_height();

    // Get initial data
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

/// Run dev logs - view logs
pub async fn logs(_ctx: &Context, follow: bool, service: Option<&str>, tail: u32) -> Result<()> {
    // Check if cluster is running
    if !docker_container_exists(CLUSTER_NAME) {
        return Err(Error::Other(
            "Cluster is not running. Start with 'inferadb dev start'.".to_string(),
        ));
    }

    let mut args = vec!["logs", "-n", "inferadb"];

    // Determine which pods to show logs from
    let (selector, service_name) = match service {
        Some("engine") => ("app.kubernetes.io/name=inferadb-engine", "engine"),
        Some("control") => ("app.kubernetes.io/name=inferadb-control", "control"),
        Some("dashboard") => ("app.kubernetes.io/name=inferadb-dashboard", "dashboard"),
        Some("fdb") => {
            args[1] = "fdb-system";
            ("app=fdb-kubernetes-operator", "fdb")
        }
        Some("mailpit") => ("app.kubernetes.io/name=mailpit", "mailpit"),
        Some(other) => {
            return Err(Error::Other(format!(
                "Unknown service: {}. Valid: engine, control, dashboard, fdb, mailpit",
                other
            )));
        }
        None => (
            "app.kubernetes.io/name in (inferadb-engine,inferadb-control,inferadb-dashboard)",
            "all",
        ),
    };

    args.push("-l");
    args.push(selector);

    args.push("--tail");
    let tail_str = tail.to_string();
    args.push(&tail_str);

    if follow {
        args.push("-f");
    }

    // Add prefix when viewing multiple services
    if service.is_none() {
        args.push("--prefix");
    }

    // Print header
    print_styled_header("Development Cluster Logs");
    println!();

    let mode = if follow { "following" } else { "tail" };
    print_prefixed_dot_leader("○", "Service", service_name);
    print_prefixed_dot_leader("○", "Mode", mode);
    if !follow {
        print_prefixed_dot_leader("○", "Lines", &tail_str);
    }
    println!();

    // Build kubectl command and pipe through grep to filter health checks
    // Quote args that contain special shell characters
    let quoted_args: Vec<String> = args
        .iter()
        .map(|arg| {
            if arg.contains('(') || arg.contains(' ') {
                format!("'{}'", arg)
            } else {
                (*arg).to_string()
            }
        })
        .collect();
    let kubectl_cmd = format!("kubectl {}", quoted_args.join(" "));
    let filter_cmd = format!(
        "{} 2>/dev/null | grep -v --line-buffered -E '/livez|/readyz'",
        kubectl_cmd
    );

    run_command_streaming("sh", &["-c", &filter_cmd], &[])?;

    Ok(())
}

/// Run dev dashboard - open web dashboard
pub async fn dashboard(_ctx: &Context) -> Result<()> {
    // Check if cluster is running
    if !docker_container_exists(CLUSTER_NAME) {
        return Err(Error::Other(
            "Cluster is not running. Start with 'inferadb dev start'.".to_string(),
        ));
    }

    // Get ingress URL
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

    let url = match hostname {
        Some(h) if !h.is_empty() => format!("https://{}", h.trim()),
        _ => {
            println!("Dashboard ingress not ready yet.");
            println!("Check status with: kubectl get ingress -n inferadb");
            return Ok(());
        }
    };

    println!("Opening dashboard: {}", url);

    // Open in browser
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
        let _ = Command::new("cmd").args(&["/C", "start", &url]).spawn();
    }

    Ok(())
}

/// Run dev reset - reset all data
pub async fn reset(_ctx: &Context, yes: bool) -> Result<()> {
    // Check if cluster is running
    if !docker_container_exists(CLUSTER_NAME) {
        return Err(Error::Other(
            "Cluster is not running. Start with 'inferadb dev start'.".to_string(),
        ));
    }

    // Gather information about what will be deleted
    let deploy_dir = get_deploy_dir();
    let can_redeploy = deploy_dir.exists();

    // Show dry run only if not skipping confirmation
    if !yes {
        let fdb_clusters = get_fdb_clusters_for_reset();
        let deployments = get_deployments_for_reset();
        let pvcs = get_pvcs_for_reset();

        print_styled_header("Reset InferaDB Development Cluster");

        // Section: What will be deleted
        print_section_header("Resources to be deleted");

        // FDB Clusters
        if fdb_clusters.is_empty() {
            print_prefixed_dot_leader("○", "FoundationDB Cluster", "none found");
        } else {
            for (name, processes, version) in &fdb_clusters {
                let detail = format!("{} ({}, {})", name, processes, version);
                print_prefixed_dot_leader("○", "FoundationDB Cluster", &detail);
            }
        }

        // Deployments
        if deployments.is_empty() {
            print_prefixed_dot_leader("○", "Deployment", "none found");
        } else {
            for (name, replicas, image) in &deployments {
                // Shorten the name for display
                let short_name = name
                    .strip_prefix("dev-inferadb-")
                    .or_else(|| name.strip_prefix("inferadb-"))
                    .unwrap_or(name);
                let detail = format!("{} replica(s), {}", replicas, image);
                print_prefixed_dot_leader("○", &format!("Deployment: {}", short_name), &detail);
            }
        }

        // PVCs
        if pvcs.is_empty() {
            print_prefixed_dot_leader("○", "Persistent Volume", "none found");
        } else {
            for (name, size, status) in &pvcs {
                // Shorten FDB PVC names for display
                let short_name = if name.starts_with("dev-inferadb-fdb-") {
                    let suffix = name.strip_prefix("dev-inferadb-fdb-").unwrap_or(name);
                    // Further simplify: "log-36529-data" -> "log", "storage-3983-data" -> "storage"
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

        // Section: What will be recreated
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

        use ferment::style::Color;
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

    use crate::tui::start_spinner;

    print_styled_header("Resetting InferaDB Development Cluster");
    println!();

    // Delete FDB cluster (will recreate with empty data)
    {
        let spin = start_spinner("Deleting FoundationDB Cluster");
        let _ = run_command_optional(
            "kubectl",
            &["delete", "foundationdbcluster", "--all", "-n", "inferadb"],
        );
        spin.success(&format_dot_leader("Deleted FoundationDB Cluster", "OK"));
    }

    // Delete InferaDB deployments
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

    // Delete PVCs
    {
        let spin = start_spinner("Deleting Persistent Volumes");
        let _ = run_command_optional("kubectl", &["delete", "pvc", "--all", "-n", "inferadb"]);
        spin.success(&format_dot_leader("Deleted Persistent Volumes", "OK"));
    }

    // Wait a moment
    {
        let spin = start_spinner("Waiting for resources to terminate");
        std::thread::sleep(Duration::from_secs(5));
        spin.success(&format_dot_leader("Resources terminated", "OK"));
    }

    // Reapply the dev overlay
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
                    // Padding before prefix, not before text
                    println!("  {}", format_reset_dot_leader(prefix, &resource, &status));
                }
            }
        }

        // Wait for FDB cluster to be ready (generates new cluster file ConfigMap)
        println!();
        {
            let spin = start_spinner("Waiting for FoundationDB cluster");
            let mut ready = false;
            for _ in 0..150 {
                // Wait up to 5 minutes (150 * 2s)
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

        // Restart engine deployment to pick up new FDB cluster file ConfigMap
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

    use ferment::style::Color;
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
