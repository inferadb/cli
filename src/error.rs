//! Error types for the InferaDB CLI.
//!
//! This module provides structured error handling with semantic exit codes
//! following the CLI specification.

use std::io;
use thiserror::Error;

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

    /// API/SDK error from the InferaDB SDK.
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
    pub fn exit_code(&self) -> i32 {
        match self {
            // Argument/config errors
            Error::Config(_) => 2,
            Error::InvalidArgument(_) => 2,
            Error::Parse(_) => 2,

            // Auth errors
            Error::AuthRequired => 3,
            Error::Credential(_) => 3,
            Error::OAuth(_) => 3,

            // Not found
            Error::ProfileNotFound(_) => 5,
            Error::OrgNotSpecified => 2,
            Error::VaultNotSpecified => 2,

            // Authorization decisions
            Error::AccessDenied => 20,
            Error::Indeterminate => 21,

            // API errors mapped by kind
            Error::Api(e) => match e.kind() {
                inferadb::ErrorKind::Unauthorized => 3,
                inferadb::ErrorKind::Forbidden => 4,
                inferadb::ErrorKind::NotFound => 5,
                inferadb::ErrorKind::Conflict => 6,
                inferadb::ErrorKind::RateLimited => 7,
                inferadb::ErrorKind::Connection => 10,
                inferadb::ErrorKind::Timeout => 10,
                inferadb::ErrorKind::Unavailable => 11,
                inferadb::ErrorKind::Internal => 11,
                _ => 1,
            },

            // IO/network
            Error::Io(_) => 10,

            // Serialization
            Error::Json(_) => 1,
            Error::Yaml(_) => 1,

            // User action
            Error::Cancelled => 1,

            // Fallback
            Error::Other(_) => 1,
        }
    }

    /// Returns true if this error should show a hint about logging in.
    pub fn should_suggest_login(&self) -> bool {
        matches!(self, Error::AuthRequired | Error::Credential(_))
            || matches!(self, Error::Api(e) if e.kind() == inferadb::ErrorKind::Unauthorized)
    }

    /// Create a configuration error.
    pub fn config(msg: impl Into<String>) -> Self {
        Error::Config(msg.into())
    }

    /// Create an invalid argument error.
    pub fn invalid_arg(msg: impl Into<String>) -> Self {
        Error::InvalidArgument(msg.into())
    }

    /// Create a parse error.
    pub fn parse(msg: impl Into<String>) -> Self {
        Error::Parse(msg.into())
    }

    /// Create a credential error.
    pub fn credential(msg: impl Into<String>) -> Self {
        Error::Credential(msg.into())
    }

    /// Create an OAuth error.
    pub fn oauth(msg: impl Into<String>) -> Self {
        Error::OAuth(msg.into())
    }

    /// Create a general error.
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
}

impl From<keyring::Error> for Error {
    fn from(err: keyring::Error) -> Self {
        Error::Credential(err.to_string())
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
