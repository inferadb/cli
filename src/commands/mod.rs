//! Command implementations for the InferaDB CLI.
//!
//! Each submodule implements a group of related commands.

mod account;
mod auth;
mod bulk;
mod check;
mod dev;
mod identity;
mod jwks;
mod orgs;
mod profiles;
mod relationships;
mod schemas;
mod shell;
mod stream;
mod tokens;

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
        Commands::Profiles(sub) => profiles_dispatch(ctx, sub).await,

        // Config commands
        Commands::Config(sub) => config_dispatch(ctx, sub).await,

        // Account commands
        Commands::Account(sub) => account_dispatch(ctx, sub).await,

        // Relationship commands
        Commands::Relationships(sub) => relationships_dispatch(ctx, sub).await,

        // Schema commands
        Commands::Schemas(sub) => schemas_dispatch(ctx, sub).await,

        // Org commands
        Commands::Orgs(sub) => orgs_dispatch(ctx, sub).await,

        // JWKS commands
        Commands::Jwks(sub) => jwks_dispatch(ctx, sub).await,

        // Token commands
        Commands::Tokens(sub) => tokens_dispatch(ctx, sub).await,

        // Bulk operations
        Commands::Export {
            output,
            resource_type,
            format,
        } => bulk::export(ctx, output.as_deref(), resource_type.as_deref(), format).await,

        Commands::Import {
            file,
            yes,
            dry_run,
            mode,
        } => bulk::import(ctx, file, *yes, *dry_run, mode).await,

        // Stream
        Commands::Stream {
            resource_type,
            relation,
        } => stream::stream(ctx, resource_type.as_deref(), relation.as_deref()).await,

        // Stats
        Commands::Stats { trends, compact } => identity::stats(ctx, *trends, *compact).await,

        // What Changed
        Commands::WhatChanged {
            since,
            until,
            focus,
            actor,
            resource,
            compact,
        } => {
            identity::what_changed(
                ctx,
                since.as_deref(),
                until.as_deref(),
                focus.as_deref(),
                actor.as_deref(),
                resource.as_deref(),
                *compact,
            )
            .await
        }

        // Interactive
        Commands::Shell => shell::shell(ctx).await,

        // Utilities
        Commands::Cheatsheet { role } => cheatsheet(ctx, role.as_deref()).await,
        Commands::Templates {
            name,
            subject,
            format,
        } => identity::templates(ctx, name.as_deref(), subject.as_deref(), format).await,
        Commands::Guide { name } => identity::guide(ctx, name.as_deref()).await,
        Commands::Dev(sub) => dev_dispatch(ctx, sub).await,
        Commands::Completion { shell } => completion(ctx, shell).await,
    }
}

// ============================================================================
// Dispatch functions for subcommands
// ============================================================================

async fn profiles_dispatch(ctx: &Context, sub: &crate::cli::ProfilesCommands) -> Result<()> {
    use crate::cli::ProfilesCommands;
    match sub {
        ProfilesCommands::List => profiles_list(ctx).await,
        ProfilesCommands::Show { name } => profiles_show(ctx, name.as_deref()).await,
        ProfilesCommands::Create {
            name,
            url,
            org,
            vault,
        } => profiles_create(ctx, name, url.as_deref(), org.as_deref(), vault.as_deref()).await,
        ProfilesCommands::Update {
            name,
            url,
            org,
            vault,
        } => profiles_update(ctx, name, url.as_deref(), org.as_deref(), vault.as_deref()).await,
        ProfilesCommands::Rename { old_name, new_name } => {
            profiles_rename(ctx, old_name, new_name).await
        }
        ProfilesCommands::Delete { name } => profiles_delete(ctx, name).await,
        ProfilesCommands::Default { name } => profiles_default(ctx, name.as_deref()).await,
    }
}

async fn config_dispatch(ctx: &Context, sub: &crate::cli::ConfigCommands) -> Result<()> {
    use crate::cli::ConfigCommands;
    match sub {
        ConfigCommands::Show { key } => identity::config_show(ctx, key.as_deref()).await,
        ConfigCommands::Edit { editor } => identity::config_edit(ctx, editor.as_deref()).await,
        ConfigCommands::Path { dir } => identity::config_path(ctx, *dir).await,
        ConfigCommands::Explain => identity::config_explain(ctx).await,
    }
}

