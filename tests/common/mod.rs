//! Common test utilities.

use std::path::PathBuf;
use tempfile::TempDir;

/// Create a temporary config directory for testing.
pub fn temp_config_dir() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    (dir, path)
}

/// Create a test profile configuration.
pub fn create_test_config(dir: &std::path::Path) -> PathBuf {
    let config_dir = dir.join("inferadb");
    std::fs::create_dir_all(&config_dir).unwrap();

    let config_path = config_dir.join("cli.yaml");
    let config = r#"
default_profile: test
profiles:
  test:
    url: "https://test.example.com"
    org: "test-org-123"
    vault: "test-vault-456"
  prod:
    url: "https://api.inferadb.com"
    org: "prod-org-789"
    vault: "prod-vault-012"
"#;

    std::fs::write(&config_path, config).unwrap();
    config_path
}
