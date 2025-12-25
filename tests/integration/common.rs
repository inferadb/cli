//! Common test utilities.

#![allow(dead_code)]

use std::path::PathBuf;
use tempfile::TempDir;

/// Create a temporary configuration directory for tests.
pub fn temp_config_dir() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("inferadb").join("cli.yaml");
    (temp_dir, config_path)
}
