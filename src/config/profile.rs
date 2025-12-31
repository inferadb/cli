//! Profile management for multi-environment support.
//!
//! A profile represents a complete target environment with URL, organization,
//! vault, and authentication credentials.

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
    pub fn is_complete(&self) -> bool {
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
#[derive(Debug, Clone)]
pub struct Credentials {
    /// Access token for API authentication.
    pub access_token: String,

    /// Optional refresh token for token renewal.
    pub refresh_token: Option<String>,

    /// Token expiration timestamp.
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Credentials {
    /// Create new credentials with just an access token.
    pub fn new(access_token: impl Into<String>) -> Self {
        Self { access_token: access_token.into(), refresh_token: None, expires_at: None }
    }

    /// Create credentials with all fields.
    pub fn with_refresh(
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            refresh_token: Some(refresh_token.into()),
            expires_at: Some(expires_at),
        }
    }

    /// Check if the credentials are expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            chrono::Utc::now() >= expires_at
        } else {
            false
        }
    }

    /// Check if the credentials will expire soon (within 5 minutes).
    pub fn expires_soon(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let threshold = chrono::Utc::now() + chrono::Duration::minutes(5);
            threshold >= expires_at
        } else {
            false
        }
    }

    /// Check if the credentials can be refreshed.
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }
}

/// Credential storage using the OS keychain.
pub struct CredentialStore {
    service: String,
}

impl CredentialStore {
    /// Create a new credential store.
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

                let refresh_token = value["refresh_token"].as_str().map(|s| s.to_string());

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
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
            Err(e) => Err(e.into()),
        }
    }

    /// Check if credentials exist for a profile.
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
        let creds = Credentials::with_refresh(
            "token",
            "refresh",
            chrono::Utc::now() + chrono::Duration::hours(1),
        );
        assert!(!creds.is_expired());
        assert!(!creds.expires_soon());

        let expired = Credentials::with_refresh(
            "token",
            "refresh",
            chrono::Utc::now() - chrono::Duration::hours(1),
        );
        assert!(expired.is_expired());
    }
}
