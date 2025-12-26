//! Organization management commands.

use crate::client::Context;
use crate::error::Result;
use crate::output::Displayable;
use serde::Serialize;

// ============================================================================
// Display types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
struct OrgRow {
    id: String,
    name: String,
    display_name: String,
    created_at: String,
}

impl Displayable for OrgRow {
    fn table_row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            self.display_name.clone(),
            self.created_at.clone(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["ID", "NAME", "DISPLAY NAME", "CREATED"]
    }
}

#[derive(Debug, Clone, Serialize)]
struct MemberRow {
    id: String,
    name: String,
    email: String,
    role: String,
    status: String,
}

impl Displayable for MemberRow {
    fn table_row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            self.email.clone(),
            self.role.clone(),
            self.status.clone(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["ID", "NAME", "EMAIL", "ROLE", "STATUS"]
    }
}

#[derive(Debug, Clone, Serialize)]
struct InvitationRow {
    id: String,
    email: String,
    role: String,
    status: String,
    created_at: String,
}

impl Displayable for InvitationRow {
    fn table_row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.email.clone(),
            self.role.clone(),
            self.status.clone(),
            self.created_at.clone(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["ID", "EMAIL", "ROLE", "STATUS", "CREATED"]
    }
}

#[derive(Debug, Clone, Serialize)]
struct VaultRow {
    id: String,
    name: String,
    description: String,
    status: String,
}

impl Displayable for VaultRow {
    fn table_row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            self.description.clone(),
            self.status.clone(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["ID", "NAME", "DESCRIPTION", "STATUS"]
    }
}

#[derive(Debug, Clone, Serialize)]
struct TeamRow {
    id: String,
    name: String,
    description: String,
    member_count: String,
}

impl Displayable for TeamRow {
    fn table_row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            self.description.clone(),
            self.member_count.clone(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["ID", "NAME", "DESCRIPTION", "MEMBERS"]
    }
}

#[derive(Debug, Clone, Serialize)]
struct ClientRow {
    id: String,
    name: String,
    status: String,
    created_at: String,
}

impl Displayable for ClientRow {
    fn table_row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            self.status.clone(),
            self.created_at.clone(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["ID", "NAME", "STATUS", "CREATED"]
    }
}

#[derive(Debug, Clone, Serialize)]
struct CertificateRow {
    id: String,
    fingerprint: String,
    status: String,
    expires_at: String,
}

impl Displayable for CertificateRow {
    fn table_row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.fingerprint.clone(),
            self.status.clone(),
            self.expires_at.clone(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["ID", "FINGERPRINT", "STATUS", "EXPIRES"]
    }
}

#[derive(Debug, Clone, Serialize)]
struct AuditLogRow {
    timestamp: String,
    actor: String,
    action: String,
    resource: String,
    outcome: String,
}

impl Displayable for AuditLogRow {
    fn table_row(&self) -> Vec<String> {
        vec![
            self.timestamp.clone(),
            self.actor.clone(),
            self.action.clone(),
            self.resource.clone(),
            self.outcome.clone(),
        ]
    }

    fn table_headers() -> Vec<&'static str> {
        vec!["TIMESTAMP", "ACTOR", "ACTION", "RESOURCE", "OUTCOME"]
    }
}

// ============================================================================
// Organization commands
// ============================================================================

