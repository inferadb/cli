//! Configuration system for the `InferaDB` CLI.
//!
//! The configuration follows XDG Base Directory Specification and supports:
//! - User config: `~/.config/inferadb/cli.yaml`
//! - Project config: `.inferadb-cli.yaml` in current directory
//! - Environment variables: `INFERADB_*`
//! - Command-line flags (highest precedence)

mod profile;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub use profile::{CredentialStore, Credentials, Profile};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Main CLI configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Name of the default profile to use.
    #[serde(default)]
    pub default_profile: Option<String>,

    /// Named profiles for different environments.
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,

    /// Output configuration.
    #[serde(default)]
    pub output: OutputConfig,
}

/// Output formatting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Default output format (table, json, yaml).
    #[serde(default = "default_format")]
    pub format: String,

    /// Color output mode (auto, always, never).
    #[serde(default = "default_color")]
    pub color: String,
}

fn default_format() -> String {
    "table".to_string()
}

fn default_color() -> String {
    "auto".to_string()
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self { format: default_format(), color: default_color() }
    }
}

impl Config {
    /// Load configuration from all sources with proper precedence.
    ///
    /// Resolution order (highest to lowest):
    /// 1. CLI flags (handled separately)
    /// 2. Environment variables
    /// 3. Project config (`.inferadb-cli.yaml`)
    /// 4. User config (`~/.config/inferadb/cli.yaml`)
    /// 5. Defaults
    pub fn load() -> Result<Self> {
        let mut config = Self::default();

        // Load user config first (lowest precedence)
        if let Some(path) = Self::user_config_path()
            && path.exists()
        {
            let user_config = Self::load_from_file(&path)?;
            config.merge(user_config);
        }

        // Load project config (higher precedence)
        let project_path = PathBuf::from(".inferadb-cli.yaml");
        if project_path.exists() {
            let project_config = Self::load_from_file(&project_path)?;
            config.merge(project_config);
        }

        // Apply environment variables (highest precedence for profiles)
        config.apply_env_overrides();

        // Auto-create default profile if none exist
        if config.profiles.is_empty() {
            config.profiles.insert("default".to_string(), Profile::default());
            config.default_profile = Some("default".to_string());
        }

        Ok(config)
    }

