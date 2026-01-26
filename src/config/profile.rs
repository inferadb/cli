//! Profile management for multi-environment support.
//!
//! A profile represents a complete target environment with URL, organization,
//! vault, and authentication credentials.

use bon::Builder;
use serde::{Deserialize, Serialize};

/// A named profile representing a complete connection target.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    /// API endpoint URL.
    #[serde(default)]
    pub url: Option<String>,

    /// Organization ID (Snowflake ID).
    #[serde(default)]
    pub org: Option<String>,

    /// Vault ID (Snowflake ID).
    #[serde(default)]
    pub vault: Option<String>,
}

impl Profile {
    /// Create a new profile with all fields specified.
    pub fn new(url: impl Into<String>, org: impl Into<String>, vault: impl Into<String>) -> Self {
        Self { url: Some(url.into()), org: Some(org.into()), vault: Some(vault.into()) }
    }

    /// Get the URL, returning an error if not set.
    pub fn url(&self) -> crate::Result<&str> {
        self.url.as_deref().ok_or_else(|| crate::error::Error::config("API URL not configured"))
    }

    /// Get the URL or a default value.
    #[must_use]
    pub fn url_or_default(&self) -> &str {
        self.url.as_deref().unwrap_or("https://api.inferadb.com")
    }

    /// Get the organization ID, returning an error if not set.
    pub fn org(&self) -> crate::Result<&str> {
        self.org.as_deref().ok_or(crate::error::Error::OrgNotSpecified)
    }

    /// Get the vault ID, returning an error if not set.
    pub fn vault(&self) -> crate::Result<&str> {
        self.vault.as_deref().ok_or(crate::error::Error::VaultNotSpecified)
    }

    /// Check if the profile has enough information to connect.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.url.is_some() && self.org.is_some() && self.vault.is_some()
    }

    /// Validate the profile has required fields.
    pub fn validate(&self) -> crate::Result<()> {
        if self.url.is_none() {
            return Err(crate::error::Error::config("Profile missing URL"));
        }
        if self.org.is_none() {
            return Err(crate::error::Error::OrgNotSpecified);
        }
        if self.vault.is_none() {
            return Err(crate::error::Error::VaultNotSpecified);
        }
        Ok(())
    }
}

/// Stored credentials for a profile.
///
/// Credentials are stored separately from the config file,
/// typically in the OS keychain.
#[derive(Debug, Clone, Builder)]
pub struct Credentials {
    /// Access token for API authentication.
    #[builder(into)]
    pub access_token: String,

    /// Optional refresh token for token renewal.
    #[builder(into)]
    pub refresh_token: Option<String>,

    /// Token expiration timestamp.
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Credentials {
    /// Check if the credentials are expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|expires_at| chrono::Utc::now() >= expires_at)
    }

    /// Check if the credentials will expire soon (within 5 minutes).
    #[must_use]
    pub fn expires_soon(&self) -> bool {
        self.expires_at.is_some_and(|expires_at| {
            let threshold = chrono::Utc::now() + chrono::Duration::minutes(5);
            threshold >= expires_at
        })
    }

    /// Check if the credentials can be refreshed.
    #[must_use]
    pub const fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }
}

/// Credential storage using the OS keychain.
pub struct CredentialStore {
    service: String,
}

impl CredentialStore {
    /// Create a new credential store.
    #[must_use]
    pub fn new() -> Self {
        Self { service: "inferadb-cli".to_string() }
    }

    /// Get the keyring entry for a profile.
    fn entry(&self, profile: &str) -> keyring::Result<keyring::Entry> {
        keyring::Entry::new(&self.service, profile)
    }

    /// Store credentials for a profile.
    pub fn store(&self, profile: &str, credentials: &Credentials) -> crate::Result<()> {
        let entry = self.entry(profile)?;

        // Store as JSON for structured data
        let data = serde_json::json!({
            "access_token": credentials.access_token,
            "refresh_token": credentials.refresh_token,
            "expires_at": credentials.expires_at,
        });

        entry.set_password(&data.to_string())?;
        Ok(())
    }

    /// Load credentials for a profile.
    pub fn load(&self, profile: &str) -> crate::Result<Option<Credentials>> {
        let entry = self.entry(profile)?;

        match entry.get_password() {
            Ok(data) => {
                let value: serde_json::Value = serde_json::from_str(&data)?;

                let access_token = value["access_token"]
                    .as_str()
                    .ok_or_else(|| crate::error::Error::credential("Missing access token"))?
                    .to_string();

                let refresh_token =
                    value["refresh_token"].as_str().map(std::string::ToString::to_string);

                let expires_at = value["expires_at"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc));

                Ok(Some(Credentials { access_token, refresh_token, expires_at }))
            },
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete credentials for a profile.
    pub fn delete(&self, profile: &str) -> crate::Result<()> {
        let entry = self.entry(profile)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()), // Already deleted is OK
            Err(e) => Err(e.into()),
        }
    }

    /// Check if credentials exist for a profile.
    #[must_use]
    pub fn exists(&self, profile: &str) -> bool {
        self.entry(profile).map(|e| e.get_password().is_ok()).unwrap_or(false)
    }
}

impl Default for CredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_new() {
        let profile = Profile::new("https://api.example.com", "org123", "vault456");
        assert_eq!(profile.url, Some("https://api.example.com".to_string()));
        assert_eq!(profile.org, Some("org123".to_string()));
        assert_eq!(profile.vault, Some("vault456".to_string()));
        assert!(profile.is_complete());
    }

    #[test]
    fn test_profile_incomplete() {
        let profile = Profile::default();
        assert!(!profile.is_complete());
    }

    #[test]
    fn test_credentials_expiry() {
        let creds = Credentials::builder()
            .access_token("token")
            .refresh_token("refresh")
            .expires_at(chrono::Utc::now() + chrono::Duration::hours(1))
            .build();
        assert!(!creds.is_expired());
        assert!(!creds.expires_soon());

        let expired = Credentials::builder()
            .access_token("token")
            .refresh_token("refresh")
            .expires_at(chrono::Utc::now() - chrono::Duration::hours(1))
            .build();
        assert!(expired.is_expired());
    }

    #[test]
    fn test_credentials_builder_defaults() {
        let creds = Credentials::builder().access_token("token").build();
        assert_eq!(creds.access_token, "token");
        assert!(creds.refresh_token.is_none());
        assert!(creds.expires_at.is_none());
        assert!(!creds.can_refresh());
    }
}
