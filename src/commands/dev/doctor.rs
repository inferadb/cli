//! Environment checking for dev commands.
//!
//! Provides dependency checking, Docker daemon status, and environment verification.

use teapot::output::error as print_error;

use super::{
    commands::{command_exists, extract_version_string, run_command_optional},
    output::{format_dot_leader, print_hint, print_phase_header, print_styled_header},
    paths::get_tailscale_creds_file,
};
use crate::{
    client::Context,
    error::{Error, Result},
    tui::{CheckResult, EnvironmentStatus},
};

/// Dependency information for doctor command.
pub struct Dependency {
    pub name: &'static str,
    pub command: &'static str,
    pub version_args: &'static [&'static str],
    pub required: bool,
    pub install_hint_mac: &'static str,
    pub install_hint_linux: &'static str,
    pub install_hint_windows: &'static str,
}

/// List of all dependencies to check.
pub const DEPENDENCIES: &[Dependency] = &[
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

/// Get the appropriate install hint for the current platform.
pub fn get_install_hint(dep: &Dependency) -> &'static str {
    if cfg!(target_os = "macos") {
        dep.install_hint_mac
    } else if cfg!(target_os = "windows") {
        dep.install_hint_windows
    } else {
        dep.install_hint_linux
    }
}

/// Check a single dependency and return the result.
pub fn check_dependency(dep: &Dependency) -> (CheckResult, bool) {
    let exists = command_exists(dep.command);
    let version = if exists {
        run_command_optional(dep.command, dep.version_args)
            .map(|v| extract_version_string(&v, dep.command))
            .unwrap_or_else(|| "installed".to_string())
    } else {
        String::new()
    };

    if exists {
        (CheckResult::success("Dependencies", dep.name, version), true)
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
        (CheckResult::optional("Dependencies", dep.name, "not found (optional)"), true)
    }
}

/// Check if Docker daemon is running.
pub fn check_docker_daemon() -> Option<(CheckResult, bool)> {
    if !command_exists("docker") {
        return None;
    }

    match run_command_optional("docker", &["info"]) {
        Some(_) => Some((CheckResult::success("Services", "Docker daemon", "RUNNING"), true)),
        None => Some((
            CheckResult::failure("Services", "Docker daemon", "not running → start Docker Desktop"),
            false,
        )),
    }
}

/// Check Tailscale connection status.
pub fn check_tailscale_connection() -> Option<CheckResult> {
    if !command_exists("tailscale") {
        return None;
    }

    match run_command_optional("tailscale", &["status", "--json"]) {
        Some(output) => {
            if output.contains("\"BackendState\"") && output.contains("\"Running\"") {
                Some(CheckResult::success("Services", "Tailscale", "CONNECTED"))
            } else {
                Some(CheckResult::optional("Services", "Tailscale", "not connected → tailscale up"))
            }
        },
        None => None,
    }
}

/// Check for cached Tailscale OAuth credentials.
pub fn check_tailscale_credentials() -> CheckResult {
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
pub fn extract_status_detail(status: &str) -> &str {
    status.trim_start_matches("✓ ").trim_start_matches("✗ ").trim_start_matches("○ ")
}

/// Format a check result with component name and dot leaders.
pub fn format_check_output(component: &str, detail: &str) -> String {
    format_dot_leader(component, detail)
}

/// Run all doctor checks and return results.
pub fn run_all_checks() -> (Vec<CheckResult>, EnvironmentStatus) {
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

    let status =
        if all_required_ok { EnvironmentStatus::Ready } else { EnvironmentStatus::NotReady };

    (results, status)
}

/// Run dev doctor - check environment readiness.
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
    use teapot::{
        output::{terminal_height, terminal_width},
        runtime::{Program, ProgramOptions},
    };

    use crate::tui::DevDoctorView;

    let width = terminal_width();
    let height = terminal_height();

    let (results, status) = run_all_checks();

    let view = DevDoctorView::new(width, height).with_status(status).with_results(results);

    let is_ready = view.is_ready();

    Program::new(view)
        .with_options(ProgramOptions::fullscreen())
        .run()
        .map_err(|e| Error::Other(e.to_string()))?;

    if is_ready { Ok(()) } else { Err(Error::Other("Missing required dependencies".to_string())) }
}
