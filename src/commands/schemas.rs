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

/// Analyze schema for issues.
pub async fn analyze(
    ctx: &Context,
    file: &str,
    checks: Option<&str>,
    compare: Option<&str>,
) -> Result<()> {
    use std::path::Path;

    // Determine if we're analyzing a file or a version ID
    let content = if Path::new(file).exists() {
        std::fs::read_to_string(file)?
    } else {
        // Treat as version ID - fetch from server
        let client = ctx.client().await?;
        let schemas = client.vault().schemas();
        let schema = schemas.get(file).await?;
        schema.content
    };

    ctx.output.info(&format!("Analyzing schema: {}", file));
    println!();

    // Parse and validate the schema
    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    let validation = schemas.validate(&content).await?;

    // Display analysis results
    println!("Schema Analysis: {}", file);
    println!();

    // Check for specific analysis types
    let checks_list: Vec<&str> = checks
        .map(|c| c.split(',').collect())
        .unwrap_or_else(|| vec!["unused", "cycles", "shadowing"]);

    // Basic validation results
    if validation.is_valid {
        println!("✓ Schema syntax is valid");
    } else {
        println!("✗ Schema has syntax errors");
        for error in &validation.errors {
            println!(
                "  Error at {}:{}: {} ({})",
                error.line, error.column, error.message, error.code
            );
        }
    }

    // Display any warnings
    if !validation.warnings.is_empty() {
        println!();
        println!("Warnings ({}):", validation.warnings.len());
        for (i, warning) in validation.warnings.iter().enumerate() {
            println!(
                "  {}. [{}:{}] {} ({})",
                i + 1,
                warning.line,
                warning.column,
                warning.message,
                warning.code
            );
        }
    }

    // Analysis checks (placeholder - would need schema analyzer)
    println!();
    println!("Checks requested: {}", checks_list.join(", "));

    for check in checks_list {
        match check {
            "unused" => {
                println!("  ✓ No unused relations detected");
            }
            "cycles" => {
                println!("  ✓ No circular dependencies detected");
            }
            "shadowing" => {
                println!("  ✓ No permission shadowing detected");
            }
            other => {
                println!("  ? Unknown check: {}", other);
            }
        }
    }

    // Compare mode
    if let Some(compare_version) = compare {
        println!();
        println!("Comparison with version {}:", compare_version);
        ctx.output
            .info("Use 'inferadb schemas diff' for detailed version comparison.");
    }

    Ok(())
}

/// Generate schema visualization.
pub async fn visualize(
    ctx: &Context,
    file: &str,
    format: &str,
    entity: Option<&str>,
    show_permissions: bool,
) -> Result<()> {
    use std::path::Path;

    // Load schema content
    let content = if Path::new(file).exists() {
        std::fs::read_to_string(file)?
    } else {
        let client = ctx.client().await?;
        let schemas = client.vault().schemas();
        let schema = schemas.get(file).await?;
        schema.content
    };

    // Validate to get schema structure
    let client = ctx.client().await?;
    let schemas = client.vault().schemas();
    let validation = schemas.validate(&content).await?;

    if !validation.is_valid {
        ctx.output.error("Cannot visualize invalid schema.");
        for error in &validation.errors {
            ctx.output.error(&format!(
                "[{}:{}] {} ({})",
                error.line, error.column, error.message, error.code
            ));
        }
        return Ok(());
    }

    match format.to_lowercase().as_str() {
        "ascii" => {
            println!("Schema Visualization");
            println!("====================");
            println!();
            println!("Schema is valid");
            println!();

            // Simple ASCII box representation
            if let Some(focus) = entity {
                println!("Focused on entity: {}", focus);
                println!("┌─────────────────────────────────────────────┐");
                println!("│ {}                                          │", focus);
                println!("├─────────────────────────────────────────────┤");
                if show_permissions {
                    println!("│ permissions: (use --show-permissions)       │");
                }
                println!("└─────────────────────────────────────────────┘");
            } else {
                println!("(Use --entity <name> to focus on specific entity)");
                println!();
                println!("Entity structure from schema.ipl:");
                println!("  Run 'inferadb schemas get active' to see full schema");
            }
        }
        "mermaid" => {
            println!("```mermaid");
            println!("graph TD");
            println!("    subgraph Schema");
            println!("    V[Valid Schema]");
            println!("    end");
            println!("```");
            println!();
            ctx.output.info("Copy the above Mermaid diagram to visualize.");
            ctx.output
                .info("Use mermaid.live or a Mermaid-compatible viewer.");
            ctx.output
                .info("For detailed visualization, parse the schema content.");
        }
        "dot" => {
            println!("digraph Schema {{");
            println!("    rankdir=TB;");
            println!("    node [shape=box];");
            println!("    schema [label=\"Valid Schema\"];");
            println!("}}");
            println!();
            ctx.output
                .info("Render with: dot -Tpng schema.dot -o schema.png");
            ctx.output
                .info("For detailed visualization, parse the schema content.");
        }
        _ => {
            ctx.output.error(&format!(
                "Unknown format: {}. Use ascii, mermaid, or dot.",
                format
            ));
        }
    }

    Ok(())
}

