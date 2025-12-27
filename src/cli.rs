//! Command-line argument parsing and command definitions.
//!
//! Uses clap with derive macros for type-safe argument parsing.
//! Help text is localized at runtime using the i18n system.

use crate::t;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

/// InferaDB CLI - Authorization Engine
#[derive(Parser, Debug)]
#[command(name = "inferadb")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Profile to use (can also use @profile syntax as first argument)
    #[arg(long, global = true, env = "INFERADB_PROFILE")]
    pub profile: Option<String>,

    /// Override organization ID
    #[arg(long, global = true, env = "INFERADB_ORG")]
    pub org: Option<String>,

    /// Override vault ID
    #[arg(short, long, global = true, env = "INFERADB_VAULT")]
    pub vault: Option<String>,

    /// Output format
    #[arg(short, long, global = true, default_value = "table", value_parser = ["table", "json", "yaml", "jsonl"])]
    pub output: String,

    /// Color output mode
    #[arg(long, global = true, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Enable debug logging
    #[arg(long, global = true)]
    pub debug: bool,

    /// Skip confirmations (answer yes to all prompts)
    #[arg(short, long, global = true)]
    pub yes: bool,

    /// Language for CLI output (e.g., en-US)
    #[arg(long, global = true, env = "INFERADB_LANG", default_value = "en-US")]
    pub lang: String,

    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// First-run setup wizard
    Init,

    /// Authenticate with InferaDB
    Login,

    /// Remove authentication
    Logout,

    /// Create a new account
    Register {
        /// Email address
        #[arg(long)]
        email: Option<String>,

        /// Display name
        #[arg(long)]
        name: Option<String>,
    },

    /// Show current user and profile info
    Whoami,

    /// Check service status
    Status,

    /// Measure latency to service
    Ping {
        /// Number of pings
        #[arg(long, short, default_value = "3")]
        count: u32,

        /// Ping control plane only
        #[arg(long)]
        control: bool,

        /// Ping engine only
        #[arg(long)]
        engine: bool,
    },

    /// Run connectivity diagnostics
    Doctor,

    /// Show service health dashboard
    Health {
        /// Watch mode (continuous refresh)
        #[arg(long, short)]
        watch: bool,

        /// Include detailed metrics
        #[arg(long)]
        verbose: bool,
    },

    /// Show CLI version
    Version {
        /// Check for updates
        #[arg(long)]
        check: bool,
    },

    /// Check authorization
    Check {
        /// Subject (e.g., user:alice)
        subject: String,

        /// Permission to check (e.g., can_view)
        permission: String,

        /// Resource (e.g., document:readme)
        resource: String,

        /// Show resolution trace
        #[arg(long)]
        trace: bool,

        /// Explain denial reason
        #[arg(long)]
        explain: bool,

        /// ABAC context as JSON
        #[arg(long)]
        context: Option<String>,
    },

    /// Simulate authorization with hypothetical changes
    Simulate {
        /// Subject
        subject: String,

        /// Permission
        permission: String,

        /// Resource
        resource: String,

        /// Relationships to add (resource#relation@subject format)
        #[arg(long = "add")]
        add_relationships: Vec<String>,

        /// Relationships to remove
        #[arg(long = "remove")]
        remove_relationships: Vec<String>,
    },

    /// Show userset expansion tree
    Expand {
        /// Resource (e.g., document:readme)
        resource: String,

        /// Relation (e.g., viewer)
        relation: String,

        /// Maximum expansion depth
        #[arg(long, default_value = "10")]
        max_depth: u32,
    },

    /// Explain how a permission is computed
    ExplainPermission {
        /// Subject
        subject: String,

        /// Permission
        permission: String,

        /// Resource
        resource: String,
    },

    /// List resources accessible by a subject
    #[command(alias = "what-can")]
    ListResources {
        /// Subject (e.g., user:alice)
        subject: String,

        /// Permission to check
        permission: String,

        /// Resource type to filter
        #[arg(long)]
        resource_type: Option<String>,
    },

    /// List subjects with access to a resource
    #[command(alias = "who-can")]
    ListSubjects {
        /// Resource (e.g., document:readme)
        resource: String,

        /// Permission to check
        permission: String,

        /// Subject type to filter
        #[arg(long)]
        subject_type: Option<String>,
    },

    /// Manage profiles
    #[command(subcommand)]
    Profiles(ProfilesCommands),

    /// Manage configuration
    #[command(subcommand)]
    Config(ConfigCommands),

    /// Manage relationships
    #[command(subcommand)]
    Relationships(RelationshipsCommands),

    /// Manage schemas
    #[command(subcommand)]
    Schemas(SchemasCommands),

    /// Manage your account
    #[command(subcommand)]
    Account(AccountCommands),

    /// Manage organizations
    #[command(subcommand)]
    Orgs(OrgsCommands),

    /// JWKS operations (debugging)
    #[command(subcommand)]
    Jwks(JwksCommands),

    /// Manage tokens
    #[command(subcommand)]
    Tokens(TokensCommands),

    /// Export relationships to file
    Export {
        /// Output file path
        #[arg(long, short)]
        output: Option<String>,

        /// Resource type filter
        #[arg(long)]
        resource_type: Option<String>,

        /// Format (json, yaml, csv)
        #[arg(long, default_value = "json")]
        format: String,
    },

    /// Import relationships from file
    Import {
        /// Input file path
        file: String,

        /// Skip confirmation
        #[arg(long)]
        yes: bool,

        /// Dry run (validate only)
        #[arg(long)]
        dry_run: bool,

        /// Import mode (merge, replace, upsert)
        #[arg(long, default_value = "upsert")]
        mode: String,
    },

    /// Watch real-time relationship changes
    Stream {
        /// Filter by resource type
        #[arg(long)]
        resource_type: Option<String>,

        /// Filter by relation
        #[arg(long)]
        relation: Option<String>,
    },

    /// Vault relationship statistics
    Stats {
        /// Include historical trends
        #[arg(long)]
        trends: bool,

        /// Compact single-line output
        #[arg(long)]
        compact: bool,
    },

    /// Recent vault changes summary
    WhatChanged {
        /// Time range (e.g., 1h, 1d, yesterday, or ISO timestamp)
        #[arg(long)]
        since: Option<String>,

        /// End time for range
        #[arg(long)]
        until: Option<String>,

        /// Focus area (schemas, relationships, permissions)
        #[arg(long)]
        focus: Option<String>,

        /// Filter by actor
        #[arg(long)]
        actor: Option<String>,

        /// Filter by resource
        #[arg(long)]
        resource: Option<String>,

        /// Compact summary output
        #[arg(long)]
        compact: bool,
    },

    /// Interactive shell (REPL)
    Shell,

    /// Show quick reference card
    Cheatsheet {
        /// Role filter (developer, admin, ops)
        #[arg(long)]
        role: Option<String>,
    },

    /// Show workflow templates
    Templates {
        /// Template name (omit to list all)
        name: Option<String>,

        /// Subject to substitute in template
        #[arg(long)]
        subject: Option<String>,

        /// Output format (text, script)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show workflow guides
    Guide {
        /// Guide name (omit to list all)
        name: Option<String>,
    },

    /// Local development environment
    #[command(subcommand)]
    Dev(DevCommands),

    /// Generate shell completions
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

/// Shell types for completion generation.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Shell {
    /// Bash shell.
    Bash,
    /// Zsh shell.
    Zsh,
    /// Fish shell.
    Fish,
    /// PowerShell.
    PowerShell,
}