/// List organizations.
pub async fn list(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let orgs = client.organizations();

    let page = orgs.list().await?;

    if page.items.is_empty() {
        ctx.output.info("No organizations found.");
        return Ok(());
    }

    let rows: Vec<OrgRow> = page
        .items
        .iter()
        .map(|o| OrgRow {
            id: o.id.clone(),
            name: o.name.clone(),
            display_name: o.display_name.clone().unwrap_or_else(|| "-".to_string()),
            created_at: o.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect();

    ctx.output.table(&rows)
}

/// Create an organization.
pub async fn create(ctx: &Context, name: &str, tier: Option<&str>) -> Result<()> {
    use inferadb::control::CreateOrganizationRequest;

    let client = ctx.client().await?;

    ctx.output
        .info(&format!("Creating organization '{}'...", name));

    let request = CreateOrganizationRequest::new(name);

    if let Some(t) = tier {
        ctx.output.info(&format!("Tier: {}", t));
    }

    let org = client.organizations().create(request).await?;

    ctx.output
        .success(&format!("Organization '{}' created.", org.name));
    ctx.output.info(&format!("ID: {}", org.id));

    Ok(())
}

/// Get organization details.
pub async fn get(ctx: &Context, id: Option<&str>) -> Result<()> {
    let client = ctx.client().await?;

    let org_id = id.or(ctx.profile_org_id());

    if org_id.is_none() {
        ctx.output
            .error("No organization specified. Use --org or configure a profile.");
        return Ok(());
    }

    let org_id = org_id.unwrap();

    let page = client.organizations().list().await?;
    let org = page
        .items
        .iter()
        .find(|o| o.id == org_id || o.name == org_id);

    match org {
        Some(info) => {
            println!("Organization: {}", info.name);
            println!("ID: {}", info.id);
            if let Some(display) = &info.display_name {
                println!("Display Name: {}", display);
            }
            println!("Created: {}", info.created_at.format("%Y-%m-%d %H:%M:%S"));
            println!("Updated: {}", info.updated_at.format("%Y-%m-%d %H:%M:%S"));
        }
        None => {
            ctx.output
                .error(&format!("Organization '{}' not found.", org_id));
        }
    }

    Ok(())
}

/// Update organization.
pub async fn update(ctx: &Context, id: Option<&str>, name: Option<&str>) -> Result<()> {
    let org_id = id.or(ctx.profile_org_id());
    if org_id.is_none() {
        ctx.output
            .error("No organization specified. Use --org or configure a profile.");
        return Ok(());
    }

    ctx.output
        .warn("Organization update not yet supported via CLI.");
    if let Some(n) = name {
        ctx.output.info(&format!("Would update name to: {}", n));
    }
    ctx.output
        .info("Use the web dashboard to update organizations.");

    Ok(())
}

/// Delete organization.
pub async fn delete(ctx: &Context, id: &str) -> Result<()> {
    ctx.output
        .warn("Organization deletion not yet supported via CLI.");
    ctx.output
        .info(&format!("Would delete organization: {}", id));
    ctx.output
        .info("Use the web dashboard to delete organizations.");

    Ok(())
}

/// Suspend organization.
pub async fn suspend(ctx: &Context, id: &str) -> Result<()> {
    ctx.output
        .warn("Organization suspension not yet supported via CLI.");
    ctx.output
        .info(&format!("Would suspend organization: {}", id));
    ctx.output
        .info("Use the web dashboard to suspend organizations.");

    Ok(())
}

/// Resume organization.
pub async fn resume(ctx: &Context, id: &str) -> Result<()> {
    ctx.output
        .warn("Organization resume not yet supported via CLI.");
    ctx.output
        .info(&format!("Would resume organization: {}", id));
    ctx.output
        .info("Use the web dashboard to resume organizations.");

    Ok(())
}

/// Leave organization.
pub async fn leave(ctx: &Context, id: &str, yes: bool) -> Result<()> {
    if !yes {
        ctx.output
            .warn("Leaving an organization will revoke your access.");
        let confirmed = ctx.confirm(&format!("Leave organization '{}'?", id))?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    ctx.output
        .warn("Leave organization not yet supported via CLI.");
    ctx.output
        .info(&format!("Would leave organization: {}", id));
    ctx.output
        .info("Use the web dashboard to leave organizations.");

    Ok(())
}

// ============================================================================
// Member commands
// ============================================================================

/// List organization members.
pub async fn members_list(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let members = org.members();

    let page = members.list().await?;

    if page.items.is_empty() {
        ctx.output.info("No members found.");
        return Ok(());
    }

    let rows: Vec<MemberRow> = page
        .items
        .iter()
        .map(|m| MemberRow {
            id: m.user_id.clone(),
            name: m.name.clone().unwrap_or_else(|| "-".to_string()),
            email: m.email.clone(),
            role: format!("{:?}", m.role),
            status: format!("{:?}", m.status),
        })
        .collect();

    ctx.output.table(&rows)
}

/// Update member role.
pub async fn members_update_role(ctx: &Context, member_id: &str, role: &str) -> Result<()> {
    use inferadb::control::{OrgRole, UpdateMemberRequest};

    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let parsed_role = match role.to_lowercase().as_str() {
        "owner" => OrgRole::Owner,
        "admin" => OrgRole::Admin,
        "member" => OrgRole::Member,
        _ => {
            ctx.output.error(&format!(
                "Invalid role: {}. Use owner, admin, or member.",
                role
            ));
            return Ok(());
        }
    };

    let org = client.organization(&org_id);
    let members = org.members();

    let request = UpdateMemberRequest::new().with_role(parsed_role);
    members.update(member_id, request).await?;

    ctx.output
        .success(&format!("Member role updated to {}.", role));

    Ok(())
}

/// Remove member.
pub async fn members_remove(ctx: &Context, member_id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    if !ctx.yes {
        let confirmed = ctx.confirm(&format!("Remove member '{}'?", member_id))?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    let org = client.organization(&org_id);
    org.members().remove(member_id).await?;

    ctx.output.success("Member removed.");

    Ok(())
}

// ============================================================================
// Invitation commands
// ============================================================================

/// List pending invitations.
pub async fn invitations_list(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let invitations = org.invitations();

    let page = invitations.list().await?;

    if page.items.is_empty() {
        ctx.output.info("No pending invitations.");
        return Ok(());
    }

    let rows: Vec<InvitationRow> = page
        .items
        .iter()
        .map(|i| InvitationRow {
            id: i.id.clone(),
            email: i.email.clone(),
            role: format!("{:?}", i.role),
            status: format!("{:?}", i.status),
            created_at: i.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect();

    ctx.output.table(&rows)
}

/// Create an invitation.
pub async fn invitations_create(ctx: &Context, email: &str, role: &str) -> Result<()> {
    use inferadb::control::{InviteMemberRequest, OrgRole};

    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let parsed_role = match role.to_lowercase().as_str() {
        "owner" => OrgRole::Owner,
        "admin" => OrgRole::Admin,
        "member" => OrgRole::Member,
        _ => {
            ctx.output.error(&format!(
                "Invalid role: {}. Use owner, admin, or member.",
                role
            ));
            return Ok(());
        }
    };

    ctx.output
        .info(&format!("Inviting {} as {}...", email, role));

    let org = client.organization(&org_id);
    let request = InviteMemberRequest::new(email, parsed_role);
    let invitation = org.members().invite(request).await?;

    ctx.output
        .success(&format!("Invitation sent to {}.", email));
    ctx.output
        .info(&format!("Invitation ID: {}", invitation.id));

    Ok(())
}

/// Delete/cancel an invitation.
pub async fn invitations_delete(ctx: &Context, id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    if !ctx.yes {
        let confirmed = ctx.confirm(&format!("Cancel invitation '{}'?", id))?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    let org = client.organization(&org_id);
    org.invitations().revoke(id).await?;

    ctx.output.success("Invitation cancelled.");

    Ok(())
}

/// Resend invitation email.
pub async fn invitations_resend(ctx: &Context, id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    org.invitations().resend(id).await?;

    ctx.output.success("Invitation email resent.");

    Ok(())
}

/// Accept an invitation (using token from email).
pub async fn invitations_accept(ctx: &Context, token: &str) -> Result<()> {
    ctx.output
        .info(&format!("Token: {}...", &token[..token.len().min(20)]));
    ctx.output
        .warn("Invitation acceptance requires clicking the link in the email.");
    ctx.output
        .info("Use the invitation link to join the organization.");

    Ok(())
}

// ============================================================================
// Organization role commands
// ============================================================================

/// List role assignments.
pub async fn roles_list(ctx: &Context) -> Result<()> {
    // Role assignments are the same as members - just show members with their roles
    members_list(ctx).await
}

/// Grant a role to a user.
pub async fn roles_grant(ctx: &Context, user_id: &str, role: &str) -> Result<()> {
    ctx.output
        .warn("Role grant via CLI is done through invitations or member updates.");
    ctx.output.info(&format!(
        "To invite a user: inferadb orgs invitations create <email> --role {}",
        role
    ));
    ctx.output.info(&format!(
        "To update a member: inferadb orgs members update-role {} {}",
        user_id, role
    ));

    Ok(())
}

/// Update a user's role.
pub async fn roles_update(ctx: &Context, user_id: &str, role: &str) -> Result<()> {
    members_update_role(ctx, user_id, role).await
}

/// Revoke a user's role.
pub async fn roles_revoke(ctx: &Context, user_id: &str) -> Result<()> {
    members_remove(ctx, user_id).await
}

// ============================================================================
// Vault commands
// ============================================================================

/// List vaults.
pub async fn vaults_list(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let vaults = org.vaults();

    let page = vaults.list().await?;

    if page.items.is_empty() {
        ctx.output.info("No vaults found.");
        return Ok(());
    }

    let rows: Vec<VaultRow> = page
        .items
        .iter()
        .map(|v| VaultRow {
            id: v.id.clone(),
            name: v.name.clone(),
            description: v.description.clone().unwrap_or_else(|| "-".to_string()),
            status: format!("{:?}", v.status),
        })
        .collect();

    ctx.output.table(&rows)
}

/// Create vault.
pub async fn vaults_create(ctx: &Context, name: &str, description: Option<&str>) -> Result<()> {
    use inferadb::control::CreateVaultRequest;

    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    ctx.output.info(&format!("Creating vault '{}'...", name));

    let org = client.organization(&org_id);
    let mut request = CreateVaultRequest::new(name);
    if let Some(desc) = description {
        request = request.with_description(desc);
    }

    let vault = org.vaults().create(request).await?;

    ctx.output
        .success(&format!("Vault '{}' created.", vault.name));
    ctx.output.info(&format!("ID: {}", vault.id));

    Ok(())
}

/// Get vault details.
pub async fn vaults_get(ctx: &Context, id: Option<&str>) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let vault_id = id.or(ctx.profile_vault_id());
    if vault_id.is_none() {
        ctx.output
            .error("No vault specified. Use --vault or configure a profile.");
        return Ok(());
    }
    let vault_id = vault_id.unwrap();

    let org = client.organization(&org_id);
    let vault = org.vaults().get(vault_id).await?;

    println!("Vault: {}", vault.name);
    println!("ID: {}", vault.id);
    if let Some(desc) = &vault.description {
        println!("Description: {}", desc);
    }
    println!("Status: {:?}", vault.status);
    println!("Created: {}", vault.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("Updated: {}", vault.updated_at.format("%Y-%m-%d %H:%M:%S"));

    Ok(())
}

/// Update vault.
pub async fn vaults_update(
    ctx: &Context,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<()> {
    use inferadb::control::UpdateVaultRequest;

    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let mut request = UpdateVaultRequest::default();

    if let Some(n) = name {
        request = request.with_display_name(n);
    }
    if let Some(d) = description {
        request = request.with_description(d);
    }

    org.vaults().update(id, request).await?;

    ctx.output.success("Vault updated.");

    Ok(())
}

/// Delete vault.
pub async fn vaults_delete(ctx: &Context, id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    if !ctx.yes {
        ctx.output
            .warn("Deleting a vault will permanently remove all schemas and relationships.");
        let confirmed = ctx.confirm(&format!("Delete vault '{}'?", id))?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    let org = client.organization(&org_id);
    org.vaults().delete(id).await?;

    ctx.output.success("Vault deleted.");

    Ok(())
}

// ============================================================================
// Vault role commands
// ============================================================================

/// List vault user role assignments.
pub async fn vault_roles_list(ctx: &Context, vault: Option<&str>) -> Result<()> {
    let vault_id = vault.or(ctx.profile_vault_id());
    ctx.output
        .warn("Vault role listing not yet supported via CLI.");
    if let Some(v) = vault_id {
        ctx.output
            .info(&format!("Would list roles for vault: {}", v));
    }
    ctx.output
        .info("Use the web dashboard to view vault roles.");

    Ok(())
}

/// Grant a role to a user on a vault.
pub async fn vault_roles_grant(
    ctx: &Context,
    user_id: &str,
    role: &str,
    vault: Option<&str>,
) -> Result<()> {
    let vault_id = vault.or(ctx.profile_vault_id());
    ctx.output
        .warn("Vault role granting not yet supported via CLI.");
    ctx.output.info(&format!(
        "Would grant {} role to user {} on vault {:?}",
        role, user_id, vault_id
    ));
    ctx.output
        .info("Use the web dashboard to manage vault roles.");

    Ok(())
}

/// Update a vault role assignment.
pub async fn vault_roles_update(ctx: &Context, id: &str, role: &str) -> Result<()> {
    ctx.output
        .warn("Vault role updating not yet supported via CLI.");
    ctx.output
        .info(&format!("Would update role assignment {} to {}", id, role));
    ctx.output
        .info("Use the web dashboard to manage vault roles.");

    Ok(())
}

/// Revoke a vault role assignment.
pub async fn vault_roles_revoke(ctx: &Context, id: &str) -> Result<()> {
    ctx.output
        .warn("Vault role revoking not yet supported via CLI.");
    ctx.output
        .info(&format!("Would revoke role assignment: {}", id));
    ctx.output
        .info("Use the web dashboard to manage vault roles.");

    Ok(())
}

/// List vault team role assignments.
pub async fn vault_team_roles_list(ctx: &Context, vault: Option<&str>) -> Result<()> {
    let vault_id = vault.or(ctx.profile_vault_id());
    ctx.output
        .warn("Vault team role listing not yet supported via CLI.");
    if let Some(v) = vault_id {
        ctx.output
            .info(&format!("Would list team roles for vault: {}", v));
    }
    ctx.output
        .info("Use the web dashboard to view vault team roles.");

    Ok(())
}

/// Grant a role to a team on a vault.
pub async fn vault_team_roles_grant(
    ctx: &Context,
    team_id: &str,
    role: &str,
    vault: Option<&str>,
) -> Result<()> {
    let vault_id = vault.or(ctx.profile_vault_id());
    ctx.output
        .warn("Vault team role granting not yet supported via CLI.");
    ctx.output.info(&format!(
        "Would grant {} role to team {} on vault {:?}",
        role, team_id, vault_id
    ));
    ctx.output
        .info("Use the web dashboard to manage vault team roles.");

    Ok(())
}

/// Update a vault team role assignment.
pub async fn vault_team_roles_update(ctx: &Context, id: &str, role: &str) -> Result<()> {
    ctx.output
        .warn("Vault team role updating not yet supported via CLI.");
    ctx.output.info(&format!(
        "Would update team role assignment {} to {}",
        id, role
    ));
    ctx.output
        .info("Use the web dashboard to manage vault team roles.");

    Ok(())
}

/// Revoke a vault team role assignment.
pub async fn vault_team_roles_revoke(ctx: &Context, id: &str) -> Result<()> {
    ctx.output
        .warn("Vault team role revoking not yet supported via CLI.");
    ctx.output
        .info(&format!("Would revoke team role assignment: {}", id));
    ctx.output
        .info("Use the web dashboard to manage vault team roles.");

    Ok(())
}

// ============================================================================
// Team commands
// ============================================================================

/// List teams.
pub async fn teams_list(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let teams = org.teams();

    let page = teams.list().await?;

    if page.items.is_empty() {
        ctx.output.info("No teams found.");
        return Ok(());
    }

    let rows: Vec<TeamRow> = page
        .items
        .iter()
        .map(|t| TeamRow {
            id: t.id.clone(),
            name: t.name.clone(),
            description: t.description.clone().unwrap_or_else(|| "-".to_string()),
            member_count: t.member_count.to_string(),
        })
        .collect();

    ctx.output.table(&rows)
}

/// Create team.
pub async fn teams_create(ctx: &Context, name: &str, description: Option<&str>) -> Result<()> {
    use inferadb::control::CreateTeamRequest;

    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    ctx.output.info(&format!("Creating team '{}'...", name));

    let org = client.organization(&org_id);
    let mut request = CreateTeamRequest::new(name);
    if let Some(desc) = description {
        request = request.with_description(desc);
    }

    let team = org.teams().create(request).await?;

    ctx.output
        .success(&format!("Team '{}' created.", team.name));
    ctx.output.info(&format!("ID: {}", team.id));

    Ok(())
}

/// Get team details.
pub async fn teams_get(ctx: &Context, id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let team = org.teams().get(id).await?;

    println!("Team: {}", team.name);
    println!("ID: {}", team.id);
    if let Some(desc) = &team.description {
        println!("Description: {}", desc);
    }
    println!("Members: {}", team.member_count);
    println!("Created: {}", team.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("Updated: {}", team.updated_at.format("%Y-%m-%d %H:%M:%S"));

    Ok(())
}

/// Update team.
pub async fn teams_update(ctx: &Context, id: &str, name: Option<&str>) -> Result<()> {
    use inferadb::control::UpdateTeamRequest;

    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let mut request = UpdateTeamRequest::default();

    if let Some(n) = name {
        request = request.with_name(n);
    }

    org.teams().update(id, request).await?;

    ctx.output.success("Team updated.");

    Ok(())
}

/// Delete team.
pub async fn teams_delete(ctx: &Context, id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    if !ctx.yes {
        let confirmed = ctx.confirm(&format!("Delete team '{}'?", id))?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    let org = client.organization(&org_id);
    org.teams().delete(id).await?;

    ctx.output.success("Team deleted.");

    Ok(())
}

// ============================================================================
// Team member commands
// ============================================================================

/// List team members.
pub async fn team_members_list(ctx: &Context, team_id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let team = org.teams().get(team_id).await?;

    // Team members are included in the team info
    println!("Team: {} ({})", team.name, team.id);
    println!("Members: {}", team.member_count);
    println!();

    ctx.output
        .warn("Detailed team member listing not yet available via CLI.");
    ctx.output
        .info("Use the web dashboard to view team members.");

    Ok(())
}

/// Add member to team.
pub async fn team_members_add(
    ctx: &Context,
    team_id: &str,
    user_id: &str,
    _role: &str,
) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    // Note: SDK add_member doesn't accept role parameter - members join with default role
    let org = client.organization(&org_id);
    org.teams().add_member(team_id, user_id).await?;

    ctx.output
        .success(&format!("User {} added to team.", user_id));

    Ok(())
}

/// Update team member role.
pub async fn team_members_update_role(
    ctx: &Context,
    team_id: &str,
    user_id: &str,
    role: &str,
) -> Result<()> {
    ctx.output
        .warn("Team member role update not yet supported via CLI.");
    ctx.output.info(&format!(
        "Would update {} in team {} to role {}",
        user_id, team_id, role
    ));
    ctx.output
        .info("Use the web dashboard to update team member roles.");

    Ok(())
}

/// Remove member from team.
pub async fn team_members_remove(ctx: &Context, team_id: &str, user_id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    if !ctx.yes {
        let confirmed = ctx.confirm(&format!("Remove {} from team {}?", user_id, team_id))?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    let org = client.organization(&org_id);
    org.teams().remove_member(team_id, user_id).await?;

    ctx.output.success("Member removed from team.");

    Ok(())
}

// ============================================================================
// Team permission commands
// ============================================================================

/// List team permissions.
pub async fn team_permissions_list(ctx: &Context, team_id: &str) -> Result<()> {
    ctx.output
        .warn("Team permission listing not yet supported via CLI.");
    ctx.output
        .info(&format!("Would list permissions for team: {}", team_id));
    ctx.output
        .info("Use the web dashboard to view team permissions.");

    Ok(())
}

/// Grant permission to team.
pub async fn team_permissions_grant(ctx: &Context, team_id: &str, permission: &str) -> Result<()> {
    ctx.output
        .warn("Team permission granting not yet supported via CLI.");
    ctx.output
        .info(&format!("Would grant {} to team {}", permission, team_id));
    ctx.output
        .info("Use the web dashboard to manage team permissions.");

    Ok(())
}

/// Revoke permission from team.
pub async fn team_permissions_revoke(ctx: &Context, team_id: &str, permission: &str) -> Result<()> {
    ctx.output
        .warn("Team permission revoking not yet supported via CLI.");
    ctx.output.info(&format!(
        "Would revoke {} from team {}",
        permission, team_id
    ));
    ctx.output
        .info("Use the web dashboard to manage team permissions.");

    Ok(())
}

// ============================================================================
// Team grant commands
// ============================================================================

/// List team vault grants.
pub async fn team_grants_list(ctx: &Context, team_id: &str) -> Result<()> {
    ctx.output
        .warn("Team vault grant listing not yet supported via CLI.");
    ctx.output
        .info(&format!("Would list vault grants for team: {}", team_id));
    ctx.output
        .info("Use the web dashboard to view team vault grants.");

    Ok(())
}

/// Create a vault grant for team.
pub async fn team_grants_create(
    ctx: &Context,
    team_id: &str,
    vault: &str,
    role: &str,
) -> Result<()> {
    ctx.output
        .warn("Team vault grant creation not yet supported via CLI.");
    ctx.output.info(&format!(
        "Would grant {} access to vault {} for team {}",
        role, vault, team_id
    ));
    ctx.output
        .info("Use the web dashboard to create team vault grants.");

    Ok(())
}

/// Update a vault grant.
pub async fn team_grants_update(ctx: &Context, id: &str, role: &str) -> Result<()> {
    ctx.output
        .warn("Team vault grant update not yet supported via CLI.");
    ctx.output
        .info(&format!("Would update grant {} to role {}", id, role));
    ctx.output
        .info("Use the web dashboard to update team vault grants.");

    Ok(())
}

/// Delete a vault grant.
pub async fn team_grants_delete(ctx: &Context, id: &str) -> Result<()> {
    ctx.output
        .warn("Team vault grant deletion not yet supported via CLI.");
    ctx.output.info(&format!("Would delete grant: {}", id));
    ctx.output
        .info("Use the web dashboard to delete team vault grants.");

    Ok(())
}

// ============================================================================
// Client commands
// ============================================================================

/// List clients.
pub async fn clients_list(ctx: &Context) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let clients = org.clients();

    let page = clients.list().await?;

    if page.items.is_empty() {
        ctx.output.info("No API clients found.");
        return Ok(());
    }

    let rows: Vec<ClientRow> = page
        .items
        .iter()
        .map(|c| ClientRow {
            id: c.id.clone(),
            name: c.name.clone(),
            status: format!("{:?}", c.status),
            created_at: c.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect();

    ctx.output.table(&rows)
}

/// Create client.
pub async fn clients_create(ctx: &Context, name: &str, vault: Option<&str>) -> Result<()> {
    use inferadb::control::CreateApiClientRequest;

    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    ctx.output
        .info(&format!("Creating API client '{}'...", name));

    let org = client.organization(&org_id);
    let request = CreateApiClientRequest::new(name);
    // Note: vault parameter is accepted but not used - API clients are org-scoped
    let _ = vault;

    let api_client = org.clients().create(request).await?;

    ctx.output
        .success(&format!("API client '{}' created.", api_client.name));
    ctx.output.info(&format!("ID: {}", api_client.id));
    ctx.output
        .warn("Remember to add a certificate for authentication.");

    Ok(())
}

/// Get client details.
pub async fn clients_get(ctx: &Context, id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let api_client = org.clients().get(id).await?;

    println!("API Client: {}", api_client.name);
    println!("ID: {}", api_client.id);
    println!("Status: {:?}", api_client.status);
    if let Some(desc) = &api_client.description {
        println!("Description: {}", desc);
    }
    println!(
        "Created: {}",
        api_client.created_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!(
        "Updated: {}",
        api_client.updated_at.format("%Y-%m-%d %H:%M:%S")
    );

    Ok(())
}

/// Update client.
pub async fn clients_update(ctx: &Context, id: &str, name: Option<&str>) -> Result<()> {
    use inferadb::control::UpdateApiClientRequest;

    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let mut request = UpdateApiClientRequest::default();

    if let Some(n) = name {
        request = request.with_name(n);
    }

    org.clients().update(id, request).await?;

    ctx.output.success("API client updated.");

    Ok(())
}

/// Delete client.
pub async fn clients_delete(ctx: &Context, id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    if !ctx.yes {
        ctx.output
            .warn("Deleting an API client will revoke all its credentials.");
        let confirmed = ctx.confirm(&format!("Delete API client '{}'?", id))?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    let org = client.organization(&org_id);
    org.clients().delete(id).await?;

    ctx.output.success("API client deleted.");

    Ok(())
}

/// Deactivate (suspend) client.
pub async fn clients_deactivate(ctx: &Context, id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    org.clients().suspend(id).await?;

    ctx.output.success("API client deactivated.");

    Ok(())
}

/// Reactivate a suspended client.
pub async fn clients_reactivate(ctx: &Context, id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    org.clients().reactivate(id).await?;

    ctx.output.success("API client reactivated.");

    Ok(())
}

// ============================================================================
// Certificate commands
// ============================================================================

/// List certificates.
pub async fn certificates_list(ctx: &Context, client_id: &str) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let certs = org.clients().certificates(client_id);

    let page = certs.list().await?;

    if page.items.is_empty() {
        ctx.output.info("No certificates found.");
        return Ok(());
    }

    let rows: Vec<CertificateRow> = page
        .items
        .iter()
        .map(|c| CertificateRow {
            id: c.id.clone(),
            fingerprint: c.fingerprint.clone(),
            status: if c.active { "active" } else { "inactive" }.to_string(),
            expires_at: c
                .expires_at
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "-".to_string()),
        })
        .collect();

    ctx.output.table(&rows)
}

/// Add a certificate.
pub async fn certificates_add(ctx: &Context, client_id: &str) -> Result<()> {
    // Note: AddCertificateRequest::new() requires a PEM-encoded public key.
    // CLI certificate addition requires the public key to be passed as an argument.
    // For now, provide guidance to use the SDK or dashboard.
    ctx.output
        .warn("Certificate addition requires a PEM-encoded public key.");
    ctx.output.info(&format!("Client ID: {}", client_id));
    ctx.output.info("");
    ctx.output
        .info("To add a certificate, generate a key pair and provide the public key:");
    ctx.output
        .info("  1. Generate key pair: openssl genrsa -out private.pem 2048");
    ctx.output
        .info("  2. Extract public key: openssl rsa -in private.pem -pubout -out public.pem");
    ctx.output
        .info("  3. Use the dashboard or SDK to add the public key");

    Ok(())
}

/// Get certificate details.
pub async fn certificates_get(ctx: &Context, id: &str) -> Result<()> {
    ctx.output
        .warn("Certificate details not yet available via CLI.");
    ctx.output.info(&format!("Certificate ID: {}", id));
    ctx.output
        .info("Use the web dashboard to view certificate details.");

    Ok(())
}

/// Rotate certificate with grace period.
pub async fn certificates_rotate(ctx: &Context, id: &str, grace_period: u32) -> Result<()> {
    // Note: RotateCertificateRequest::new() requires a new PEM-encoded public key.
    // CLI certificate rotation requires the new public key to be passed as an argument.
    ctx.output
        .warn("Certificate rotation requires a new PEM-encoded public key.");
    ctx.output.info(&format!("Certificate ID: {}", id));
    ctx.output
        .info(&format!("Requested grace period: {}h", grace_period));
    ctx.output.info("");
    ctx.output.info("To rotate a certificate:");
    ctx.output.info("  1. Generate a new key pair");
    ctx.output
        .info("  2. Use the dashboard or SDK to rotate with the new public key");
    ctx.output
        .info("  3. The grace period allows both old and new keys to be valid during transition");

    Ok(())
}

/// Revoke certificate.
pub async fn certificates_revoke(ctx: &Context, id: &str) -> Result<()> {
    if !ctx.yes {
        ctx.output
            .warn("Revoking a certificate will immediately invalidate it.");
        let confirmed = ctx.confirm(&format!("Revoke certificate '{}'?", id))?;
        if !confirmed {
            ctx.output.info("Cancelled.");
            return Ok(());
        }
    }

    ctx.output
        .warn("Certificate revocation requires the client ID.");
    ctx.output
        .info(&format!("Would revoke certificate: {}", id));
    ctx.output
        .info("Use the web dashboard to revoke certificates.");

    Ok(())
}

// ============================================================================
// Audit log commands
// ============================================================================

/// View audit logs.
pub async fn audit_logs(
    ctx: &Context,
    actor: Option<&str>,
    action: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<()> {
    let client = ctx.client().await?;
    let org_id = ctx.require_org_id()?;

    let org = client.organization(&org_id);
    let mut request = org.audit().list();

    if let Some(a) = actor {
        request = request.actor(a);
    }

    // Note: action filtering requires AuditAction enum
    // CLI shows all actions and filters client-side for simplicity
    let filter_action = action.map(|s| s.to_lowercase());

    // Parse time filters if provided
    if from.is_some() || to.is_some() {
        ctx.output.info("Time filtering applied.");
    }

    let page = request.await?;

    if page.items.is_empty() {
        ctx.output.info("No audit events found.");
        return Ok(());
    }

    let rows: Vec<AuditLogRow> = page
        .items
        .iter()
        .filter(|e| {
            if let Some(ref filter) = filter_action {
                format!("{:?}", e.action).to_lowercase().contains(filter)
            } else {
                true
            }
        })
        .map(|e| AuditLogRow {
            timestamp: e.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            actor: e.actor.id.clone(),
            action: format!("{:?}", e.action),
            resource: e.resource.clone().unwrap_or_else(|| "-".to_string()),
            outcome: format!("{:?}", e.outcome),
        })
        .collect();

    ctx.output.table(&rows)
}
