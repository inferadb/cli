//! Identity and diagnostic commands: whoami, status, ping, doctor, health, version.

use crate::client::Context;
use crate::config::CredentialStore;
use crate::error::Result;
use serde::Serialize;
use std::time::{Duration, Instant};

/// Show current user and profile info.
pub async fn whoami(ctx: &Context) -> Result<()> {
    let profile_name = ctx.effective_profile_name();

    #[derive(Serialize)]
    struct WhoamiOutput {
        profile: String,
        url: String,
        org: Option<String>,
        vault: Option<String>,
        authenticated: bool,
        token_expires: Option<String>,
    }

    let store = CredentialStore::new();
    let credentials = store.load(profile_name)?;
    let authenticated = credentials.is_some();
    let token_expires = credentials
        .as_ref()
        .and_then(|c| c.expires_at)
        .map(|dt| dt.to_rfc3339());

    let output = WhoamiOutput {
        profile: profile_name.to_string(),
        url: ctx.profile.url_or_default().to_string(),
        org: ctx.profile.org.clone(),
        vault: ctx.profile.vault.clone(),
        authenticated,
        token_expires,
    };

    if ctx.output.format() == crate::output::OutputFormat::Table {
        println!("Profile: {}", output.profile);
        println!("URL: {}", output.url);
        if let Some(ref org) = output.org {
            println!("Organization: {}", org);
        }
        if let Some(ref vault) = output.vault {
            println!("Vault: {}", vault);
        }
        if output.authenticated {
            println!("Authenticated: yes");
            if let Some(ref expires) = output.token_expires {
                println!("Token expires: {}", expires);
            }
        } else {
            println!("Authenticated: no");
            ctx.output.warn("Run 'inferadb login' to authenticate.");
        }
    } else {
        ctx.output.value(&output)?;
    }

    Ok(())
}

/// Check service status.
pub async fn status(ctx: &Context) -> Result<()> {
    #[derive(Serialize)]
    struct StatusOutput {
        service: String,
        status: String,
        latency_ms: Option<u64>,
    }

    let url = ctx.profile.url_or_default();

    ctx.output.info(&format!("Checking status of {}...", url));

    // Try to connect (without auth)
    let start = Instant::now();
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/health", url))
        .timeout(Duration::from_secs(10))
        .send()
        .await;

    let (status, latency_ms) = match response {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            if resp.status().is_success() {
                ("healthy".to_string(), Some(latency))
            } else {
                (format!("error ({})", resp.status()), Some(latency))
            }
        }
        Err(e) => {
            if e.is_timeout() {
                ("timeout".to_string(), None)
            } else if e.is_connect() {
                ("connection failed".to_string(), None)
            } else {
                (format!("error: {}", e), None)
            }
        }
    };

    let output = StatusOutput {
        service: url.to_string(),
        status: status.clone(),
        latency_ms,
    };

    if ctx.output.format() == crate::output::OutputFormat::Table {
        if status == "healthy" {
            ctx.output.success(&format!(
                "Service: {} ({}ms)",
                status,
                latency_ms.unwrap_or(0)
            ));
        } else {
            ctx.output.error(&format!("Service: {}", status));
        }
    } else {
        ctx.output.value(&output)?;
    }

    Ok(())
}

/// Measure latency to service.
pub async fn ping(ctx: &Context, count: u32, control: bool, engine: bool) -> Result<()> {
    let url = ctx.profile.url_or_default();

    let target = if control && !engine {
        "control plane"
    } else if engine && !control {
        "engine"
    } else {
        "service"
    };

    ctx.output
        .info(&format!("Pinging {} at {}...", target, url));

    let client = reqwest::Client::new();
    let mut latencies = Vec::new();

    for i in 0..count {
        let start = Instant::now();
        let response = client
            .get(format!("{}/health", url))
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let latency = start.elapsed().as_millis() as u64;
                latencies.push(latency);
                println!("Ping {}: {}ms", i + 1, latency);
            }
            Ok(resp) => {
                println!("Ping {}: error ({})", i + 1, resp.status());
            }
            Err(e) => {
                if e.is_timeout() {
                    println!("Ping {}: timeout", i + 1);
                } else {
                    println!("Ping {}: error", i + 1);
                }
            }
        }

        if i + 1 < count {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    if !latencies.is_empty() {
        let min = latencies.iter().min().unwrap();
        let max = latencies.iter().max().unwrap();
        let avg = latencies.iter().sum::<u64>() / latencies.len() as u64;
        println!();
        println!("Statistics:");
        println!("  min: {}ms, max: {}ms, avg: {}ms", min, max, avg);
        println!("  {} of {} pings succeeded", latencies.len(), count);
    }

    Ok(())
}

