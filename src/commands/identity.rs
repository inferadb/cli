//! Identity and diagnostic commands: whoami, status, ping, doctor, health, version.

use std::time::{Duration, Instant};

use serde::Serialize;

use crate::{client::Context, config::CredentialStore, error::Result};

/// Show current user and profile info.
pub async fn whoami(ctx: &Context) -> Result<()> {
    #[derive(Serialize)]
    struct WhoamiOutput {
        profile: String,
        url: String,
        org: Option<String>,
        vault: Option<String>,
        authenticated: bool,
        token_expires: Option<String>,
    }

    let profile_name = ctx.effective_profile_name();
    let store = CredentialStore::new();
    let credentials = store.load(profile_name)?;
    let authenticated = credentials.is_some();
    let token_expires = credentials.as_ref().and_then(|c| c.expires_at).map(|dt| dt.to_rfc3339());

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
            println!("Organization: {org}");
        }
        if let Some(ref vault) = output.vault {
            println!("Vault: {vault}");
        }
        if output.authenticated {
            println!("Authenticated: yes");
            if let Some(ref expires) = output.token_expires {
                println!("Token expires: {expires}");
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

    ctx.output.info(&format!("Checking status of {url}..."));

    // Try to connect (without auth)
    let start = Instant::now();
    let client = reqwest::Client::new();
    let response =
        client.get(format!("{url}/health")).timeout(Duration::from_secs(10)).send().await;

    let (status, latency_ms) = match response {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            if resp.status().is_success() {
                ("healthy".to_string(), Some(latency))
            } else {
                (format!("error ({})", resp.status()), Some(latency))
            }
        },
        Err(e) => {
            if e.is_timeout() {
                ("timeout".to_string(), None)
            } else if e.is_connect() {
                ("connection failed".to_string(), None)
            } else {
                (format!("error: {e}"), None)
            }
        },
    };

    let output = StatusOutput { service: url.to_string(), status: status.clone(), latency_ms };

    if ctx.output.format() == crate::output::OutputFormat::Table {
        if status == "healthy" {
            ctx.output.success(&format!("Service: {} ({}ms)", status, latency_ms.unwrap_or(0)));
        } else {
            ctx.output.error(&format!("Service: {status}"));
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

    ctx.output.info(&format!("Pinging {target} at {url}..."));

    let client = reqwest::Client::new();
    let mut latencies = Vec::new();

    for i in 0..count {
        let start = Instant::now();
        let response =
            client.get(format!("{url}/health")).timeout(Duration::from_secs(5)).send().await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let latency = start.elapsed().as_millis() as u64;
                latencies.push(latency);
                println!("Ping {}: {}ms", i + 1, latency);
            },
            Ok(resp) => {
                println!("Ping {}: error ({})", i + 1, resp.status());
            },
            Err(e) => {
                if e.is_timeout() {
                    println!("Ping {}: timeout", i + 1);
                } else {
                    println!("Ping {}: error", i + 1);
                }
            },
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
        println!("  min: {min}ms, max: {max}ms, avg: {avg}ms");
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
                match tokio::net::lookup_host(format!("{host}:443")).await {
                    Ok(_) => {
                        println!("✓ ({}ms)", start.elapsed().as_millis());
                    },
                    Err(e) => {
                        println!("✗ Failed: {e}");
                    },
                }
            } else {
                println!("✗ Invalid URL");
            }
        },
        Err(e) => {
            println!("✗ Invalid URL: {e}");
        },
    }

    // HTTPS check
    print!("TLS connection... ");
    let start = Instant::now();
    match client.get(format!("{url}/health")).timeout(Duration::from_secs(10)).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("✓ ({}ms)", start.elapsed().as_millis());
            } else {
                println!("⚠ Response: {}", resp.status());
            }
        },
        Err(e) => {
            if e.is_timeout() {
                println!("✗ Timeout");
            } else if e.is_connect() {
                println!("✗ Connection failed");
            } else {
                println!("✗ Error: {e}");
            }
        },
    }

    // Auth check
    print!("Authentication... ");
    let store = CredentialStore::new();
    let profile_name = ctx.effective_profile_name();
    if let Some(creds) = store.load(profile_name)? {
        if creds.is_expired() {
            println!("⚠ Token expired");
            println!("   Run: inferadb login");
        } else if creds.expires_soon() {
            println!("⚠ Token expires soon");
            println!("   Consider running: inferadb login");
        } else {
            println!("✓ Valid token");
        }
    } else {
        println!("✗ Not authenticated");
        println!("   Run: inferadb login");
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
    let response =
        client.get(format!("{url}/health")).timeout(Duration::from_secs(10)).send().await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            println!("Status: ✓ healthy");
            println!("Latency: {}ms", start.elapsed().as_millis());

            if verbose && let Ok(body) = resp.text().await {
                println!();
                println!("Response:");
                println!("{body}");
            }
        },
        Ok(resp) => {
            println!("Status: ⚠ degraded ({})", resp.status());
        },
        Err(e) => {
            if e.is_timeout() {
                println!("Status: ✗ timeout");
            } else {
                println!("Status: ✗ unreachable");
            }
        },
    }

    println!();
    println!("Last checked: {}", chrono::Utc::now().to_rfc3339());
    Ok(())
}