    /// Load configuration from a YAML file.
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            Error::config(format!("Failed to read config file {}: {}", path.display(), e))
        })?;

        serde_yaml::from_str(&contents).map_err(|e| {
            Error::config(format!("Failed to parse config file {}: {}", path.display(), e))
        })
    }

    /// Save configuration to the user config file.
    pub fn save(&self) -> Result<()> {
        let path = Self::user_config_path()
            .ok_or_else(|| Error::config("Cannot determine config directory"))?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = serde_yaml::to_string(self)?;
        std::fs::write(&path, contents)?;

        Ok(())
    }

    /// Merge another config into this one (other takes precedence).
    fn merge(&mut self, other: Self) {
        if other.default_profile.is_some() {
            self.default_profile = other.default_profile;
        }

        for (name, profile) in other.profiles {
            self.profiles.insert(name, profile);
        }

        if other.output.format != default_format() {
            self.output.format = other.output.format;
        }

        if other.output.color != default_color() {
            self.output.color = other.output.color;
        }
    }

    /// Apply environment variable overrides.
    fn apply_env_overrides(&mut self) {
        if let Ok(profile) = std::env::var("INFERADB_PROFILE") {
            self.default_profile = Some(profile);
        }

        // Build a profile from env vars if they're set
        let url = std::env::var("INFERADB_URL").ok();
        let org = std::env::var("INFERADB_ORG").ok();
        let vault = std::env::var("INFERADB_VAULT").ok();

        if url.is_some() || org.is_some() || vault.is_some() {
            // Create or update an "env" profile for env var overrides
            let env_profile = self.profiles.entry("env".to_string()).or_default();

            if let Some(u) = url {
                env_profile.url = Some(u);
            }
            if let Some(o) = org {
                env_profile.org = Some(o);
            }
            if let Some(v) = vault {
                env_profile.vault = Some(v);
            }
        }
    }

    /// Get the path to the user config file.
    ///
    /// Follows XDG Base Directory Specification:
    /// - Uses `XDG_CONFIG_HOME/inferadb/cli.yaml` if set
    /// - Falls back to `~/.config/inferadb/cli.yaml`
    #[must_use]
    pub fn user_config_path() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join("cli.yaml"))
    }

    /// Get the path to the user config directory.
    ///
    /// Cross-platform behavior:
    /// - If `XDG_CONFIG_HOME` is set, uses `$XDG_CONFIG_HOME/inferadb`
    /// - Linux/macOS: Falls back to `~/.config/inferadb` (XDG default)
    /// - Windows: Falls back to `%APPDATA%\inferadb`
    #[must_use]
    pub fn config_dir() -> Option<PathBuf> {
        // Check XDG_CONFIG_HOME first (works on all platforms if explicitly set)
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME")
            && !xdg_config.is_empty()
        {
            return Some(PathBuf::from(xdg_config).join("inferadb"));
        }

        #[cfg(windows)]
        {
            // Windows: use %APPDATA%\inferadb
            dirs::config_dir().map(|p| p.join("inferadb"))
        }

        #[cfg(not(windows))]
        {
            // Unix (Linux/macOS): use ~/.config/inferadb (XDG default)
            dirs::home_dir().map(|p| p.join(".config").join("inferadb"))
        }
    }

    /// Get the path to the state directory.
    ///
    /// Cross-platform behavior:
    /// - If `XDG_STATE_HOME` is set, uses `$XDG_STATE_HOME/inferadb`
    /// - Linux/macOS: Falls back to `~/.local/state/inferadb` (XDG default)
    /// - Windows: Falls back to `%LOCALAPPDATA%\inferadb`
    #[must_use]
    pub fn state_dir() -> Option<PathBuf> {
        // Check XDG_STATE_HOME first
        if let Ok(xdg_state) = std::env::var("XDG_STATE_HOME")
            && !xdg_state.is_empty()
        {
            return Some(PathBuf::from(xdg_state).join("inferadb"));
        }

        #[cfg(windows)]
        {
            // Windows: use %LOCALAPPDATA%\inferadb
            dirs::data_local_dir().map(|p| p.join("inferadb"))
        }

        #[cfg(not(windows))]
        {
            // Unix (Linux/macOS): use ~/.local/state/inferadb (XDG default)
            dirs::home_dir().map(|p| p.join(".local").join("state").join("inferadb"))
        }
    }

    /// Get the path to the data directory.
    ///
    /// Cross-platform behavior:
    /// - If `XDG_DATA_HOME` is set, uses `$XDG_DATA_HOME/inferadb`
    /// - Linux/macOS: Falls back to `~/.local/share/inferadb` (XDG default)
    /// - Windows: Falls back to `%APPDATA%\inferadb`
    ///
    /// Used for user-specific data files (e.g., deploy repository clone).
    #[must_use]
    pub fn data_dir() -> Option<PathBuf> {
        // Check XDG_DATA_HOME first
        if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME")
            && !xdg_data.is_empty()
        {
            return Some(PathBuf::from(xdg_data).join("inferadb"));
        }

        #[cfg(windows)]
        {
            // Windows: use %APPDATA%\inferadb (same as config on Windows)
            dirs::data_dir().map(|p| p.join("inferadb"))
        }

        #[cfg(not(windows))]
        {
            // Unix (Linux/macOS): use ~/.local/share/inferadb (XDG default)
            dirs::home_dir().map(|p| p.join(".local").join("share").join("inferadb"))
        }
    }

    /// Get a profile by name.
    #[must_use]
    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    /// Get the default profile.
    #[must_use]
    pub fn get_default_profile(&self) -> Option<&Profile> {
        self.default_profile.as_ref().and_then(|name| self.profiles.get(name))
    }

    /// Get the effective profile, considering overrides.
    ///
    /// # Arguments
    /// * `profile_name` - Optional explicit profile name
    /// * `org_override` - Optional org override from CLI
    /// * `vault_override` - Optional vault override from CLI
    pub fn get_effective_profile(
        &self,
        profile_name: Option<&str>,
        org_override: Option<&str>,
        vault_override: Option<&str>,
    ) -> Result<Profile> {
        // Start with the named or default profile
        let base_profile = if let Some(name) = profile_name {
            self.get_profile(name)
                .cloned()
                .ok_or_else(|| Error::ProfileNotFound(name.to_string()))?
        } else {
            self.get_default_profile().cloned().unwrap_or_default()
        };

        // Apply CLI overrides
        let mut profile = base_profile;

        if let Some(org) = org_override {
            profile.org = Some(org.to_string());
        }

        if let Some(vault) = vault_override {
            profile.vault = Some(vault.to_string());
        }

        Ok(profile)
    }

    /// Create or update a profile.
    pub fn set_profile(&mut self, name: String, profile: Profile) {
        self.profiles.insert(name, profile);
    }

    /// Remove a profile.
    pub fn remove_profile(&mut self, name: &str) -> Option<Profile> {
        self.profiles.remove(name)
    }

    /// Set the default profile.
    pub fn set_default(&mut self, name: Option<String>) {
        self.default_profile = name;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.default_profile.is_none());
        assert!(config.profiles.is_empty());
        assert_eq!(config.output.format, "table");
        assert_eq!(config.output.color, "auto");
    }

    #[test]
    fn test_profile_lookup() {
        let mut config = Config::default();
        config.profiles.insert(
            "test".to_string(),
            Profile {
                url: Some("https://test.example.com".to_string()),
                org: Some("org123".to_string()),
                vault: Some("vault456".to_string()),
            },
        );

        assert!(config.get_profile("test").is_some());
        assert!(config.get_profile("nonexistent").is_none());
    }

    #[test]
    fn test_effective_profile_with_overrides() {
        let mut config = Config::default();
        config.profiles.insert(
            "test".to_string(),
            Profile {
                url: Some("https://test.example.com".to_string()),
                org: Some("org123".to_string()),
                vault: Some("vault456".to_string()),
            },
        );

        let profile =
            config.get_effective_profile(Some("test"), None, Some("override789")).unwrap();

        assert_eq!(profile.org, Some("org123".to_string()));
        assert_eq!(profile.vault, Some("override789".to_string()));
    }
}
