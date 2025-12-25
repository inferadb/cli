//! Command-line argument parsing and command definitions.
//!
//! Uses clap with derive macros for type-safe argument parsing.

use clap::{Parser, Subcommand, ValueEnum};

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

    /// Manage organizations
    #[command(subcommand)]
    Orgs(OrgsCommands),

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

    /// Interactive shell (REPL)
    Shell,

    /// Show quick reference card
    Cheatsheet {
        /// Role filter (developer, admin, ops)
        #[arg(long)]
        role: Option<String>,
    },

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

    /// Manage organization members
    #[command(subcommand)]
    Members(MembersCommands),

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

    /// Deactivate client (revoke all credentials)
    Deactivate {
        /// Client ID
        id: String,
    },
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