/// Show CLI version.
pub async fn version(ctx: &Context, check_updates: bool) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");

    println!("{name} {version}");

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
                    println!("{p}");
                }
            },
            "output.format" => println!("{}", ctx.config.output.format),
            "output.color" => println!("{}", ctx.config.output.color),
            _ => {
                return Err(crate::error::Error::invalid_arg(format!("Unknown key: {k}")));
            },
        }
    } else {
        let yaml = serde_yaml::to_string(&ctx.config)?;
        print!("{yaml}");
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
        .map(std::string::ToString::to_string)
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
        println!("  Organization: {org}");
    }
    if let Some(ref vault) = ctx.profile.vault {
        println!("  Vault: {vault}");
    }
    Ok(())
}

/// Show vault statistics.
pub async fn stats(ctx: &Context, trends: bool, compact: bool) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();

    // Get schema info
    let schemas = vault.schemas();
    let schema_list = schemas.list().await?;
    let active_schema = schemas.get_active().await.ok();

    // Get relationship sample (to count)
    let rels = vault.relationships();
    let sample = rels.list().limit(1000).await?;
    let rel_count = sample.relationships.len();
    let has_more = sample.next_cursor.is_some();

    if compact {
        // Single-line compact output
        print!("schemas:{}", schema_list.items.len());
        if let Some(ref active) = active_schema {
            print!("(v{} active)", active.version);
        }
        print!(" relationships:{rel_count}");
        if has_more {
            print!("+");
        }
        println!();
        return Ok(());
    }

    println!("Vault Statistics");
    println!("================");
    println!();

    // Schema stats
    println!("Schemas:");
    println!("  Versions: {}", schema_list.items.len());
    if let Some(active) = active_schema {
        println!("  Active: v{} ({})", active.version, active.id);
    } else {
        println!("  Active: (none)");
    }

    // Relationship stats
    println!();
    println!("Relationships:");
    if has_more {
        println!("  Count: {rel_count}+ (sampled)");
    } else {
        println!("  Count: {rel_count}");
    }

    if trends {
        println!();
        println!("Trends:");
        ctx.output.info("Historical trends require time-series data.");
        ctx.output.info("Use 'inferadb what-changed --since 1d' to see recent activity.");
    }

    println!();
    ctx.output.info("For detailed stats, use the InferaDB Dashboard.");

    Ok(())
}

