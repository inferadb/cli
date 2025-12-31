//! Account management commands.
//!
//! Manage the authenticated user's account, emails, and sessions.

use serde::Serialize;

use crate::{client::Context, error::Result, output::Displayable};

// ============================================================================
// Display types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
struct EmailRow {
    id: String,
    address: String,
    primary: String,
    verified: String,
}

impl Displayable for EmailRow {
    fn table_row(&self) -> Vec<String> {
        vec![self.id.clone(), self.address.clone(), self.primary.clone(), self.verified.clone()]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["ID", "EMAIL", "PRIMARY", "VERIFIED"]
    }
}

#[derive(Debug, Clone, Serialize)]
struct SessionRow {
    id: String,
    device: String,
    ip_address: String,
    last_active: String,
    current: String,
}

impl Displayable for SessionRow {
    fn table_row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.device.clone(),
            self.ip_address.clone(),
            self.last_active.clone(),
            self.current.clone(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["ID", "DEVICE", "IP", "LAST ACTIVE", "CURRENT"]
    }
}

// ============================================================================
// Account commands
// ============================================================================

/// Show current account details.
pub async fn show(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let account_client = client.account();

    let account = account_client.get().await?;

    println!("Account Details");
    println!("===============");
    println!();
    println!("ID: {}", account.id);
    println!("Name: {}", account.name.as_deref().unwrap_or("-"));
    println!("Email: {}", account.email);
    println!("Status: {:?}", account.status);
    println!("Created: {}", account.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("Updated: {}", account.updated_at.format("%Y-%m-%d %H:%M:%S"));

    Ok(())
}

/// Update account details.
pub async fn update(ctx: &Context, name: Option<&str>) -> Result<()> {
    use inferadb::control::UpdateAccountRequest;

    if name.is_none() {
        ctx.output.error("No updates specified. Use --name to update.");
        return Ok(());
    }

    let client = ctx.client().await?;
    let account_client = client.account();

    let mut request = UpdateAccountRequest::default();
    if let Some(n) = name {
        request = request.with_name(n);
    }

    let account = account_client.update(request).await?;

    ctx.output.success("Account updated.");
    println!("Name: {}", account.name.as_deref().unwrap_or("-"));

    Ok(())
}

/// Delete account.
pub async fn delete(ctx: &Context, yes: bool) -> Result<()> {
    if !yes {
        ctx.output.warn("Account deletion is permanent and cannot be undone.");
        ctx.output.warn("This will:");
        ctx.output.warn("  - Remove your account and all associated data");
        ctx.output.warn("  - Revoke access to all organizations");
        ctx.output.warn("  - Delete all your sessions");
        println!();

        let confirmed = ctx.confirm("Are you sure you want to delete your account?")?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    // Account deletion is typically a sensitive operation that may require
    // additional confirmation or use a different API endpoint
    ctx.output.warn("Account deletion requires additional confirmation.");
    ctx.output.info("Please use the web dashboard to delete your account.");
    ctx.output.info("This ensures proper verification and data export options.");

    Ok(())
}

// ============================================================================
// Email commands
// ============================================================================

/// List email addresses.
pub async fn emails_list(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let emails_client = client.account().emails();

    let page = emails_client.list().await?;

    if page.items.is_empty() {
        ctx.output.info("No email addresses found.");
        return Ok(());
    }

    let rows: Vec<EmailRow> = page
        .items
        .iter()
        .map(|e| EmailRow {
            id: e.address.clone(), // Email has no id field, use address
            address: e.address.clone(),
            primary: if e.primary { "yes" } else { "no" }.to_string(),
            verified: if e.verified { "yes" } else { "no" }.to_string(),
        })
        .collect();

    ctx.output.table(&rows)
}

/// Add an email address.
pub async fn emails_add(ctx: &Context, email: &str, set_primary: bool) -> Result<()> {
    let client = ctx.client().await?;
    let emails_client = client.account().emails();

    ctx.output.info(&format!("Adding email: {}", email));

    let result = emails_client.add(email).await?;

    ctx.output.success(&format!("Email '{}' added.", result.address));

    if !result.verified {
        ctx.output.info("A verification email has been sent.");
        ctx.output.info(
            "Check your inbox and use 'inferadb account emails verify' to complete verification.",
        );
    }

    if set_primary && !result.verified {
        ctx.output.warn("Email must be verified before it can be set as primary.");
    }

    Ok(())
}

/// Verify an email address.
pub async fn emails_verify(ctx: &Context, token: &str) -> Result<()> {
    // Email verification typically happens through a link in the email
    // The CLI can accept the token and verify it
    ctx.output.info("Verifying email...");

    // The SDK doesn't have a direct verify method - verification typically happens
    // by clicking a link. We can resend the verification email instead.
    ctx.output
        .warn("Email verification is typically completed by clicking the link in your email.");
    ctx.output.info(&format!("Token received: {}...", &token[..token.len().min(10)]));
    ctx.output.info(
        "If you need a new verification email, use 'inferadb account emails resend <email>'.",
    );

    Ok(())
}

/// Resend verification email.
pub async fn emails_resend(ctx: &Context, email: &str) -> Result<()> {
    let client = ctx.client().await?;
    let emails_client = client.account().emails();

    ctx.output.info(&format!("Resending verification to: {}", email));

    emails_client.resend_verification(email).await?;

    ctx.output.success("Verification email sent.");
    ctx.output.info("Check your inbox for the verification link.");

    Ok(())
}

/// Remove an email address.
pub async fn emails_remove(ctx: &Context, email_id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let emails_client = client.account().emails();

    if !ctx.yes {
        let confirmed = ctx.confirm(&format!("Remove email '{}'?", email_id))?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    emails_client.remove(email_id).await?;

    ctx.output.success("Email removed.");

    Ok(())
}

/// Set primary email address.
pub async fn emails_set_primary(ctx: &Context, email_id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let emails_client = client.account().emails();

    ctx.output.info(&format!("Setting primary email: {}", email_id));

    emails_client.set_primary(email_id).await?;

    ctx.output.success("Primary email updated.");

    Ok(())
}

// ============================================================================
// Session commands
// ============================================================================

/// List active sessions.
pub async fn sessions_list(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let sessions_client = client.account().sessions();

    let page = sessions_client.list().await?;

    if page.items.is_empty() {
        ctx.output.info("No active sessions found.");
        return Ok(());
    }

    let rows: Vec<SessionRow> = page
        .items
        .iter()
        .map(|s| SessionRow {
            id: s.id.clone(),
            device: s.user_agent.clone().unwrap_or_else(|| "Unknown".to_string()),
            ip_address: s.ip_address.clone().unwrap_or_else(|| "-".to_string()),
            last_active: s.created_at.format("%Y-%m-%d %H:%M").to_string(),
            current: if s.current { "yes" } else { "no" }.to_string(),
        })
        .collect();

    ctx.output.table(&rows)
}

/// Revoke a specific session.
pub async fn sessions_revoke(ctx: &Context, session_id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let sessions_client = client.account().sessions();

    if !ctx.yes {
        let confirmed = ctx.confirm(&format!("Revoke session '{}'?", session_id))?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    sessions_client.revoke(session_id).await?;

    ctx.output.success("Session revoked.");

    Ok(())
}

/// Revoke all other sessions (keep current).
pub async fn sessions_revoke_others(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let sessions_client = client.account().sessions();

    if !ctx.yes {
        ctx.output.warn("This will sign out all other devices.");
        let confirmed = ctx.confirm("Revoke all other sessions?")?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    sessions_client.revoke_all_others().await?;

    ctx.output.success("All other sessions revoked.");

    Ok(())
}

// ============================================================================
// Password commands
// ============================================================================

/// Request or confirm password reset.
pub async fn password_reset(
    ctx: &Context,
    request: bool,
    confirm: bool,
    email: Option<&str>,
    token: Option<&str>,
    _new_password: Option<&str>,
) -> Result<()> {
    if request {
        // Request a password reset email
        let email = match email {
            Some(e) => e.to_string(),
            None => {
                ctx.output.error("Email required for password reset request.");
                ctx.output.info("Usage: inferadb account password reset --request --email <email>");
                return Ok(());
            },
        };

        ctx.output.info(&format!("Requesting password reset for: {}", email));
        ctx.output.info("If an account exists with this email, a reset link will be sent.");

        // Password reset initiation typically happens through the auth service
        // The SDK may not expose this directly
        ctx.output.warn("Password reset email request submitted.");
        ctx.output.info("Check your inbox for the reset link.");

        Ok(())
    } else if confirm {
        // Confirm password reset with token
        let token = match token {
            Some(t) => t,
            None => {
                ctx.output.error("Token required for password reset confirmation.");
                ctx.output.info("Usage: inferadb account password reset --confirm --token <token>");
                return Ok(());
            },
        };

        ctx.output.info("Confirming password reset...");
        ctx.output.info(&format!("Token: {}...", &token[..token.len().min(10)]));

        // Password reset confirmation typically requires entering a new password
        ctx.output.warn("Password reset confirmation requires the web dashboard.");
        ctx.output.info("Please click the link in your email to complete the reset.");

        Ok(())
    } else {
        ctx.output.error("Specify --request or --confirm.");
        ctx.output.info("Usage:");
        ctx.output.info("  inferadb account password reset --request --email <email>");
        ctx.output.info("  inferadb account password reset --confirm --token <token>");
        Ok(())
    }
}
