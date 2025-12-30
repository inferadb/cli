//! Docker operations for dev commands.
//!
//! Provides utilities for working with Docker containers, images, and the local registry.

use super::commands::run_command_optional;
use super::constants::{CLUSTER_NAME, REGISTRY_NAME};

/// Check if a Docker container exists.
pub fn docker_container_exists(name: &str) -> bool {
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

/// Get all Docker containers for the cluster.
pub fn get_cluster_containers() -> Vec<String> {
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

/// Check if cluster containers are paused.
pub fn are_containers_paused() -> bool {
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

/// Check if the registry container exists.
pub fn registry_exists() -> bool {
    docker_container_exists(REGISTRY_NAME)
}

/// Check if the cluster container exists.
pub fn cluster_exists() -> bool {
    docker_container_exists(CLUSTER_NAME)
}
