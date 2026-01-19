//! Kubernetes operations for dev commands.
//!
//! Provides kubectl abstractions for common operations.

use super::{
    commands::{run_command, run_command_optional},
    constants::INFERADB_NAMESPACE,
};

/// Get JSON output from kubectl for a resource type.
///
/// Returns parsed JSON value or None if the command fails.
pub fn kubectl_get_json(resource: &str, namespace: &str) -> Option<serde_json::Value> {
    let output =
        run_command_optional("kubectl", &["get", resource, "-n", namespace, "-o", "json"])?;
    serde_json::from_str(&output).ok()
}

/// Get JSON output from kubectl with a label selector.
pub fn kubectl_get_json_with_selector(
    resource: &str,
    namespace: &str,
    selector: &str,
) -> Option<serde_json::Value> {
    let output = run_command_optional(
        "kubectl",
        &["get", resource, "-n", namespace, "-l", selector, "-o", "json"],
    )?;
    serde_json::from_str(&output).ok()
}

/// Generic helper to extract items from kubectl JSON output.
///
/// Takes a mapper function that extracts the desired data from each item.
pub fn kubectl_list<T, F>(resource: &str, namespace: &str, mapper: F) -> Vec<T>
where
    F: Fn(&serde_json::Value) -> Option<T>,
{
    let Some(json) = kubectl_get_json(resource, namespace) else {
        return Vec::new();
    };

    json.get("items")
        .and_then(|i| i.as_array())
        .map(|items| items.iter().filter_map(&mapper).collect())
        .unwrap_or_default()
}

/// Generic helper to extract items from kubectl JSON output with a label selector.
pub fn kubectl_list_with_selector<T, F>(
    resource: &str,
    namespace: &str,
    selector: &str,
    mapper: F,
) -> Vec<T>
where
    F: Fn(&serde_json::Value) -> Option<T>,
{
    let Some(json) = kubectl_get_json_with_selector(resource, namespace, selector) else {
        return Vec::new();
    };

    json.get("items")
        .and_then(|i| i.as_array())
        .map(|items| items.iter().filter_map(&mapper).collect())
        .unwrap_or_default()
}

/// Get the current kubectl context.
pub fn kubectl_current_context() -> Option<String> {
    run_command_optional("kubectl", &["config", "current-context"]).map(|s| s.trim().to_string())
}

/// Set the kubectl context.
pub fn kubectl_use_context(context: &str) -> Result<(), String> {
    run_command("kubectl", &["config", "use-context", context]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Create a namespace if it doesn't exist.
pub fn ensure_namespace(namespace: &str) -> Result<(), String> {
    // Check if namespace exists
    if run_command_optional("kubectl", &["get", "namespace", namespace]).is_some() {
        return Ok(());
    }

    // Create namespace
    run_command("kubectl", &["create", "namespace", namespace]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get `InferaDB` deployments in the inferadb namespace.
/// Returns: Vec<(name, replicas, `image_tag`)>
pub fn get_inferadb_deployments() -> Vec<(String, String, String)> {
    let selector =
        "app.kubernetes.io/name in (inferadb-engine,inferadb-control,inferadb-dashboard)";

    kubectl_list_with_selector("deployments", INFERADB_NAMESPACE, selector, |item| {
        let name = item.pointer("/metadata/name").and_then(|v| v.as_str())?.to_string();

        let replicas = item
            .pointer("/spec/replicas")
            .and_then(serde_json::Value::as_i64)
            .map_or_else(|| "1".to_string(), |r| r.to_string());

        // Get image and extract just the tag
        let image = item
            .pointer("/spec/template/spec/containers/0/image")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let image_tag = image.rfind(':').map_or_else(
            || image.rfind('/').map_or(image, |slash_pos| &image[slash_pos + 1..]),
            |tag_pos| &image[tag_pos + 1..],
        );

        Some((name, replicas, image_tag.to_string()))
    })
}

/// Get PVCs in the inferadb namespace.
/// Returns: Vec<(name, size, status)>
pub fn get_pvcs() -> Vec<(String, String, String)> {
    kubectl_list("pvc", INFERADB_NAMESPACE, |item| {
        let name = item.pointer("/metadata/name").and_then(|v| v.as_str())?.to_string();

        let size = item
            .pointer("/spec/resources/requests/storage")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let status =
            item.pointer("/status/phase").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();

        Some((name, size, status))
    })
}

/// Check if a Helm repo exists.
pub fn helm_repo_exists(name: &str) -> bool {
    run_command_optional("helm", &["repo", "list", "-o", "json"]).is_some_and(|output| {
        output.contains(&format!("\"{name}\"")) || output.contains(&format!("\"name\":\"{name}\""))
    })
}

/// Add a Helm repository.
pub fn helm_repo_add(name: &str, url: &str) -> Result<(), String> {
    run_command("helm", &["repo", "add", name, url]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Update Helm repositories.
pub fn helm_repo_update() -> Result<(), String> {
    run_command("helm", &["repo", "update"]).map_err(|e| e.to_string())?;
    Ok(())
}