async fn account_dispatch(ctx: &Context, sub: &crate::cli::AccountCommands) -> Result<()> {
    use crate::cli::{AccountCommands, EmailsCommands, PasswordCommands, SessionsCommands};
    match sub {
        AccountCommands::Show => account::show(ctx).await,
        AccountCommands::Update { name } => account::update(ctx, name.as_deref()).await,
        AccountCommands::Delete { yes } => account::delete(ctx, *yes).await,
        AccountCommands::Emails(email_cmd) => match email_cmd {
            EmailsCommands::List => account::emails_list(ctx).await,
            EmailsCommands::Add { email, primary } => {
                account::emails_add(ctx, email, *primary).await
            }
            EmailsCommands::Verify { token } => account::emails_verify(ctx, token).await,
            EmailsCommands::Resend { email } => account::emails_resend(ctx, email).await,
            EmailsCommands::Remove { id } => account::emails_remove(ctx, id).await,
            EmailsCommands::SetPrimary { id } => account::emails_set_primary(ctx, id).await,
        },
        AccountCommands::Sessions(session_cmd) => match session_cmd {
            SessionsCommands::List => account::sessions_list(ctx).await,
            SessionsCommands::Revoke { id } => account::sessions_revoke(ctx, id).await,
            SessionsCommands::RevokeOthers => account::sessions_revoke_others(ctx).await,
        },
        AccountCommands::Password(password_cmd) => match password_cmd {
            PasswordCommands::Reset {
                request,
                confirm,
                email,
                token,
                new_password,
            } => {
                account::password_reset(
                    ctx,
                    *request,
                    *confirm,
                    email.as_deref(),
                    token.as_deref(),
                    new_password.as_deref(),
                )
                .await
            }
        },
    }
}