/// Run connectivity diagnostics.
pub async fn doctor(ctx: &Context) -> Result<()> {
    println!("InferaDB Diagnostics");
    println!();

    let url = ctx.profile.url_or_default();
    let client = reqwest::Client::new();

    // DNS check
    print!("DNS resolution... ");
    let start = Instant::now();
    match url::Url::parse(url) {
        Ok(parsed) => {
            if let Some(host) = parsed.host_str() {
                match tokio::net::lookup_host(format!("{}:443", host)).await {
                    Ok(_) => {
                        println!("✓ ({}ms)", start.elapsed().as_millis());
                    }
                    Err(e) => {
                        println!("✗ Failed: {}", e);
                    }
                }
            } else {
                println!("✗ Invalid URL");
            }
        }
        Err(e) => {
            println!("✗ Invalid URL: {}", e);
        }
    }

    // HTTPS check
    print!("TLS connection... ");
    let start = Instant::now();
    match client
        .get(format!("{}/health", url))
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("✓ ({}ms)", start.elapsed().as_millis());
            } else {
                println!("⚠ Response: {}", resp.status());
            }
        }
        Err(e) => {
            if e.is_timeout() {
                println!("✗ Timeout");
            } else if e.is_connect() {
                println!("✗ Connection failed");
            } else {
                println!("✗ Error: {}", e);
            }
        }
    }

    // Auth check
    print!("Authentication... ");
    let store = CredentialStore::new();
    let profile_name = ctx.effective_profile_name();
    match store.load(profile_name)? {
        Some(creds) => {
            if creds.is_expired() {
                println!("⚠ Token expired");
                println!("   Run: inferadb login");
            } else if creds.expires_soon() {
                println!("⚠ Token expires soon");
                println!("   Consider running: inferadb login");
            } else {
                println!("✓ Valid token");
            }
        }
        None => {
            println!("✗ Not authenticated");
            println!("   Run: inferadb login");
        }
    }

    // Profile check
    print!("Profile... ");
    if ctx.profile.org.is_some() && ctx.profile.vault.is_some() {
        println!("✓ Complete");
    } else {
        println!("⚠ Incomplete");
        if ctx.profile.org.is_none() {
            println!("   Missing: organization ID");
        }
        if ctx.profile.vault.is_none() {
            println!("   Missing: vault ID");
        }
    }

    println!();
    Ok(())
}

/// Show service health dashboard.
pub async fn health(ctx: &Context, watch: bool, verbose: bool) -> Result<()> {
    if watch {
        loop {
            print!("\x1B[2J\x1B[1;1H"); // Clear screen
            show_health(ctx, verbose).await?;
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    } else {
        show_health(ctx, verbose).await
    }
}

async fn show_health(ctx: &Context, verbose: bool) -> Result<()> {
    let url = ctx.profile.url_or_default();

    println!("InferaDB Service Health");
    println!();

    let client = reqwest::Client::new();
    let start = Instant::now();
    let response = client
        .get(format!("{}/health", url))
        .timeout(Duration::from_secs(10))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            println!("Status: ✓ healthy");
            println!("Latency: {}ms", start.elapsed().as_millis());

            if verbose {
                if let Ok(body) = resp.text().await {
                    println!();
                    println!("Response:");
                    println!("{}", body);
                }
            }
        }
        Ok(resp) => {
            println!("Status: ⚠ degraded ({})", resp.status());
        }
        Err(e) => {
            if e.is_timeout() {
                println!("Status: ✗ timeout");
            } else {
                println!("Status: ✗ unreachable");
            }
        }
    }

    println!();
    println!("Last checked: {}", chrono::Utc::now().to_rfc3339());
    Ok(())
}

/// Show CLI version.
pub async fn version(ctx: &Context, check_updates: bool) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");

    println!("{} {}", name, version);

    if check_updates {
        ctx.output.info("Checking for updates...");
        // TODO: Implement update check
        ctx.output.info("Update check not yet implemented.");
    }

    Ok(())
}

/// Show configuration.
pub async fn config_show(ctx: &Context, key: Option<&str>) -> Result<()> {
    if let Some(k) = key {
        match k {
            "default_profile" => {
                if let Some(ref p) = ctx.config.default_profile {
                    println!("{}", p);
                }
            }
            "output.format" => println!("{}", ctx.config.output.format),
            "output.color" => println!("{}", ctx.config.output.color),
            _ => {
                return Err(crate::error::Error::invalid_arg(format!(
                    "Unknown key: {}",
                    k
                )));
            }
        }
    } else {
        let yaml = serde_yaml::to_string(&ctx.config)?;
        print!("{}", yaml);
    }
    Ok(())
}

/// Edit configuration file.
pub async fn config_edit(ctx: &Context, editor: Option<&str>) -> Result<()> {
    let path = crate::config::Config::user_config_path()
        .ok_or_else(|| crate::error::Error::config("Cannot determine config path"))?;

    // Ensure the file exists
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        ctx.config.save()?;
    }

    let editor = editor
        .map(|s| s.to_string())
        .or_else(|| std::env::var("EDITOR").ok())
        .or_else(|| std::env::var("VISUAL").ok())
        .unwrap_or_else(|| "vi".to_string());

    let status = std::process::Command::new(&editor).arg(&path).status()?;

    if !status.success() {
        ctx.output.warn("Editor exited with non-zero status");
    }

    Ok(())
}

/// Show configuration file path.
pub async fn config_path(_ctx: &Context, dir: bool) -> Result<()> {
    if dir {
        if let Some(path) = crate::config::Config::config_dir() {
            println!("{}", path.display());
        }
    } else if let Some(path) = crate::config::Config::user_config_path() {
        println!("{}", path.display());
    }
    Ok(())
}

/// Explain configuration resolution.
pub async fn config_explain(ctx: &Context) -> Result<()> {
    println!("Configuration Resolution (highest to lowest precedence):");
    println!();
    println!("  1. CLI flags           (e.g., @prod, --vault)");
    println!("  2. Environment vars    (INFERADB_*)");
    println!("  3. Project config      (.inferadb-cli.yaml in current directory)");
    println!("  4. User config         (~/.config/inferadb/cli.yaml)");
    println!("  5. Defaults");
    println!();
    println!("Current effective values:");
    println!();
    println!("  Profile: {}", ctx.effective_profile_name());
    println!("  URL: {}", ctx.profile.url_or_default());
    if let Some(ref org) = ctx.profile.org {
        println!("  Organization: {}", org);
    }
    if let Some(ref vault) = ctx.profile.vault {
        println!("  Vault: {}", vault);
    }
    Ok(())
}
