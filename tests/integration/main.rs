//! Integration tests for the `InferaDB` CLI.

#![allow(clippy::unwrap_used)] // Tests can use unwrap for cleaner assertions

mod common;

use assert_cmd::Command;
use predicates::prelude::*;

/// Helper to create a command for the inferadb binary.
fn inferadb_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("inferadb"))
}

/// Test that the CLI shows help.
#[test]
fn test_help() {
    inferadb_cmd().arg("--help").assert().success().stdout(predicate::str::contains("InferaDB"));
}

/// Test that the CLI shows version.
#[test]
fn test_version() {
    inferadb_cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

/// Test that unrecognized commands fail.
#[test]
fn test_unknown_command() {
    inferadb_cmd().arg("unknown-command").assert().failure();
}

/// Test whoami command without auth.
#[test]
fn test_whoami_without_auth() {
    inferadb_cmd()
        .arg("whoami")
        .assert()
        .success()
        .stdout(predicate::str::contains("Authenticated: no"));
}

/// Test profiles list creates default profile when no config exists.
#[test]
fn test_profiles_list_creates_default() {
    // Use a temp config directory with no existing config
    inferadb_cmd()
        .env("XDG_CONFIG_HOME", "/tmp/inferadb-test-empty")
        .arg("profiles")
        .arg("list")
        .assert()
        .success()
        // A default profile should be auto-created
        .stdout(predicate::str::contains("default"));
}

/// Test check command format.
#[test]
fn test_check_help() {
    inferadb_cmd()
        .args(["check", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("SUBJECT"))
        .stdout(predicate::str::contains("PERMISSION"))
        .stdout(predicate::str::contains("RESOURCE"));
}

/// Test cheatsheet command.
#[test]
fn test_cheatsheet() {
    inferadb_cmd()
        .arg("cheatsheet")
        .assert()
        .success()
        .stdout(predicate::str::contains("Cheatsheet"));
}

/// Test @profile prefix parsing.
#[test]
fn test_profile_prefix() {
    use inferadb_cli::cli::parse_profile_prefix;

    let args = vec!["inferadb".to_string(), "@prod".to_string(), "check".to_string()];
    let (profile, remaining) = parse_profile_prefix(args);
    assert_eq!(profile, Some("prod".to_string()));
    assert_eq!(remaining.len(), 2);
}

#[cfg(test)]
mod config_tests {
    use inferadb_cli::config::{Config, Profile};
    use tempfile::TempDir;

    #[test]
    fn test_config_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("cli.yaml");

        let mut config = Config::default();
        config
            .profiles
            .insert("test".to_string(), Profile::new("https://example.com", "org123", "vault456"));
        config.default_profile = Some("test".to_string());

        // Save
        let yaml = serde_yaml::to_string(&config).unwrap();
        std::fs::write(&config_path, yaml).unwrap();

        // Load
        let loaded: Config =
            serde_yaml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(loaded.default_profile, Some("test".to_string()));
        assert!(loaded.profiles.contains_key("test"));
    }

    #[test]
    fn test_profile_validation() {
        let profile = Profile::default();
        assert!(!profile.is_complete());

        let complete = Profile::new("https://api.inferadb.com", "org123", "vault456");
        assert!(complete.is_complete());
    }
}

#[cfg(test)]
mod output_tests {
    use inferadb_cli::output::OutputFormat;
    use teapot::components::{Column, Table};

    #[test]
    fn test_output_format_parse() {
        assert_eq!(OutputFormat::parse("table").unwrap(), OutputFormat::Table);
        assert_eq!(OutputFormat::parse("json").unwrap(), OutputFormat::Json);
        assert_eq!(OutputFormat::parse("yaml").unwrap(), OutputFormat::Yaml);
        assert!(OutputFormat::parse("invalid").is_err());
    }

    #[test]
    fn test_table_render() {
        let table = Table::new()
            .columns(vec![Column::new("Name"), Column::new("Value")])
            .rows(vec![vec!["foo".to_string(), "bar".to_string()]])
            .focused(false);

        let output = table.render();
        assert!(output.contains("Name"));
        assert!(output.contains("foo"));
    }
}
