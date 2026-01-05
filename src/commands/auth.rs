//! Authentication commands: login, logout, register.

use teapot::forms::{Form, Group, InputField};

use crate::{
    client::{Context, OAuthFlow, auth},
    error::Result,
    t, tui,
};

/// Log in to `InferaDB` via OAuth.
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
        ctx.output.info(&t!("msg-not-logged-in", "profile" => &profile_name));
        return Ok(());
    }

    if !ctx.yes && !ctx.confirm(&t!("msg-logging-out", "profile" => &profile_name))? {
        ctx.output.info(&t!("msg-cancelled"));
        return Ok(());
    }

    auth::clear_credentials(&profile_name)?;
    ctx.output.success(&t!("msg-logout-success", "profile" => &profile_name));
    Ok(())
}

/// Register a new account.
pub async fn register(ctx: &Context, email: Option<&str>, name: Option<&str>) -> Result<()> {
    // If both args provided, use them directly
    let (email, name) = if let (Some(e), Some(n)) = (email, name) {
        (e.to_string(), n.to_string())
    } else {
        // Build a form for missing fields
        let mut form =
            Form::new().title("Registration").description("Create your InferaDB account");

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

        let Some(results) = tui::run_form(form)? else {
            ctx.output.info("Registration cancelled.");
            return Ok(());
        };

        let email = email
            .map(std::string::ToString::to_string)
            .or_else(|| results.get_string("email").map(std::string::ToString::to_string))
            .unwrap_or_default();

        let name = name
            .map(std::string::ToString::to_string)
            .or_else(|| results.get_string("name").map(std::string::ToString::to_string))
            .unwrap_or_default();

        (email, name)
    };

    if email.is_empty() || name.is_empty() {
        return Err(crate::error::Error::invalid_arg(t!("msg-email-name-required")));
    }

    ctx.output.info(&t!("msg-registration-not-implemented"));
    ctx.output.info(&t!("msg-registration-email-name", "email" => &email, "name" => &name));

    Ok(())
}
