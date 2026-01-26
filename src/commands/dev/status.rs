//! Status command for dev cluster.
//!
//! Displays cluster status, node status, pod status, and URLs.

use std::sync::Arc;

use teapot::style::{Color, RESET};

use super::{
    commands::run_command_optional,
    constants::{TIP_RESUME_CLUSTER, TIP_START_CLUSTER},
    docker::{are_containers_paused, cluster_exists},
    output::{
        print_colored_prefix_dot_leader, print_hint, print_prefixed_dot_leader,
        print_section_header, print_styled_header,
    },
};
use crate::{
    client::Context,
    error::{Error, Result},
    tui::{ClusterStatus, RefreshFn, RefreshResult, TabData},
};

// ============================================================================
// Cluster Status
// ============================================================================

/// Get current cluster status.
pub fn get_cluster_status() -> ClusterStatus {
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
// Public API
// ============================================================================

/// Run dev status - show cluster status.
pub async fn dev_status(ctx: &Context, interactive: bool) -> Result<()> {
    if interactive && crate::tui::is_interactive(ctx) {
        return status_interactive();
    }
    status_with_spinners();
    Ok(())
}

// ============================================================================
// Streaming Mode
// ============================================================================

/// Status with inline spinners.
fn status_with_spinners() {
    let green = Color::Green.to_ansi_fg();
    let yellow = Color::Yellow.to_ansi_fg();
    let red = Color::Red.to_ansi_fg();
    let reset = RESET;

    print_styled_header("InferaDB Development Cluster Status");
    println!();

    let cluster_status = get_cluster_status();
    match cluster_status {
        ClusterStatus::Offline => {
            let prefix = format!("{red}✗{reset}");
            let status = format!("{red}NOT RUNNING{reset}");
            print_colored_prefix_dot_leader(&prefix, 1, "Cluster", &status);
            println!();
            print_hint(TIP_START_CLUSTER);
            return;
        },
        ClusterStatus::Paused => {
            let prefix = format!("{yellow}⚠{reset}");
            let status = format!("{yellow}STOPPED{reset}");
            print_colored_prefix_dot_leader(&prefix, 1, "Cluster", &status);
            println!();
            print_hint(TIP_RESUME_CLUSTER);
            return;
        },
        ClusterStatus::Online => {
            let prefix = format!("{green}✓{reset}");
            print_colored_prefix_dot_leader(&prefix, 1, "Cluster", "RUNNING");
        },
        ClusterStatus::Unknown => {
            let prefix = format!("{yellow}○{reset}");
            print_colored_prefix_dot_leader(&prefix, 1, "Cluster", "UNKNOWN");
        },
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
}

/// Print formatted node status.
fn print_nodes_status() {
    let output = run_command_optional("kubectl", &["get", "nodes", "-o", "json"]);

    let green = Color::Green.to_ansi_fg();
    let red = Color::Red.to_ansi_fg();
    let reset = RESET;

    if let Some(output) = output
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&output)
        && let Some(items) = json["items"].as_array()
    {
        for node in items {
            let name = node["metadata"]["name"].as_str().unwrap_or("");
            let labels = &node["metadata"]["labels"];
            let is_control_plane = labels.get("node-role.kubernetes.io/control-plane").is_some();

            let ready = node["status"]["conditions"]
                .as_array()
                .and_then(|conditions| conditions.iter().find(|c| c["type"] == "Ready"))
                .is_some_and(|c| c["status"] == "True");

            let role = if is_control_plane { "control-plane" } else { "worker" };
            let status = if ready {
                format!("{green}Ready{reset} ({role})")
            } else {
                format!("{red}NotReady{reset} ({role})")
            };

            let display_name = name.strip_prefix("inferadb-dev-").unwrap_or(name);
            print_prefixed_dot_leader(" ", display_name, &status);
        }
    }
}

/// Print formatted pod status.
fn print_pods_status() {
    let green = Color::Green.to_ansi_fg();
    let yellow = Color::Yellow.to_ansi_fg();
    let red = Color::Red.to_ansi_fg();
    let reset = RESET;

    let inferadb_pods = run_command_optional(
        "kubectl",
        &[
            "get",
            "pods",
            "-n",
            "inferadb",
            "-o",
            "jsonpath={range .items[*]}{.metadata.name}|{.status.phase}|{.status.containerStatuses[*].ready}{\"\\n\"}{end}",
        ],
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

            let ready_count = ready_statuses.split_whitespace().filter(|s| *s == "true").count();
            let total_count = ready_statuses.split_whitespace().count().max(1);

            let status = match phase {
                "Running" => format!("{green}{ready_count}/{total_count} Running{reset}"),
                "Pending" => format!("{yellow}{ready_count}/{total_count} Pending{reset}"),
                _ => format!("{red}{ready_count}/{total_count} {phase}{reset}"),
            };

            let display_name = {
                let base = name
                    .strip_prefix("dev-inferadb-")
                    .or_else(|| name.strip_prefix("inferadb-"))
                    .unwrap_or(name);
                base.split('-')
                    .take_while(|s| !s.chars().all(|c| c.is_ascii_digit()) && s.len() < 9)
                    .collect::<Vec<_>>()
                    .join("-")
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
            if let Some((name, status)) = format_pod(line)
                && seen_names.insert(name.clone())
            {
                print_prefixed_dot_leader(" ", &name, &status);
            }
        }
    }
}

/// Print formatted URLs.
fn print_urls_status() {
    let output = run_command_optional(
        "kubectl",
        &[
            "get",
            "ingress",
            "-n",
            "inferadb",
            "-o",
            "jsonpath={range .items[*]}{.metadata.name}|{.status.loadBalancer.ingress[0].hostname}{\"\\n\"}{end}",
        ],
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

                let url = format!("https://{hostname}");
                print_prefixed_dot_leader(" ", label, &url);
            }
        }
    }
}

// ============================================================================
// Interactive Mode
// ============================================================================

/// Status interactive TUI mode.
fn status_interactive() -> Result<()> {
    use teapot::{
        output::{terminal_height, terminal_width},
        runtime::{Program, ProgramOptions},
    };

    use crate::tui::DevStatusView;

    let width = terminal_width();
    let height = terminal_height();

    let initial_data = fetch_status_data();

    let view = DevStatusView::builder()
        .width(width)
        .height(height)
        .refresh_fn(Arc::new(fetch_status_data) as RefreshFn)
        .cluster_status(initial_data.cluster_status)
        .urls_data(initial_data.urls)
        .services_data(initial_data.services)
        .nodes_data(initial_data.nodes)
        .pods_data(initial_data.pods)
        .build();

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

    RefreshResult { cluster_status, urls, services, nodes, pods }
}