/// Profile management commands.
#[derive(Subcommand, Debug)]
pub enum ProfilesCommands {
    /// List all profiles
    List,

    /// Show profile details
    Show {
        /// Profile name
        name: Option<String>,
    },

    /// Create a new profile
    Create {
        /// Profile name
        name: String,

        /// API URL
        #[arg(long)]
        url: Option<String>,

        /// Organization ID
        #[arg(long)]
        org: Option<String>,

        /// Vault ID
        #[arg(long)]
        vault: Option<String>,
    },

    /// Update an existing profile
    Update {
        /// Profile name
        name: String,

        /// API URL
        #[arg(long)]
        url: Option<String>,

        /// Organization ID
        #[arg(long)]
        org: Option<String>,

        /// Vault ID
        #[arg(long)]
        vault: Option<String>,
    },

    /// Rename a profile
    Rename {
        /// Current name
        old_name: String,

        /// New name
        new_name: String,
    },

    /// Delete a profile
    Delete {
        /// Profile name
        name: String,
    },

    /// Set the default profile
    Default {
        /// Profile name (omit to show current default)
        name: Option<String>,
    },
}

/// Configuration commands.
#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Show current configuration
    Show {
        /// Specific key to show
        key: Option<String>,
    },

    /// Edit configuration file
    Edit {
        /// Editor to use
        #[arg(long)]
        editor: Option<String>,
    },

    /// Show configuration file path
    Path {
        /// Show directory instead of file
        #[arg(long)]
        dir: bool,
    },

    /// Explain configuration resolution
    Explain,
}

