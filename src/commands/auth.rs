//! Authentication commands: login, logout, register, init.

use crate::client::{auth, Context, OAuthFlow};
use crate::config::Profile;
use crate::error::Result;

/// First-run setup wizard.
pub async fn init(ctx: &Context) -> Result<()> {
    ctx.output.info("Welcome to InferaDB CLI!");
    ctx.output.info("");

    // Check if already configured
    if !ctx.config.profiles.is_empty() {
        ctx.output.warn("You already have profiles configured.");
        if !ctx.confirm("Do you want to create a new profile?")? {
            return Ok(());
        }
    }

    // Get profile name
    let profile_name = prompt_input("Profile name (default: 'default'): ")?;
    let profile_name = if profile_name.is_empty() {
        "default".to_string()
    } else {
        profile_name
    };

    // Get API URL
    let url = prompt_input("API URL (default: https://api.inferadb.com): ")?;
    let url = if url.is_empty() {
        "https://api.inferadb.com".to_string()
    } else {
        url
    };

    // Authenticate
    ctx.output.info("Authenticating...");
    let oauth = OAuthFlow::new()?;
    let credentials = oauth.authenticate().await?;

    // Store credentials
    auth::store_credentials(&profile_name, &credentials)?;
    ctx.output.success("Authentication successful!");

    // Get org and vault (could fetch from API after auth)
    ctx.output.info("");
    ctx.output.info("Enter your organization and vault IDs.");
    ctx.output
        .info("You can find these in the InferaDB dashboard.");

    let org = prompt_input("Organization ID: ")?;
    let vault = prompt_input("Vault ID: ")?;

    // Create and save profile
    let profile = Profile::new(&url, &org, &vault);

    let mut config = ctx.config.clone();
    config.set_profile(profile_name.clone(), profile);
    if config.default_profile.is_none() {
        config.set_default(Some(profile_name.clone()));
    }
    config.save()?;

    ctx.output.success(&format!(
        "Profile '{}' created and set as default!",
        profile_name
    ));
    ctx.output.info("");
    ctx.output.info("You're all set! Try:");
    ctx.output.info("  inferadb whoami");
    ctx.output
        .info("  inferadb check user:alice can_view document:readme");

    Ok(())
}

/// Log in to InferaDB via OAuth.
pub async fn login(ctx: &Context) -> Result<()> {
    let profile_name = ctx.effective_profile_name().to_string();

    ctx.output
        .info(&format!("Logging in as profile '{}'...", profile_name));

    let oauth = OAuthFlow::new()?;
    let credentials = oauth.authenticate().await?;

    auth::store_credentials(&profile_name, &credentials)?;

    ctx.output.success("Login successful!");
    Ok(())
}

/// Log out (remove stored credentials).
pub async fn logout(ctx: &Context) -> Result<()> {
    let profile_name = ctx.effective_profile_name().to_string();

    if !auth::has_credentials(&profile_name) {
        ctx.output
            .info(&format!("Profile '{}' is not logged in.", profile_name));
        return Ok(());
    }

    if !ctx.yes && !ctx.confirm(&format!("Log out from profile '{}'?", profile_name))? {
        ctx.output.info("Cancelled.");
        return Ok(());
    }

    auth::clear_credentials(&profile_name)?;
    ctx.output
        .success(&format!("Logged out from profile '{}'.", profile_name));
    Ok(())
}

/// Register a new account.
pub async fn register(ctx: &Context, email: Option<&str>, name: Option<&str>) -> Result<()> {
    let email = match email {
        Some(e) => e.to_string(),
        None => prompt_input("Email: ")?,
    };

    let name = match name {
        Some(n) => n.to_string(),
        None => prompt_input("Name: ")?,
    };

    if email.is_empty() || name.is_empty() {
        return Err(crate::error::Error::invalid_arg(
            "Email and name are required",
        ));
    }

    ctx.output.info("Registration not yet implemented.");
    ctx.output
        .info(&format!("Email: {}, Name: {}", email, name));

    Ok(())
}

/// Prompt for user input.
fn prompt_input(prompt: &str) -> Result<String> {
    use std::io::{self, BufRead, Write};

    eprint!("{}", prompt);
    io::stderr().flush()?;

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;

    Ok(line.trim().to_string())
}
