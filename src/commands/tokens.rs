//! Token management commands.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;

use crate::{client::Context, config::CredentialStore, error::Result, output::Displayable};

#[derive(Debug, Clone, Serialize)]
struct TokenRow {
    profile: String,
    status: String,
    expires: String,
    can_refresh: String,
}

impl Displayable for TokenRow {
    fn table_row(&self) -> Vec<String> {
        vec![
            self.profile.clone(),
            self.status.clone(),
            self.expires.clone(),
            self.can_refresh.clone(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["PROFILE", "STATUS", "EXPIRES", "REFRESHABLE"]
    }
}

/// Generate a new token.
///
/// Note: Tokens are obtained via the OAuth login flow, not generated directly.
pub async fn generate(ctx: &Context, ttl: Option<&str>, role: Option<&str>) -> Result<()> {
    ctx.output.warn("Token generation is not supported via CLI.");
    ctx.output.info("Use 'inferadb login' to authenticate and obtain tokens.");

    if let Some(t) = ttl {
        ctx.output.info(&format!("Requested TTL: {t}"));
    }
    if let Some(r) = role {
        ctx.output.info(&format!("Requested role: {r}"));
    }

    ctx.output.info("For API clients with custom tokens, use the web dashboard.");

    Ok(())
}

/// List tokens for all configured profiles.
pub async fn list(ctx: &Context) -> Result<()> {
    let store = CredentialStore::new();
    let mut rows = Vec::new();

    // Check each configured profile for credentials
    for name in ctx.config.profiles.keys() {
        let (status, expires, can_refresh) = if let Ok(Some(creds)) = store.load(name) {
            let status = if creds.is_expired() {
                "expired".to_string()
            } else if creds.expires_soon() {
                "expires soon".to_string()
            } else {
                "valid".to_string()
            };

            let expires = creds.expires_at.map_or_else(
                || "unknown".to_string(),
                |dt| dt.format("%Y-%m-%d %H:%M").to_string(),
            );

            let can_refresh = if creds.can_refresh() { "yes" } else { "no" }.to_string();
            (status, expires, can_refresh)
        } else {
            ("not authenticated".to_string(), "-".to_string(), "-".to_string())
        };

        rows.push(TokenRow { profile: name.clone(), status, expires, can_refresh });
    }

    // Also check "default" if not in profiles
    if !ctx.config.profiles.contains_key("default")
        && let Ok(Some(creds)) = store.load("default")
    {
        let status = if creds.is_expired() {
            "expired".to_string()
        } else if creds.expires_soon() {
            "expires soon".to_string()
        } else {
            "valid".to_string()
        };

        let expires = creds
            .expires_at
            .map_or_else(|| "unknown".to_string(), |dt| dt.format("%Y-%m-%d %H:%M").to_string());

        let can_refresh = if creds.can_refresh() { "yes" } else { "no" }.to_string();

        rows.push(TokenRow { profile: "default".to_string(), status, expires, can_refresh });
    }

    if rows.is_empty() {
        ctx.output.info("No tokens found.");
        ctx.output.info("Run 'inferadb login' to authenticate.");
        return Ok(());
    }

    ctx.output.table(&rows)
}

/// Revoke a token by profile name.
pub async fn revoke(ctx: &Context, id: &str) -> Result<()> {
    let store = CredentialStore::new();

    if !store.exists(id) {
        ctx.output.error(&format!("No credentials found for profile '{id}'."));
        return Ok(());
    }

    if !ctx.confirm(&format!("Revoke token for profile '{id}'?"))? {
        ctx.output.info("Cancelled.");
        return Ok(());
    }

    store.delete(id)?;
    ctx.output.success(&format!("Token for profile '{id}' has been revoked."));

    Ok(())
}

/// Refresh the current token.
pub async fn refresh(ctx: &Context) -> Result<()> {
    let profile_name = ctx.effective_profile_name();
    let store = CredentialStore::new();

    let Some(creds) = store.load(profile_name)? else {
        ctx.output.error("Not authenticated. Run 'inferadb login' first.");
        return Ok(());
    };

    if !creds.can_refresh() {
        ctx.output.error("Current token cannot be refreshed.");
        ctx.output.info("Run 'inferadb login' to obtain a new token.");
        return Ok(());
    }

    ctx.output.warn("Token refresh not yet implemented.");
    ctx.output.info("Run 'inferadb login' to obtain a fresh token.");

    Ok(())
}

/// Inspect token details.
pub async fn inspect(ctx: &Context, token: Option<&str>, verify: bool) -> Result<()> {
    let token_to_inspect = if let Some(t) = token {
        t.to_string()
    } else {
        // Use current profile's token
        let profile_name = ctx.effective_profile_name();
        let store = CredentialStore::new();

        let Some(creds) = store.load(profile_name)? else {
            ctx.output.error("Not authenticated. Run 'inferadb login' first.");
            return Ok(());
        };
        creds.access_token
    };

    // Decode JWT token (without verification)
    let parts: Vec<&str> = token_to_inspect.split('.').collect();
    if parts.len() != 3 {
        ctx.output.error("Invalid token format (expected JWT).");
        return Ok(());
    }

    // Decode header
    println!("Token Header:");
    match decode_jwt_part(parts[0]) {
        Ok(header) => {
            println!("{}", serde_json::to_string_pretty(&header)?);
        },
        Err(e) => {
            ctx.output.error(&format!("Failed to decode header: {e}"));
        },
    }

    println!();

    // Decode payload
    println!("Token Payload:");
    match decode_jwt_part(parts[1]) {
        Ok(payload) => {
            println!("{}", serde_json::to_string_pretty(&payload)?);

            // Show human-readable expiration
            if let Some(exp) = payload.get("exp").and_then(serde_json::Value::as_i64) {
                let exp_time = chrono::DateTime::from_timestamp(exp, 0);
                if let Some(dt) = exp_time {
                    let now = chrono::Utc::now();
                    if dt > now {
                        let duration = dt - now;
                        println!();
                        println!(
                            "Expires: {} (in {})",
                            dt.format("%Y-%m-%d %H:%M:%S UTC"),
                            format_duration(duration)
                        );
                    } else {
                        println!();
                        println!(
                            "Expired: {} ({} ago)",
                            dt.format("%Y-%m-%d %H:%M:%S UTC"),
                            format_duration(now - dt)
                        );
                    }
                }
            }

            // Show issued at
            if let Some(iat) = payload.get("iat").and_then(serde_json::Value::as_i64)
                && let Some(dt) = chrono::DateTime::from_timestamp(iat, 0)
            {
                println!("Issued: {}", dt.format("%Y-%m-%d %H:%M:%S UTC"));
            }
        },
        Err(e) => {
            ctx.output.error(&format!("Failed to decode payload: {e}"));
        },
    }

    if verify {
        println!();
        ctx.output.warn("Signature verification not yet implemented.");
        ctx.output.info("The token signature was not verified against the JWKS.");
    }

    Ok(())
}

fn decode_jwt_part(encoded: &str) -> Result<serde_json::Value> {
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| crate::error::Error::config(format!("Base64 decode error: {e}")))?;
    let json: serde_json::Value = serde_json::from_slice(&decoded)?;
    Ok(json)
}

fn format_duration(duration: chrono::Duration) -> String {
    let total_secs = duration.num_seconds();
    if total_secs < 0 {
        return format_duration(-duration);
    }

    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let mins = (total_secs % 3600) / 60;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}
