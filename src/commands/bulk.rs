//! Bulk export and import operations.

use crate::client::Context;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A relationship for export/import.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportedRelationship {
    resource: String,
    relation: String,
    subject: String,
}

/// Export format wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportData {
    version: String,
    relationships: Vec<ExportedRelationship>,
}

/// Export relationships to a file.
pub async fn export(
    ctx: &Context,
    output: Option<&str>,
    resource_type: Option<&str>,
    format: &str,
) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();
    let rels = vault.relationships();

    ctx.output.info("Exporting relationships...");

    // Collect relationships
    let mut relationships = Vec::new();

    let mut req = rels.list().limit(1000);

    if let Some(rt) = resource_type {
        req = req.resource(format!("{}:*", rt));
    }

    let page = req.await?;

    for rel in page.iter() {
        relationships.push(ExportedRelationship {
            resource: rel.resource().to_string(),
            relation: rel.relation().to_string(),
            subject: rel.subject().to_string(),
        });
    }

    // If there are more, notify user
    if page.has_more() {
        ctx.output
            .warn("More relationships available. Only first 1000 exported.");
        ctx.output
            .info("Use pagination to export more (not yet implemented).");
    }

    if relationships.is_empty() {
        ctx.output.info("No relationships found to export.");
        return Ok(());
    }

    ctx.output
        .info(&format!("Found {} relationships.", relationships.len()));

    // Format the data
    let export_data = ExportData {
        version: "1.0".to_string(),
        relationships,
    };

    let content = match format {
        "json" => serde_json::to_string_pretty(&export_data)?,
        "yaml" | "yml" => serde_yaml::to_string(&export_data)?,
        "csv" => {
            let mut csv = String::from("resource,relation,subject\n");
            for rel in &export_data.relationships {
                csv.push_str(&format!(
                    "{},{},{}\n",
                    rel.resource, rel.relation, rel.subject
                ));
            }
            csv
        }
        _ => {
            ctx.output.error(&format!(
                "Unknown format: {}. Use json, yaml, or csv.",
                format
            ));
            return Ok(());
        }
    };

    // Write to file or stdout
    match output {
        Some(path) => {
            std::fs::write(path, &content)?;
            ctx.output.success(&format!(
                "Exported {} relationships to {}",
                export_data.relationships.len(),
                path
            ));
        }
        None => {
            println!("{}", content);
        }
    }

    Ok(())
}

/// Import relationships from a file.
pub async fn import(ctx: &Context, file: &str, yes: bool, dry_run: bool, mode: &str) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();
    let rels = vault.relationships();

    // Read and parse the file
    let path = Path::new(file);
    if !path.exists() {
        ctx.output.error(&format!("File not found: {}", file));
        return Ok(());
    }

    let content = std::fs::read_to_string(path)?;

    // Detect format from extension or content
    let relationships = if file.ends_with(".csv") {
        parse_csv(&content)?
    } else if file.ends_with(".yaml") || file.ends_with(".yml") {
        let data: ExportData = serde_yaml::from_str(&content)?;
        data.relationships
    } else {
        // Try JSON first, then YAML
        if let Ok(data) = serde_json::from_str::<ExportData>(&content) {
            data.relationships
        } else {
            let data: ExportData = serde_yaml::from_str(&content)?;
            data.relationships
        }
    };

    if relationships.is_empty() {
        ctx.output.info("No relationships to import.");
        return Ok(());
    }

    ctx.output.info(&format!(
        "Found {} relationships to import.",
        relationships.len()
    ));
    ctx.output.info(&format!("Mode: {}", mode));

    if dry_run {
        ctx.output.warn("Dry run mode - no changes will be made.");

        // Validate relationships
        let mut valid = 0;
        let mut invalid = 0;

        for rel in &relationships {
            if rel.resource.contains(':') && rel.subject.contains(':') && !rel.relation.is_empty() {
                valid += 1;
            } else {
                invalid += 1;
                ctx.output.warn(&format!(
                    "Invalid relationship: {} {} {}",
                    rel.resource, rel.relation, rel.subject
                ));
            }
        }

        ctx.output
            .info(&format!("Valid: {}, Invalid: {}", valid, invalid));
        return Ok(());
    }

    // Confirm import
    if !yes {
        let confirmed = ctx.confirm(&format!(
            "Import {} relationships in {} mode?",
            relationships.len(),
            mode
        ))?;
        if !confirmed {
            ctx.output.info("Import cancelled.");
            return Ok(());
        }
    }

    // Perform import based on mode
    match mode {
        "merge" | "upsert" => {
            // Write relationships (upsert is the default behavior)
            let mut success = 0;
            let mut failed = 0;

            for rel in &relationships {
                let relationship =
                    inferadb::Relationship::new(&rel.resource, &rel.relation, &rel.subject);

                match rels.write(relationship).await {
                    Ok(_) => success += 1,
                    Err(e) => {
                        failed += 1;
                        if ctx.debug {
                            ctx.output.warn(&format!(
                                "Failed to write {} {} {}: {}",
                                rel.resource, rel.relation, rel.subject, e
                            ));
                        }
                    }
                }
            }

            ctx.output.success(&format!(
                "Imported {} relationships ({} failed).",
                success, failed
            ));
        }
        "replace" => {
            ctx.output
                .warn("Replace mode will delete all existing relationships first.");

            if !yes {
                let confirmed =
                    ctx.confirm("Are you sure you want to delete all existing relationships?")?;
                if !confirmed {
                    ctx.output.info("Import cancelled.");
                    return Ok(());
                }
            }

            // Delete all existing relationships using delete_where
            ctx.output.info("Deleting existing relationships...");

            // Use the delete_where builder to delete all relationships
            // For now, we'll list and delete individually since delete_where might not delete everything
            let mut deleted = 0;
            loop {
                let page = rels.list().limit(100).await?;
                if page.relationships.is_empty() {
                    break;
                }

                for rel in page.iter() {
                    let relationship =
                        inferadb::Relationship::new(rel.resource(), rel.relation(), rel.subject());
                    if rels.delete(relationship).await.is_ok() {
                        deleted += 1;
                    }
                }
            }

            ctx.output
                .info(&format!("Deleted {} relationships.", deleted));

            // Now write the new relationships
            let mut success = 0;
            for rel in &relationships {
                let relationship =
                    inferadb::Relationship::new(&rel.resource, &rel.relation, &rel.subject);
                if rels.write(relationship).await.is_ok() {
                    success += 1;
                }
            }

            ctx.output
                .success(&format!("Imported {} relationships.", success));
        }
        _ => {
            ctx.output.error(&format!(
                "Unknown mode: {}. Use merge, upsert, or replace.",
                mode
            ));
        }
    }

    Ok(())
}

fn parse_csv(content: &str) -> Result<Vec<ExportedRelationship>> {
    let mut relationships = Vec::new();
    let mut lines = content.lines();

    // Skip header
    if let Some(header) = lines.next() {
        // Verify it's a valid header
        let parts: Vec<&str> = header.split(',').collect();
        if parts.len() < 3 {
            return Err(crate::error::Error::config(
                "Invalid CSV format: expected resource,relation,subject",
            ));
        }
    }

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            continue;
        }

        relationships.push(ExportedRelationship {
            resource: parts[0].trim().to_string(),
            relation: parts[1].trim().to_string(),
            subject: parts[2].trim().to_string(),
        });
    }

    Ok(relationships)
}