async fn relationships_dispatch(
    ctx: &Context,
    sub: &crate::cli::RelationshipsCommands,
) -> Result<()> {
    use crate::cli::RelationshipsCommands;
    match sub {
        RelationshipsCommands::List {
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
        RelationshipsCommands::Add {
            subject,
            relation,
            resource,
            if_not_exists,
        } => relationships_add(ctx, subject, relation, resource, *if_not_exists).await,
        RelationshipsCommands::Delete {
            subject,
            relation,
            resource,
            if_exists,
        } => relationships_delete(ctx, subject, relation, resource, *if_exists).await,
        RelationshipsCommands::History { resource, from, to } => {
            relationships::history(ctx, resource.as_deref(), from.as_deref(), to.as_deref()).await
        }
        RelationshipsCommands::Validate { file } => {
            relationships::validate(ctx, file.as_deref()).await
        }
    }
}

async fn schemas_dispatch(ctx: &Context, sub: &crate::cli::SchemasCommands) -> Result<()> {
    use crate::cli::SchemasCommands;
    match sub {
        SchemasCommands::Init { path, template } => schemas::init(ctx, path, template).await,
        SchemasCommands::List { all } => schemas::list(ctx, *all).await,
        SchemasCommands::Get { id } => schemas::get(ctx, id).await,
        SchemasCommands::Preview { file, base, impact } => {
            schemas::preview(ctx, file, base.as_deref(), *impact).await
        }
        SchemasCommands::Push {
            file,
            activate,
            message,
            dry_run,
        } => schemas::push(ctx, file, *activate, message.as_deref(), *dry_run).await,
        SchemasCommands::Activate { id, diff, canary } => {
            schemas::activate_with_options(ctx, id, *diff, *canary).await
        }
        SchemasCommands::Rollback { version } => schemas::rollback(ctx, version.as_deref()).await,
        SchemasCommands::Validate { file, strict: _ } => schemas::validate(ctx, file).await,
        SchemasCommands::Format { file, write } => schemas::format(ctx, file, *write).await,
        SchemasCommands::Diff {
            from,
            to,
            impact: _,
        } => schemas::diff(ctx, from, to).await,
        SchemasCommands::Test {
            tests,
            schema,
            name,
        } => schemas::test(ctx, tests.as_deref(), schema.as_deref(), name.as_deref()).await,
        SchemasCommands::Watch {
            file,
            test,
            auto_push,
        } => schemas::watch(ctx, file, *test, *auto_push).await,
        SchemasCommands::Canary(canary_cmd) => schemas::canary_dispatch(ctx, canary_cmd).await,
        SchemasCommands::Analyze {
            file,
            checks,
            compare,
        } => schemas::analyze(ctx, file, checks.as_deref(), compare.as_deref()).await,
        SchemasCommands::Visualize {
            file,
            format,
            entity,
            show_permissions,
        } => schemas::visualize(ctx, file, format, entity.as_deref(), *show_permissions).await,
        SchemasCommands::Copy {
            version,
            from_vault,
            to_vault,
            from_org,
            to_org,
            activate,
            dry_run,
        } => {
            schemas::copy(
                ctx,
                version.as_deref(),
                from_vault.as_deref(),
                to_vault,
                from_org.as_deref(),
                to_org.as_deref(),
                *activate,
                *dry_run,
            )
            .await
        }
        SchemasCommands::Migrate { from, to, format } => {
            schemas::migrate(ctx, from.as_deref(), to, format).await
        }
    }
}

async fn orgs_dispatch(ctx: &Context, sub: &crate::cli::OrgsCommands) -> Result<()> {
    use crate::cli::{
        CertificatesCommands, ClientsCommands, InvitationsCommands, MembersCommands,
        OrgRolesCommands, OrgsCommands, TeamGrantsCommands, TeamMembersCommands,
        TeamPermissionsCommands, TeamsCommands, VaultRolesCommands, VaultTeamRolesCommands,
        VaultsCommands,
    };
    match sub {
        OrgsCommands::List => orgs::list(ctx).await,
        OrgsCommands::Create { name, tier } => orgs::create(ctx, name, tier.as_deref()).await,
        OrgsCommands::Get { id } => orgs::get(ctx, id.as_deref()).await,
        OrgsCommands::Update { id, name } => {
            orgs::update(ctx, id.as_deref(), name.as_deref()).await
        }
        OrgsCommands::Delete { id } => orgs::delete(ctx, id).await,
        OrgsCommands::Suspend { id } => orgs::suspend(ctx, id).await,
        OrgsCommands::Resume { id } => orgs::resume(ctx, id).await,
        OrgsCommands::Leave { id, yes } => orgs::leave(ctx, id, *yes).await,

        // Members
        OrgsCommands::Members(mem_cmd) => match mem_cmd {
            MembersCommands::List => orgs::members_list(ctx).await,
            MembersCommands::UpdateRole { member_id, role } => {
                orgs::members_update_role(ctx, member_id, role).await
            }
            MembersCommands::Remove { member_id } => orgs::members_remove(ctx, member_id).await,
        },

        // Invitations
        OrgsCommands::Invitations(inv_cmd) => match inv_cmd {
            InvitationsCommands::List => orgs::invitations_list(ctx).await,
            InvitationsCommands::Create { email, role } => {
                orgs::invitations_create(ctx, email, role).await
            }
            InvitationsCommands::Delete { id } => orgs::invitations_delete(ctx, id).await,
            InvitationsCommands::Resend { id } => orgs::invitations_resend(ctx, id).await,
            InvitationsCommands::Accept { token } => orgs::invitations_accept(ctx, token).await,
        },

        // Org Roles
        OrgsCommands::Roles(role_cmd) => match role_cmd {
            OrgRolesCommands::List => orgs::roles_list(ctx).await,
            OrgRolesCommands::Grant { user_id, role } => {
                orgs::roles_grant(ctx, user_id, role).await
            }
            OrgRolesCommands::Update { user_id, role } => {
                orgs::roles_update(ctx, user_id, role).await
            }
            OrgRolesCommands::Revoke { user_id } => orgs::roles_revoke(ctx, user_id).await,
        },

        // Vaults
        OrgsCommands::Vaults(vault_cmd) => match vault_cmd {
            VaultsCommands::List => orgs::vaults_list(ctx).await,
            VaultsCommands::Create { name, description } => {
                orgs::vaults_create(ctx, name, description.as_deref()).await
            }
            VaultsCommands::Get { id } => orgs::vaults_get(ctx, id.as_deref()).await,
            VaultsCommands::Update {
                id,
                name,
                description,
            } => orgs::vaults_update(ctx, id, name.as_deref(), description.as_deref()).await,
            VaultsCommands::Delete { id } => orgs::vaults_delete(ctx, id).await,
            VaultsCommands::Roles(role_cmd) => match role_cmd {
                VaultRolesCommands::List { vault } => {
                    orgs::vault_roles_list(ctx, vault.as_deref()).await
                }
                VaultRolesCommands::Grant {
                    user_id,
                    role,
                    vault,
                } => orgs::vault_roles_grant(ctx, user_id, role, vault.as_deref()).await,
                VaultRolesCommands::Update { id, role } => {
                    orgs::vault_roles_update(ctx, id, role).await
                }
                VaultRolesCommands::Revoke { id } => orgs::vault_roles_revoke(ctx, id).await,
            },
            VaultsCommands::TeamRoles(role_cmd) => match role_cmd {
                VaultTeamRolesCommands::List { vault } => {
                    orgs::vault_team_roles_list(ctx, vault.as_deref()).await
                }
                VaultTeamRolesCommands::Grant {
                    team_id,
                    role,
                    vault,
                } => orgs::vault_team_roles_grant(ctx, team_id, role, vault.as_deref()).await,
                VaultTeamRolesCommands::Update { id, role } => {
                    orgs::vault_team_roles_update(ctx, id, role).await
                }
                VaultTeamRolesCommands::Revoke { id } => {
                    orgs::vault_team_roles_revoke(ctx, id).await
                }
            },
        },

        // Teams
        OrgsCommands::Teams(team_cmd) => match team_cmd {
            TeamsCommands::List => orgs::teams_list(ctx).await,
            TeamsCommands::Create { name, description } => {
                orgs::teams_create(ctx, name, description.as_deref()).await
            }
            TeamsCommands::Get { id } => orgs::teams_get(ctx, id).await,
            TeamsCommands::Update { id, name } => {
                orgs::teams_update(ctx, id, name.as_deref()).await
            }
            TeamsCommands::Delete { id } => orgs::teams_delete(ctx, id).await,
            TeamsCommands::Members(mem_cmd) => match mem_cmd {
                TeamMembersCommands::List { team_id } => {
                    orgs::team_members_list(ctx, team_id).await
                }
                TeamMembersCommands::Add {
                    team_id,
                    user_id,
                    role,
                } => orgs::team_members_add(ctx, team_id, user_id, role).await,
                TeamMembersCommands::UpdateRole {
                    team_id,
                    user_id,
                    role,
                } => orgs::team_members_update_role(ctx, team_id, user_id, role).await,
                TeamMembersCommands::Remove { team_id, user_id } => {
                    orgs::team_members_remove(ctx, team_id, user_id).await
                }
            },
            TeamsCommands::Permissions(perm_cmd) => match perm_cmd {
                TeamPermissionsCommands::List { team_id } => {
                    orgs::team_permissions_list(ctx, team_id).await
                }
                TeamPermissionsCommands::Grant {
                    team_id,
                    permission,
                } => orgs::team_permissions_grant(ctx, team_id, permission).await,
                TeamPermissionsCommands::Revoke {
                    team_id,
                    permission,
                } => orgs::team_permissions_revoke(ctx, team_id, permission).await,
            },
            TeamsCommands::Grants(grant_cmd) => match grant_cmd {
                TeamGrantsCommands::List { team_id } => orgs::team_grants_list(ctx, team_id).await,
                TeamGrantsCommands::Create {
                    team_id,
                    vault,
                    role,
                } => orgs::team_grants_create(ctx, team_id, vault, role).await,
                TeamGrantsCommands::Update { id, role } => {
                    orgs::team_grants_update(ctx, id, role).await
                }
                TeamGrantsCommands::Delete { id } => orgs::team_grants_delete(ctx, id).await,
            },
        },

        // Clients
        OrgsCommands::Clients(client_cmd) => match client_cmd {
            ClientsCommands::List => orgs::clients_list(ctx).await,
            ClientsCommands::Create { name, vault } => {
                orgs::clients_create(ctx, name, vault.as_deref()).await
            }
            ClientsCommands::Get { id } => orgs::clients_get(ctx, id).await,
            ClientsCommands::Update { id, name } => {
                orgs::clients_update(ctx, id, name.as_deref()).await
            }
            ClientsCommands::Delete { id } => orgs::clients_delete(ctx, id).await,
            ClientsCommands::Deactivate { id } => orgs::clients_deactivate(ctx, id).await,
            ClientsCommands::Reactivate { id } => orgs::clients_reactivate(ctx, id).await,
            ClientsCommands::Certificates(cert_cmd) => match cert_cmd {
                CertificatesCommands::List { client_id } => {
                    orgs::certificates_list(ctx, client_id).await
                }
                CertificatesCommands::Add { client_id } => {
                    orgs::certificates_add(ctx, client_id).await
                }
                CertificatesCommands::Get { id } => orgs::certificates_get(ctx, id).await,
                CertificatesCommands::Rotate { id, grace_period } => {
                    orgs::certificates_rotate(ctx, id, *grace_period).await
                }
                CertificatesCommands::Revoke { id } => orgs::certificates_revoke(ctx, id).await,
            },
        },

        // Audit logs
        OrgsCommands::AuditLogs {
            actor,
            action,
            from,
            to,
        } => {
            orgs::audit_logs(
                ctx,
                actor.as_deref(),
                action.as_deref(),
                from.as_deref(),
                to.as_deref(),
            )
            .await
        }
    }
}

async fn jwks_dispatch(ctx: &Context, sub: &crate::cli::JwksCommands) -> Result<()> {
    use crate::cli::JwksCommands;
    match sub {
        JwksCommands::Get => jwks::get(ctx).await,
        JwksCommands::GetKey { kid } => jwks::get_key(ctx, kid).await,
        JwksCommands::WellKnown => jwks::well_known(ctx).await,
    }
}

async fn tokens_dispatch(ctx: &Context, sub: &crate::cli::TokensCommands) -> Result<()> {
    use crate::cli::TokensCommands;
    match sub {
        TokensCommands::Generate { ttl, role } => {
            tokens::generate(ctx, ttl.as_deref(), role.as_deref()).await
        }
        TokensCommands::List => tokens::list(ctx).await,
        TokensCommands::Revoke { id } => tokens::revoke(ctx, id).await,
        TokensCommands::Refresh => tokens::refresh(ctx).await,
        TokensCommands::Inspect { token, verify } => {
            tokens::inspect(ctx, token.as_deref(), *verify).await
        }
    }
}

async fn dev_dispatch(ctx: &Context, sub: &crate::cli::DevCommands) -> Result<()> {
    use crate::cli::DevCommands;
    match sub {
        DevCommands::Doctor { interactive } => dev::doctor(ctx, *interactive).await,
        DevCommands::Start {
            skip_build,
            interactive,
            tailscale_client,
            tailscale_secret,
            force,
            commit,
        } => {
            dev::start(
                ctx,
                *skip_build,
                *interactive,
                tailscale_client.clone(),
                tailscale_secret.clone(),
                *force,
                commit.as_deref(),
            )
            .await
        }
        DevCommands::Stop {
            destroy,
            yes,
            with_credentials,
            interactive,
        } => dev::stop(ctx, *destroy, *yes, *with_credentials, *interactive).await,
        DevCommands::Up {
            skip_build,
            interactive,
            tailscale_client,
            tailscale_secret,
            force,
            commit,
        } => {
            eprintln!("Hint: 'dev up' is now 'dev start'\n");
            dev::start(
                ctx,
                *skip_build,
                *interactive,
                tailscale_client.clone(),
                tailscale_secret.clone(),
                *force,
                commit.as_deref(),
            )
            .await
        }
        DevCommands::Down {
            destroy,
            yes,
            with_credentials,
            interactive,
        } => {
            eprintln!("Hint: 'dev down' is now 'dev stop'\n");
            dev::stop(ctx, *destroy, *yes, *with_credentials, *interactive).await
        }
        DevCommands::Status { interactive } => dev::dev_status(ctx, *interactive).await,
        DevCommands::Logs {
            follow,
            service,
            tail,
        } => dev::logs(ctx, *follow, service.as_deref(), *tail).await,
        DevCommands::Dashboard => dev::dashboard(ctx).await,
        DevCommands::Reset { yes } => dev::reset(ctx, *yes).await,
    }
}

// ============================================================================
// Utility commands
// ============================================================================

async fn cheatsheet(_ctx: &Context, _role: Option<&str>) -> Result<()> {
    println!("InferaDB CLI Cheatsheet");
    println!("=======================");
    println!();
    println!("Authentication:");
    println!("  inferadb login                    # Log in via browser");
    println!("  inferadb logout                   # Log out");
    println!("  inferadb whoami                   # Show current user");
    println!();
    println!("Account Management:");
    println!("  inferadb account show             # View account details");
    println!("  inferadb account emails list      # List email addresses");
    println!("  inferadb account sessions list    # List active sessions");
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
    println!("Organization Management:");
    println!("  inferadb orgs list                # List organizations");
    println!("  inferadb orgs members list        # List members");
    println!("  inferadb orgs invitations create user@example.com");
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
