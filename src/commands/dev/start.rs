//! Start command for dev cluster.
//!
//! Handles creating, resuming, and setting up the local development cluster.

use std::{
    fs,
    io::Write as IoWrite,
    process::{Command, Stdio},
    time::Duration,
};

use teapot::style::{Color, RESET};

use super::{
    commands::{command_exists, run_command, run_command_optional},
    constants::{
        CLUSTER_NAME, CONTAINER_STABILIZE_DELAY_SECS, CONTROL_REPO_URL, DASHBOARD_REPO_URL,
        DEPLOY_REPO_URL, ENGINE_REPO_URL, HELM_TAILSCALE_REPO, HELM_TAILSCALE_URL, KUBE_CONTEXT,
        KUBERNETES_VERSION, REGISTRY_NAME, REGISTRY_PORT, TALOS_CONTROLPLANES, TALOS_PROVISIONER,
        TALOS_WAIT_TIMEOUT, TALOS_WORKERS,
    },
    docker::{
        are_containers_paused, docker_container_exists, get_cluster_containers, get_container_ip,
        is_container_paused, is_docker_running, pull_image,
    },
    kubernetes::{
        ensure_namespace, helm_repo_add, helm_repo_exists, helm_repo_update,
        kubectl_current_context, kubectl_use_context, wait_for_deployment,
    },
    output::{
        StartStep, StepOutcome, print_hint, print_phase_header, print_styled_header, run_step,
        run_step_with_result,
    },
    paths::{get_config_dir, get_control_dir, get_dashboard_dir, get_deploy_dir, get_engine_dir},
    tailscale::{
        get_tailnet_info, get_tailscale_credentials, load_tailscale_credentials,
        save_tailscale_credentials,
    },
};
use crate::{
    client::Context,
    error::{Error, Result},
    tui::InstallStep,
};

// ============================================================================
// Public API
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

// ============================================================================
// Repository Cloning
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
            &["clone", "--recurse-submodules", "--quiet", repo_url, target_dir.to_str().unwrap()],
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
        return Err(format!("Failed to clone {repo_url}"));
    }

    if let Some(ref_spec) = commit {
        if run_command_optional("git", &["-C", target_dir.to_str().unwrap(), "checkout", ref_spec])
            .is_none()
        {
            return Err(format!("Failed to checkout '{ref_spec}'"));
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
        Ok(Some(_)) => Ok(StepOutcome::Skipped),
        Ok(None) => Ok(StepOutcome::Success),
        Err(e) => Err(format!("Failed to clone {name}: {e}")),
    }
}

// ============================================================================
// Setup Steps
// ============================================================================

/// Step: Create the configuration directory.
fn step_create_config_dir() -> std::result::Result<Option<String>, String> {
    let config_dir = get_config_dir();

    if config_dir.exists() {
        return Ok(Some("already exists".to_string()));
    }

    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {e}"))?;

    Ok(None)
}

/// Step: Set up Helm repositories.
fn step_setup_helm() -> std::result::Result<Option<String>, String> {
    if !command_exists("helm") {
        return Ok(Some("Helm not installed, skipping".to_string()));
    }

    // Add tailscale repo if it doesn't exist
    if !helm_repo_exists(HELM_TAILSCALE_REPO) {
        helm_repo_add(HELM_TAILSCALE_REPO, HELM_TAILSCALE_URL)?;
    }

    // Update all repos
    let _ = helm_repo_update();

    Ok(Some("Helm repositories configured".to_string()))
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

// ============================================================================
// Infrastructure Setup
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
                        &format!("name={CLUSTER_NAME}"),
                        "--format",
                        "{{.Name}}",
                    ],
                )
                .and_then(|s| s.lines().next().map(std::string::ToString::to_string))
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
                        &format!("{REGISTRY_PORT}:5000"),
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

            let outcome =
                if registry_existed { StepOutcome::Skipped } else { StepOutcome::Success };

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
        return Ok(StepOutcome::Skipped);
    }

    let mut built_count = 0;
    for (name, dir) in &components {
        let dockerfile = dir.join("Dockerfile");
        if dockerfile.exists() {
            run_command(
                "docker",
                &["build", "-t", &format!("{name}:latest"), dir.to_str().unwrap()],
            )
            .map_err(|e| e.to_string())?;
            run_command(
                "docker",
                &[
                    "tag",
                    &format!("{name}:latest"),
                    &format!("localhost:{REGISTRY_PORT}/{name}:latest"),
                ],
            )
            .map_err(|e| e.to_string())?;
            run_command("docker", &["push", &format!("localhost:{REGISTRY_PORT}/{name}:latest")])
                .map_err(|e| e.to_string())?;
            built_count += 1;
        }
    }

    if built_count == 0 {
        return Ok(StepOutcome::Skipped);
    }

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
        stdin.write_all(yaml.as_bytes()).map_err(|e| e.to_string())?;
    }
    child.wait().map_err(|e| e.to_string())?;
    Ok(())
}

