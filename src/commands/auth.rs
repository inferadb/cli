//! Authentication commands: login, logout, register, init.

use crate::client::{auth, Context, OAuthFlow};
use crate::config::Profile;
use crate::error::Result;
use crate::t;
use crate::tui;

use ferment::forms::{Form, Group, InputField, NoteField};

/// First-run setup wizard.
pub async fn init(ctx: &Context) -> Result<()> {
    ctx.output.info(&t!("msg-init-welcome"));
    ctx.output.info("");

    // Check if already configured
    if !ctx.config.profiles.is_empty() {
        ctx.output.warn(&t!("msg-init-already-configured"));
        if !ctx.confirm(&t!("msg-init-create-new-profile"))? {
            return Ok(());
        }
    }

    // Build the setup form
    let form = Form::new()
        .title("InferaDB Setup")
        .description("Configure your InferaDB CLI environment")
        .group(
            Group::new()
                .title("Profile Configuration")
                .field(
                    InputField::new("profile_name")
                        .title(t!("prompt-profile-name"))
                        .placeholder("default")
                        .description("A name to identify this configuration")
                        .build(),
                )
                .field(
                    InputField::new("url")
                        .title(t!("prompt-api-url"))
                        .placeholder("https://api.inferadb.com")
                        .description("The InferaDB API endpoint URL")
                        .build(),
                ),
        )
        .group(
            Group::new().field(
                NoteField::new(
                    "You will now be redirected to your browser to authenticate.\n\
                     Complete the login process and return here.",
                )
                .title("Authentication")
                .build(),
            ),
        );

    // Run the form
    let results = match tui::run_form(form)? {
        Some(r) => r,
        None => {
            ctx.output.info("Setup cancelled.");
            return Ok(());
        }
    };

    // Extract values
    let profile_name = results
        .get_string("profile_name")
        .filter(|s| !s.is_empty())
        .unwrap_or("default")
        .to_string();

    let url = results
        .get_string("url")
        .filter(|s| !s.is_empty())
        .unwrap_or("https://api.inferadb.com")
        .to_string();

    // Authenticate with spinner
    let credentials = tui::spin(&t!("progress-authenticating"), async {
        let oauth = OAuthFlow::new()?;
        oauth.authenticate().await
    })
    .await?;

    // Store credentials
    auth::store_credentials(&profile_name, &credentials)?;
    ctx.output.success(&t!("msg-login-success"));

    // Second form for org and vault IDs
    let org_form = Form::new()
        .title("Organization & Vault")
        .description(format!(
            "{}\n{}",
            t!("msg-init-enter-ids"),
            t!("msg-init-find-in-dashboard")
        ))
        .group(
            Group::new()
                .field(
                    InputField::new("org")
                        .title(t!("prompt-org-id"))
                        .placeholder("org_xxxxxxxx")
                        .build(),
                )
                .field(
                    InputField::new("vault")
                        .title(t!("prompt-vault-id"))
                        .placeholder("vault_xxxxxxxx")
                        .build(),
                ),
        );

    let org_results = match tui::run_form(org_form)? {
        Some(r) => r,
        None => {
            ctx.output.info("Setup cancelled.");
            return Ok(());
        }
    };

    let org = org_results.get_string("org").unwrap_or("").to_string();
    let vault = org_results.get_string("vault").unwrap_or("").to_string();

    // Create and save profile
    let profile = Profile::new(&url, &org, &vault);

    let mut config = ctx.config.clone();
    config.set_profile(profile_name.clone(), profile);
    if config.default_profile.is_none() {
        config.set_default(Some(profile_name.clone()));
    }
    config.save()?;

    ctx.output
        .success(&t!("msg-init-profile-created", "name" => &profile_name));
    ctx.output.info("");
    ctx.output.info(&t!("msg-init-all-set"));
    ctx.output.info(&format!("  {}", t!("msg-init-try-whoami")));
    ctx.output.info(&format!("  {}", t!("msg-init-try-check")));

    Ok(())
}

/// Log in to InferaDB via OAuth.
pub async fn login(ctx: &Context) -> Result<()> {
    let profile_name = ctx.effective_profile_name().to_string();

    // Authenticate with spinner
    let credentials = tui::spin(t!("msg-logging-in", "profile" => &profile_name), async {
        let oauth = OAuthFlow::new()?;
        oauth.authenticate().await
    })
    .await?;

    auth::store_credentials(&profile_name, &credentials)?;

    ctx.output.success(&t!("msg-login-success"));
    Ok(())
}

/// Log out (remove stored credentials).
pub async fn logout(ctx: &Context) -> Result<()> {
    let profile_name = ctx.effective_profile_name().to_string();

    if !auth::has_credentials(&profile_name) {
        ctx.output
            .info(&t!("msg-not-logged-in", "profile" => &profile_name));
        return Ok(());
    }

    if !ctx.yes && !ctx.confirm(&t!("msg-logging-out", "profile" => &profile_name))? {
        ctx.output.info(&t!("msg-cancelled"));
        return Ok(());
    }

    auth::clear_credentials(&profile_name)?;
    ctx.output
        .success(&t!("msg-logout-success", "profile" => &profile_name));
    Ok(())
}

/// Register a new account.
pub async fn register(ctx: &Context, email: Option<&str>, name: Option<&str>) -> Result<()> {
    // If both args provided, use them directly
    let (email, name) = if let (Some(e), Some(n)) = (email, name) {
        (e.to_string(), n.to_string())
    } else {
        // Build a form for missing fields
        let mut form = Form::new()
            .title("Registration")
            .description("Create your InferaDB account");

        let mut group = Group::new();

        if email.is_none() {
            group = group.field(
                InputField::new("email")
                    .title(t!("prompt-email"))
                    .placeholder("user@example.com")
                    .required()
                    .build(),
            );
        }

        if name.is_none() {
            group = group.field(
                InputField::new("name")
                    .title(t!("prompt-name"))
                    .placeholder("Your Name")
                    .required()
                    .build(),
            );
        }

        form = form.group(group);

        let results = match tui::run_form(form)? {
            Some(r) => r,
            None => {
                ctx.output.info("Registration cancelled.");
                return Ok(());
            }
        };

        let email = email
            .map(|e| e.to_string())
            .or_else(|| results.get_string("email").map(|s| s.to_string()))
            .unwrap_or_default();

        let name = name
            .map(|n| n.to_string())
            .or_else(|| results.get_string("name").map(|s| s.to_string()))
            .unwrap_or_default();

        (email, name)
    };

    if email.is_empty() || name.is_empty() {
        return Err(crate::error::Error::invalid_arg(t!(
            "msg-email-name-required"
        )));
    }

    ctx.output.info(&t!("msg-registration-not-implemented"));
    ctx.output
        .info(&t!("msg-registration-email-name", "email" => &email, "name" => &name));

    Ok(())
}
