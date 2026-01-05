//! JWKS (JSON Web Key Set) commands for debugging JWT verification.

use crate::{client::Context, error::Result};

/// Get the full JWKS from the control plane.
pub async fn get(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let jwks_client = client.jwks();

    ctx.output.info("Fetching JWKS from control plane...");

    let jwks = jwks_client.get().await?;

    println!("JSON Web Key Set");
    println!("================");
    println!();
    println!("Keys: {}", jwks.keys.len());
    println!();

    for (i, key) in jwks.keys.iter().enumerate() {
        println!("Key {}:", i + 1);
        println!("  kid: {}", key.kid.as_deref().unwrap_or("(not set)"));
        println!("  kty: {}", key.kty);
        println!("  alg: {}", key.alg.as_deref().unwrap_or("(not set)"));
        println!("  use: {}", key.use_.as_deref().unwrap_or("(not set)"));
        println!();
    }

    Ok(())
}

/// Get a specific key by key ID (kid).
pub async fn get_key(ctx: &Context, kid: &str) -> Result<()> {
    let client = ctx.client().await?;
    let jwks_client = client.jwks();

    ctx.output.info(&format!("Looking up key: {kid}"));

    let jwks = jwks_client.get().await?;

    // Find the key with matching kid
    let key = jwks.keys.iter().find(|k| k.kid.as_deref() == Some(kid));

    if let Some(key) = key {
        println!("Key Details");
        println!("===========");
        println!();
        println!("  kid: {}", key.kid.as_deref().unwrap_or("(not set)"));
        println!("  kty: {}", key.kty);
        println!("  alg: {}", key.alg.as_deref().unwrap_or("(not set)"));
        println!("  use: {}", key.use_.as_deref().unwrap_or("(not set)"));

        // Output full key as JSON
        println!();
        println!("Full key:");
        println!("{}", serde_json::to_string_pretty(&key)?);
    } else {
        ctx.output.error(&format!("Key not found: {kid}"));
        ctx.output.info("Available keys:");
        for key in &jwks.keys {
            if let Some(kid) = &key.kid {
                println!("  - {kid}");
            }
        }
    }

    Ok(())
}

/// Get JWKS from the .well-known endpoint.
pub async fn well_known(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let jwks_client = client.jwks();

    ctx.output.info("Fetching JWKS from .well-known endpoint...");

    let jwks = jwks_client.get_well_known().await?;

    println!("JSON Web Key Set (.well-known)");
    println!("==============================");
    println!();
    println!("Keys: {}", jwks.keys.len());
    println!();

    for (i, key) in jwks.keys.iter().enumerate() {
        println!("Key {}:", i + 1);
        println!("  kid: {}", key.kid.as_deref().unwrap_or("(not set)"));
        println!("  kty: {}", key.kty);
        println!("  alg: {}", key.alg.as_deref().unwrap_or("(not set)"));
        println!("  use: {}", key.use_.as_deref().unwrap_or("(not set)"));
        println!();
    }

    Ok(())
}
