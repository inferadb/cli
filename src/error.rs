//! Error types for the `InferaDB` CLI.
//!
//! This module provides structured error handling with semantic exit codes
//! following the CLI specification.

use std::{borrow::Cow, io};

use thiserror::Error;

use crate::t;

/// CLI-specific error type.
#[derive(Debug, Error)]
pub enum Error {
    /// Configuration error (invalid config file, missing required values).
    #[error("Configuration error: {0}")]
    Config(String),

    /// Authentication required but not available.
    #[error("Authentication required. Run 'inferadb login' first.")]
    AuthRequired,

    /// Specified profile does not exist.
    #[error("Profile '{0}' not found. Run 'inferadb profiles list' to see available profiles.")]
    ProfileNotFound(String),

    /// Organization not specified and no default configured.
    #[error("Organization not specified. Use --org or configure a default profile.")]
    OrgNotSpecified,

    /// Vault not specified and no default configured.
    #[error("Vault not specified. Use --vault or configure a default profile.")]
    VaultNotSpecified,

    /// API/SDK error from the `InferaDB` SDK.
    #[error("{0}")]
    Api(#[from] inferadb::Error),

    /// IO error (file operations, network, etc.).
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML serialization/deserialization error.
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// Invalid command-line argument.
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Resource parsing error (invalid tuple format, etc.).
    #[error("Parse error: {0}")]
    Parse(String),

    /// Keyring/credential storage error.
    #[error("Credential storage error: {0}")]
    Credential(String),

    /// OAuth authentication error.
    #[error("Authentication error: {0}")]
    OAuth(String),

    /// User cancelled an operation.
    #[error("Operation cancelled")]
    Cancelled,

    /// Authorization check resulted in denial.
    #[error("Access denied")]
    AccessDenied,

    /// Authorization check was indeterminate.
    #[error("Authorization check indeterminate")]
    Indeterminate,

    /// General/unspecified error.
    #[error("{0}")]
    Other(String),
}

/// Convenient Result type alias for CLI operations.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Returns the exit code for this error type.
    ///
    /// Exit codes follow the specification in CLI Development.md:
    /// - 0: Success
    /// - 1: General error
    /// - 2: Invalid arguments
    /// - 3: Authentication required
    /// - 4: Permission denied
    /// - 5: Resource not found
    /// - 6: Conflict
    /// - 7: Rate limited
    /// - 10: Network error
    /// - 11: Server error
    /// - 20: Authorization denied (check command)
    /// - 21: Indeterminate
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            // Argument/config errors
            Self::Config(_)
            | Self::InvalidArgument(_)
            | Self::Parse(_)
            | Self::OrgNotSpecified
            | Self::VaultNotSpecified => 2,

            // Auth errors
            Self::AuthRequired | Self::Credential(_) | Self::OAuth(_) => 3,

            // Not found
            Self::ProfileNotFound(_) => 5,

            // Authorization decisions
            Self::AccessDenied => 20,
            Self::Indeterminate => 21,

            // API errors mapped by kind
            Self::Api(e) => match e.kind() {
                inferadb::ErrorKind::Unauthorized => 3,
                inferadb::ErrorKind::Forbidden => 4,
                inferadb::ErrorKind::NotFound => 5,
                inferadb::ErrorKind::Conflict => 6,
                inferadb::ErrorKind::RateLimited => 7,
                inferadb::ErrorKind::Connection | inferadb::ErrorKind::Timeout => 10,
                inferadb::ErrorKind::Unavailable | inferadb::ErrorKind::Internal => 11,
                _ => 1,
            },

            // IO/network
            Self::Io(_) => 10,

            // Serialization, User action, Fallback
            Self::Json(_) | Self::Yaml(_) | Self::Cancelled | Self::Other(_) => 1,
        }
    }

    /// Returns true if this error should show a hint about logging in.
    #[must_use]
    pub fn should_suggest_login(&self) -> bool {
        matches!(self, Self::AuthRequired | Self::Credential(_))
            || matches!(self, Self::Api(e) if e.kind() == inferadb::ErrorKind::Unauthorized)
    }

    /// Create a configuration error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create an invalid argument error.
    pub fn invalid_arg(msg: impl Into<String>) -> Self {
        Self::InvalidArgument(msg.into())
    }

    /// Create a parse error.
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }

    /// Create a credential error.
    pub fn credential(msg: impl Into<String>) -> Self {
        Self::Credential(msg.into())
    }

    /// Create an OAuth error.
    pub fn oauth(msg: impl Into<String>) -> Self {
        Self::OAuth(msg.into())
    }

    /// Create a general error.
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Get a localized error message.
    ///
    /// Uses the i18n system if initialized, otherwise falls back to
    /// the default error message.
    #[must_use]
    pub fn localized_message(&self) -> Cow<'_, str> {
        // Check if i18n is initialized
        let Some(_i18n) = crate::i18n::try_get() else {
            // Fall back to thiserror-generated message
            return Cow::Owned(self.to_string());
        };

        match self {
            Self::AuthRequired => Cow::Owned(t!("error-auth-required")),
            Self::ProfileNotFound(name) => {
                Cow::Owned(t!("error-profile-not-found", "name" => name))
            },
            Self::OrgNotSpecified => Cow::Owned(t!("error-org-required")),
            Self::VaultNotSpecified => Cow::Owned(t!("error-vault-required")),
            Self::Config(details) => Cow::Owned(t!("error-config-parse", "details" => details)),
            Self::InvalidArgument(details) => {
                Cow::Owned(t!("error-invalid-argument", "details" => details))
            },
            Self::AccessDenied => Cow::Owned(t!("error-permission-denied")),
            Self::Cancelled => Cow::Borrowed("Operation cancelled"),
            Self::Indeterminate => Cow::Borrowed("Authorization check indeterminate"),

            // For API errors, use the SDK's message with our prefix
            Self::Api(e) => Cow::Owned(t!("error-api-error", "message" => &e.to_string())),

            // For IO errors, include the underlying message
            Self::Io(e) => Cow::Owned(format!("IO error: {e}")),

            // Serialization errors keep their technical messages
            Self::Json(e) => Cow::Owned(format!("JSON error: {e}")),
            Self::Yaml(e) => Cow::Owned(format!("YAML error: {e}")),

            // Parse and credential errors include their details
            Self::Parse(details) => Cow::Owned(format!("Parse error: {details}")),
            Self::Credential(details) => Cow::Owned(format!("Credential storage error: {details}")),
            Self::OAuth(details) => Cow::Owned(format!("Authentication error: {details}")),

            // Other errors pass through
            Self::Other(msg) => Cow::Borrowed(msg),
        }
    }
}

impl From<keyring::Error> for Error {
    fn from(err: keyring::Error) -> Self {
        Self::Credential(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_codes() {
        assert_eq!(Error::Config("test".into()).exit_code(), 2);
        assert_eq!(Error::AuthRequired.exit_code(), 3);
        assert_eq!(Error::ProfileNotFound("test".into()).exit_code(), 5);
        assert_eq!(Error::AccessDenied.exit_code(), 20);
        assert_eq!(Error::Indeterminate.exit_code(), 21);
    }

    #[test]
    fn test_should_suggest_login() {
        assert!(Error::AuthRequired.should_suggest_login());
        assert!(Error::Credential("test".into()).should_suggest_login());
        assert!(!Error::Config("test".into()).should_suggest_login());
    }
}
