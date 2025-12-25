//! Command implementations for the InferaDB CLI.
//!
//! Each submodule implements a group of related commands.

mod auth;
mod check;
mod identity;
mod profiles;
mod relationships;

pub use auth::{login, logout};
pub use check::check;
pub use identity::{doctor, health, ping, status, whoami};
pub use profiles::{
    profiles_create, profiles_default, profiles_delete, profiles_list, profiles_rename,
    profiles_show, profiles_update,
};
pub use relationships::{relationships_add, relationships_delete, relationships_list};

use crate::cli::Commands;
use crate::client::Context;
use crate::error::Result;

/// Execute a CLI command.
pub async fn execute(ctx: &Context, command: &Commands) -> Result<()> {
    match command {
        // Auth commands
        Commands::Init => auth::init(ctx).await,
        Commands::Login => login(ctx).await,
        Commands::Logout => logout(ctx).await,
        Commands::Register { email, name } => {
            auth::register(ctx, email.as_deref(), name.as_deref()).await
        }

        // Identity commands
        Commands::Whoami => whoami(ctx).await,
        Commands::Status => status(ctx).await,
        Commands::Ping {
            count,
            control,
            engine,
        } => ping(ctx, *count, *control, *engine).await,
        Commands::Doctor => doctor(ctx).await,
        Commands::Health { watch, verbose } => health(ctx, *watch, *verbose).await,
        Commands::Version { check } => identity::version(ctx, *check).await,

        // Authorization commands
        Commands::Check {
            subject,
            permission,
            resource,
            trace,
            explain,
            context,
        } => {
            check(
                ctx,
                subject,
                permission,
                resource,
                *trace,
                *explain,
                context.as_deref(),
            )
            .await
        }

        Commands::Simulate {
            subject,
            permission,
            resource,
            add_relationships,
            remove_relationships,
        } => {
            check::simulate(
                ctx,
                subject,
                permission,
                resource,
                add_relationships,
                remove_relationships,
            )
            .await
        }

        Commands::Expand {
            resource,
            relation,
            max_depth,
        } => check::expand(ctx, resource, relation, *max_depth).await,

        Commands::ExplainPermission {
            subject,
            permission,
            resource,
        } => check::explain_permission(ctx, subject, permission, resource).await,

        Commands::ListResources {
            subject,
            permission,
            resource_type,
        } => check::list_resources(ctx, subject, permission, resource_type.as_deref()).await,

        Commands::ListSubjects {
            resource,
            permission,
            subject_type,
        } => check::list_subjects(ctx, resource, permission, subject_type.as_deref()).await,

        // Profile commands
        Commands::Profiles(sub) => match sub {
            crate::cli::ProfilesCommands::List => profiles_list(ctx).await,
            crate::cli::ProfilesCommands::Show { name } => {
                profiles_show(ctx, name.as_deref()).await
            }
            crate::cli::ProfilesCommands::Create {
                name,
                url,
                org,
                vault,
            } => profiles_create(ctx, name, url.as_deref(), org.as_deref(), vault.as_deref()).await,
            crate::cli::ProfilesCommands::Update {
                name,
                url,
                org,
                vault,
            } => profiles_update(ctx, name, url.as_deref(), org.as_deref(), vault.as_deref()).await,
            crate::cli::ProfilesCommands::Rename { old_name, new_name } => {
                profiles_rename(ctx, old_name, new_name).await
            }
            crate::cli::ProfilesCommands::Delete { name } => profiles_delete(ctx, name).await,
            crate::cli::ProfilesCommands::Default { name } => {
                profiles_default(ctx, name.as_deref()).await
            }
        },

        // Config commands
        Commands::Config(sub) => match sub {
            crate::cli::ConfigCommands::Show { key } => {
                identity::config_show(ctx, key.as_deref()).await
            }
            crate::cli::ConfigCommands::Edit { editor } => {
                identity::config_edit(ctx, editor.as_deref()).await
            }
            crate::cli::ConfigCommands::Path { dir } => identity::config_path(ctx, *dir).await,
            crate::cli::ConfigCommands::Explain => identity::config_explain(ctx).await,
        },

        // Relationship commands
        Commands::Relationships(sub) => match sub {
            crate::cli::RelationshipsCommands::List {
                resource,
                subject,
                relation,
                limit,
                cursor,
            } => {
                relationships_list(
                    ctx,
                    resource.as_deref(),
                    subject.as_deref(),
                    relation.as_deref(),
                    *limit,
                    cursor.as_deref(),
                )
                .await
            }
            crate::cli::RelationshipsCommands::Add {
                subject,
                relation,
                resource,
                if_not_exists,
            } => relationships_add(ctx, subject, relation, resource, *if_not_exists).await,
            crate::cli::RelationshipsCommands::Delete {
                subject,
                relation,
                resource,
                if_exists,
            } => relationships_delete(ctx, subject, relation, resource, *if_exists).await,
            crate::cli::RelationshipsCommands::History { resource, from, to } => {
                relationships::history(ctx, resource.as_deref(), from.as_deref(), to.as_deref())
                    .await
            }
            crate::cli::RelationshipsCommands::Validate { file } => {
                relationships::validate(ctx, file.as_deref()).await
            }
        },

        // Schema commands
        Commands::Schemas(sub) => schemas_dispatch(ctx, sub).await,

        // Org commands
        Commands::Orgs(sub) => orgs_dispatch(ctx, sub).await,

        // Token commands
        Commands::Tokens(sub) => tokens_dispatch(ctx, sub).await,

        // Bulk operations
        Commands::Export {
            output,
            resource_type,
            format,
        } => bulk_export(ctx, output.as_deref(), resource_type.as_deref(), format).await,

        Commands::Import {
            file,
            yes,
            dry_run,
            mode,
        } => bulk_import(ctx, file, *yes, *dry_run, mode).await,

        // Stream
        Commands::Stream {
            resource_type,
            relation,
        } => stream(ctx, resource_type.as_deref(), relation.as_deref()).await,

        // Interactive
        Commands::Shell => shell(ctx).await,

        // Utilities
        Commands::Cheatsheet { role } => cheatsheet(ctx, role.as_deref()).await,
        Commands::Completion { shell } => completion(ctx, shell).await,
    }
}

