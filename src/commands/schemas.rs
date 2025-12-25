//! Schema management commands.

use crate::client::Context;
use crate::error::Result;
use crate::output::Displayable;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct SchemaRow {
    version: String,
    status: String,
    created_at: String,
    activated_at: String,
}

impl Displayable for SchemaRow {
    fn table_row(&self) -> Vec<String> {
        vec![
            self.version.clone(),
            self.status.clone(),
            self.created_at.clone(),
            self.activated_at.clone(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["VERSION", "STATUS", "CREATED", "ACTIVATED"]
    }
}

/// List schema versions.
pub async fn list(ctx: &Context, all: bool) -> Result<()> {
    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    let page = if all {
        schemas.list().await?
    } else {
        // By default, show recent schemas
        schemas.list().limit(20).await?
    };

    if page.items.is_empty() {
        ctx.output.info("No schemas found.");
        ctx.output
            .info("Push your first schema with 'inferadb schemas push <file>'.");
        return Ok(());
    }

    let rows: Vec<SchemaRow> = page
        .items
        .iter()
        .map(|s| SchemaRow {
            version: s.version.clone(),
            status: s.status.to_string(),
            created_at: s.created_at.format("%Y-%m-%d %H:%M").to_string(),
            activated_at: s
                .activated_at
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "-".to_string()),
        })
        .collect();

    ctx.output.table(&rows)
}

/// Get schema content.
pub async fn get(ctx: &Context, id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    let schema = if id == "active" {
        schemas.get_active().await?
    } else {
        schemas.get(id).await?
    };

    // For schema content, just print it directly
    println!("{}", schema.content);
    Ok(())
}

/// Preview schema changes.
pub async fn preview(ctx: &Context, file: &str, base: Option<&str>, impact: bool) -> Result<()> {
    let content = std::fs::read_to_string(file)?;

    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    // Validate the schema first
    ctx.output.info("Validating schema...");
    let validation = schemas.validate(&content).await?;

    if !validation.is_valid() {
        ctx.output.error("Schema validation failed:");
        for err in &validation.errors {
            eprintln!("  Line {}: {} [{}]", err.line, err.message, err.code);
        }
        return Err(crate::error::Error::parse("Schema validation failed"));
    }

    if validation.has_warnings() {
        ctx.output.warn("Warnings:");
        for warn in &validation.warnings {
            eprintln!("  Line {}: {} [{}]", warn.line, warn.message, warn.code);
        }
    }

    ctx.output.success("Schema is valid!");

    // If impact analysis requested and we have a base version, show diff
    if impact {
        let base_version = base.unwrap_or("active");
        ctx.output
            .info(&format!("Comparing against version '{}'...", base_version));

        // Push to get a version, then diff
        // For now, just show validation result
        ctx.output
            .info("Impact analysis requires pushing the schema first.");
        ctx.output
            .info("Use 'inferadb schemas push <file>' to create a version, then use 'inferadb schemas diff'.");
    }

    Ok(())
}

/// Push schema to vault.
pub async fn push(
    ctx: &Context,
    file: &str,
    activate: bool,
    message: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    let content = std::fs::read_to_string(file)?;

    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    if dry_run {
        ctx.output.info("Dry run: validating schema...");
        let validation = schemas.validate(&content).await?;

        if !validation.is_valid() {
            ctx.output.error("Schema validation failed:");
            for err in &validation.errors {
                eprintln!("  Line {}: {} [{}]", err.line, err.message, err.code);
            }
            return Err(crate::error::Error::parse("Schema validation failed"));
        }

        ctx.output.success("Schema is valid (dry run).");
        if validation.has_warnings() {
            ctx.output.warn("Warnings:");
            for warn in &validation.warnings {
                eprintln!("  Line {}: {} [{}]", warn.line, warn.message, warn.code);
            }
        }
        return Ok(());
    }

    ctx.output.info("Pushing schema...");
    let result = schemas.push(&content).await?;

    if !result.validation.is_valid() {
        ctx.output.error("Schema validation failed:");
        for err in &result.validation.errors {
            eprintln!("  Line {}: {} [{}]", err.line, err.message, err.code);
        }
        return Err(crate::error::Error::parse("Schema validation failed"));
    }

    let version = &result.schema.version;
    ctx.output
        .success(&format!("Schema version {} created.", version));

    if let Some(msg) = message {
        ctx.output.info(&format!("Message: {}", msg));
    }

    if result.validation.has_warnings() {
        ctx.output.warn("Warnings:");
        for warn in &result.validation.warnings {
            eprintln!("  Line {}: {} [{}]", warn.line, warn.message, warn.code);
        }
    }

    if activate {
        ctx.output.info("Activating schema...");
        schemas.activate(version).await?;
        ctx.output
            .success(&format!("Schema version {} is now active.", version));
    } else {
        ctx.output.info(&format!(
            "To activate: inferadb schemas activate {}",
            version
        ));
    }

    Ok(())
}

/// Activate a schema version.
#[allow(dead_code)]
pub async fn activate(ctx: &Context, version: &str) -> Result<()> {
    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    ctx.output
        .info(&format!("Activating schema version {}...", version));
    let schema = schemas.activate(version).await?;

    ctx.output
        .success(&format!("Schema version {} is now active.", schema.version));
    Ok(())
}

/// Rollback to a previous schema version.
pub async fn rollback(ctx: &Context, version: Option<&str>) -> Result<()> {
    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    let target_version = if let Some(v) = version {
        v.to_string()
    } else {
        // Find the previous version
        let page = schemas.list().limit(10).await?;
        let active_idx = page.items.iter().position(|s| s.status.is_active());

        match active_idx {
            Some(idx) if idx + 1 < page.items.len() => page.items[idx + 1].version.clone(),
            _ => {
                ctx.output
                    .error("No previous version found to rollback to.");
                return Ok(());
            }
        }
    };

    if !ctx.yes && !ctx.confirm(&format!("Rollback to schema version {}?", target_version))? {
        ctx.output.info("Cancelled.");
        return Ok(());
    }

    ctx.output
        .info(&format!("Rolling back to version {}...", target_version));
    schemas.activate(&target_version).await?;
    ctx.output.success(&format!(
        "Rolled back to schema version {}.",
        target_version
    ));

    Ok(())
}

/// Show diff between schema versions.
pub async fn diff(ctx: &Context, from: &str, to: &str) -> Result<()> {
    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    let diff = schemas.diff(from, to).await?;

    println!("Schema diff: {} -> {}", diff.from_version, diff.to_version);
    println!();

    if diff.changes.is_empty() {
        println!("No changes between versions.");
        return Ok(());
    }

    println!(
        "Backward compatible: {}",
        if diff.is_backward_compatible {
            "yes"
        } else {
            "NO"
        }
    );
    println!();
    println!("Changes:");

    for change in &diff.changes {
        let breaking_marker = if change.is_breaking {
            " [BREAKING]"
        } else {
            ""
        };
        let entity = change
            .entity_type
            .as_deref()
            .map(|e| format!(" ({})", e))
            .unwrap_or_default();
        println!(
            "  {} {}{}{}",
            change.change_type, change.description, entity, breaking_marker
        );
    }

    Ok(())
}

/// Validate schema without pushing.
pub async fn validate(ctx: &Context, file: &str) -> Result<()> {
    let content = std::fs::read_to_string(file)?;

    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    let validation = schemas.validate(&content).await?;

    if validation.is_valid() {
        ctx.output.success("Schema is valid!");
        if validation.has_warnings() {
            ctx.output.warn("Warnings:");
            for warn in &validation.warnings {
                eprintln!("  Line {}: {} [{}]", warn.line, warn.message, warn.code);
            }
        }
        Ok(())
    } else {
        ctx.output.error("Schema validation failed:");
        for err in &validation.errors {
            eprintln!("  Line {}: {} [{}]", err.line, err.message, err.code);
        }
        Err(crate::error::Error::parse("Schema validation failed"))
    }
}

/// Initialize a schema project.
pub async fn init(ctx: &Context, path: &str, template: &str) -> Result<()> {
    use std::fs;
    use std::path::Path;

    let schema_dir = Path::new(path);
    let schema_file = schema_dir.join("schema.ipl");

    if schema_file.exists() {
        ctx.output.warn(&format!(
            "Schema file already exists: {}",
            schema_file.display()
        ));
        if !ctx.confirm("Overwrite?")? {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    // Create directory if needed
    if !schema_dir.exists() {
        fs::create_dir_all(schema_dir)?;
    }

    let content = match template {
        "blank" => {
            r#"// InferaDB Schema (IPL)
// Documentation: https://docs.inferadb.com/schema

entity User {
    // Define relations and permissions here
}
"#
        }
        "basic" => {
            r#"// InferaDB Schema (IPL)
// Basic document sharing model

entity User {}

entity Organization {
    relations {
        admin: User
        member: User
    }

    permissions {
        manage: admin
        view: admin | member
    }
}

entity Document {
    relations {
        owner: User
        editor: User
        viewer: User
        parent: Organization
    }

    permissions {
        delete: owner | parent.admin
        edit: owner | editor | parent.admin
        view: owner | editor | viewer | parent.member
    }
}
"#
        }
        "rbac" => {
            r#"// InferaDB Schema (IPL)
// Role-based access control model

entity User {}

entity Role {
    relations {
        assignee: User
    }
}

entity Resource {
    relations {
        admin_role: Role
        editor_role: Role
        viewer_role: Role
    }

    permissions {
        admin: admin_role.assignee
        edit: admin | editor_role.assignee
        view: admin | edit | viewer_role.assignee
    }
}
"#
        }
        _ => {
            return Err(crate::error::Error::invalid_arg(format!(
                "Unknown template '{}'. Use: blank, basic, rbac",
                template
            )));
        }
    };

    fs::write(&schema_file, content)?;

    ctx.output
        .success(&format!("Created schema at: {}", schema_file.display()));
    ctx.output.info("");
    ctx.output.info("Next steps:");
    ctx.output
        .info("  1. Edit the schema file to define your authorization model");
    ctx.output
        .info("  2. Validate: inferadb schemas validate schema.ipl");
    ctx.output
        .info("  3. Push: inferadb schemas push schema.ipl --activate");

    Ok(())
}

/// Delete a schema version.
#[allow(dead_code)]
pub async fn delete(ctx: &Context, version: &str) -> Result<()> {
    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    // Check if it's the active version
    let schema = schemas.get(version).await?;
    if schema.status.is_active() {
        ctx.output.error("Cannot delete the active schema version.");
        ctx.output
            .info("Activate a different version first, then delete this one.");
        return Ok(());
    }

    if !ctx.yes && !ctx.confirm(&format!("Delete schema version {}?", version))? {
        ctx.output.info("Cancelled.");
        return Ok(());
    }

    schemas.delete(version).await?;
    ctx.output
        .success(&format!("Schema version {} deleted.", version));

    Ok(())
}

/// Activate a schema version with options.
pub async fn activate_with_options(
    ctx: &Context,
    version: &str,
    show_diff: bool,
    canary_percent: Option<u8>,
) -> Result<()> {
    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    // Show diff if requested
    if show_diff {
        ctx.output.info("Comparing with active schema...");
        let diff_result = schemas.diff("active", version).await?;

        if diff_result.changes.is_empty() {
            ctx.output.info("No changes from active version.");
        } else {
            println!(
                "Backward compatible: {}",
                if diff_result.is_backward_compatible {
                    "yes"
                } else {
                    "NO"
                }
            );
            println!();
            println!("Changes:");
            for change in &diff_result.changes {
                let breaking_marker = if change.is_breaking {
                    " [BREAKING]"
                } else {
                    ""
                };
                println!(
                    "  {} {}{}",
                    change.change_type, change.description, breaking_marker
                );
            }
            println!();
        }

        if !ctx.yes && !ctx.confirm("Proceed with activation?")? {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    // Canary deployment
    if let Some(percent) = canary_percent {
        ctx.output
            .info(&format!("Starting canary deployment at {}%...", percent));
        // Note: Canary deployment API would be used here when available
        ctx.output
            .warn("Canary deployment not yet supported by SDK.");
        ctx.output.info("Proceeding with full activation.");
    }

    ctx.output
        .info(&format!("Activating schema version {}...", version));
    let schema = schemas.activate(version).await?;

    ctx.output
        .success(&format!("Schema version {} is now active.", schema.version));
    Ok(())
}

/// Format a schema file.
pub async fn format(ctx: &Context, file: &str, write: bool) -> Result<()> {
    let content = std::fs::read_to_string(file)?;

    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    // Validate first to check for syntax errors
    let validation = schemas.validate(&content).await?;
    if !validation.is_valid() {
        ctx.output.error("Cannot format: schema has syntax errors");
        for err in &validation.errors {
            eprintln!("  Line {}: {} [{}]", err.line, err.message, err.code);
        }
        return Err(crate::error::Error::parse("Schema has syntax errors"));
    }

    // Note: Schema formatting API would be used here when available
    // For now, we'll just validate and report
    ctx.output
        .warn("Schema formatting not yet supported by SDK.");
    ctx.output.info("Schema is valid and properly structured.");

    if write {
        ctx.output
            .info("No changes written (formatting not available).");
    }

    Ok(())
}

/// Run schema tests.
pub async fn test(
    ctx: &Context,
    tests_file: Option<&str>,
    schema_file: Option<&str>,
    name_filter: Option<&str>,
) -> Result<()> {
    // Default test file
    let tests_path = tests_file.unwrap_or("schema.test.yaml");
    let schema_path = schema_file.unwrap_or("schema.ipl");

    if !std::path::Path::new(tests_path).exists() {
        ctx.output
            .error(&format!("Test file not found: {}", tests_path));
        ctx.output.info("Create a test file with check assertions.");
        ctx.output.info("");
        ctx.output.info("Example schema.test.yaml:");
        ctx.output.info("  tests:");
        ctx.output.info("    - name: owner can edit");
        ctx.output
            .info("      check: user:alice can edit doc:readme");
        ctx.output.info("      expect: allow");
        return Ok(());
    }

    if !std::path::Path::new(schema_path).exists() {
        ctx.output
            .error(&format!("Schema file not found: {}", schema_path));
        return Ok(());
    }

    ctx.output
        .info(&format!("Running tests from {}...", tests_path));

    if let Some(filter) = name_filter {
        ctx.output.info(&format!("Filtering by: {}", filter));
    }

    // Note: Schema testing would parse the test file and run checks
    // For now, show placeholder
    ctx.output.warn("Schema testing not yet implemented.");
    ctx.output
        .info("Use 'inferadb check' commands to manually verify authorization.");

    Ok(())
}

/// Watch for schema changes.
pub async fn watch(ctx: &Context, file: &str, run_tests: bool, auto_push: bool) -> Result<()> {
    use std::path::Path;

    if !Path::new(file).exists() {
        ctx.output
            .error(&format!("Schema file not found: {}", file));
        return Ok(());
    }

    ctx.output
        .info(&format!("Watching {} for changes...", file));
    if run_tests {
        ctx.output.info("Will run tests on change.");
    }
    if auto_push {
        ctx.output.info("Will auto-push on successful validation.");
    }

    // Note: File watching would use notify crate
    ctx.output.warn("File watching not yet implemented.");
    ctx.output
        .info("Use 'inferadb schemas validate' or 'inferadb schemas push' manually.");

    Ok(())
}

/// Dispatch canary subcommands.
pub async fn canary_dispatch(ctx: &Context, cmd: &crate::cli::CanaryCommands) -> Result<()> {
    use crate::cli::CanaryCommands;

    match cmd {
        CanaryCommands::Status => {
            ctx.output.info("Checking canary deployment status...");
            // Note: Canary status API would be used here
            ctx.output
                .warn("Canary deployments not yet supported by SDK.");
            ctx.output.info("No active canary deployment.");
        }
        CanaryCommands::Promote { wait } => {
            ctx.output.info("Promoting canary deployment...");
            if *wait {
                ctx.output.info("Waiting for completion...");
            }
            ctx.output
                .warn("Canary deployments not yet supported by SDK.");
        }
        CanaryCommands::Rollback => {
            ctx.output.info("Rolling back canary deployment...");
            ctx.output
                .warn("Canary deployments not yet supported by SDK.");
        }
    }

    Ok(())
}
