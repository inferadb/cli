//! Shell command wrappers for dev commands.
//!
//! Provides utilities for running external commands with various options
//! for output handling, error handling, and streaming.

use std::process::{Command, Stdio};

use crate::error::{Error, Result};

/// Check if a command is available in PATH.
pub fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run a command and return its output.
pub fn run_command(cmd: &str, args: &[&str]) -> Result<String> {
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

/// Run a command, returning Ok(output) on success or None on failure.
pub fn run_command_optional(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

/// Run a command with live output streaming.
pub fn run_command_streaming(cmd: &str, args: &[&str], env_vars: &[(&str, &str)]) -> Result<()> {
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

/// Normalize a version string to "vX.Y.Z" format, including build number if present.
pub fn normalize_version(raw: &str) -> String {
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
pub fn extract_version_string(output: &str, command: &str) -> String {
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

/// Parse a kubectl apply output line into (resource, status).
/// Example: "deployment.apps/inferadb-control created" -> ("deployment.apps/inferadb-control", "created")
pub fn parse_kubectl_apply_line(line: &str) -> Option<(String, String)> {
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
