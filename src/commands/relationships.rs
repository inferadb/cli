//! Relationship management commands.

use inferadb::Relationship;
use serde::Serialize;

use crate::{client::Context, error::Result, output::Displayable};

#[derive(Debug, Clone, Serialize)]
struct RelationshipRow {
    resource: String,
    relation: String,
    subject: String,
}

impl Displayable for RelationshipRow {
    fn table_row(&self) -> Vec<String> {
        vec![self.resource.clone(), self.relation.clone(), self.subject.clone()]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["RESOURCE", "RELATION", "SUBJECT"]
    }
}

/// List relationships.
pub async fn relationships_list(
    ctx: &Context,
    resource: Option<&str>,
    subject: Option<&str>,
    relation: Option<&str>,
    limit: u32,
    cursor: Option<&str>,
) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();

    // Build the list query with filters
    let mut query = vault.relationships().list();

    if let Some(r) = resource {
        query = query.resource(r);
    }
    if let Some(s) = subject {
        query = query.subject(s);
    }
    if let Some(rel) = relation {
        query = query.relation(rel);
    }

    query = query.limit(limit as usize);

    if let Some(c) = cursor {
        query = query.cursor(c);
    }

    let response = query.await?;

    if response.relationships.is_empty() {
        ctx.output.info("No relationships found.");
        return Ok(());
    }

    let rows: Vec<RelationshipRow> = response
        .relationships
        .iter()
        .map(|rel| RelationshipRow {
            resource: rel.resource().to_string(),
            relation: rel.relation().to_string(),
            subject: rel.subject().to_string(),
        })
        .collect();

    ctx.output.table(&rows)?;

    if !ctx.output.is_quiet() && rows.len() == limit as usize {
        ctx.output.info(&format!("\nShowing {limit} results. Use --cursor for pagination."));
    }

    Ok(())
}

/// Add a relationship.
pub async fn relationships_add(
    ctx: &Context,
    subject: &str,
    relation: &str,
    resource: &str,
    if_not_exists: bool,
) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();

    // Note: SDK uses Relationship::new(resource, relation, subject)
    // but CLI uses "subject relation resource" order for readability
    let rel = Relationship::new(resource, relation, subject);

    let result = vault.relationships().write(rel).await;

    match result {
        Ok(_) => {
            ctx.output.success(&format!("Added: {subject} {relation} {resource}"));
        },
        Err(e) => {
            // Check if it's a duplicate error
            if if_not_exists && e.kind() == inferadb::ErrorKind::Conflict {
                ctx.output.info("Relationship already exists.");
                return Ok(());
            }
            return Err(e.into());
        },
    }

    Ok(())
}

/// Delete a relationship.
pub async fn relationships_delete(
    ctx: &Context,
    subject: &str,
    relation: &str,
    resource: &str,
    if_exists: bool,
) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();

    let rel = Relationship::new(resource, relation, subject);

    let result = vault.relationships().delete(rel).await;

    match result {
        Ok(()) => {
            ctx.output.success(&format!("Deleted: {subject} {relation} {resource}"));
        },
        Err(e) => {
            if if_exists && e.kind() == inferadb::ErrorKind::NotFound {
                ctx.output.info("Relationship does not exist.");
                return Ok(());
            }
            return Err(e.into());
        },
    }

    Ok(())
}

/// Show relationship history.
pub async fn history(
    ctx: &Context,
    _resource: Option<&str>,
    _from: Option<&str>,
    _to: Option<&str>,
) -> Result<()> {
    ctx.output.info("Relationship history not yet implemented.");
    Ok(())
}

/// Validate relationships against schema.
pub async fn validate(ctx: &Context, file: Option<&str>) -> Result<()> {
    if let Some(f) = file {
        ctx.output.info(&format!("Validating relationships in '{f}'..."));
        // TODO: Read file and validate
    } else {
        ctx.output.info("Validating relationships in current vault...");
        // TODO: Validate against active schema
    }
    ctx.output.info("Validation not yet implemented.");
    Ok(())
}