/// Relationship management commands.
#[derive(Subcommand, Debug)]
pub enum RelationshipsCommands {
    /// List relationships
    List {
        /// Filter by resource
        #[arg(long)]
        resource: Option<String>,

        /// Filter by subject
        #[arg(long)]
        subject: Option<String>,

        /// Filter by relation
        #[arg(long)]
        relation: Option<String>,

        /// Maximum results
        #[arg(long, default_value = "100")]
        limit: u32,

        /// Pagination cursor
        #[arg(long)]
        cursor: Option<String>,
    },

    /// Add a relationship
    Add {
        /// Subject (e.g., user:alice)
        subject: String,

        /// Relation (e.g., viewer)
        relation: String,

        /// Resource (e.g., document:readme)
        resource: String,

        /// Succeed if relationship already exists
        #[arg(long)]
        if_not_exists: bool,
    },

    /// Delete a relationship
    Delete {
        /// Subject
        subject: String,

        /// Relation
        relation: String,

        /// Resource
        resource: String,

        /// Succeed if relationship doesn't exist
        #[arg(long)]
        if_exists: bool,
    },

    /// Show relationship history
    History {
        /// Resource filter
        #[arg(long)]
        resource: Option<String>,

        /// Time range start
        #[arg(long)]
        from: Option<String>,

        /// Time range end
        #[arg(long)]
        to: Option<String>,
    },

    /// Validate relationships against schema
    Validate {
        /// File to validate
        file: Option<String>,
    },
}

/// Schema management commands.
#[derive(Subcommand, Debug)]
pub enum SchemasCommands {
    /// Initialize a schema project
    Init {
        /// Directory to initialize
        #[arg(default_value = ".")]
        path: String,

        /// Schema template
        #[arg(long, default_value = "blank")]
        template: String,
    },

    /// List schema versions
    List {
        /// Include inactive versions
        #[arg(long)]
        all: bool,
    },

    /// Get schema content
    Get {
        /// Schema ID (or "active" for current)
        id: String,
    },

    /// Preview schema changes
    Preview {
        /// Schema file
        file: String,

        /// Base version to compare against
        #[arg(long)]
        base: Option<String>,

        /// Show detailed impact analysis
        #[arg(long)]
        impact: bool,
    },

    /// Push schema to vault
    Push {
        /// Schema file
        file: String,

        /// Activate immediately
        #[arg(long)]
        activate: bool,

        /// Change message
        #[arg(long, short)]
        message: Option<String>,

        /// Dry run (validate only)
        #[arg(long)]
        dry_run: bool,
    },

    /// Activate a schema version
    Activate {
        /// Schema ID
        id: String,

        /// Show diff before activating
        #[arg(long)]
        diff: bool,

        /// Use canary deployment
        #[arg(long)]
        canary: Option<u8>,
    },

    /// Rollback to previous schema
    Rollback {
        /// Target version (default: previous)
        version: Option<String>,
    },

    /// Validate schema syntax
    Validate {
        /// Schema file
        file: String,

        /// Strict mode
        #[arg(long)]
        strict: bool,
    },

    /// Format schema file
    Format {
        /// Schema file
        file: String,

        /// Write changes to file
        #[arg(long)]
        write: bool,
    },

    /// Compare schema versions
    Diff {
        /// First version
        from: String,

        /// Second version
        to: String,

        /// Show impact analysis
        #[arg(long)]
        impact: bool,
    },

    /// Run schema tests
    Test {
        /// Test file
        #[arg(long)]
        tests: Option<String>,

        /// Schema to test against
        #[arg(long)]
        schema: Option<String>,

        /// Filter by test name
        #[arg(long)]
        name: Option<String>,
    },