// Placeholder implementations for commands not yet implemented

async fn schemas_dispatch(_ctx: &Context, _sub: &crate::cli::SchemasCommands) -> Result<()> {
    eprintln!("Schema commands not yet implemented");
    Ok(())
}

async fn orgs_dispatch(_ctx: &Context, _sub: &crate::cli::OrgsCommands) -> Result<()> {
    eprintln!("Organization commands not yet implemented");
    Ok(())
}

async fn tokens_dispatch(_ctx: &Context, _sub: &crate::cli::TokensCommands) -> Result<()> {
    eprintln!("Token commands not yet implemented");
    Ok(())
}

async fn bulk_export(
    _ctx: &Context,
    _output: Option<&str>,
    _resource_type: Option<&str>,
    _format: &str,
) -> Result<()> {
    eprintln!("Export not yet implemented");
    Ok(())
}

async fn bulk_import(
    _ctx: &Context,
    _file: &str,
    _yes: bool,
    _dry_run: bool,
    _mode: &str,
) -> Result<()> {
    eprintln!("Import not yet implemented");
    Ok(())
}

async fn stream(
    _ctx: &Context,
    _resource_type: Option<&str>,
    _relation: Option<&str>,
) -> Result<()> {
    eprintln!("Stream not yet implemented");
    Ok(())
}

async fn shell(_ctx: &Context) -> Result<()> {
    eprintln!("Interactive shell not yet implemented");
    Ok(())
}

async fn cheatsheet(_ctx: &Context, _role: Option<&str>) -> Result<()> {
    println!("InferaDB CLI Cheatsheet");
    println!("=======================");
    println!();
    println!("Authentication:");
    println!("  inferadb login                    # Log in via browser");
    println!("  inferadb logout                   # Log out");
    println!("  inferadb whoami                   # Show current user");
    println!();
    println!("Authorization Checks:");
    println!("  inferadb check user:alice view doc:readme");
    println!("  inferadb check user:alice view doc:readme --explain");
    println!();
    println!("Relationships:");
    println!("  inferadb relationships list");
    println!("  inferadb relationships add user:alice viewer doc:readme");
    println!("  inferadb relationships delete user:alice viewer doc:readme");
    println!();
    println!("Profiles:");
    println!("  inferadb profiles list");
    println!("  inferadb @prod check user:alice view doc:readme");
    println!();
    println!("For more: inferadb --help");
    Ok(())
}

async fn completion(_ctx: &Context, shell: &crate::cli::Shell) -> Result<()> {
    use clap::CommandFactory;
    use clap_complete::{generate, Generator};

    let mut cmd = crate::cli::Cli::command();

    fn print_completions<G: Generator>(gen: G, cmd: &mut clap::Command) {
        generate(gen, cmd, cmd.get_name().to_string(), &mut std::io::stdout());
    }

    match shell {
        crate::cli::Shell::Bash => print_completions(clap_complete::shells::Bash, &mut cmd),
        crate::cli::Shell::Zsh => print_completions(clap_complete::shells::Zsh, &mut cmd),
        crate::cli::Shell::Fish => print_completions(clap_complete::shells::Fish, &mut cmd),
        crate::cli::Shell::PowerShell => {
            print_completions(clap_complete::shells::PowerShell, &mut cmd)
        }
    }

    Ok(())
}
