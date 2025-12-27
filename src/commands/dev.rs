//! Local development environment commands.
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
use ferment::output::{
    error as print_error, header as print_header, info as print_info, phase as print_phase,
    success as print_success, warning as print_warning,
};

// Constants
const CLUSTER_NAME: &str = "inferadb-dev";
const KUBE_CONTEXT: &str = "admin@inferadb-dev";
const REGISTRY_NAME: &str = "inferadb-registry";
const REGISTRY_PORT: u16 = 5050;
const DEPLOY_REPO_URL: &str = "https://github.com/inferadb/deploy.git";
/// Prefix for Tailscale devices created by dev environment ingress resources
const TAILSCALE_DEVICE_PREFIX: &str = "inferadb-dev-";

// Tip messages
const TIP_START_CLUSTER: &str = "Run 'inferadb dev start' to start the cluster";
const TIP_RESUME_CLUSTER: &str = "Run 'inferadb dev start' to resume the cluster";

/// Get the deploy directory path (~/.local/share/inferadb/deploy)
fn get_deploy_dir() -> PathBuf {
    Config::data_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share/inferadb"))
        .join("deploy")
}

/// Get the Tailscale credentials file path
fn get_tailscale_creds_file() -> PathBuf {
    Config::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config/inferadb"))
        .join("tailscale-credentials")
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

/// Extract a meaningful version string from command output
fn extract_version_string(output: &str, command: &str) -> String {
    // Handle command-specific output formats
    match command {
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
    }
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

/// Pause all cluster containers
fn pause_cluster_containers() -> Result<()> {
    let containers = get_cluster_containers();
    if containers.is_empty() {
        return Ok(());
    }

    println!("Pausing {} container(s)...", containers.len());
    for container in &containers {
        if let Err(e) = run_command("docker", &["pause", container]) {
            // Ignore errors for already paused containers
            if !e.to_string().contains("already paused") {
                eprintln!("  Warning: Failed to pause {}: {}", container, e);
            }
        } else {
            println!("  Paused: {}", container);
        }
    }
    Ok(())
}

/// Unpause all cluster containers
fn unpause_cluster_containers() -> Result<()> {
    let containers = get_cluster_containers();
    if containers.is_empty() {
        return Ok(());
    }

    println!("Resuming {} container(s)...", containers.len());
    for container in &containers {
        if let Err(e) = run_command("docker", &["unpause", container]) {
            // Ignore errors for containers that aren't paused
            if !e.to_string().contains("not paused") {
                eprintln!("  Warning: Failed to unpause {}: {}", container, e);
            }
        } else {
            println!("  Resumed: {}", container);
        }
    }
    Ok(())
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

/// Print a formatted header for non-interactive mode with dimmed slashes.
fn print_styled_header(title: &str) {
    use ferment::style::Color;
    let dim = Color::BrightBlack.to_ansi_fg();
    let reset = "\x1b[0m";
    println!("\n{}//{}  {}  {}//{}", dim, reset, title, dim, reset);
    println!();
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
            CheckResult::success("Services", "Docker daemon", "running"),
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
                Some(CheckResult::success("Services", "Tailscale", "connected"))
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
        CheckResult::success("Configuration", "Tailscale OAuth", "credentials cached")
    } else {
        CheckResult::optional(
            "Configuration",
            "Tailscale OAuth",
            "will be prompted during dev start",
        )
    }
}

/// Check deploy repository status.
fn check_deploy_repository() -> CheckResult {
    let deploy_dir = get_deploy_dir();
    if deploy_dir.exists() {
        CheckResult::success(
            "Configuration",
            "Deployment",
            deploy_dir.display().to_string(),
        )
    } else {
        CheckResult::optional("Configuration", "Deployment", "pending install")
    }
}

/// Extract the detail from a CheckResult status string.
fn extract_status_detail(status: &str) -> &str {
    status
        .trim_start_matches("✓ ")
        .trim_start_matches("✗ ")
        .trim_start_matches("○ ")
}

/// Format a check result with component name for spinner output.
fn format_check_output(component: &str, detail: &str) -> String {
    format!("{}: {}", component, detail)
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

    // Check deploy repository
    results.push(check_deploy_repository());

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

    // Check dependencies
    for dep in DEPENDENCIES {
        let spin = start_spinner(format!("Check {}", dep.name));
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

    // Check Docker daemon
    if command_exists("docker") {
        let spin = start_spinner("Check Docker daemon");
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
            spin.success("Docker daemon: skipped");
        }
    }

    // Check Tailscale connection (optional)
    if command_exists("tailscale") {
        let spin = start_spinner("Check Tailscale");
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
            spin.warning("Tailscale: could not check status");
        }
    }

    // Check Tailscale credentials
    {
        let spin = start_spinner("Check Tailscale OAuth");
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

    // Check deploy repository
    {
        let spin = start_spinner("Check deployment repository");
        let result = check_deploy_repository();
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

    // Print overall status
    println!();
    if all_required_ok {
        print_success("Environment ready");
        println!();
        print_info("Run 'inferadb dev start' to start the development cluster");
        Ok(())
    } else {
        print_error("Environment not ready - missing required dependencies");
        Err(Error::Other("Missing required dependencies".to_string()))
    }
}

/// Run doctor with full-screen TUI.
fn doctor_interactive() -> Result<()> {
    use crate::tui::DoctorView;
    use ferment::output::{terminal_height, terminal_width};
    use ferment::runtime::{Program, ProgramOptions};

    let width = terminal_width();
    let height = terminal_height();

    let (results, status) = run_all_checks();

    let view = DoctorView::new(width, height)
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

/// Step: Clone the deployment repository.
fn step_clone_repo(
    deploy_dir: &std::path::Path,
    force: bool,
    commit: Option<&str>,
) -> std::result::Result<Option<String>, String> {
    if deploy_dir.exists() {
        if force {
            fs::remove_dir_all(deploy_dir)
                .map_err(|e| format!("Failed to remove {}: {}", deploy_dir.display(), e))?;
        } else {
            return Ok(Some(format!(
                "{} (already installed)",
                deploy_dir.display()
            )));
        }
    }

    if let Some(parent) = deploy_dir.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    let clone_ok = if commit.is_some() {
        run_command_optional(
            "git",
            &[
                "clone",
                "--quiet",
                DEPLOY_REPO_URL,
                deploy_dir.to_str().unwrap(),
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
                "--quiet",
                DEPLOY_REPO_URL,
                deploy_dir.to_str().unwrap(),
            ],
        )
        .is_some()
    };

    if !clone_ok {
        return Err("Failed to clone repository".to_string());
    }

    if let Some(ref_spec) = commit {
        if run_command_optional(
            "git",
            &["-C", deploy_dir.to_str().unwrap(), "checkout", ref_spec],
        )
        .is_none()
        {
            return Err(format!("Failed to checkout '{}'", ref_spec));
        }
    }

    let _ = run_command_optional(
        "git",
        &[
            "-C",
            deploy_dir.to_str().unwrap(),
            "submodule",
            "update",
            "--init",
            "--recursive",
            "--quiet",
        ],
    );

    Ok(Some(deploy_dir.display().to_string()))
}

/// Step: Create the configuration directory.
fn step_create_config_dir() -> std::result::Result<Option<String>, String> {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".config"))
        .join("inferadb");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    Ok(Some(config_dir.display().to_string()))
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

/// Step: Pull Docker images.
fn step_pull_docker_images() -> std::result::Result<Option<String>, String> {
    if !command_exists("docker") || run_command_optional("docker", &["info"]).is_none() {
        return Ok(Some("Docker not available, skipping".to_string()));
    }

    if run_command_optional("docker", &["pull", "-q", "registry:2"]).is_some() {
        Ok(Some("Registry image pulled".to_string()))
    } else {
        Ok(Some("Registry (will pull during dev start)".to_string()))
    }
}

/// Step: Validate the installation.
fn step_validate_installation(
    deploy_dir: &std::path::Path,
) -> std::result::Result<Option<String>, String> {
    let validations = [
        ("flux/apps/dev", "Flux development overlay"),
        ("flux/apps/base", "Flux base manifests"),
        ("scripts", "Deployment scripts"),
    ];

    let mut missing = Vec::new();
    for (path, description) in validations {
        if !deploy_dir.join(path).exists() {
            missing.push(description);
        }
    }

    if missing.is_empty() {
        Ok(Some("All components verified".to_string()))
    } else {
        Err(format!("Missing: {}", missing.join(", ")))
    }
}

/// Run dev install - clone deploy repository and set up dependencies
pub async fn install(
    ctx: &Context,
    force: bool,
    commit: Option<&str>,
    interactive: bool,
) -> Result<()> {
    if interactive && crate::tui::is_interactive(ctx) {
        return install_interactive(force, commit);
    }

    install_with_spinners(force, commit)
}

/// Run install with inline spinners for each step
fn install_with_spinners(force: bool, commit: Option<&str>) -> Result<()> {
    use crate::tui::start_spinner;

    let deploy_dir = get_deploy_dir();

    print_styled_header("InferaDB Development Cluster Setup");

    // Step 1: Clone deployment repository
    let spin = start_spinner("Clone deployment repository");
    match step_clone_repo(&deploy_dir, force, commit) {
        Ok(Some(msg)) => spin.success(&msg),
        Ok(None) => spin.success(&deploy_dir.display().to_string()),
        Err(e) => {
            spin.error(&e);
            return Err(Error::Other(e));
        }
    }

    // Step 2: Create config directory
    let spin = start_spinner("Create configuration directory");
    match step_create_config_dir() {
        Ok(Some(msg)) => spin.success(&msg),
        Ok(None) => spin.success("Done"),
        Err(e) => {
            spin.error(&e);
            return Err(Error::Other(e));
        }
    }

    // Step 3: Set up Helm repositories
    let spin = start_spinner("Set up Helm repositories");
    match step_setup_helm() {
        Ok(Some(msg)) if msg.contains("skipping") => spin.info(&msg),
        Ok(Some(msg)) => spin.success(&msg),
        Ok(None) => spin.success("Done"),
        Err(e) => {
            spin.error(&e);
            return Err(Error::Other(e));
        }
    }

    // Step 4: Pre-pull Docker images
    let spin = start_spinner("Pull Docker images");
    match step_pull_docker_images() {
        Ok(Some(msg)) if msg.contains("skipping") || msg.contains("will pull") => spin.info(&msg),
        Ok(Some(msg)) => spin.success(&msg),
        Ok(None) => spin.success("Done"),
        Err(e) => {
            spin.error(&e);
            return Err(Error::Other(e));
        }
    }

    // Step 5: Validate installation
    let spin = start_spinner("Validate installation");
    let validation_result = step_validate_installation(&deploy_dir);
    let all_valid = validation_result.is_ok();
    match validation_result {
        Ok(Some(msg)) => spin.success(&msg),
        Ok(None) => spin.success("Done"),
        Err(e) => spin.error(&e),
    }

    // Summary
    println!();
    if all_valid {
        print_success("Installation complete");
        println!();
        print_info("Run 'inferadb dev start' to start the development cluster");
    } else {
        print_warning("Installation complete - with warnings");
        println!();
        print_info("Some files missing. Try: inferadb dev install --force");
    }

    Ok(())
}

/// Run interactive install with TUI
fn install_interactive(force: bool, commit: Option<&str>) -> Result<()> {
    use crate::tui::{InstallStep, InstallView};
    use ferment::runtime::{Program, ProgramOptions};

    let deploy_dir = get_deploy_dir();
    let commit_owned = commit.map(|s| s.to_string());

    let steps = vec![
        InstallStep::with_executor("Clone deployment repository", {
            let deploy_dir = deploy_dir.clone();
            let commit = commit_owned.clone();
            move || step_clone_repo(&deploy_dir, force, commit.as_deref())
        }),
        InstallStep::with_executor("Create configuration directory", step_create_config_dir),
        InstallStep::with_executor("Set up Helm repositories", step_setup_helm),
        InstallStep::with_executor("Pull Docker images", step_pull_docker_images),
        InstallStep::with_executor("Validate installation", {
            let deploy_dir = deploy_dir.clone();
            move || step_validate_installation(&deploy_dir)
        }),
    ];

    let view = InstallView::new(steps);

    Program::new(view)
        .with_options(ProgramOptions::fullscreen())
        .run()
        .map_err(|e| Error::Other(e.to_string()))?;

    Ok(())
}

/// Ensure deploy repository is cloned
fn ensure_deploy_repo() -> Result<PathBuf> {
    let deploy_dir = get_deploy_dir();
    if !deploy_dir.exists() {
        println!("Deploy repository not found. Cloning...");
        println!();

        // Ensure parent directory exists
        if let Some(parent) = deploy_dir.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                Error::Other(format!(
                    "Failed to create directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        run_command_streaming(
            "git",
            &[
                "clone",
                "--depth",
                "1",
                DEPLOY_REPO_URL,
                deploy_dir.to_str().unwrap(),
            ],
            &[],
        )?;

        println!();
        println!("✓ Deploy repository cloned successfully!");
        println!();
    }
    Ok(deploy_dir)
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
        println!("Using cached Tailscale credentials");
        return Ok((id, secret));
    }

    // Prompt for credentials
    println!("Tailscale OAuth credentials required for Kubernetes operator.\n");
    println!("=== Setup Instructions ===\n");
    println!("Step 1: Enable HTTPS on your tailnet (one-time setup)");
    println!("  Go to: https://login.tailscale.com/admin/dns");
    println!("  Scroll to 'HTTPS Certificates' and click 'Enable HTTPS'\n");
    println!("Step 2: Create tags (one-time setup)");
    println!("  Go to: https://login.tailscale.com/admin/acls/tags");
    println!("  Create tag 'k8s-operator' with yourself as owner");
    println!("  Create tag 'k8s' with 'tag:k8s-operator' as owner\n");
    println!("Step 3: Create OAuth client");
    println!("  Go to: https://login.tailscale.com/admin/settings/oauth");
    println!("  Click 'Generate OAuth client'");
    println!("  Add scopes:");
    println!("    - Devices → Core: Read & Write, tag: k8s-operator");
    println!("    - Keys → Auth Keys: Read & Write, tag: k8s-operator");
    println!("  Click 'Generate client' and copy the credentials\n");

    print!("Client ID: ");
    io::stdout().flush().unwrap();
    let mut client_id = String::new();
    io::stdin()
        .read_line(&mut client_id)
        .map_err(|e| Error::Other(format!("Failed to read input: {}", e)))?;
    let client_id = client_id.trim().to_string();

    print!("Client Secret: ");
    io::stdout().flush().unwrap();
    let mut client_secret = String::new();
    io::stdin()
        .read_line(&mut client_secret)
        .map_err(|e| Error::Other(format!("Failed to read input: {}", e)))?;
    let client_secret = client_secret.trim().to_string();

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

/// Run dev uninstall - completely remove local dev environment
pub async fn uninstall(_ctx: &Context, yes: bool) -> Result<()> {
    print_header("Uninstalling InferaDB Local Development Environment");

    // Show what will be removed
    let deploy_dir = get_deploy_dir();
    let creds_file = get_tailscale_creds_file();
    let config_dir = Config::config_dir().unwrap_or_else(|| PathBuf::from(".config/inferadb"));
    let data_dir = Config::data_dir().unwrap_or_else(|| PathBuf::from(".local/share/inferadb"));
    let state_dir = Config::state_dir().unwrap_or_else(|| PathBuf::from(".local/state/inferadb"));

    println!("This will remove:");
    println!();

    // Check cluster
    if docker_container_exists(CLUSTER_NAME) {
        let status = if are_containers_paused() {
            "paused"
        } else {
            "running"
        };
        println!("  • Talos cluster '{}' ({})", CLUSTER_NAME, status);
    }

    // Check registry
    if docker_container_exists(REGISTRY_NAME) {
        println!("  • Local Docker registry '{}'", REGISTRY_NAME);
    }

    // Check deploy directory
    if deploy_dir.exists() {
        println!("  • Deploy repository: {}", deploy_dir.display());
    }

    // Check data directory (excluding deploy which is listed separately)
    if data_dir.exists() && data_dir != deploy_dir.parent().unwrap_or(&data_dir) {
        println!("  • Data directory: {}", data_dir.display());
    }

    // Check state directory
    if state_dir.exists() {
        println!("  • State directory: {}", state_dir.display());
    }

    // Check config directory
    if config_dir.exists() {
        println!("  • Config directory: {}", config_dir.display());
    }

    // Check for dev-related Docker images
    let dev_images = get_dev_docker_images();
    if !dev_images.is_empty() {
        println!(
            "  • {} Docker image(s) (inferadb-*, ghcr.io/siderolabs/*)",
            dev_images.len()
        );
    }

    // Check kubectl/talosctl contexts
    let has_kube_context =
        run_command_optional("kubectl", &["config", "get-contexts", "-o", "name"])
            .map(|o| o.lines().any(|l| l == KUBE_CONTEXT))
            .unwrap_or(false);
    let has_talos_context = run_command_optional("talosctl", &["config", "contexts"])
        .map(|o| o.contains(CLUSTER_NAME))
        .unwrap_or(false);

    if has_kube_context || has_talos_context {
        println!("  • Kubernetes/Talos configuration contexts");
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

    // 1. Destroy cluster if running
    if docker_container_exists(CLUSTER_NAME) {
        println!("Destroying Talos cluster...");

        // Clean up Tailscale devices first
        cleanup_tailscale_devices()?;

        // Destroy cluster
        let _ = run_command_streaming(
            "talosctl",
            &["cluster", "destroy", "--name", CLUSTER_NAME],
            &[],
        );
    }

    // 2. Remove registry container
    if docker_container_exists(REGISTRY_NAME) {
        println!("Removing local registry...");
        let _ = run_command_optional("docker", &["stop", REGISTRY_NAME]);
        let _ = run_command_optional("docker", &["rm", "-f", REGISTRY_NAME]);
    }

    // 3. Clean up contexts
    cleanup_stale_contexts();

    // 4. Remove Docker images
    if !dev_images.is_empty() {
        println!("Removing Docker images...");
        for image in &dev_images {
            print!("  Removing {}... ", image);
            if run_command_optional("docker", &["rmi", "-f", image]).is_some() {
                println!("done");
            } else {
                println!("failed (may be in use)");
            }
        }
    }

    // 5. Remove deploy directory
    if deploy_dir.exists() {
        println!("Removing deploy repository...");
        if let Err(e) = fs::remove_dir_all(&deploy_dir) {
            eprintln!(
                "  Warning: Failed to remove {}: {}",
                deploy_dir.display(),
                e
            );
        } else {
            println!("  Removed: {}", deploy_dir.display());
        }
    }

    // 6. Remove data directory if empty after deploy removal
    if data_dir.exists() {
        // Try to remove - will fail if not empty, which is fine
        if fs::remove_dir(&data_dir).is_ok() {
            println!("  Removed empty data directory: {}", data_dir.display());
        }
    }

    // 7. Remove state directory
    if state_dir.exists() {
        println!("Removing state directory...");
        if let Err(e) = fs::remove_dir_all(&state_dir) {
            eprintln!("  Warning: Failed to remove {}: {}", state_dir.display(), e);
        } else {
            println!("  Removed: {}", state_dir.display());
        }
    }

    // 8. Remove Tailscale credentials (but keep other config)
    if creds_file.exists() {
        println!("Removing Tailscale credentials...");
        if let Err(e) = fs::remove_file(&creds_file) {
            eprintln!(
                "  Warning: Failed to remove {}: {}",
                creds_file.display(),
                e
            );
        } else {
            println!("  Removed: {}", creds_file.display());
        }
    }

    // Note: We preserve the config directory since it may have CLI profiles and settings

    println!();
    print_header("Uninstall Complete");
    println!("The following were preserved:");
    println!("  • CLI configuration: {}", config_dir.display());
    println!("  • System tools: docker, talosctl, kubectl, helm");
    println!();
    print_info("To reinstall: inferadb dev install");

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
pub async fn start(_ctx: &Context, skip_build: bool) -> Result<()> {
    // Check if cluster containers exist and are paused
    if docker_container_exists(CLUSTER_NAME) {
        if are_containers_paused() {
            print_header("Resuming Paused InferaDB Development Cluster");

            // Unpause all cluster containers
            unpause_cluster_containers()?;

            // Also unpause the registry if it exists
            if docker_container_exists(REGISTRY_NAME) {
                println!("Resuming local registry...");
                let _ = run_command_optional("docker", &["unpause", REGISTRY_NAME]);
            }

            // Wait a moment for containers to stabilize
            println!("\nWaiting for cluster to stabilize...");
            std::thread::sleep(Duration::from_secs(3));

            // Verify cluster is healthy
            print_phase("Verifying cluster status");
            run_command_streaming("kubectl", &["get", "nodes"], &[])?;

            println!();
            print_header("Cluster Resumed Successfully");
            print_info("Run 'inferadb dev status' for full cluster status.");
            return Ok(());
        } else {
            print_warning(&format!(
                "Cluster '{}' already exists and is running.",
                CLUSTER_NAME
            ));
            print_info("Run 'inferadb dev status' to check its status.");
            print_info("To recreate it, first run: inferadb dev stop --destroy");
            return Err(Error::Other("Cluster already running".to_string()));
        }
    }

    print_header("Creating Local Talos Cluster for InferaDB Development");

    // Ensure deploy repo is cloned
    let deploy_dir = ensure_deploy_repo()?;

    // Check prerequisites
    print_phase("Checking prerequisites");
    for cmd in &["docker", "talosctl", "kubectl", "helm"] {
        if !command_exists(cmd) {
            print_error(&format!("{} is not installed", cmd));
            return Err(Error::Other(format!(
                "{} is not installed. Run 'inferadb dev doctor' for setup instructions.",
                cmd
            )));
        }
    }

    // Check Docker is running
    if run_command_optional("docker", &["info"]).is_none() {
        return Err(Error::Other(
            "Docker daemon is not running. Please start Docker first.".to_string(),
        ));
    }

    // Get Tailscale credentials
    let (ts_client_id, ts_client_secret) = get_tailscale_credentials()?;

    print_success("Prerequisites OK");
    println!();

    // Clean up stale contexts
    print_phase("Cleaning up stale contexts");
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

    // Clean up kubectl contexts
    if let Some(contexts) =
        run_command_optional("kubectl", &["config", "get-contexts", "-o", "name"])
    {
        if contexts.lines().any(|l| l == KUBE_CONTEXT) {
            let _ = run_command_optional("kubectl", &["config", "delete-context", KUBE_CONTEXT]);
            let _ = run_command_optional("kubectl", &["config", "delete-cluster", CLUSTER_NAME]);
            let _ = run_command_optional("kubectl", &["config", "delete-user", KUBE_CONTEXT]);
        }
    }

    // Create cluster
    print_phase("Creating Talos cluster with Docker provisioner");
    println!("  Cluster name: {}", CLUSTER_NAME);
    println!("  Build images: {}", !skip_build);
    println!();

    run_command_streaming(
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
        &[],
    )?;

    println!();
    print_success("Cluster created successfully!");
    println!();

    // Set kubectl context
    print_phase(&format!("Setting kubectl context to {}", KUBE_CONTEXT));
    run_command("kubectl", &["config", "use-context", KUBE_CONTEXT])?;

    // Verify cluster is ready
    print_phase("Verifying cluster is ready");
    run_command_streaming("kubectl", &["get", "nodes"], &[])?;
    println!();

    // Bootstrap Flux
    let flux_dir = deploy_dir.join("flux/clusters/dev-local/flux-system");
    print_phase("Bootstrapping Flux");
    if flux_dir.join("gotk-components.yaml").exists() {
        run_command_streaming(
            "kubectl",
            &[
                "apply",
                "-f",
                flux_dir.join("gotk-components.yaml").to_str().unwrap(),
            ],
            &[],
        )?;
        run_command_streaming(
            "kubectl",
            &[
                "apply",
                "-f",
                flux_dir.join("gotk-sync.yaml").to_str().unwrap(),
            ],
            &[],
        )?;
        print_success("Flux bootstrapped successfully!");
    } else {
        print_info("Flux manifests not found. Skipping Flux bootstrap.");
    }
    println!();

    // Start local registry
    print_header("Setting up container registry");

    if docker_container_exists(REGISTRY_NAME) {
        println!("Using existing local registry...");
    } else {
        println!("Starting local registry...");

        // Get the Docker network used by Talos cluster
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

        run_command_streaming(
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
            &[],
        )?;

        // Wait for registry to be ready
        println!("Waiting for registry to be ready...");
        std::thread::sleep(Duration::from_secs(3));
    }

    let registry_ip = get_container_ip(REGISTRY_NAME)
        .ok_or_else(|| Error::Other("Failed to get registry IP".to_string()))?;

    println!(
        "Registry available at {}:5000 (in-cluster) and localhost:{} (host)\n",
        registry_ip, REGISTRY_PORT
    );

    // Configure Talos for insecure registry
    println!("Configuring Talos nodes for insecure registry access...");

    let controlplane_ip = get_container_ip(&format!("{}-controlplane-1", CLUSTER_NAME));
    let worker_ip = get_container_ip(&format!("{}-worker-1", CLUSTER_NAME));

    let patch_content = format!(
        r#"machine:
  registries:
    mirrors:
      {}:5000:
        endpoints:
          - http://{}:5000
    config:
      {}:5000:
        tls:
          insecureSkipVerify: true
"#,
        registry_ip, registry_ip, registry_ip
    );

    let patch_file = std::env::temp_dir().join("talos-registry-patch.yaml");
    fs::write(&patch_file, &patch_content)?;

    for node_ip in [controlplane_ip, worker_ip].into_iter().flatten() {
        println!("  Patching Talos node {}...", node_ip);
        let _ = run_command_optional(
            "talosctl",
            &[
                "patch",
                "machineconfig",
                "--nodes",
                &node_ip,
                "--patch",
                &format!("@{}", patch_file.display()),
            ],
        );
    }
    fs::remove_file(&patch_file).ok();
    println!();

    // Build and push images
    if !skip_build {
        print_header("Building and pushing container images");

        // Determine repo root (parent of deploy dir for installed case, or from cwd for dev)
        let repo_root = if deploy_dir == get_deploy_dir() {
            // Installed case - check if we're in the main repo
            let cwd = env::current_dir().unwrap_or_default();
            if cwd.join("engine").exists() {
                cwd
            } else {
                // Can't build from installed deploy repo alone
                println!("Warning: Cannot find source directories for building images.");
                println!("Run from the InferaDB repository root to build images,");
                println!("or use --skip-build to use pre-built images.\n");
                PathBuf::new()
            }
        } else {
            deploy_dir.parent().unwrap_or(&deploy_dir).to_path_buf()
        };

        if repo_root.join("engine").exists() {
            for (name, dir) in &[
                ("inferadb-engine", "engine"),
                ("inferadb-control", "control"),
                ("inferadb-dashboard", "dashboard"),
            ] {
                let dockerfile = repo_root.join(dir).join("Dockerfile");
                if dockerfile.exists() {
                    println!("Building {} image...", name);
                    run_command_streaming(
                        "docker",
                        &[
                            "build",
                            "-t",
                            &format!("{}:latest", name),
                            repo_root.join(dir).to_str().unwrap(),
                        ],
                        &[],
                    )?;

                    run_command_streaming(
                        "docker",
                        &[
                            "tag",
                            &format!("{}:latest", name),
                            &format!("localhost:{}/{}:latest", REGISTRY_PORT, name),
                        ],
                        &[],
                    )?;

                    run_command_streaming(
                        "docker",
                        &[
                            "push",
                            &format!("localhost:{}/{}:latest", REGISTRY_PORT, name),
                        ],
                        &[],
                    )?;

                    println!("{} image built and pushed!\n", name);
                } else {
                    println!("Warning: {}/Dockerfile not found, skipping...", dir);
                }
            }
        }
    } else {
        println!("Skipping image builds (--skip-build specified)");
        println!(
            "Note: Images must already exist in registry at localhost:{}\n",
            REGISTRY_PORT
        );
    }

    // Create namespaces
    print_header("Setting up Kubernetes resources");
    print_phase("Creating namespaces");

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
        let mut child = Command::new("kubectl")
            .args(["apply", "-f", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| Error::Other(format!("Failed to create namespace: {}", e)))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(yaml.as_bytes())?;
        }
        child.wait()?;
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
    print_phase("Installing local-path-provisioner");
    run_command_streaming(
        "kubectl",
        &["apply", "-f", "https://raw.githubusercontent.com/rancher/local-path-provisioner/v0.0.26/deploy/local-path-storage.yaml"],
        &[],
    )?;

    run_command_streaming(
        "kubectl",
        &[
            "patch",
            "storageclass",
            "local-path",
            "-p",
            r#"{"metadata": {"annotations":{"storageclass.kubernetes.io/is-default-class":"true"}}}"#,
        ],
        &[],
    )?;

    // Install Tailscale Operator
    print_header("Installing Tailscale Kubernetes Operator");

    print_phase("Adding Tailscale Helm repository");
    let _ = run_command_optional(
        "helm",
        &[
            "repo",
            "add",
            "tailscale",
            "https://pkgs.tailscale.com/helmcharts",
        ],
    );
    run_command_streaming("helm", &["repo", "update", "tailscale"], &[])?;

    print_phase("Installing Tailscale Operator");
    run_command_streaming(
        "helm",
        &[
            "upgrade",
            "--install",
            "tailscale-operator",
            "tailscale/tailscale-operator",
            "--namespace",
            "tailscale-system",
            "--set",
            &format!("oauth.clientId={}", ts_client_id),
            "--set",
            &format!("oauth.clientSecret={}", ts_client_secret),
            "--set",
            "apiServerProxyConfig.mode=noauth",
            "--wait",
            "--timeout",
            "5m",
        ],
        &[],
    )?;
    print_success("Tailscale Operator installed!");
    println!();

    // Install FDB operator
    print_header("Installing FoundationDB Operator");

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
        run_command_streaming(
            "kubectl",
            &["apply", "-f", &format!("{}/{}", fdb_url, crd)],
            &[],
        )?;
    }

    // Wait for CRDs
    println!("Waiting for FoundationDB CRDs...");
    run_command_streaming(
        "kubectl",
        &[
            "wait",
            "--for=condition=established",
            "--timeout=60s",
            "crd/foundationdbclusters.apps.foundationdb.org",
        ],
        &[],
    )?;

    // Install RBAC
    run_command_streaming(
        "kubectl",
        &[
            "apply",
            "-f",
            &format!("{}/rbac/cluster_role.yaml", fdb_url),
        ],
        &[],
    )?;
    run_command_streaming(
        "kubectl",
        &[
            "apply",
            "-f",
            &format!("{}/rbac/role.yaml", fdb_url),
            "-n",
            "fdb-system",
        ],
        &[],
    )?;

    // Install operator deployment (with sed-like modifications)
    println!("Installing FDB operator deployment...");
    let manager_yaml = run_command(
        "curl",
        &["-s", &format!("{}/deployment/manager.yaml", fdb_url)],
    )?;

    // Apply modifications:
    // 1. Fix serviceAccountName reference
    // 2. Remove WATCH_NAMESPACE env var block (from WATCH_NAMESPACE line through fieldPath: line)
    //    This enables global namespace watching mode
    let yaml_with_sa_fix = manager_yaml.replace(
        "serviceAccountName: fdb-kubernetes-operator-controller-manager",
        "serviceAccountName: controller-manager",
    );

    // Remove the WATCH_NAMESPACE block using range deletion (like sed '/WATCH_NAMESPACE/,/fieldPath:/d')
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
            // Skip lines inside the block
            continue;
        }
        modified_lines.push(line);
    }
    let modified_yaml = modified_lines.join("\n");

    let mut child = Command::new("kubectl")
        .args(["apply", "-n", "fdb-system", "-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(modified_yaml.as_bytes())?;
    }
    child.wait()?;

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

        let mut child = Command::new("kubectl")
            .args(["apply", "-f", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(binding_yaml.as_bytes())?;
        }
        child.wait()?;
    }

    // Create FDB sidecar RBAC
    println!("Creating FDB sidecar RBAC...");
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

    let mut child = Command::new("kubectl")
        .args(["apply", "-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(sidecar_rbac.as_bytes())?;
    }
    child.wait()?;

    // Wait for FDB operator
    println!("○ Waiting for FDB operator to be ready ...");
    println!("  Containers must download and initialize FoundationDB binaries.");
    println!("  This will take several minutes.");

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(300);

    loop {
        if start.elapsed() > timeout {
            println!("\nERROR: FDB operator did not become ready within 5 minutes");
            println!("Check status with: kubectl get pods -n fdb-system");
            return Err(Error::Other("FDB operator timeout".to_string()));
        }

        // Show current status
        if let Some(status) = run_command_optional(
            "kubectl",
            &[
                "get",
                "pods",
                "-n",
                "fdb-system",
                "-o",
                "wide",
                "--no-headers",
            ],
        ) {
            println!("  [{:3}s] {}", start.elapsed().as_secs(), status.trim());
        }

        // Check if ready
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
            println!("FDB operator is ready!");
            break;
        }

        std::thread::sleep(Duration::from_secs(10));
    }

    // Generate registry patch
    print_header("Deploying InferaDB applications");

    let registry_patch = format!(
        r#"# Auto-generated by inferadb dev start - patches images to use local registry
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
    fs::write(&patch_file, &registry_patch)?;

    // Apply dev overlay
    run_command_streaming(
        "kubectl",
        &[
            "apply",
            "-k",
            deploy_dir.join("flux/apps/dev").to_str().unwrap(),
        ],
        &[],
    )?;

    println!("\nApplications deployed!");
    println!("Note: It may take a few minutes for all pods to be ready.");
    println!("Monitor progress with: kubectl get pods -n inferadb -w\n");

    // Wait for ingress and display access info
    println!("Waiting for Tailscale ingress to be ready...");
    std::thread::sleep(Duration::from_secs(10));

    let ingress_hostname = run_command_optional(
        "kubectl",
        &[
            "get",
            "ingress",
            "inferadb-api-tailscale",
            "-n",
            "inferadb",
            "-o",
            "jsonpath={.status.loadBalancer.ingress[0].hostname}",
        ],
    );

    let tailnet_suffix = ingress_hostname
        .as_ref()
        .and_then(|h| h.strip_prefix("inferadb-api."))
        .map(|s| s.to_string())
        .or_else(get_tailnet_info);

    print_header("Development Environment Ready");
    println!("Cluster context: {}", KUBE_CONTEXT);
    println!();

    if let Some(suffix) = &tailnet_suffix {
        println!("Access your services:");
        println!("  https://inferadb-api.{}       - API", suffix);
        println!("  https://inferadb-dashboard.{} - Dashboard", suffix);
    } else {
        println!("Access your services:");
        println!("  https://inferadb-api.<your-tailnet>.ts.net       - API");
        println!("  https://inferadb-dashboard.<your-tailnet>.ts.net - Dashboard");
        println!("\n  (Check 'kubectl get ingress -n inferadb' for exact URLs once ready)");
    }

    println!("\nAPI routes:");
    println!("  /control/v1/*       -> Control (auth, organizations, users, vaults, tokens)");
    println!("  /access/v1/*        -> Engine (evaluate, relationships, expand, lookup)");
    println!("  /.well-known/*      -> Control (JWKS discovery)");
    println!("  /healthz, /readyz   -> Control (health checks)");

    println!("\nUseful commands:");
    println!("  inferadb dev status         # Cluster status");
    println!("  inferadb dev logs           # View logs");
    println!("  kubectl get pods -n inferadb");
    println!("  kubectl get ingress -n inferadb");

    println!("\nTo destroy this cluster, run:");
    println!("  inferadb dev stop");

    Ok(())
}

/// Clean up Tailscale devices via API
fn cleanup_tailscale_devices() -> Result<()> {
    let (client_id, client_secret) = match load_tailscale_credentials() {
        Some(creds) => creds,
        None => {
            println!("Note: Tailscale credentials not found. Orphaned devices may remain.");
            println!(
                "      You can manually remove them at: https://login.tailscale.com/admin/machines"
            );
            return Ok(());
        }
    };

    println!("Cleaning up Tailscale devices...");

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
            println!("  Could not obtain Tailscale API token. Skipping device cleanup.");
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
        println!("  No devices found or API unavailable. Skipping device cleanup.");
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

                        // Extract short name for display (remove tailnet suffix)
                        let display_name = device_name.split('.').next().unwrap_or(device_name);
                        println!(
                            "  Removing Tailscale device: {} ({})",
                            display_name, device_id
                        );

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

    println!("  Tailscale cleanup complete.");
    Ok(())
}

/// Run dev stop - pause or destroy local development cluster
///
/// By default, this pauses all cluster containers so they can be quickly resumed.
/// Use --destroy to fully tear down and remove all containers and configuration.
pub async fn stop(_ctx: &Context, destroy: bool) -> Result<()> {
    // Check if cluster exists
    if !docker_container_exists(CLUSTER_NAME) {
        println!("No cluster found for '{}'.", CLUSTER_NAME);

        // Clean up any stale contexts even if containers are gone
        cleanup_stale_contexts();

        println!("Nothing to stop.");
        return Ok(());
    }

    // Check if already paused
    if are_containers_paused() && !destroy {
        println!("Cluster '{}' is already paused.", CLUSTER_NAME);
        println!("Use 'inferadb dev start' to resume, or 'inferadb dev stop --destroy' to remove.");
        return Ok(());
    }

    if destroy {
        // Full teardown
        print_header("Destroying Local Talos Cluster");
        println!("Cluster name: {}", CLUSTER_NAME);
        println!();

        // Stop and remove local registry
        if docker_container_exists(REGISTRY_NAME) {
            println!("Stopping and removing local registry...");
            let _ = run_command_optional("docker", &["stop", REGISTRY_NAME]);
            let _ = run_command_optional("docker", &["rm", REGISTRY_NAME]);
        }

        // Clean up Tailscale devices
        cleanup_tailscale_devices()?;

        // Destroy the cluster
        println!("Destroying Talos cluster...");
        run_command_streaming(
            "talosctl",
            &["cluster", "destroy", "--name", CLUSTER_NAME],
            &[],
        )?;

        // Clean up contexts
        cleanup_stale_contexts();

        println!();
        print_success("Cluster destroyed successfully!");
        println!();
        print_header("Teardown Complete");
    } else {
        // Pause containers (default behavior)
        print_header("Pausing Local Talos Cluster");
        println!("Cluster name: {}", CLUSTER_NAME);
        println!();

        // Pause the cluster containers
        pause_cluster_containers()?;

        // Also pause the registry if it exists
        if docker_container_exists(REGISTRY_NAME) {
            println!("Pausing local registry...");
            let _ = run_command_optional("docker", &["pause", REGISTRY_NAME]);
        }

        println!();
        print_success("Cluster paused successfully!");
        println!();
        print_info("To resume:  inferadb dev start");
        print_info("To destroy: inferadb dev stop --destroy");
        println!();
        print_header("Pause Complete");
    }

    Ok(())
}

/// Clean up stale talosctl and kubectl contexts
fn cleanup_stale_contexts() {
    // Clean up talosctl contexts
    if let Some(contexts) = run_command_optional("talosctl", &["config", "contexts"]) {
        for line in contexts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1].starts_with(CLUSTER_NAME) {
                println!("Cleaning up stale talosctl context: {}...", parts[1]);
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
            println!("Cleaning up stale kubectl context...");
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
fn status_with_spinners() -> Result<()> {
    use crate::tui::start_spinner;

    print_styled_header("InferaDB Development Environment Status");

    // Check cluster status
    let handle = start_spinner("Checking cluster status...");
    let cluster_status = get_cluster_status();
    match cluster_status {
        ClusterStatus::Offline => {
            handle.failure("Cluster is not running");
            println!();
            print_info(TIP_START_CLUSTER);
            return Ok(());
        }
        ClusterStatus::Paused => {
            handle.warning("Cluster is paused");
            println!();
            print_info(TIP_RESUME_CLUSTER);
            return Ok(());
        }
        ClusterStatus::Online => {
            handle.success("Cluster is running");
        }
        ClusterStatus::Unknown => {
            handle.warning("Cluster status unknown");
        }
    }

    // Check kubectl context
    let handle = start_spinner("Checking kubectl context...");
    if let Some(current) = run_command_optional("kubectl", &["config", "current-context"]) {
        let context = current.trim();
        if context == KUBE_CONTEXT {
            handle.success(&format!("kubectl context: {}", context));
        } else {
            handle.warning(&format!(
                "kubectl context: {} (expected: {})",
                context, KUBE_CONTEXT
            ));
        }
    } else {
        handle.warning("kubectl context not configured");
    }
    println!();

    // Nodes
    print_phase("Nodes");
    run_command_streaming("kubectl", &["get", "nodes", "-o", "wide"], &[])?;
    println!();

    // InferaDB Pods
    print_phase("InferaDB Pods");
    run_command_streaming(
        "kubectl",
        &["get", "pods", "-n", "inferadb", "-o", "wide"],
        &[],
    )?;
    println!();

    // FDB Pods
    print_phase("FoundationDB Pods");
    run_command_streaming(
        "kubectl",
        &["get", "pods", "-n", "fdb-system", "-o", "wide"],
        &[],
    )?;
    println!();

    // Ingress/URLs
    print_phase("Ingress (Access URLs)");
    run_command_streaming("kubectl", &["get", "ingress", "-n", "inferadb"], &[])?;
    println!();

    // Services
    print_phase("Services");
    run_command_streaming("kubectl", &["get", "services", "-n", "inferadb"], &[])?;

    Ok(())
}

/// Run status in full-screen interactive TUI mode.
fn status_interactive() -> Result<()> {
    use crate::tui::StatusView;
    use ferment::output::{terminal_height, terminal_width};
    use ferment::runtime::{Program, ProgramOptions};

    let width = terminal_width();
    let height = terminal_height();

    // Get initial data
    let initial_data = fetch_status_data();

    let view = StatusView::new(width, height)
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
    let selector = match service {
        Some("engine") => "app=inferadb-engine",
        Some("control") => "app=inferadb-control",
        Some("dashboard") => "app=inferadb-dashboard",
        Some("fdb") => {
            args[1] = "fdb-system";
            "app=fdb-kubernetes-operator"
        }
        Some(other) => {
            return Err(Error::Other(format!(
                "Unknown service: {}. Valid: engine, control, dashboard, fdb",
                other
            )));
        }
        None => "app.kubernetes.io/part-of=inferadb",
    };

    args.push("-l");
    args.push(selector);

    args.push("--tail");
    let tail_str = tail.to_string();
    args.push(&tail_str);

    if follow {
        args.push("-f");
    }

    print_phase(&format!(
        "Showing logs for {}",
        service.unwrap_or("all services")
    ));
    run_command_streaming("kubectl", &args, &[])?;

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
            "inferadb-dashboard-tailscale",
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

    if !yes {
        println!("This will delete all data in the development cluster!");
        print!("Are you sure? [y/N] ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    print_header("Resetting Development Cluster Data");

    // Delete FDB cluster (will recreate with empty data)
    print_phase("Deleting FoundationDB cluster");
    let _ = run_command_optional(
        "kubectl",
        &["delete", "foundationdbcluster", "--all", "-n", "inferadb"],
    );

    // Delete InferaDB deployments
    print_phase("Deleting InferaDB deployments");
    for deploy in &["inferadb-engine", "inferadb-control", "inferadb-dashboard"] {
        let _ = run_command_optional(
            "kubectl",
            &["delete", "deployment", deploy, "-n", "inferadb"],
        );
    }

    // Delete PVCs
    print_phase("Deleting persistent volume claims");
    let _ = run_command_optional("kubectl", &["delete", "pvc", "--all", "-n", "inferadb"]);

    // Wait a moment
    std::thread::sleep(Duration::from_secs(5));

    // Reapply the dev overlay
    let deploy_dir = get_deploy_dir();
    if deploy_dir.exists() {
        println!();
        print_phase("Redeploying applications");
        run_command_streaming(
            "kubectl",
            &[
                "apply",
                "-k",
                deploy_dir.join("flux/apps/dev").to_str().unwrap(),
            ],
            &[],
        )?;
    }

    println!();
    print_success("Cluster data reset complete!");
    print_info("Applications are being recreated. Monitor with:");
    println!("  kubectl get pods -n inferadb -w");

    Ok(())
}

/// Run dev import - import seed data
pub async fn import(_ctx: &Context, file: &str) -> Result<()> {
    // Check if cluster is running
    if !docker_container_exists(CLUSTER_NAME) {
        return Err(Error::Other(
            "Cluster is not running. Start with 'inferadb dev start'.".to_string(),
        ));
    }

    if !std::path::Path::new(file).exists() {
        return Err(Error::Other(format!("File not found: {}", file)));
    }

    println!("Importing data from {}...", file);

    // Get the control pod name
    let pod = run_command(
        "kubectl",
        &[
            "get",
            "pods",
            "-n",
            "inferadb",
            "-l",
            "app=inferadb-control",
            "-o",
            "jsonpath={.items[0].metadata.name}",
        ],
    )?;
    let pod = pod.trim();

    if pod.is_empty() {
        return Err(Error::Other(
            "Control plane pod not found. Is the cluster fully deployed?".to_string(),
        ));
    }

    // Copy file to pod
    run_command_streaming(
        "kubectl",
        &["cp", file, &format!("inferadb/{}:/tmp/import.json", pod)],
        &[],
    )?;

    // Execute import (this assumes the control plane has an import endpoint/command)
    println!("Note: Data import requires the control plane to support bulk import.");
    println!(
        "You can use 'inferadb bulk import {}' directly if authenticated.",
        file
    );

    Ok(())
}

/// Run dev export - export data
pub async fn export(_ctx: &Context, output: &str) -> Result<()> {
    // Check if cluster is running
    if !docker_container_exists(CLUSTER_NAME) {
        return Err(Error::Other(
            "Cluster is not running. Start with 'inferadb dev start'.".to_string(),
        ));
    }

    println!("Exporting data to {}...", output);

    // Get the control pod name
    let pod = run_command(
        "kubectl",
        &[
            "get",
            "pods",
            "-n",
            "inferadb",
            "-l",
            "app=inferadb-control",
            "-o",
            "jsonpath={.items[0].metadata.name}",
        ],
    )?;
    let pod = pod.trim();

    if pod.is_empty() {
        return Err(Error::Other(
            "Control plane pod not found. Is the cluster fully deployed?".to_string(),
        ));
    }

    println!("Note: Data export requires the control plane to support bulk export.");
    println!(
        "You can use 'inferadb bulk export {}' directly if authenticated.",
        output
    );

    Ok(())
}