    /// Watch for schema changes
    Watch {
        /// Schema file to watch
        #[arg(default_value = "schema.ipl")]
        file: String,

        /// Run tests on change
        #[arg(long)]
        test: bool,

        /// Auto-push on successful validation
        #[arg(long)]
        auto_push: bool,
    },

    /// Canary deployment management
    #[command(subcommand)]
    Canary(CanaryCommands),

    /// Analyze schema for issues
    Analyze {
        /// Schema file or version ID
        file: String,

        /// Specific checks to run (comma-separated: unused,cycles,shadowing)
        #[arg(long)]
        checks: Option<String>,

        /// Compare against another version
        #[arg(long)]
        compare: Option<String>,
    },

    /// Generate schema visualization
    Visualize {
        /// Schema file or version ID
        file: String,

        /// Output format (mermaid, dot, ascii)
        #[arg(short, long, default_value = "ascii")]
        format: String,

        /// Focus on specific entity
        #[arg(long)]
        entity: Option<String>,

        /// Show permission inheritance
        #[arg(long)]
        show_permissions: bool,
    },

    /// Copy schema between vaults
    Copy {
        /// Schema version ID (or omit for active)
        version: Option<String>,

        /// Source vault (if not current)
        #[arg(long)]
        from_vault: Option<String>,

        /// Target vault
        #[arg(long)]
        to_vault: String,

        /// Source organization (for cross-org copy)
        #[arg(long)]
        from_org: Option<String>,

        /// Target organization (for cross-org copy)
        #[arg(long)]
        to_org: Option<String>,

        /// Activate in target vault
        #[arg(long)]
        activate: bool,

        /// Preview without copying
        #[arg(long)]
        dry_run: bool,
    },