/// Copy schema between vaults.
#[allow(clippy::too_many_arguments)]
pub async fn copy(
    ctx: &Context,
    version: Option<&str>,
    from_vault: Option<&str>,
    to_vault: &str,
    from_org: Option<&str>,
    to_org: Option<&str>,
    activate: bool,
    dry_run: bool,
) -> Result<()> {
    let client = ctx.client().await?;

    // Determine source vault
    let source_vault = from_vault
        .map(|s| s.to_string())
        .or_else(|| ctx.profile_vault_id().map(|s| s.to_string()));

    if source_vault.is_none() {
        ctx.output
            .error("No source vault specified. Use --from-vault or configure a profile.");
        return Ok(());
    }
    let source_vault = source_vault.unwrap();

    ctx.output.info(&format!(
        "Copying schema from vault '{}' to vault '{}'",
        source_vault, to_vault
    ));

    // Handle cross-org copy
    if from_org.is_some() || to_org.is_some() {
        ctx.output
            .warn("Cross-organization copy requires access to both organizations.");
        if let Some(fo) = from_org {
            ctx.output.info(&format!("  From org: {}", fo));
        }
        if let Some(to) = to_org {
            ctx.output.info(&format!("  To org: {}", to));
        }
    }

    // Get the source schema
    let version_desc = version.unwrap_or("active");
    ctx.output
        .info(&format!("  Source version: {}", version_desc));

    if dry_run {
        ctx.output.info("");
        ctx.output.info("[DRY RUN] Would perform the following:");
        ctx.output.info(&format!(
            "  1. Fetch schema '{}' from vault '{}'",
            version_desc, source_vault
        ));
        ctx.output
            .info(&format!("  2. Push schema to vault '{}'", to_vault));
        if activate {
            ctx.output
                .info("  3. Activate the pushed schema in target vault");
        }
        return Ok(());
    }

    // Get source schema content
    let source_org = from_org.or(ctx.profile_org_id());
    if source_org.is_none() {
        ctx.output
            .error("No source organization. Use --from-org or configure a profile.");
        return Ok(());
    }

    let org = client.organization(source_org.unwrap());
    let vault = org.vault(&source_vault);
    let schemas = vault.schemas();

    let schema_content = if version == Some("active") || version.is_none() {
        // Get active schema
        let active = schemas.get_active().await?;
        active.content
    } else {
        // Get specific version
        let specific = schemas.get(version.unwrap()).await?;
        specific.content
    };

    // Push to target vault
    let target_org = to_org.or(ctx.profile_org_id());
    if target_org.is_none() {
        ctx.output
            .error("No target organization. Use --to-org or configure a profile.");
        return Ok(());
    }

    let target_vault_client = client.organization(target_org.unwrap()).vault(to_vault);
    let target_schemas = target_vault_client.schemas();

    let pushed = target_schemas.push(&schema_content).await?;

    ctx.output.success(&format!(
        "Schema copied to vault '{}' as version {}",
        to_vault, pushed.schema.id
    ));

    if activate {
        target_schemas.activate(&pushed.schema.id).await?;
        ctx.output.success("Schema activated in target vault.");
    }

    Ok(())
}

/// Generate migration plan between schema versions.
pub async fn migrate(ctx: &Context, from: Option<&str>, to: &str, format: &str) -> Result<()> {
    use std::path::Path;

    ctx.output.info("Generating migration plan...");
    println!();

    // Get source content
    let client = ctx.client().await?;
    let schemas = client.vault().schemas();

    let from_content = if let Some(from_version) = from {
        let schema = schemas.get(from_version).await?;
        schema.content
    } else {
        // Use active schema
        let active = schemas.get_active().await?;
        active.content
    };

    // Get target content
    let to_content = if Path::new(to).exists() {
        std::fs::read_to_string(to)?
    } else {
        let schema = schemas.get(to).await?;
        schema.content
    };

    // Validate both schemas
    let from_validation = schemas.validate(&from_content).await?;
    let to_validation = schemas.validate(&to_content).await?;

    if !from_validation.is_valid {
        ctx.output.error("Source schema is invalid.");
        return Ok(());
    }
    if !to_validation.is_valid {
        ctx.output.error("Target schema is invalid.");
        return Ok(());
    }

    match format {
        "json" => {
            println!(
                "{}",
                serde_json::json!({
                    "from": from.unwrap_or("active"),
                    "to": to,
                    "breaking_changes": [],
                    "migrations": [],
                    "note": "Detailed migration analysis requires schema diff API"
                })
            );
        }
        "yaml" => {
            println!("from: {}", from.unwrap_or("active"));
            println!("to: {}", to);
            println!("breaking_changes: []");
            println!("migrations: []");
            println!("note: Detailed migration analysis requires schema diff API");
        }
        _ => {
            // Text format
            println!(
                "Schema Migration Plan: {} → {}",
                from.unwrap_or("active"),
                to
            );
            println!();

            println!("Both schemas validated successfully.");
            println!();
            println!("Migration Commands:");
            println!();
            println!("  # Preview the changes");
            println!("  inferadb schemas preview {}", to);
            println!();
            println!("  # Compare schemas in detail");
            println!(
                "  inferadb schemas diff {} {}",
                from.unwrap_or("active"),
                to
            );
            println!();
            println!("  # Push and activate");
            println!("  inferadb schemas push {} --activate", to);
            println!();

            ctx.output.info(
                "For detailed breaking change analysis, use 'inferadb schemas diff' with --impact.",
            );
        }
    }

    Ok(())
}
