//! Docker operations for dev commands.
//!
//! Provides utilities for working with Docker containers, images, and the local registry.

use super::{
    commands::{run_command, run_command_optional},
    constants::{CLUSTER_NAME, REGISTRY_NAME},
};

/// Check if a Docker container exists.
pub fn docker_container_exists(name: &str) -> bool {
    run_command_optional(
        "docker",
        &["ps", "-a", "--filter", &format!("name={name}"), "--format", "{{.Names}}"],
    )
    .is_some_and(|output| output.lines().any(|line| line.contains(name)))
}

/// Check if a specific container is paused.
pub fn is_container_paused(container: &str) -> bool {
    run_command_optional("docker", &["inspect", container, "--format", "{{.State.Paused}}"])
        .is_some_and(|output| output.trim() == "true")
}

/// Get all Docker containers for the cluster.
pub fn get_cluster_containers() -> Vec<String> {
    run_command_optional(
        "docker",
        &["ps", "-a", "--filter", &format!("name={CLUSTER_NAME}"), "--format", "{{.Names}}"],
    )
    .map(|output| output.lines().filter(|line| !line.is_empty()).map(String::from).collect())
    .unwrap_or_default()
}

/// Get expected cluster container names.
pub fn get_expected_cluster_containers() -> Vec<String> {
    vec![format!("{}-controlplane-1", CLUSTER_NAME), format!("{}-worker-1", CLUSTER_NAME)]
}

/// Check if cluster containers are paused.
pub fn are_containers_paused() -> bool {
    run_command_optional(
        "docker",
        &[
            "ps",
            "-a",
            "--filter",
            &format!("name={CLUSTER_NAME}"),
            "--filter",
            "status=paused",
            "--format",
            "{{.Names}}",
        ],
    )
    .is_some_and(|output| !output.trim().is_empty())
}

/// Get Docker container IP on a specific network.
pub fn get_container_ip(container_name: &str) -> Option<String> {
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

/// Get list of dev-related Docker images.
pub fn get_dev_docker_images() -> Vec<String> {
    let mut images = Vec::new();

    // Get inferadb-* images
    if let Some(output) = run_command_optional(
        "docker",
        &["images", "--format", "{{.Repository}}:{{.Tag}}", "inferadb-*"],
    ) {
        for line in output.lines() {
            if !line.is_empty() && !line.contains("<none>") {
                images.push(line.to_string());
            }
        }
    }

    // Get Talos-related images
    if let Some(output) =
        run_command_optional("docker", &["images", "--format", "{{.Repository}}:{{.Tag}}"])
    {
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

/// Remove a Docker image by name.
///
/// Returns true if the image was removed, false otherwise.
pub fn remove_image(image: &str) -> bool {
    run_command_optional("docker", &["rmi", "-f", image]).is_some()
}

/// Check if the registry container exists.
pub fn registry_exists() -> bool {
    docker_container_exists(REGISTRY_NAME)
}

/// Check if the cluster container exists.
pub fn cluster_exists() -> bool {
    docker_container_exists(CLUSTER_NAME)
}

/// Check if Docker daemon is running.
pub fn is_docker_running() -> bool {
    run_command_optional("docker", &["info"]).is_some()
}

/// Pull a Docker image.
pub fn pull_image(image: &str) -> Result<(), String> {
    run_command("docker", &["pull", image]).map(|_| ()).map_err(|e| e.to_string())
}