    /// Generate migration plan between versions
    Migrate {
        /// Source version
        #[arg(long)]
        from: Option<String>,

        /// Target version or file
        #[arg(long)]
        to: String,

        /// Output format (text, json, yaml)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

/// Canary deployment commands.
#[derive(Subcommand, Debug)]
pub enum CanaryCommands {
    /// Show canary status
    Status,

    /// Promote canary to full deployment
    Promote {
        /// Wait for completion
        #[arg(long)]
        wait: bool,
    },

    /// Rollback canary deployment
    Rollback,
}

/// Organization management commands.
#[derive(Subcommand, Debug)]
pub enum OrgsCommands {
    /// List organizations
    List,

    /// Create organization
    Create {
        /// Organization name
        name: String,

        /// Tier (starter, pro, enterprise)
        #[arg(long)]
        tier: Option<String>,
    },

    /// Get organization details
    Get {
        /// Organization ID
        id: Option<String>,
    },

    /// Update organization
    Update {
        /// Organization ID
        id: Option<String>,

        /// New name
        #[arg(long)]
        name: Option<String>,
    },

    /// Delete organization
    Delete {
        /// Organization ID
        id: String,
    },

    /// Suspend organization
    Suspend {
        /// Organization ID
        id: String,
    },

    /// Resume organization
    Resume {
        /// Organization ID
        id: String,
    },

    /// Leave organization
    Leave {
        /// Organization ID
        id: String,

        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },

    /// Manage organization members
    #[command(subcommand)]
    Members(MembersCommands),

    /// Manage organization invitations
    #[command(subcommand)]
    Invitations(InvitationsCommands),

    /// Manage organization roles
    #[command(subcommand)]
    Roles(OrgRolesCommands),

    /// Manage organization vaults
    #[command(subcommand)]
    Vaults(VaultsCommands),

    /// Manage organization teams
    #[command(subcommand)]
    Teams(TeamsCommands),

    /// Manage organization clients
    #[command(subcommand)]
    Clients(ClientsCommands),

    /// View audit logs
    AuditLogs {
        /// Filter by actor
        #[arg(long)]
        actor: Option<String>,

        /// Filter by action
        #[arg(long)]
        action: Option<String>,

        /// Time range start
        #[arg(long)]
        from: Option<String>,

        /// Time range end
        #[arg(long)]
        to: Option<String>,
    },
}

/// Invitation management commands.
#[derive(Subcommand, Debug)]
pub enum InvitationsCommands {
    /// List pending invitations
    List,

    /// Create an invitation
    Create {
        /// Email address to invite
        email: String,

        /// Role to assign (owner, admin, member)
        #[arg(long, default_value = "member")]
        role: String,
    },

    /// Delete/cancel an invitation
    Delete {
        /// Invitation ID
        id: String,
    },

    /// Resend invitation email
    Resend {
        /// Invitation ID
        id: String,
    },

    /// Accept an invitation (using token from email)
    Accept {
        /// Invitation token
        token: String,
    },
}

/// Organization role management commands.
#[derive(Subcommand, Debug)]
pub enum OrgRolesCommands {
    /// List role assignments
    List,

    /// Grant a role to a user
    Grant {
        /// User ID
        user_id: String,

        /// Role (owner, admin, member)
        role: String,
    },

    /// Update a user's role
    Update {
        /// User ID
        user_id: String,

        /// New role
        role: String,
    },

    /// Revoke a user's role
    Revoke {
        /// User ID
        user_id: String,
    },
}

/// Member management commands.
#[derive(Subcommand, Debug)]
pub enum MembersCommands {
    /// List organization members
    List,

    /// Update member role
    UpdateRole {
        /// Member ID
        member_id: String,

        /// New role (owner, admin, member)
        role: String,
    },

    /// Remove member
    Remove {
        /// Member ID
        member_id: String,
    },
}

/// Vault management commands.
#[derive(Subcommand, Debug)]
pub enum VaultsCommands {
    /// List vaults
    List,

    /// Create vault
    Create {
        /// Vault name
        name: String,

        /// Description
        #[arg(long)]
        description: Option<String>,
    },

    /// Get vault details
    Get {
        /// Vault ID
        id: Option<String>,
    },

    /// Update vault
    Update {
        /// Vault ID
        id: String,

        /// New name
        #[arg(long)]
        name: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,
    },

    /// Delete vault
    Delete {
        /// Vault ID
        id: String,
    },

    /// Manage vault user roles
    #[command(subcommand)]
    Roles(VaultRolesCommands),

    /// Manage vault team roles
    #[command(subcommand)]
    TeamRoles(VaultTeamRolesCommands),
}

/// Vault user role management commands.
#[derive(Subcommand, Debug)]
pub enum VaultRolesCommands {
    /// List user role assignments
    List {
        /// Vault ID
        #[arg(long)]
        vault: Option<String>,
    },

    /// Grant a role to a user
    Grant {
        /// User ID
        user_id: String,

        /// Role (admin, manager, editor, writer, reader)
        role: String,

        /// Vault ID
        #[arg(long)]
        vault: Option<String>,
    },

    /// Update a user's role
    Update {
        /// Role assignment ID
        id: String,

        /// New role
        role: String,
    },

    /// Revoke a user's role
    Revoke {
        /// Role assignment ID
        id: String,
    },
}

/// Vault team role management commands.
#[derive(Subcommand, Debug)]
pub enum VaultTeamRolesCommands {
    /// List team role assignments
    List {
        /// Vault ID
        #[arg(long)]
        vault: Option<String>,
    },

    /// Grant a role to a team
    Grant {
        /// Team ID
        team_id: String,

        /// Role (admin, manager, editor, writer, reader)
        role: String,

        /// Vault ID
        #[arg(long)]
        vault: Option<String>,
    },

    /// Update a team's role
    Update {
        /// Role assignment ID
        id: String,

        /// New role
        role: String,
    },

    /// Revoke a team's role
    Revoke {
        /// Role assignment ID
        id: String,
    },
}

/// Team management commands.
#[derive(Subcommand, Debug)]
pub enum TeamsCommands {
    /// List teams
    List,

    /// Create team
    Create {
        /// Team name
        name: String,

        /// Description
        #[arg(long)]
        description: Option<String>,
    },

    /// Get team details
    Get {
        /// Team ID or name
        id: String,
    },

    /// Update team
    Update {
        /// Team ID
        id: String,

        /// New name
        #[arg(long)]
        name: Option<String>,
    },

    /// Delete team
    Delete {
        /// Team ID
        id: String,
    },

    /// Manage team members
    #[command(subcommand)]
    Members(TeamMembersCommands),

    /// Manage team permissions
    #[command(subcommand)]
    Permissions(TeamPermissionsCommands),

    /// Manage team vault grants
    #[command(subcommand)]
    Grants(TeamGrantsCommands),
}

/// Team member management commands.
#[derive(Subcommand, Debug)]
pub enum TeamMembersCommands {
    /// List team members
    List {
        /// Team ID
        team_id: String,
    },

    /// Add member to team
    Add {
        /// Team ID
        team_id: String,

        /// User ID
        user_id: String,

        /// Role (maintainer, member)
        #[arg(long, default_value = "member")]
        role: String,
    },

    /// Update member role
    UpdateRole {
        /// Team ID
        team_id: String,

        /// User ID
        user_id: String,

        /// New role
        role: String,
    },

    /// Remove member from team
    Remove {
        /// Team ID
        team_id: String,

        /// User ID
        user_id: String,
    },
}

/// Team permission management commands.
#[derive(Subcommand, Debug)]
pub enum TeamPermissionsCommands {
    /// List team permissions
    List {
        /// Team ID
        team_id: String,
    },

    /// Grant permission to team
    Grant {
        /// Team ID
        team_id: String,

        /// Permission (e.g., OrgPermVaultCreate)
        permission: String,
    },

    /// Revoke permission from team
    Revoke {
        /// Team ID
        team_id: String,

        /// Permission
        permission: String,
    },
}

/// Team vault grant management commands.
#[derive(Subcommand, Debug)]
pub enum TeamGrantsCommands {
    /// List team vault grants
    List {
        /// Team ID
        team_id: String,
    },

    /// Create a vault grant for team
    Create {
        /// Team ID
        team_id: String,

        /// Vault ID
        #[arg(long)]
        vault: String,

        /// Role (admin, manager, editor, writer, reader)
        #[arg(long)]
        role: String,
    },

    /// Update a vault grant
    Update {
        /// Grant ID
        id: String,

        /// New role
        #[arg(long)]
        role: String,
    },

    /// Delete a vault grant
    Delete {
        /// Grant ID
        id: String,
    },
}

/// Client management commands.
#[derive(Subcommand, Debug)]
pub enum ClientsCommands {
    /// List clients
    List,

    /// Create client
    Create {
        /// Client name
        name: String,

        /// Vault ID
        #[arg(long)]
        vault: Option<String>,
    },

    /// Get client details
    Get {
        /// Client ID or name
        id: String,
    },

    /// Update client
    Update {
        /// Client ID
        id: String,

        /// New name
        #[arg(long)]
        name: Option<String>,
    },

    /// Delete client
    Delete {
        /// Client ID
        id: String,
    },

    /// Deactivate client (suspend, revoke all credentials)
    Deactivate {
        /// Client ID
        id: String,
    },

    /// Reactivate a suspended client
    Reactivate {
        /// Client ID
        id: String,
    },

    /// Manage client certificates
    #[command(subcommand)]
    Certificates(CertificatesCommands),
}

/// Certificate management commands.
#[derive(Subcommand, Debug)]
pub enum CertificatesCommands {
    /// List certificates
    List {
        /// Client ID
        client_id: String,
    },

    /// Add a certificate
    Add {
        /// Client ID
        client_id: String,
    },

    /// Get certificate details
    Get {
        /// Certificate ID
        id: String,
    },

    /// Rotate certificate with grace period
    Rotate {
        /// Certificate ID
        id: String,

        /// Grace period in hours (both certs valid during this time)
        #[arg(long, default_value = "24")]
        grace_period: u32,
    },

    /// Revoke certificate
    Revoke {
        /// Certificate ID
        id: String,
    },
}

/// Account management commands.
#[derive(Subcommand, Debug)]
pub enum AccountCommands {
    /// Show account details
    Show,

    /// Update account
    Update {
        /// New name
        #[arg(long)]
        name: Option<String>,
    },

    /// Delete account
    Delete {
        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },

    /// Manage email addresses
    #[command(subcommand)]
    Emails(EmailsCommands),

    /// Manage sessions
    #[command(subcommand)]
    Sessions(SessionsCommands),

    /// Password management
    #[command(subcommand)]
    Password(PasswordCommands),
}

/// Email management commands.
#[derive(Subcommand, Debug)]
pub enum EmailsCommands {
    /// List email addresses
    List,

    /// Add an email address
    Add {
        /// Email address
        email: String,

        /// Set as primary after verification
        #[arg(long)]
        primary: bool,
    },

    /// Verify an email address
    Verify {
        /// Verification token from email
        #[arg(long)]
        token: String,
    },

    /// Resend verification email
    Resend {
        /// Email address
        email: String,
    },

    /// Remove an email address
    Remove {
        /// Email ID
        id: String,
    },

    /// Set primary email address
    SetPrimary {
        /// Email ID
        id: String,
    },
}

/// Session management commands.
#[derive(Subcommand, Debug)]
pub enum SessionsCommands {
    /// List active sessions
    List,

    /// Revoke a specific session
    Revoke {
        /// Session ID
        id: String,
    },

    /// Revoke all other sessions (keep current)
    RevokeOthers,
}

/// Password management commands.
#[derive(Subcommand, Debug)]
pub enum PasswordCommands {
    /// Reset password
    Reset {
        /// Request a password reset
        #[arg(long)]
        request: bool,

        /// Confirm password reset with token
        #[arg(long)]
        confirm: bool,

        /// Email address (for request)
        #[arg(long)]
        email: Option<String>,

        /// Reset token (for confirm)
        #[arg(long)]
        token: Option<String>,

        /// New password (for confirm)
        #[arg(long)]
        new_password: Option<String>,
    },
}

/// JWKS commands for debugging JWT verification.
#[derive(Subcommand, Debug)]
pub enum JwksCommands {
    /// Get JSON Web Key Set
    Get,

    /// Get a specific key by ID
    GetKey {
        /// Key ID (kid)
        kid: String,
    },

    /// Get JWKS from .well-known endpoint
    WellKnown,
}

/// Token management commands.
#[derive(Subcommand, Debug)]
pub enum TokensCommands {
    /// Generate a new token
    Generate {
        /// Token TTL (e.g., 1h, 30m, 1d)
        #[arg(long)]
        ttl: Option<String>,

        /// Token role
        #[arg(long)]
        role: Option<String>,
    },

    /// List tokens
    List,

    /// Revoke a token
    Revoke {
        /// Token ID
        id: String,
    },

    /// Refresh current token
    Refresh,

    /// Inspect token details
    Inspect {
        /// Token to inspect (default: current)
        token: Option<String>,

        /// Verify signature
        #[arg(long)]
        verify: bool,
    },
}

/// Local development environment commands.
#[derive(Subcommand, Debug)]
pub enum DevCommands {
    /// Check if host environment is ready for development
    Doctor {
        /// Run in full-screen interactive TUI mode
        #[arg(long, short)]
        interactive: bool,
    },

    /// Install deploy repository (~/.inferadb/deploy)
    Install {
        /// Remove and re-clone if already present
        #[arg(long)]
        force: bool,

        /// Clone a specific commit, tag, or branch
        #[arg(long)]
        commit: Option<String>,

        /// Run in full-screen interactive TUI mode
        #[arg(long, short)]
        interactive: bool,
    },

    /// Completely remove local dev environment
    Uninstall {
        /// Skip confirmation prompt
        #[arg(long, short)]
        yes: bool,
    },

    /// Start local development cluster
    Start {
        /// Skip building container images
        #[arg(long)]
        skip_build: bool,
    },

    /// Stop local development cluster (pause containers)
    Stop {
        /// Fully destroy the cluster instead of pausing
        #[arg(long)]
        destroy: bool,
    },

    /// Start local development cluster (alias for 'start')
    #[command(hide = true)]
    Up {
        /// Skip building container images
        #[arg(long)]
        skip_build: bool,
    },

    /// Stop local development cluster (alias for 'stop')
    #[command(hide = true)]
    Down {
        /// Fully destroy the cluster instead of pausing
        #[arg(long)]
        destroy: bool,
    },

    /// Show cluster status
    Status {
        /// Run in full-screen interactive TUI mode
        #[arg(long, short)]
        interactive: bool,
    },

    /// View logs
    Logs {
        /// Follow log output
        #[arg(short, long)]
        follow: bool,

        /// Service to show logs for (engine, control, dashboard, fdb)
        #[arg(short, long)]
        service: Option<String>,

        /// Number of lines to show
        #[arg(long, default_value = "100")]
        tail: u32,
    },

    /// Open dashboard in browser
    Dashboard,

    /// Reset all cluster data
    Reset {
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },

    /// Import data into cluster
    Import {
        /// File to import
        file: String,
    },

    /// Export data from cluster
    Export {
        /// Output file
        output: String,
    },
}

impl Cli {
    /// Get the CLI command with localized help text.
    ///
    /// This applies translations from the i18n system to all command
    /// and argument help text.
    pub fn command_localized() -> clap::Command {
        let cmd = Self::command();
        localize_command(cmd)
    }
}

/// Recursively localize a command and all its subcommands.
fn localize_command(mut cmd: clap::Command) -> clap::Command {
    let name = cmd.get_name().to_string();

    // Localize command about text based on command name
    let about_key = format!("cmd-{}-about", name);
    if let Some(_i18n) = crate::i18n::try_get() {
        let translated = t!(&about_key);
        // Only apply if we got a real translation (not the key back)
        if translated != about_key {
            cmd = cmd.about(translated);
        }
    }

    // Localize global options for root command
    if name == "inferadb" {
        cmd = localize_root_args(cmd);
    }

    // Recursively localize subcommands
    let subcommands: Vec<clap::Command> = cmd.get_subcommands().cloned().collect();
    for subcmd in subcommands {
        cmd = cmd.mut_subcommand(subcmd.get_name(), |_| localize_command(subcmd.clone()));
    }

    cmd
}

/// Localize the root command arguments.
fn localize_root_args(mut cmd: clap::Command) -> clap::Command {
    if crate::i18n::try_get().is_none() {
        return cmd;
    }

    // Apply translations to global arguments
    let arg_translations = [
        ("profile", "cli-profile-help"),
        ("org", "cli-org-help"),
        ("vault", "cli-vault-help"),
        ("output", "cli-output-help"),
        ("color", "cli-color-help"),
        ("quiet", "cli-quiet-help"),
        ("yes", "cli-yes-help"),
        ("debug", "cli-debug-help"),
    ];

    for (arg_name, key) in arg_translations {
        let translated = t!(key);
        if translated != key {
            cmd = cmd.mut_arg(arg_name, |arg| arg.help(translated.clone()));
        }
    }

    // Update the about text
    let about = t!("cli-about");
    if about != "cli-about" {
        cmd = cmd.about(about);
    }

    cmd
}

/// Parse the @profile prefix from command-line arguments.
///
/// The CLI supports `@profile` as the first argument to select a profile,
/// which is more ergonomic than `--profile`.
///
/// Returns the profile name (if any) and the remaining arguments.
pub fn parse_profile_prefix(args: Vec<String>) -> (Option<String>, Vec<String>) {
    if args.len() < 2 {
        return (None, args);
    }

    // First arg is the binary name
    let mut iter = args.into_iter();
    let binary = iter.next().unwrap();

    // Check if second arg starts with @
    let second = iter.next().unwrap();
    if second.starts_with('@') && second.len() > 1 {
        let profile = second[1..].to_string();
        let remaining: Vec<String> = std::iter::once(binary).chain(iter).collect();
        (Some(profile), remaining)
    } else {
        let remaining: Vec<String> = std::iter::once(binary)
            .chain(std::iter::once(second))
            .chain(iter)
            .collect();
        (None, remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_profile_prefix() {
        let args = vec![
            "inferadb".to_string(),
            "@prod".to_string(),
            "check".to_string(),
        ];
        let (profile, remaining) = parse_profile_prefix(args);
        assert_eq!(profile, Some("prod".to_string()));
        assert_eq!(remaining, vec!["inferadb", "check"]);
    }

    #[test]
    fn test_parse_profile_prefix_no_profile() {
        let args = vec!["inferadb".to_string(), "check".to_string()];
        let (profile, remaining) = parse_profile_prefix(args);
        assert!(profile.is_none());
        assert_eq!(remaining, vec!["inferadb", "check"]);
    }

    #[test]
    fn test_parse_profile_prefix_at_sign_only() {
        let args = vec!["inferadb".to_string(), "@".to_string(), "check".to_string()];
        let (profile, remaining) = parse_profile_prefix(args);
        assert!(profile.is_none());
        assert_eq!(remaining, vec!["inferadb", "@", "check"]);
    }
}
