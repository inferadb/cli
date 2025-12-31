//! Interactive shell (REPL) for InferaDB.

use std::io::{self, BufRead, Write};

use crate::{client::Context, error::Result};

/// Start an interactive shell.
pub async fn shell(ctx: &Context) -> Result<()> {
    println!("InferaDB Interactive Shell");
    println!("Type 'help' for available commands, 'exit' to quit.");
    println!();

    let stdin = io::stdin();
    let reader = stdin.lock();

    print_prompt();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                ctx.output.error(&format!("Read error: {}", e));
                break;
            },
        };

        let line = line.trim();
        if line.is_empty() {
            print_prompt();
            continue;
        }

        // Parse command
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            print_prompt();
            continue;
        }

        match parts[0] {
            "help" | "?" => {
                print_help();
            },
            "exit" | "quit" | "q" => {
                println!("Goodbye!");
                break;
            },
            "check" => {
                if parts.len() < 4 {
                    println!("Usage: check <subject> <permission> <resource>");
                } else {
                    execute_check(ctx, parts[1], parts[2], parts[3]).await;
                }
            },
            "add" | "write" => {
                if parts.len() < 4 {
                    println!("Usage: add <subject> <relation> <resource>");
                } else {
                    execute_add(ctx, parts[1], parts[2], parts[3]).await;
                }
            },
            "delete" | "rm" => {
                if parts.len() < 4 {
                    println!("Usage: delete <subject> <relation> <resource>");
                } else {
                    execute_delete(ctx, parts[1], parts[2], parts[3]).await;
                }
            },
            "list" | "ls" => {
                execute_list(ctx, parts.get(1).copied()).await;
            },
            "status" => {
                execute_status(ctx).await;
            },
            "clear" => {
                // ANSI escape to clear screen
                print!("\x1B[2J\x1B[1;1H");
                let _ = io::stdout().flush();
            },
            cmd => {
                println!("Unknown command: {}. Type 'help' for available commands.", cmd);
            },
        }

        print_prompt();
    }

    Ok(())
}

fn print_prompt() {
    print!("inferadb> ");
    let _ = io::stdout().flush();
}

fn print_help() {
    println!("Available commands:");
    println!();
    println!("  check <subject> <permission> <resource>");
    println!("      Check if subject has permission on resource");
    println!("      Example: check user:alice view doc:readme");
    println!();
    println!("  add <subject> <relation> <resource>");
    println!("      Add a relationship");
    println!("      Example: add user:alice viewer doc:readme");
    println!();
    println!("  delete <subject> <relation> <resource>");
    println!("      Delete a relationship");
    println!("      Example: delete user:alice viewer doc:readme");
    println!();
    println!("  list [resource_type]");
    println!("      List relationships (optionally filter by resource type)");
    println!();
    println!("  status");
    println!("      Show connection status");
    println!();
    println!("  clear");
    println!("      Clear the screen");
    println!();
    println!("  help | ?");
    println!("      Show this help message");
    println!();
    println!("  exit | quit | q");
    println!("      Exit the shell");
    println!();
}

async fn execute_check(ctx: &Context, subject: &str, permission: &str, resource: &str) {
    match ctx.client().await {
        Ok(client) => {
            let vault = client.vault();
            match vault.check(subject, permission, resource).await {
                Ok(allowed) => {
                    if allowed {
                        println!("ALLOWED: {} can {} {}", subject, permission, resource);
                    } else {
                        println!("DENIED: {} cannot {} {}", subject, permission, resource);
                    }
                },
                Err(e) => {
                    println!("Error: {}", e);
                },
            }
        },
        Err(e) => {
            println!("Connection error: {}", e);
        },
    }
}

async fn execute_add(ctx: &Context, subject: &str, relation: &str, resource: &str) {
    match ctx.client().await {
        Ok(client) => {
            let vault = client.vault();
            let rels = vault.relationships();
            let relationship = inferadb::Relationship::new(resource, relation, subject);
            match rels.write(relationship).await {
                Ok(_) => {
                    println!("Added: {} -[{}]-> {}", subject, relation, resource);
                },
                Err(e) => {
                    println!("Error: {}", e);
                },
            }
        },
        Err(e) => {
            println!("Connection error: {}", e);
        },
    }
}

async fn execute_delete(ctx: &Context, subject: &str, relation: &str, resource: &str) {
    match ctx.client().await {
        Ok(client) => {
            let vault = client.vault();
            let rels = vault.relationships();
            let relationship = inferadb::Relationship::new(resource, relation, subject);
            match rels.delete(relationship).await {
                Ok(_) => {
                    println!("Deleted: {} -[{}]-> {}", subject, relation, resource);
                },
                Err(e) => {
                    println!("Error: {}", e);
                },
            }
        },
        Err(e) => {
            println!("Connection error: {}", e);
        },
    }
}

async fn execute_list(ctx: &Context, resource_type: Option<&str>) {
    match ctx.client().await {
        Ok(client) => {
            let vault = client.vault();
            let rels = vault.relationships();
            let mut req = rels.list().limit(50);

            if let Some(rt) = resource_type {
                req = req.resource(format!("{}:*", rt));
            }

            match req.await {
                Ok(page) => {
                    if page.relationships.is_empty() {
                        println!("No relationships found.");
                    } else {
                        for rel in page.iter() {
                            println!(
                                "{} -[{}]-> {}",
                                rel.subject(),
                                rel.relation(),
                                rel.resource()
                            );
                        }
                        if page.has_more() {
                            println!("... (more results available)");
                        }
                    }
                },
                Err(e) => {
                    println!("Error: {}", e);
                },
            }
        },
        Err(e) => {
            println!("Connection error: {}", e);
        },
    }
}

async fn execute_status(ctx: &Context) {
    println!("Profile: {}", ctx.effective_profile_name());
    if let Some(org) = ctx.profile_org_id() {
        println!("Organization: {}", org);
    }
    if let Some(vault) = ctx.profile_vault_id() {
        println!("Vault: {}", vault);
    }
    println!("Authenticated: {}", ctx.is_authenticated());

    match ctx.client().await {
        Ok(client) => {
            println!("API URL: {}", ctx.profile.url_or_default());
            match client.health().await {
                Ok(health) => {
                    println!("Service Status: {}", health.status);
                },
                Err(e) => {
                    println!("Service Status: Error - {}", e);
                },
            }
        },
        Err(e) => {
            println!("Connection: Error - {}", e);
        },
    }
}
