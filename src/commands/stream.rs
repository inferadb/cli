//! Stream command for real-time relationship changes.

use crate::client::Context;
use crate::error::Result;
use futures::StreamExt;

/// Watch real-time relationship changes.
pub async fn stream(
    ctx: &Context,
    resource_type: Option<&str>,
    relation: Option<&str>,
) -> Result<()> {
    let client = ctx.client().await?;
    let vault = client.vault();

    ctx.output.info("Connecting to watch stream...");

    // Build watch request with optional filters
    let mut watch = vault.watch();

    if let Some(rt) = resource_type {
        use inferadb::vault::watch::WatchFilter;
        watch = watch.filter(WatchFilter::resource_type(rt));
        ctx.output
            .info(&format!("Filtering by resource type: {}", rt));
    }

    if let Some(rel) = relation {
        use inferadb::vault::watch::WatchFilter;
        watch = watch.filter(WatchFilter::relation(rel));
        ctx.output.info(&format!("Filtering by relation: {}", rel));
    }

    ctx.output.info("Watching for changes... (Ctrl+C to stop)");
    println!();

    // Start the watch stream
    let mut stream = watch.run().await?;

    // Process events
    while let Some(event) = stream.next().await {
        match event {
            Ok(event) => {
                let op = if event.operation.is_create() {
                    "+"
                } else {
                    "-"
                };

                println!(
                    "[{}] {} -[{}]-> {}",
                    op,
                    event.relationship.subject(),
                    event.relationship.relation(),
                    event.relationship.resource()
                );
            }
            Err(e) => {
                ctx.output.error(&format!("Stream error: {}", e));
                // Continue watching after transient errors
            }
        }
    }

    ctx.output.info("Stream ended.");

    Ok(())
}
