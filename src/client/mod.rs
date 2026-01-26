//! Client module for `InferaDB` API interactions.
//!
//! Wraps the `inferadb` SDK with CLI-specific functionality like
//! profile-based configuration and credential management.

pub mod auth;

pub use auth::OAuthFlow;
use bon::bon;
use inferadb::{
    BearerCredentialsConfig, Client, VaultClient,
    client::OrganizationClient,
    control::{AccountClient, JwksClient, OrganizationsClient},
};

use crate::{
    config::{Config, CredentialStore, Credentials, Profile},
    error::{Error, Result},
};

/// CLI client that wraps the `InferaDB` SDK client.
pub struct CliClient {
    inner: Client,
    org_id: String,
    vault_id: String,
}

impl CliClient {
    /// Create a new CLI client from a profile and credentials.
    pub async fn from_profile(profile: &Profile, credentials: &Credentials) -> Result<Self> {
        let url = profile.url_or_default();
        let org_id = profile.org()?.to_string();
        let vault_id = profile.vault()?.to_string();

        let inner = Client::builder()
            .url(url)
            .credentials(BearerCredentialsConfig::new(&credentials.access_token))
            .build()
            .await?;

        Ok(Self { inner, org_id, vault_id })
    }

    /// Create a CLI client using configuration and stored credentials.
    ///
    /// # Arguments
    /// * `config` - CLI configuration
    /// * `profile_name` - Optional explicit profile name
    /// * `org_override` - Optional organization override
    /// * `vault_override` - Optional vault override
    pub async fn from_config(
        config: &Config,
        profile_name: Option<&str>,
        org_override: Option<&str>,
        vault_override: Option<&str>,
    ) -> Result<Self> {
        let profile = config.get_effective_profile(profile_name, org_override, vault_override)?;

        // Determine which profile name to use for credentials
        let cred_profile = profile_name
            .map(std::string::ToString::to_string)
            .or_else(|| config.default_profile.clone())
            .unwrap_or_else(|| "default".to_string());

        // Load credentials from keychain
        let store = CredentialStore::new();
        let credentials = store.load(&cred_profile)?.ok_or(Error::AuthRequired)?;

        // Check if credentials are expired
        if credentials.is_expired() {
            return Err(Error::AuthRequired);
        }

        Self::from_profile(&profile, &credentials).await
    }

    /// Get the underlying `InferaDB` client.
    #[must_use]
    pub const fn inner(&self) -> &Client {
        &self.inner
    }

    /// Get a vault client for the configured organization and vault.
    #[must_use]
    pub fn vault(&self) -> VaultClient {
        self.inner.organization(&self.org_id).vault(&self.vault_id)
    }

    /// Get the organization ID.
    #[must_use]
    pub fn org_id(&self) -> &str {
        &self.org_id
    }

    /// Get the vault ID.
    #[must_use]
    pub fn vault_id(&self) -> &str {
        &self.vault_id
    }

    /// Get an organizations client for listing and creating organizations.
    #[must_use]
    pub fn organizations(&self) -> OrganizationsClient {
        self.inner.organizations()
    }

    /// Get a client for organization-level operations.
    pub fn organization(&self, org_id: impl Into<String>) -> OrganizationClient {
        self.inner.organization(org_id)
    }

    /// Get an account client for the current user.
    #[must_use]
    pub fn account(&self) -> AccountClient {
        self.inner.account()
    }

    /// Get a JWKS client for key operations.
    #[must_use]
    pub fn jwks(&self) -> JwksClient {
        self.inner.jwks()
    }

    /// Check service health.
    pub async fn health(&self) -> Result<inferadb::HealthResponse> {
        Ok(self.inner.health().await?)
    }

    /// Wait for service to be ready.
    pub async fn wait_ready(&self, timeout: std::time::Duration) -> Result<()> {
        Ok(self.inner.wait_ready(timeout).await?)
    }
}

/// Context for CLI command execution.
///
/// Contains everything needed to execute CLI commands:
/// - Configuration
/// - Resolved profile
/// - Optional client (lazy-initialized)
pub struct Context {
    /// CLI configuration.
    pub config: Config,

    /// Effective profile after applying overrides.
    pub profile: Profile,

    /// Profile name being used.
    pub profile_name: Option<String>,

    /// Output configuration.
    pub output: crate::output::Output,

    /// Skip confirmations.
    pub yes: bool,

    /// Debug mode.
    pub debug: bool,
}

#[bon]
impl Context {
    /// Create a new context from CLI options.
    #[builder]
    pub fn new(
        profile_name: Option<String>,
        org_override: Option<String>,
        vault_override: Option<String>,
        output_format: String,
        color: String,
        #[builder(default)] quiet: bool,
        #[builder(default)] yes: bool,
        #[builder(default)] debug: bool,
    ) -> Result<Self> {
        let config = Config::load()?;

        let profile = config.get_effective_profile(
            profile_name.as_deref(),
            org_override.as_deref(),
            vault_override.as_deref(),
        )?;

        let output = crate::output::Output::from_cli(&output_format, &color, quiet)?;

        Ok(Self { config, profile, profile_name, output, yes, debug })
    }

    /// Create a client using the context configuration.
    pub async fn client(&self) -> Result<CliClient> {
        CliClient::from_config(&self.config, self.profile_name.as_deref(), None, None).await
    }

    /// Get credentials for the current profile.
    pub fn credentials(&self) -> Result<Credentials> {
        let store = CredentialStore::new();
        let profile_name = self
            .profile_name
            .as_deref()
            .or(self.config.default_profile.as_deref())
            .unwrap_or("default");

        store.load(profile_name)?.ok_or(Error::AuthRequired)
    }

    /// Check if the user is authenticated.
    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        self.credentials().is_ok()
    }

    /// Prompt for confirmation (respects --yes flag).
    pub fn confirm(&self, message: &str) -> Result<bool> {
        if self.yes {
            return Ok(true);
        }

        crate::tui::confirm(message)
    }

    /// Prompt for confirmation for dangerous operations (respects --yes flag).
    pub fn confirm_danger(&self, message: &str) -> Result<bool> {
        if self.yes {
            return Ok(true);
        }

        crate::tui::confirm_danger(message)
    }

    /// Get the profile name being used.
    #[must_use]
    pub fn effective_profile_name(&self) -> &str {
        self.profile_name.as_deref().or(self.config.default_profile.as_deref()).unwrap_or("default")
    }

    /// Get the organization ID from the profile.
    #[must_use]
    pub fn profile_org_id(&self) -> Option<&str> {
        self.profile.org.as_deref()
    }

    /// Get the vault ID from the profile.
    #[must_use]
    pub fn profile_vault_id(&self) -> Option<&str> {
        self.profile.vault.as_deref()
    }

    /// Require an organization ID from the profile.
    pub fn require_org_id(&self) -> Result<String> {
        self.profile.org.clone().ok_or_else(|| {
            Error::config("No organization configured. Use --org or set org in your profile.")
        })
    }

    /// Require a vault ID from the profile.
    pub fn require_vault_id(&self) -> Result<String> {
        self.profile.vault.clone().ok_or_else(|| {
            Error::config("No vault configured. Use --vault or set vault in your profile.")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        // This test requires no config file to exist
        let ctx = Context::builder()
            .org_override("org123".to_string())
            .vault_override("vault456".to_string())
            .output_format("table".to_string())
            .color("never".to_string())
            .yes(true)
            .build();

        assert!(ctx.is_ok());
    }
}
