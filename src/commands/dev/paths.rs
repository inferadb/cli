//! Path helpers for dev commands.
//!
//! Provides consistent paths for configuration, data, and state directories.

use std::path::PathBuf;

use crate::config::Config;

/// Get the deploy directory path (~/.local/share/inferadb/deploy).
pub fn get_deploy_dir() -> PathBuf {
    Config::data_dir().unwrap_or_else(|| PathBuf::from(".local/share/inferadb")).join("deploy")
}

/// Get the engine directory path (~/.local/share/inferadb/engine).
pub fn get_engine_dir() -> PathBuf {
    Config::data_dir().unwrap_or_else(|| PathBuf::from(".local/share/inferadb")).join("engine")
}

/// Get the control directory path (~/.local/share/inferadb/control).
pub fn get_control_dir() -> PathBuf {
    Config::data_dir().unwrap_or_else(|| PathBuf::from(".local/share/inferadb")).join("control")
}

/// Get the dashboard directory path (~/.local/share/inferadb/dashboard).
pub fn get_dashboard_dir() -> PathBuf {
    Config::data_dir().unwrap_or_else(|| PathBuf::from(".local/share/inferadb")).join("dashboard")
}

/// Get the Tailscale credentials file path.
pub fn get_tailscale_creds_file() -> PathBuf {
    Config::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config/inferadb"))
        .join("tailscale-credentials")
}

/// Get the state directory path (~/.local/state/inferadb).
pub fn get_state_dir() -> PathBuf {
    Config::state_dir().unwrap_or_else(|| PathBuf::from(".local/state/inferadb"))
}

/// Get the config directory path (~/.config/inferadb).
pub fn get_config_dir() -> PathBuf {
    Config::config_dir().unwrap_or_else(|| PathBuf::from(".config/inferadb"))
}

/// Get the data directory path (~/.local/share/inferadb).
pub fn get_data_dir() -> PathBuf {
    Config::data_dir().unwrap_or_else(|| PathBuf::from(".local/share/inferadb"))
}