/// Set up Kubernetes resources.
#[allow(clippy::unnecessary_wraps)]
fn setup_kubernetes_resources(_registry_ip: &str) -> std::result::Result<StepOutcome, String> {
    // Create namespaces
    let namespaces = ["inferadb", "fdb-system", "local-path-storage", "tailscale-system"];
    for ns in &namespaces {
        ensure_namespace(ns)?;
    }

    // Label namespaces for privileged pod security
    for ns in &namespaces {
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

    // Install local-path-provisioner for storage
    run_command("kubectl", &["apply", "-f", "https://raw.githubusercontent.com/rancher/local-path-provisioner/v0.0.26/deploy/local-path-storage.yaml"])
        .map_err(|e| e.to_string())?;
    run_command("kubectl", &["patch", "storageclass", "local-path", "-p", r#"{"metadata": {"annotations":{"storageclass.kubernetes.io/is-default-class":"true"}}}"#])
        .map_err(|e| e.to_string())?;

    Ok(StepOutcome::Success)
}

// ============================================================================
// Operator Installation
// ============================================================================

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
            &format!("oauth.clientId={client_id}"),
            "--set",
            &format!("oauth.clientSecret={client_secret}"),
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

/// Install `FoundationDB` operator.
fn install_fdb_operator() -> std::result::Result<StepOutcome, String> {
    let fdb_version = "v2.19.0";
    let fdb_url = format!(
        "https://raw.githubusercontent.com/FoundationDB/fdb-kubernetes-operator/{fdb_version}/config"
    );

    for crd in &[
        "crd/bases/apps.foundationdb.org_foundationdbclusters.yaml",
        "crd/bases/apps.foundationdb.org_foundationdbbackups.yaml",
        "crd/bases/apps.foundationdb.org_foundationdbrestores.yaml",
    ] {
        run_command("kubectl", &["apply", "-f", &format!("{fdb_url}/{crd}")])
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

    run_command("kubectl", &["apply", "-f", &format!("{fdb_url}/rbac/cluster_role.yaml")])
        .map_err(|e| e.to_string())?;
    run_command(
        "kubectl",
        &["apply", "-f", &format!("{fdb_url}/rbac/role.yaml"), "-n", "fdb-system"],
    )
    .map_err(|e| e.to_string())?;

    let manager_yaml = run_command("curl", &["-s", &format!("{fdb_url}/deployment/manager.yaml")])
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
        stdin.write_all(modified_lines.join("\n").as_bytes()).map_err(|e| e.to_string())?;
    }
    child.wait().map_err(|e| e.to_string())?;

    for (name, role) in &[
        ("fdb-operator-manager-role-global", "manager-role"),
        ("fdb-operator-manager-clusterrolebinding", "manager-clusterrole"),
    ] {
        let binding_yaml = format!(
            r"apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: {name}
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: {role}
subjects:
- kind: ServiceAccount
  name: controller-manager
  namespace: fdb-system
"
        );
        apply_yaml(&binding_yaml)?;
    }

    // Wait for FDB operator deployment to be ready
    wait_for_deployment("controller-manager", "fdb-system", "5m")?;

    Ok(StepOutcome::Success)
}

// ============================================================================
// Deployment
// ============================================================================

/// Deploy `InferaDB` applications and return tailnet suffix.
fn deploy_inferadb(
    deploy_dir: &std::path::Path,
    registry_ip: &str,
) -> std::result::Result<(StepOutcome, Option<String>), String> {
    let registry_patch = format!(
        r"# Auto-generated by inferadb dev start
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
          image: {registry_ip}:5000/inferadb-engine:latest
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
          image: {registry_ip}:5000/inferadb-control:latest
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
          image: {registry_ip}:5000/inferadb-dashboard:latest
"
    );

    let patch_file = deploy_dir.join("flux/apps/dev/registry-patch.yaml");
    fs::write(&patch_file, &registry_patch).map_err(|e| e.to_string())?;

    run_command("kubectl", &["apply", "-k", deploy_dir.join("flux/apps/dev").to_str().unwrap()])
        .map_err(|e| e.to_string())?;

    std::thread::sleep(Duration::from_secs(10));

    let tailnet_suffix = get_tailnet_info();

    Ok((StepOutcome::Success, tailnet_suffix))
}

// ============================================================================
// Output Helpers
// ============================================================================

/// Show final success output with URLs and hints.
fn show_final_success(tailnet_suffix: Option<&str>) {
    let green = Color::Green.to_ansi_fg();
    let reset = RESET;

    println!();
    println!("{green}✓{reset} Development cluster ready");
    println!();

    if let Some(suffix) = tailnet_suffix {
        println!("  API: https://inferadb-api.{suffix}");
        println!("  Dashboard: https://inferadb-dashboard.{suffix}");
    } else {
        println!("  API: https://inferadb-api.<your-tailnet>.ts.net");
        println!("  Dashboard: https://inferadb-dashboard.<your-tailnet>.ts.net");
    }

    println!();
    print_hint("Run 'inferadb dev status' for cluster details");
    print_hint("Run 'inferadb dev stop' to pause or destroy the cluster");
}

// ============================================================================
// Interactive Mode
// ============================================================================

/// Start interactive TUI mode.
fn start_interactive(skip_build: bool, force: bool, commit: Option<&str>) -> Result<()> {
    use teapot::runtime::{Program, ProgramOptions};

    use crate::tui::DevStartView;

    let deploy_dir = get_deploy_dir();
    let commit_owned = commit.map(std::string::ToString::to_string);

    let view = DevStartView::new(skip_build)
        .with_prereq_checker({
            let deploy_dir = deploy_dir.clone();
            move || {
                // Check prerequisites
                for cmd in &["docker", "talosctl", "kubectl", "helm"] {
                    if !command_exists(cmd) {
                        return Err(format!(
                            "{cmd} is not installed. Run 'inferadb dev doctor' for setup instructions."
                        ));
                    }
                }

                // Check Docker is running
                if !is_docker_running() {
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
            let commit = commit_owned;
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

    let result = Program::new(view).with_options(ProgramOptions::fullscreen()).run();

    match result {
        Ok(view) if view.is_success() => {
            // Show success message - try to get tailnet info
            let tailnet_suffix = get_tailnet_info();
            show_final_success(tailnet_suffix.as_deref());
            Ok(())
        },
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
) -> Vec<InstallStep> {
    let deploy_dir_owned = deploy_dir.to_path_buf();
    let commit_owned = commit.map(std::string::ToString::to_string);
    let is_paused = docker_container_exists(CLUSTER_NAME) && are_containers_paused();

    let mut steps = Vec::new();

    // Phase 0: Resume paused cluster if needed
    if is_paused {
        // Add a step for each cluster container
        let containers = get_cluster_containers();
        for container in containers {
            let container_name = container.clone();
            steps.push(InstallStep::with_executor(format!("Resuming {container}"), move || {
                run_command("docker", &["unpause", &container_name]).map(|_| None).or_else(|e| {
                    if e.to_string().contains("not paused") { Ok(None) } else { Err(e.to_string()) }
                })
            }));
        }

        // Resume registry
        if docker_container_exists(REGISTRY_NAME) {
            steps.push(InstallStep::with_executor(format!("Resuming {REGISTRY_NAME}"), || {
                let _ = run_command_optional("docker", &["unpause", REGISTRY_NAME]);
                Ok(None)
            }));
        }

        steps.push(InstallStep::with_executor("Waiting for containers to stabilize", || {
            std::thread::sleep(Duration::from_secs(CONTAINER_STABILIZE_DELAY_SECS));
            Ok(Some("ready".to_string()))
        }));
    }

    // Phase 1: Conditioning environment
    steps.push(InstallStep::with_executor("Cloning deployment repository", {
        let deploy_dir = deploy_dir_owned;
        let commit = commit_owned;
        move || step_clone_repo(&deploy_dir, force, commit.as_deref())
    }));
    steps.push(InstallStep::with_executor(
        "Creating configuration directory",
        step_create_config_dir,
    ));
    steps.push(InstallStep::with_executor("Setting up Helm repositories", step_setup_helm));

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
                TALOS_WORKERS,
                "--controlplanes",
                TALOS_CONTROLPLANES,
                "--provisioner",
                TALOS_PROVISIONER,
                "--kubernetes-version",
                KUBERNETES_VERSION,
                "--wait-timeout",
                TALOS_WAIT_TIMEOUT,
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
    steps.push(InstallStep::with_executor("Verifying cluster is ready", || {
        match run_command("kubectl", &["get", "nodes"]) {
            Ok(_) => Ok(Some("Verified".to_string())),
            Err(e) => Err(e.to_string()),
        }
    }));

    steps
}

// ============================================================================
// Streaming Mode
// ============================================================================

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
            let in_progress = format!("Resuming {container}");
            let completed = format!("Resumed {container}");
            run_step(&StartStep::with_ok(&in_progress, &completed), || {
                if !is_container_paused(&container_name) {
                    return Ok(StepOutcome::Skipped);
                }
                run_command("docker", &["unpause", &container_name])
                    .map(|_| StepOutcome::Success)
                    .or_else(|e| {
                        if e.to_string().contains("not paused") {
                            Ok(StepOutcome::Skipped)
                        } else {
                            Err(e.to_string())
                        }
                    })
            })?;
        }

        if docker_container_exists(REGISTRY_NAME) {
            run_step(
                &StartStep::with_ok(
                    &format!("Resuming {REGISTRY_NAME}"),
                    &format!("Resumed {REGISTRY_NAME}"),
                ),
                || {
                    if !is_container_paused(REGISTRY_NAME) {
                        return Ok(StepOutcome::Skipped);
                    }
                    run_command("docker", &["unpause", REGISTRY_NAME])
                        .map(|_| StepOutcome::Success)
                        .or_else(|e| {
                            if e.to_string().contains("not paused") {
                                Ok(StepOutcome::Skipped)
                            } else {
                                Err(e.to_string())
                            }
                        })
                },
            )?;
        }

        run_step(
            &StartStep::with_ok("Waiting for containers to stabilize", "Containers stabilized"),
            || {
                std::thread::sleep(Duration::from_secs(CONTAINER_STABILIZE_DELAY_SECS));
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
        &StartStep::with_ok("Cloning deployment repository", "Cloned deployment repository"),
        || match step_clone_repo(&deploy_dir, force, commit) {
            Ok(Some(_)) => Ok(StepOutcome::Skipped),
            Ok(None) => Ok(StepOutcome::Success),
            Err(e) => Err(e),
        },
    )?;

    let engine_dir = get_engine_dir();
    run_step(&StartStep::with_ok("Cloning engine repository", "Cloned engine repository"), || {
        step_clone_component("engine", ENGINE_REPO_URL, &engine_dir, force)
    })?;

    let control_dir = get_control_dir();
    run_step(
        &StartStep::with_ok("Cloning control repository", "Cloned control repository"),
        || step_clone_component("control", CONTROL_REPO_URL, &control_dir, force),
    )?;

    let dashboard_dir = get_dashboard_dir();
    run_step(
        &StartStep::with_ok("Cloning dashboard repository", "Cloned dashboard repository"),
        || step_clone_component("dashboard", DASHBOARD_REPO_URL, &dashboard_dir, force),
    )?;

    run_step(
        &StartStep::with_ok("Creating configuration directory", "Created configuration directory"),
        || match step_create_config_dir() {
            Ok(Some(_)) => Ok(StepOutcome::Skipped),
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
            if helm_repo_exists(HELM_TAILSCALE_REPO) {
                return Ok(StepOutcome::Skipped);
            }
            helm_repo_add(HELM_TAILSCALE_REPO, HELM_TAILSCALE_URL).map(|()| StepOutcome::Success)
        },
    )?;

    run_step(
        &StartStep::with_ok("Updating Helm repositories", "Updated Helm repositories"),
        || helm_repo_update().map(|()| StepOutcome::Success),
    )?;

    run_step(
        &StartStep::with_ok("Pulling Docker registry image", "Pulled Docker registry image"),
        || pull_image("registry:2").map(|()| StepOutcome::Success),
    )?;

    // Phase 2: Setting up cluster
    print_phase_header("Setting up cluster");

    run_step(&StartStep::with_ok("Checking prerequisites", "Checked prerequisites"), || {
        for cmd in &["docker", "talosctl", "kubectl", "helm"] {
            if !command_exists(cmd) {
                return Err(format!(
                    "{cmd} is not installed. Run 'inferadb dev doctor' for setup instructions."
                ));
            }
        }
        if !is_docker_running() {
            return Err("Docker daemon is not running. Please start Docker first.".to_string());
        }
        Ok(StepOutcome::Success)
    })?;

    let (ts_client_id, ts_client_secret) = get_tailscale_credentials()?;
    let cluster_already_exists = docker_container_exists(CLUSTER_NAME);

    run_step(&StartStep::with_ok("Cleaning stale contexts", "Cleaned stale contexts"), || {
        if cluster_already_exists {
            return Ok(StepOutcome::Skipped);
        }
        cleanup_stale_contexts();
        Ok(StepOutcome::Success)
    })?;

    run_step(
        &StartStep::with_ok("Provisioning Talos cluster", "Provisioned Talos cluster"),
        || {
            if cluster_already_exists {
                if run_command_optional("kubectl", &["--context", KUBE_CONTEXT, "get", "nodes"])
                    .is_some()
                {
                    return Ok(StepOutcome::Skipped);
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
                    TALOS_WORKERS,
                    "--controlplanes",
                    TALOS_CONTROLPLANES,
                    "--provisioner",
                    TALOS_PROVISIONER,
                    "--kubernetes-version",
                    KUBERNETES_VERSION,
                    "--wait-timeout",
                    TALOS_WAIT_TIMEOUT,
                ],
            )
            .map(|_| StepOutcome::Success)
            .map_err(|e| e.to_string())
        },
    )?;

    run_step(&StartStep::with_ok("Setting kubectl context", "Set kubectl context"), || {
        if let Some(current) = kubectl_current_context() {
            if current == KUBE_CONTEXT {
                return Ok(StepOutcome::Skipped);
            }
        }
        kubectl_use_context(KUBE_CONTEXT).map(|()| StepOutcome::Success)
    })?;

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
        &StartStep::with_ok("Setting up Kubernetes resources", "Set up Kubernetes resources"),
        || setup_kubernetes_resources(&registry_ip),
    )?;

    run_step(
        &StartStep::with_ok("Installing Tailscale operator", "Installed Tailscale operator"),
        || install_tailscale_operator(&ts_client_id, &ts_client_secret),
    )?;

    run_step(
        &StartStep::with_ok("Installing FoundationDB operator", "Installed FoundationDB operator"),
        install_fdb_operator,
    )?;

    let tailnet_suffix = run_step_with_result(
        &StartStep::with_ok("Deploying InferaDB", "Deployed InferaDB"),
        || deploy_inferadb(&deploy_dir, &registry_ip),
    )?;

    show_final_success(tailnet_suffix.as_deref());
    Ok(())
}
