//! Profile management commands.

use crate::client::Context;
use crate::config::Profile;
use crate::error::{Error, Result};
use crate::output::Displayable;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct ProfileRow {
    name: String,
    url: String,
    org: String,
    vault: String,
    default: bool,
    authenticated: bool,
}

impl Displayable for ProfileRow {
    fn table_row(&self) -> Vec<String> {
        let default_marker = if self.default { "*" } else { "" };
        let auth_marker = if self.authenticated { "✓" } else { "" };
        vec![
            format!("{}{}", self.name, default_marker),
            self.url.clone(),
            self.org.clone(),
            self.vault.clone(),
            auth_marker.to_string(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["NAME", "URL", "ORG", "VAULT", "AUTH"]
    }
}

/// List all profiles.
pub async fn profiles_list(ctx: &Context) -> Result<()> {
    let store = crate::config::CredentialStore::new();
    let default_profile = ctx.config.default_profile.as_deref();

    if ctx.config.profiles.is_empty() {
        ctx.output.info("No profiles configured.");
        ctx.output.info("Run 'inferadb login' to authenticate.");
        return Ok(());
    }

    let mut rows: Vec<ProfileRow> = ctx
        .config
        .profiles
        .iter()
        .map(|(name, profile)| ProfileRow {
            name: name.clone(),
            url: profile.url.clone().unwrap_or_else(|| "-".to_string()),
            org: profile.org.clone().unwrap_or_else(|| "-".to_string()),
            vault: profile.vault.clone().unwrap_or_else(|| "-".to_string()),
            default: default_profile == Some(name.as_str()),
            authenticated: store.exists(name),
        })
        .collect();

    // Sort by name, with default first
    rows.sort_by(|a, b| {
        if a.default {
            std::cmp::Ordering::Less
        } else if b.default {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    ctx.output.table(&rows)?;

    if !ctx.output.is_quiet() {
        ctx.output.info("");
        ctx.output.info("* = default profile");
    }

    Ok(())
}

/// Show profile details.
pub async fn profiles_show(ctx: &Context, name: Option<&str>) -> Result<()> {
    let profile_name = name
        .or(ctx.config.default_profile.as_deref())
        .ok_or_else(|| Error::config("No profile specified and no default set"))?;

    let profile = ctx
        .config
        .get_profile(profile_name)
        .ok_or_else(|| Error::ProfileNotFound(profile_name.to_string()))?;

    let store = crate::config::CredentialStore::new();
    let authenticated = store.exists(profile_name);

    #[derive(Serialize)]
    struct ProfileDetails {
        name: String,
        url: Option<String>,
        org: Option<String>,
        vault: Option<String>,
        is_default: bool,
        authenticated: bool,
    }

    let details = ProfileDetails {
        name: profile_name.to_string(),
        url: profile.url.clone(),
        org: profile.org.clone(),
        vault: profile.vault.clone(),
        is_default: ctx.config.default_profile.as_deref() == Some(profile_name),
        authenticated,
    };

    if ctx.output.format() == crate::output::OutputFormat::Table {
        println!("Profile: {}", details.name);
        if details.is_default {
            println!("  (default)");
        }
        println!();
        if let Some(ref url) = details.url {
            println!("URL: {}", url);
        }
        if let Some(ref org) = details.org {
            println!("Organization: {}", org);
        }
        if let Some(ref vault) = details.vault {
            println!("Vault: {}", vault);
        }
        println!(
            "Authenticated: {}",
            if authenticated { "yes" } else { "no" }
        );
    } else {
        ctx.output.value(&details)?;
    }

    Ok(())
}

/// Create a new profile.
pub async fn profiles_create(
    ctx: &Context,
    name: &str,
    url: Option<&str>,
    org: Option<&str>,
    vault: Option<&str>,
) -> Result<()> {
    if ctx.config.profiles.contains_key(name) {
        return Err(Error::config(format!("Profile '{}' already exists", name)));
    }

    let profile = Profile {
        url: url.map(|s| s.to_string()),
        org: org.map(|s| s.to_string()),
        vault: vault.map(|s| s.to_string()),
    };

    let mut config = ctx.config.clone();
    config.set_profile(name.to_string(), profile);

    // Set as default if it's the first profile
    if config.default_profile.is_none() {
        config.set_default(Some(name.to_string()));
    }

    config.save()?;

    ctx.output.success(&format!("Profile '{}' created.", name));

    if config.default_profile.as_deref() == Some(name) {
        ctx.output.info("Set as default profile.");
    }

    Ok(())
}

/// Update an existing profile.
pub async fn profiles_update(
    ctx: &Context,
    name: &str,
    url: Option<&str>,
    org: Option<&str>,
    vault: Option<&str>,
) -> Result<()> {
    let mut config = ctx.config.clone();

    let profile = config
        .profiles
        .get_mut(name)
        .ok_or_else(|| Error::ProfileNotFound(name.to_string()))?;

    if let Some(u) = url {
        profile.url = Some(u.to_string());
    }
    if let Some(o) = org {
        profile.org = Some(o.to_string());
    }
    if let Some(v) = vault {
        profile.vault = Some(v.to_string());
    }

    config.save()?;

    ctx.output.success(&format!("Profile '{}' updated.", name));

    Ok(())
}

/// Rename a profile.
pub async fn profiles_rename(ctx: &Context, old_name: &str, new_name: &str) -> Result<()> {
    if old_name == new_name {
        return Err(Error::invalid_arg("Old and new names are the same"));
    }

    let mut config = ctx.config.clone();

    if config.profiles.contains_key(new_name) {
        return Err(Error::config(format!(
            "Profile '{}' already exists",
            new_name
        )));
    }

    let profile = config
        .profiles
        .remove(old_name)
        .ok_or_else(|| Error::ProfileNotFound(old_name.to_string()))?;

    config.profiles.insert(new_name.to_string(), profile);

    // Update default if needed
    if config.default_profile.as_deref() == Some(old_name) {
        config.set_default(Some(new_name.to_string()));
    }

    config.save()?;

    // Rename credentials too
    let store = crate::config::CredentialStore::new();
    if let Ok(Some(creds)) = store.load(old_name) {
        let _ = store.store(new_name, &creds);
        let _ = store.delete(old_name);
    }

    ctx.output.success(&format!(
        "Profile '{}' renamed to '{}'.",
        old_name, new_name
    ));

    Ok(())
}

/// Delete a profile.
pub async fn profiles_delete(ctx: &Context, name: &str) -> Result<()> {
    if !ctx.config.profiles.contains_key(name) {
        return Err(Error::ProfileNotFound(name.to_string()));
    }

    if !ctx.yes && !ctx.confirm(&format!("Delete profile '{}'?", name))? {
        ctx.output.info("Cancelled.");
        return Ok(());
    }

    let mut config = ctx.config.clone();
    config.remove_profile(name);

    // Clear default if it was this profile
    if config.default_profile.as_deref() == Some(name) {
        config.set_default(None);
        ctx.output.warn("Default profile was removed. Set a new default with 'inferadb profiles default <name>'.");
    }

    config.save()?;

    // Remove credentials
    let store = crate::config::CredentialStore::new();
    let _ = store.delete(name);

    ctx.output.success(&format!("Profile '{}' deleted.", name));

    Ok(())
}

/// Set the default profile.
pub async fn profiles_default(ctx: &Context, name: Option<&str>) -> Result<()> {
    match name {
        Some(n) => {
            if !ctx.config.profiles.contains_key(n) {
                return Err(Error::ProfileNotFound(n.to_string()));
            }

            let mut config = ctx.config.clone();
            config.set_default(Some(n.to_string()));
            config.save()?;

            ctx.output
                .success(&format!("Default profile set to '{}'.", n));
        }
        None => match &ctx.config.default_profile {
            Some(p) => println!("{}", p),
            None => {
                ctx.output.info("No default profile set.");
                ctx.output
                    .info("Set one with 'inferadb profiles default <name>'.");
            }
        },
    }

    Ok(())
}