/// Show recent changes in the vault.
pub async fn what_changed(
    ctx: &Context,
    since: Option<&str>,
    until: Option<&str>,
    focus: Option<&str>,
    actor: Option<&str>,
    resource: Option<&str>,
    compact: bool,
) -> Result<()> {
    // Parse the "since" time
    let since_time = parse_time_spec(since.unwrap_or("1d"));
    let until_time = until.map(parse_time_spec);

    ctx.output.info(&format!("Showing changes since {}", since.unwrap_or("1 day ago")));
    if let Some(u) = until {
        ctx.output.info(&format!("Until {u}"));
    }
    println!();

    let client = ctx.client().await?;

    // Get audit logs (they track changes)
    // Need org ID from profile
    let org_id = ctx
        .profile
        .org
        .as_deref()
        .ok_or_else(|| crate::error::Error::config("No organization configured"))?;

    let org = client.organization(org_id);
    let logs = org.audit().list().await?;

    // Filter by time and other criteria
    let filtered_logs: Vec<_> = logs
        .items
        .iter()
        .filter(|log| {
            // Time filter - timestamp is already DateTime<Utc>
            if log.timestamp < since_time {
                return false;
            }
            if let Some(ref ut) = until_time
                && log.timestamp > *ut
            {
                return false;
            }

            // Actor filter
            if let Some(a) = actor
                && !log.actor.id.contains(a)
            {
                return false;
            }

            // Resource filter
            if let Some(r) = resource {
                if let Some(ref log_resource) = log.resource {
                    if !log_resource.contains(r) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            // Focus filter
            if let Some(f) = focus {
                let action_str = format!("{:?}", log.action).to_lowercase();
                match f.to_lowercase().as_str() {
                    "schemas" => {
                        if !action_str.contains("schema") {
                            return false;
                        }
                    },
                    "relationships" => {
                        if !action_str.contains("relationship") && !action_str.contains("tuple") {
                            return false;
                        }
                    },
                    "permissions" => {
                        if !action_str.contains("permission") && !action_str.contains("role") {
                            return false;
                        }
                    },
                    _ => {},
                }
            }

            true
        })
        .collect();

    if compact {
        // Compact summary
        let schema_changes = filtered_logs
            .iter()
            .filter(|l| format!("{:?}", l.action).to_lowercase().contains("schema"))
            .count();
        let rel_changes = filtered_logs
            .iter()
            .filter(|l| {
                let a = format!("{:?}", l.action).to_lowercase();
                a.contains("relationship") || a.contains("tuple")
            })
            .count();
        let other_changes = filtered_logs.len() - schema_changes - rel_changes;

        println!(
            "changes:{} (schemas:{}, relationships:{}, other:{})",
            filtered_logs.len(),
            schema_changes,
            rel_changes,
            other_changes
        );
        return Ok(());
    }

    if filtered_logs.is_empty() {
        println!("No changes found in the specified time range.");
        return Ok(());
    }

    println!("Recent Changes ({} events)", filtered_logs.len());
    println!("=============================");
    println!();

    // Group by type
    let mut schema_changes = Vec::new();
    let mut rel_changes = Vec::new();
    let mut other_changes = Vec::new();

    for log in &filtered_logs {
        let action_str = format!("{:?}", log.action).to_lowercase();
        if action_str.contains("schema") {
            schema_changes.push(log);
        } else if action_str.contains("relationship") || action_str.contains("tuple") {
            rel_changes.push(log);
        } else {
            other_changes.push(log);
        }
    }

    if !schema_changes.is_empty() {
        println!("Schema Changes ({}):", schema_changes.len());
        for log in schema_changes.iter().take(5) {
            println!(
                "  {} - {:?} by {}",
                log.timestamp.format("%Y-%m-%d %H:%M:%S"),
                log.action,
                log.actor.id
            );
        }
        if schema_changes.len() > 5 {
            println!("  ... and {} more", schema_changes.len() - 5);
        }
        println!();
    }

    if !rel_changes.is_empty() {
        println!("Relationship Changes ({}):", rel_changes.len());
        for log in rel_changes.iter().take(5) {
            println!(
                "  {} - {:?} by {}",
                log.timestamp.format("%Y-%m-%d %H:%M:%S"),
                log.action,
                log.actor.id
            );
        }
        if rel_changes.len() > 5 {
            println!("  ... and {} more", rel_changes.len() - 5);
        }
        println!();
    }

    if !other_changes.is_empty() {
        println!("Other Changes ({}):", other_changes.len());
        for log in other_changes.iter().take(5) {
            println!(
                "  {} - {:?} by {}",
                log.timestamp.format("%Y-%m-%d %H:%M:%S"),
                log.action,
                log.actor.id
            );
        }
        if other_changes.len() > 5 {
            println!("  ... and {} more", other_changes.len() - 5);
        }
        println!();
    }

    ctx.output.info("For full details, use 'inferadb orgs audit-logs'");

    Ok(())
}

/// Show workflow templates.
pub async fn templates(
    ctx: &Context,
    name: Option<&str>,
    subject: Option<&str>,
    format: &str,
) -> Result<()> {
    match name {
        None => {
            // List all templates
            println!("Available Templates");
            println!("===================");
            println!();
            println!("COMMON WORKFLOWS");
            println!("  user-offboarding       Remove all access for a departing user");
            println!("  batch-check            Check multiple permissions efficiently");
            println!("  export-backup          Create a complete vault backup");
            println!("  permission-audit       Audit who has access to what");
            println!();
            println!("SCHEMA OPERATIONS");
            println!("  schema-migration       Safely migrate schema with breaking changes");
            println!("  canary-deploy          Deploy schema with canary rollout");
            println!("  schema-rollback        Emergency rollback procedure");
            println!();
            println!("ADMINISTRATION");
            println!("  new-vault-setup        Set up a new vault with common patterns");
            println!("  token-rotation         Rotate vault tokens safely");
            println!("  audit-report           Generate compliance audit report");
            println!();
            println!("DEBUGGING");
            println!("  debug-denial           Investigate why access was denied");
            println!("  compare-access         Compare access between two users");
            println!();
            println!("Use 'inferadb templates <name>' for details.");
        },
        Some(template_name) => {
            let sub = subject.unwrap_or("user:example-subject");

            match template_name {
                "user-offboarding" => {
                    show_user_offboarding_template(sub, format);
                },
                "batch-check" => {
                    show_batch_check_template(sub, format);
                },
                "debug-denial" => {
                    show_debug_denial_template(sub, format);
                },
                "schema-migration" => {
                    show_schema_migration_template(format);
                },
                "schema-rollback" => {
                    show_schema_rollback_template(format);
                },
                "export-backup" => {
                    show_export_backup_template(format);
                },
                _ => {
                    ctx.output.error(&format!("Unknown template: {template_name}"));
                    ctx.output.info("Run 'inferadb templates' to see available templates.");
                },
            }
        },
    }
    Ok(())
}

fn show_user_offboarding_template(subject: &str, format: &str) {
    if format == "script" {
        println!("#!/bin/bash");
        println!("# User Offboarding Script");
        println!("# Generated by: inferadb templates user-offboarding");
        println!();
        println!("USER=\"{subject}\"");
        println!();
        println!("# Step 1: Export current access");
        println!(
            "inferadb relationships list --subject \"$USER\" -o json > \"${{USER}}-access.json\""
        );
        println!();
        println!("# Step 2: Remove all relationships");
        println!("inferadb relationships list --subject \"$USER\" -o json | \\");
        println!("  jq -r '.[] | \"\\(.resource) \\(.relation) \\(.subject)\"' | \\");
        println!("  while read resource relation subject; do");
        println!("    inferadb relationships delete \"$subject\" \"$relation\" \"$resource\"");
        println!("  done");
        println!();
        println!("# Step 3: Verify");
        println!("inferadb relationships list --subject \"$USER\"");
    } else {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("User Offboarding Workflow");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("Use this workflow to safely remove all access for a departing user.");
        println!();
        println!("SUBJECT: {subject}");
        println!();
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("# Step 1: Review what the user has access to");
        println!("inferadb relationships list --subject {subject} -o json > access-backup.json");
        println!();
        println!("# Step 2: Remove all relationships for this user");
        println!("# (Review the backup file first!)");
        println!("inferadb relationships list --subject {subject} | while read rel; do");
        println!("  inferadb relationships delete $rel");
        println!("done");
        println!();
        println!("# Step 3: Verify all access is removed");
        println!("inferadb relationships list --subject {subject}");
        println!();
        println!("TIP: Use --format script to get an executable script version.");
    }
}

fn show_batch_check_template(subject: &str, format: &str) {
    if format == "script" {
        println!("#!/bin/bash");
        println!("# Batch Permission Check");
        println!();
        println!("USER=\"{subject}\"");
        println!("RESOURCES=(\"document:readme\" \"folder:private\" \"project:main\")");
        println!("PERMISSIONS=(\"view\" \"edit\" \"admin\")");
        println!();
        println!("for resource in \"${{RESOURCES[@]}}\"; do");
        println!("  for permission in \"${{PERMISSIONS[@]}}\"; do");
        println!("    result=$(inferadb check \"$USER\" \"$permission\" \"$resource\" 2>&1)");
        println!("    echo \"$USER $permission $resource: $result\"");
        println!("  done");
        println!("done");
    } else {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Batch Permission Check");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("Check multiple permissions efficiently.");
        println!();
        println!("SUBJECT: {subject}");
        println!();
        println!("# Check multiple resources");
        println!("for resource in document:a document:b document:c; do");
        println!("  inferadb check {subject} view $resource");
        println!("done");
        println!();
        println!("# Check with JSON output for parsing");
        println!("inferadb check {subject} view document:readme -o json");
    }
}

fn show_debug_denial_template(subject: &str, format: &str) {
    if format == "script" {
        println!("#!/bin/bash");
        println!("# Debug Access Denial");
        println!();
        println!("USER=\"{subject}\"");
        println!("RESOURCE=\"document:example\"");
        println!("PERMISSION=\"view\"");
        println!();
        println!("echo \"Checking permission...\"");
        println!("inferadb check \"$USER\" \"$PERMISSION\" \"$RESOURCE\" --explain");
        println!();
        println!("echo \"Expanding userset...\"");
        println!("inferadb expand \"$RESOURCE\" viewer");
        println!();
        println!("echo \"User's relationships...\"");
        println!("inferadb relationships list --subject \"$USER\"");
    } else {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Debug Access Denial");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("Investigate why access was denied.");
        println!();
        println!("SUBJECT: {subject}");
        println!();
        println!("# Step 1: Check with explanation");
        println!("inferadb check {subject} view document:example --explain");
        println!();
        println!("# Step 2: See who can access the resource");
        println!("inferadb expand document:example viewer");
        println!();
        println!("# Step 3: Check user's relationships");
        println!("inferadb relationships list --subject {subject}");
        println!();
        println!("# Step 4: Check active schema");
        println!("inferadb schemas get active");
    }
}

fn show_schema_migration_template(format: &str) {
    if format == "script" {
        println!("#!/bin/bash");
        println!("# Safe Schema Migration");
        println!();
        println!("SCHEMA_FILE=\"schema.ipl\"");
        println!();
        println!("echo \"Step 1: Validate schema\"");
        println!("inferadb schemas validate \"$SCHEMA_FILE\" || exit 1");
        println!();
        println!("echo \"Step 2: Run tests\"");
        println!("inferadb schemas test --schema \"$SCHEMA_FILE\" || exit 1");
        println!();
        println!("echo \"Step 3: Preview changes\"");
        println!("inferadb schemas preview \"$SCHEMA_FILE\"");
        println!();
        println!("read -p \"Continue with push? (y/n) \" -n 1 -r");
        println!("echo");
        println!("if [[ $REPLY =~ ^[Yy]$ ]]; then");
        println!("  inferadb schemas push \"$SCHEMA_FILE\" --activate");
        println!("fi");
    } else {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Safe Schema Migration");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("Follow these steps to safely migrate your schema.");
        println!();
        println!("# Step 1: Validate the schema");
        println!("inferadb schemas validate schema.ipl");
        println!();
        println!("# Step 2: Run tests against new schema");
        println!("inferadb schemas test --schema schema.ipl");
        println!();
        println!("# Step 3: Preview changes");
        println!("inferadb schemas preview schema.ipl");
        println!();
        println!("# Step 4: Push and activate (or use canary)");
        println!("inferadb schemas push schema.ipl --activate");
        println!("# OR: inferadb schemas push schema.ipl");
        println!("#     inferadb schemas activate <id> --canary 10");
    }
}

fn show_schema_rollback_template(format: &str) {
    if format == "script" {
        println!("#!/bin/bash");
        println!("# Emergency Schema Rollback");
        println!();
        println!("echo \"Current schema versions:\"");
        println!("inferadb schemas list --all");
        println!();
        println!("echo \"Rolling back to previous version...\"");
        println!("inferadb schemas rollback");
        println!();
        println!("echo \"Verifying rollback:\"");
        println!("inferadb schemas list");
    } else {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Emergency Schema Rollback");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("Use this if you need to quickly rollback a schema.");
        println!();
        println!("# Step 1: List available versions");
        println!("inferadb schemas list --all");
        println!();
        println!("# Step 2: Rollback to previous version");
        println!("inferadb schemas rollback");
        println!();
        println!("# OR: Rollback to specific version");
        println!("inferadb schemas rollback <version-id>");
        println!();
        println!("# Step 3: Verify the active schema");
        println!("inferadb schemas get active");
    }
}

fn show_export_backup_template(format: &str) {
    if format == "script" {
        println!("#!/bin/bash");
        println!("# Vault Backup Script");
        println!();
        println!("DATE=$(date +%Y%m%d_%H%M%S)");
        println!("BACKUP_DIR=\"backup_$DATE\"");
        println!();
        println!("mkdir -p \"$BACKUP_DIR\"");
        println!();
        println!("echo \"Exporting relationships...\"");
        println!("inferadb export --output \"$BACKUP_DIR/relationships.json\"");
        println!();
        println!("echo \"Exporting active schema...\"");
        println!("inferadb schemas get active > \"$BACKUP_DIR/schema.ipl\"");
        println!();
        println!("echo \"Backup complete: $BACKUP_DIR\"");
    } else {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Complete Vault Backup");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("Create a complete backup of your vault.");
        println!();
        println!("# Step 1: Export relationships");
        println!("inferadb export --output relationships.json");
        println!();
        println!("# Step 2: Export active schema");
        println!("inferadb schemas get active > schema.ipl");
        println!();
        println!("# Step 3: (Optional) Export all schema versions");
        println!("inferadb schemas list --all -o json > schema-versions.json");
        println!();
        println!("TIP: Use --format script for a runnable backup script.");
    }
}

/// Show workflow guides.
pub async fn guide(ctx: &Context, name: Option<&str>) -> Result<()> {
    match name {
        None => {
            println!("Available Guides");
            println!("================");
            println!();
            println!("GETTING STARTED");
            println!("  quickstart             First-time setup and basic usage");
            println!("  concepts               Core concepts: subjects, resources, relations");
            println!();
            println!("SCHEMA DEVELOPMENT");
            println!("  schema-best-practices  Writing maintainable schemas");
            println!("  schema-deployment      Safe deployment workflow");
            println!("  schema-testing         Writing effective tests");
            println!();
            println!("OPERATIONS");
            println!("  production-checklist   Pre-production readiness checklist");
            println!("  incident-response      Handling authorization incidents");
            println!();
            println!("SECURITY");
            println!("  security-best-practices Token management, audit, access control");
            println!();
            println!("Use 'inferadb guide <name>' for details.");
        },
        Some(guide_name) => match guide_name {
            "quickstart" => show_quickstart_guide(),
            "concepts" => show_concepts_guide(),
            "schema-deployment" => show_schema_deployment_guide(),
            "production-checklist" => show_production_checklist_guide(),
            "incident-response" => show_incident_response_guide(),
            _ => {
                ctx.output.error(&format!("Unknown guide: {guide_name}"));
                ctx.output.info("Run 'inferadb guide' to see available guides.");
            },
        },
    }
    Ok(())
}

fn show_quickstart_guide() {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("InferaDB Quickstart Guide");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("1. AUTHENTICATE");
    println!("   inferadb login");
    println!();
    println!("2. VERIFY CONNECTION");
    println!("   inferadb whoami");
    println!("   inferadb status");
    println!();
    println!("3. CREATE A SCHEMA");
    println!("   inferadb schemas init");
    println!("   # Edit schema.ipl");
    println!("   inferadb schemas validate schema.ipl");
    println!("   inferadb schemas push schema.ipl --activate");
    println!();
    println!("4. ADD RELATIONSHIPS");
    println!("   inferadb relationships add user:alice viewer document:readme");
    println!();
    println!("5. CHECK PERMISSIONS");
    println!("   inferadb check user:alice view document:readme");
    println!();
    println!("NEXT STEPS:");
    println!("  - inferadb guide concepts         Learn core concepts");
    println!("  - inferadb guide schema-deployment  Safe deployment workflow");
    println!("  - inferadb cheatsheet             Quick command reference");
}

fn show_concepts_guide() {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Core Concepts Guide");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("SUBJECTS");
    println!("  Who is requesting access. Format: type:id");
    println!("  Examples: user:alice, team:engineering, service:api");
    println!();
    println!("RESOURCES");
    println!("  What is being accessed. Format: type:id");
    println!("  Examples: document:readme, folder:private, project:main");
    println!();
    println!("RELATIONS");
    println!("  How subjects relate to resources.");
    println!("  Examples: viewer, editor, owner, member");
    println!();
    println!("PERMISSIONS");
    println!("  Actions that can be performed. Derived from relations.");
    println!("  Examples: view, edit, delete, admin");
    println!();
    println!("RELATIONSHIPS");
    println!("  A tuple: (resource, relation, subject)");
    println!("  Example: document:readme#viewer@user:alice");
    println!("  Meaning: user:alice is a viewer of document:readme");
    println!();
    println!("SCHEMA");
    println!("  Defines entity types, relations, and permission rules.");
    println!("  Written in IPL (InferaDB Policy Language).");
}

fn show_schema_deployment_guide() {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Safe Schema Deployment Guide");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("RECOMMENDED WORKFLOW");
    println!();
    println!("1. DEVELOP LOCALLY");
    println!("   - Edit your schema.ipl file");
    println!("   - Run 'inferadb schemas validate schema.ipl' frequently");
    println!("   - Use 'inferadb schemas format schema.ipl --write' for consistency");
    println!();
    println!("2. TEST THOROUGHLY");
    println!("   - Write tests in schema.test.yaml");
    println!("   - Run 'inferadb schemas test'");
    println!("   - Test edge cases and permission boundaries");
    println!();
    println!("3. PREVIEW CHANGES");
    println!("   - Run 'inferadb schemas preview schema.ipl'");
    println!("   - Review breaking changes carefully");
    println!("   - Diff against current: 'inferadb schemas diff active schema.ipl'");
    println!();
    println!("4. DEPLOY SAFELY");
    println!("   - Option A: Direct push");
    println!("     inferadb schemas push schema.ipl --activate");
    println!();
    println!("   - Option B: Staged rollout (recommended for production)");
    println!("     inferadb schemas push schema.ipl");
    println!("     inferadb schemas activate <id> --canary 10");
    println!("     # Monitor, then promote");
    println!("     inferadb schemas canary promote");
    println!();
    println!("5. MONITOR");
    println!("   - Check 'inferadb stats' for anomalies");
    println!("   - Review 'inferadb what-changed --since 1h'");
    println!("   - Have rollback ready: 'inferadb schemas rollback'");
}

fn show_production_checklist_guide() {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Production Readiness Checklist");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("[ ] AUTHENTICATION");
    println!("    - Service accounts configured for each service");
    println!("    - Token rotation policy in place");
    println!("    - No shared credentials between environments");
    println!();
    println!("[ ] SCHEMA");
    println!("    - Schema tested with comprehensive test suite");
    println!("    - Schema validated in staging environment");
    println!("    - Rollback procedure documented and tested");
    println!();
    println!("[ ] MONITORING");
    println!("    - Health checks configured");
    println!("    - Alerting on permission denials spikes");
    println!("    - Audit logs being collected and stored");
    println!();
    println!("[ ] BACKUP");
    println!("    - Regular relationship exports scheduled");
    println!("    - Schema version history preserved");
    println!("    - Recovery procedure documented");
    println!();
    println!("[ ] ACCESS CONTROL");
    println!("    - Principle of least privilege applied");
    println!("    - Organization roles properly assigned");
    println!("    - API client permissions scoped appropriately");
}

fn show_incident_response_guide() {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Incident Response Guide");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("USERS UNEXPECTEDLY DENIED ACCESS");
    println!();
    println!("  1. Check service health:");
    println!("     inferadb health");
    println!();
    println!("  2. Verify the user's access:");
    println!("     inferadb check user:affected view resource:example --explain");
    println!();
    println!("  3. Check recent changes:");
    println!("     inferadb what-changed --since 1h");
    println!();
    println!("  4. If schema issue, rollback:");
    println!("     inferadb schemas rollback");
    println!();
    println!("USERS HAVE UNEXPECTED ACCESS");
    println!();
    println!("  1. Identify what they can access:");
    println!("     inferadb list-resources user:suspect view");
    println!();
    println!("  2. Check their relationships:");
    println!("     inferadb relationships list --subject user:suspect");
    println!();
    println!("  3. Check group memberships:");
    println!("     inferadb expand resource:sensitive viewer");
    println!();
    println!("  4. Remove inappropriate access:");
    println!("     inferadb relationships delete user:suspect viewer resource:sensitive");
}

/// Parse a time specification like "1h", "1d", "yesterday", or ISO timestamp.
fn parse_time_spec(spec: &str) -> chrono::DateTime<chrono::Utc> {
    use chrono::{Duration, Timelike, Utc};

    let now = Utc::now();

    // Try ISO format first
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(spec) {
        return dt.with_timezone(&Utc);
    }

    // Try relative formats
    match spec.to_lowercase().as_str() {
        "yesterday" => now - Duration::days(1),
        "today" => now - Duration::hours(i64::from(now.hour())),
        _ => {
            // Try parsing as duration like "1h", "1d", "30m"
            if let Some(stripped) = spec.strip_suffix('h')
                && let Ok(hours) = stripped.parse::<i64>()
            {
                return now - Duration::hours(hours);
            }
            if let Some(stripped) = spec.strip_suffix('d')
                && let Ok(days) = stripped.parse::<i64>()
            {
                return now - Duration::days(days);
            }
            if let Some(stripped) = spec.strip_suffix('m')
                && let Ok(mins) = stripped.parse::<i64>()
            {
                return now - Duration::minutes(mins);
            }
            if let Some(stripped) = spec.strip_suffix('w')
                && let Ok(weeks) = stripped.parse::<i64>()
            {
                return now - Duration::weeks(weeks);
            }

            // Default to 1 day ago
            now - Duration::days(1)
        },
    }
}
