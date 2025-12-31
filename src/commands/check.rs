//! Authorization check commands.

use serde::Serialize;

use crate::{
    client::Context,
    error::{Error, Result},
};

/// Check authorization.
pub async fn check(
    ctx: &Context,
    subject: &str,
    permission: &str,
    resource: &str,
    trace: bool,
    explain: bool,
    context_json: Option<&str>,
) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();

    #[derive(Serialize)]
    struct CheckResult {
        subject: String,
        permission: String,
        resource: String,
        allowed: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    }

    // Build check request
    let mut check_req = vault.check(subject, permission, resource);

    // Add ABAC context if provided
    if let Some(json) = context_json {
        let context_map: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_str(json)
                .map_err(|e| Error::parse(format!("Invalid context JSON: {}", e)))?;

        let mut ctx_builder = inferadb::Context::new();
        for (key, value) in context_map {
            let context_value = match value {
                serde_json::Value::String(s) => inferadb::ContextValue::String(s),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        inferadb::ContextValue::Integer(i)
                    } else if let Some(f) = n.as_f64() {
                        inferadb::ContextValue::Float(f)
                    } else {
                        continue;
                    }
                },
                serde_json::Value::Bool(b) => inferadb::ContextValue::Bool(b),
                _ => continue,
            };
            ctx_builder = ctx_builder.with(key, context_value);
        }
        check_req = check_req.with_context(ctx_builder);
    }

    // Execute check
    let allowed = check_req.await?;

    let result = CheckResult {
        subject: subject.to_string(),
        permission: permission.to_string(),
        resource: resource.to_string(),
        allowed,
        reason: None, // TODO: Add when explain is implemented
    };

    if ctx.output.format() == crate::output::OutputFormat::Table {
        if allowed {
            ctx.output.success(&format!("{} {} {} → allowed", subject, permission, resource));
        } else {
            ctx.output.error(&format!("{} {} {} → denied", subject, permission, resource));

            if explain {
                ctx.output.info("");
                ctx.output.info("To see why access was denied, use 'inferadb explain-permission'");
            }

            // For scripting: return exit code 20 for authorization denied
            return Err(Error::AccessDenied);
        }
    } else {
        ctx.output.value(&result)?;
        if !allowed {
            return Err(Error::AccessDenied);
        }
    }

    if trace {
        ctx.output.info("");
        ctx.output.info("Trace:");
        ctx.output.info("  (Trace output not yet implemented)");
    }

    Ok(())
}

/// Simulate authorization with hypothetical changes.
pub async fn simulate(
    ctx: &Context,
    subject: &str,
    permission: &str,
    resource: &str,
    add_relationships: &[String],
    remove_relationships: &[String],
) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();

    let mut sim = vault.simulate();

    // Add hypothetical relationships
    for rel_str in add_relationships {
        let rel = parse_relationship(rel_str)?;
        sim = sim.add_relationship(rel);
    }

    // Remove hypothetical relationships
    for rel_str in remove_relationships {
        let rel = parse_relationship(rel_str)?;
        sim = sim.remove_relationship(rel);
    }

    // Execute simulated check
    let result = sim.check(subject, permission, resource).await?;

    if result.allowed {
        ctx.output
            .success(&format!("With changes: {} {} {} → allowed", subject, permission, resource));
    } else {
        ctx.output
            .error(&format!("With changes: {} {} {} → denied", subject, permission, resource));
    }

    Ok(())
}

/// Show userset expansion tree.
pub async fn expand(ctx: &Context, resource: &str, relation: &str, _max_depth: u32) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();

    ctx.output.info(&format!("Expanding {}#{}", resource, relation));
    ctx.output.info("");

    // TODO: Implement expand using SDK
    // For now, list subjects as a fallback
    let subjects: Vec<String> =
        vault.subjects().with_permission(relation).on_resource(resource).collect().await?;

    if subjects.is_empty() {
        ctx.output.info("(no subjects found)");
    } else {
        for subject in subjects {
            println!("  └─ {}", subject);
        }
    }

    Ok(())
}

/// Explain how a permission is computed.
pub async fn explain_permission(
    ctx: &Context,
    subject: &str,
    permission: &str,
    resource: &str,
) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();

    let explanation = vault
        .explain_permission()
        .subject(subject)
        .permission(permission)
        .resource(resource)
        .await?;

    ctx.output.info(&format!("Explaining: {} {} {}", subject, permission, resource));
    ctx.output.info("");

    ctx.output.value(&explanation)?;

    Ok(())
}

/// List resources accessible by a subject.
pub async fn list_resources(
    ctx: &Context,
    subject: &str,
    permission: &str,
    resource_type_filter: Option<&str>,
) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();

    let mut query = vault.resources().accessible_by(subject).with_permission(permission);

    if let Some(rt) = resource_type_filter {
        query = query.resource_type(rt);
    }

    let resources: Vec<String> = query.collect().await?;

    if resources.is_empty() {
        ctx.output.info("No accessible resources found.");
    } else {
        for resource in &resources {
            println!("{}", resource);
        }
        ctx.output.info(&format!("\nTotal: {} resources", resources.len()));
    }

    Ok(())
}

/// List subjects with access to a resource.
pub async fn list_subjects(
    ctx: &Context,
    resource: &str,
    permission: &str,
    subject_type_filter: Option<&str>,
) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();

    let mut query = vault.subjects().with_permission(permission).on_resource(resource);

    if let Some(st) = subject_type_filter {
        query = query.subject_type(st);
    }

    let subjects: Vec<String> = query.collect().await?;

    if subjects.is_empty() {
        ctx.output.info("No subjects with access found.");
    } else {
        for subject in &subjects {
            println!("{}", subject);
        }
        ctx.output.info(&format!("\nTotal: {} subjects", subjects.len()));
    }

    Ok(())
}

/// Parse a relationship string in the format "resource#relation@subject".
fn parse_relationship(s: &str) -> Result<inferadb::Relationship<'static>> {
    s.parse().map_err(|_| {
        Error::parse(format!(
            "Invalid relationship format: '{}'. Expected: resource#relation@subject",
            s
        ))
    })
}
