# InferaDB CLI Development Overview

An ideal InferaDB CLI should feel familiar to people who use `kubectl`, `git`, `aws`, and `fga`—composable subcommands, great `--help`, clear resources, and safe defaults.

---

## Table of Contents

### Part 1: Getting Started

- [Core Design Principles](#core-design-principles)
- [Command Hierarchy](#command-hierarchy)
- [Getting Started](#getting-started)
- [Understanding Profiles](#understanding-profiles) *(start here if new)*

### Part 2: Configuration & Authentication

- [Name Resolution](#name-resolution)
- [Configuration](#configuration)
- [Configuration File](#configuration-file)
- [Environment Variables](#environment-variables)
- [Authentication](#authentication)

### Part 3: Identity & Diagnostics

- [Identity & Diagnostics](#identity--diagnostics) (whoami, status, ping, doctor, health, stats)
- [What Changed](#what-changed)

### Part 4: Account & Organization Management

- [Account Management](#account-management)
- [Organization Management](#organization-management)
- [Organization Members](#organization-members)
- [Organization Invitations](#organization-invitations)
- [Teams](#teams)
- [Organization Roles](#organization-roles)

### Part 5: Vault & Client Management

- [Vault Management](#vault-management)
- [Client Management](#client-management)
- [Audit Logs](#audit-logs)

### Part 6: Schema Development

- [Schema Management](#schema-management)
- [JWKS (JSON Web Key Sets)](#jwks-json-web-key-sets)

### Part 7: Authorization & Relationships

- [Authorization Queries](#authorization-queries) (check, simulate, expand, explain-permission)
- [Relationship Management](#relationship-management)
- [Stream](#stream)
- [Token Management](#token-management)
- [Bulk Operations](#bulk-operations) (export, import)

### Part 8: CLI Reference

- [Global Flags](#global-flags)
- [Value Formats](#value-formats)
- [Tuple Format](#tuple-format)
- [Exit Codes](#exit-codes)
- [Error Handling & Diagnostics](#error-handling--diagnostics)

### Part 9: Developer Experience

- [Shell Completion](#shell-completion)
- [Output Formatting](#output-formatting)
- [Security](#security) (token protection, credential storage, credential rotation)
- [Built-in Help & Examples](#built-in-help--examples)
- [Interactive Mode](#interactive-mode)

### Part 10: Appendix

- [Planned Features](#planned-features)
- [Troubleshooting](#troubleshooting)
- [Examples](#examples)
- [Sources](#sources)

---

---

<!-- ═══════════════════════════════════════════════════════════════════════════
     PART 1: GETTING STARTED
     ═══════════════════════════════════════════════════════════════════════════ -->

## Core Design Principles

- Nouns and verbs: `inferadb vaults list`, `inferadb check`.
- Idempotent commands: `apply` and `sync` should be safe to re-run.
- First-class environments: `@profile` shorthand, profiles in config file.
- Human-friendly by default, machine-friendly via `-o json|yaml|table`.
- Profile-based defaults: each profile is a complete target (URL + org + vault).
- Destructive operations require `--yes` or interactive confirmation.
- Name resolution: use names or IDs interchangeably where practical.

## Command Hierarchy

```text
inferadb
├── init                          # First-run setup wizard
├── login / logout / register     # Authentication
├── whoami                        # Current user and profile info
├── status                        # Service health
├── ping                          # Latency measurement
├── doctor                        # Connectivity diagnostics
├── health                        # Operational dashboard
├── version                       # Version and update check
├── stats                         # Vault relationship statistics
├── what-changed                  # Recent vault changes summary
├── config                        # Configuration management
│   ├── show / edit / path
├── cheatsheet                    # Quick reference card (--role, --format)
├── templates                     # Copy-paste workflow templates
├── guide                         # Opinionated workflow guides
│
├── profiles                      # Multi-environment management
│   ├── create / list / show / update / rename / delete / default
│
├── account                       # Current user management
│   ├── show / update / delete
│   ├── emails (list / add / verify / remove / set-primary)
│   ├── sessions (list / revoke / revoke-others)
│   └── password (reset)
│
├── orgs                          # Organization management
│   ├── list / create / get / update / delete
│   ├── suspend / resume / leave
│   ├── members (list / update-role / remove)
│   ├── invitations (list / create / delete / accept)
│   ├── teams
│   │   ├── list / create / get / update / delete
│   │   ├── members (list / add / remove / update-role)
│   │   ├── permissions (list / grant / revoke)
│   │   └── grants (list / create / update / delete)
│   ├── roles (list / grant / update / revoke)    # Org-level user role assignments
│   ├── vaults (list / create / get / update / delete)
│   │   ├── roles (list / grant / update / revoke)      # Vault user roles
│   │   ├── team-roles (list / grant / update / revoke) # Vault team roles
│   │   └── tokens (list / generate / revoke)
│   ├── clients
│   │   ├── list / create / get / update / delete / deactivate
│   │   └── certificates (list / create / get / revoke / delete)
│   └── audit-logs
│
├── schemas                       # IPL schema management
│   ├── init / list / get / preview / push
│   ├── activate / rollback / diff / pre-flight
│   ├── canary (status / promote / rollback)
│   ├── validate / format / analyze
│   ├── test                      # Run authorization test cases
│   ├── visualize                 # Generate diagrams
│   ├── watch                     # Auto-validate on change
│   ├── develop                   # Unified dev workflow (--auto-push)
│   ├── copy                      # Copy schema between vaults
│   └── migrate                   # Generate migration helpers
│
├── jwks                          # JSON Web Key Sets (debugging)
│
├── check                         # Authorization evaluation
├── simulate                      # What-if testing
├── expand                        # Userset tree visualization
├── explain-permission            # Permission hierarchy explanation
├── list-resources                # Resources accessible by subject
│   └── (alias: what-can)
├── list-subjects                 # Subjects with access to resource
│   └── (alias: who-can)
│
├── relationships                 # Tuple management
│   ├── list / add / delete / history / validate
│
├── stream                        # Real-time relationship changes
│
├── tokens                        # Vault token management
│   ├── generate / list / revoke / refresh / inspect
│
├── export / import               # Bulk operations
│
├── shell                         # Interactive REPL
│
└── dev                           # Local development (planned)
    ├── up / down / status / logs / dashboard
```

---

## Getting Started

### First-Run Setup

```bash
inferadb init
```

Interactive wizard that:

1. Creates or selects a profile
2. Opens browser for OAuth PKCE authentication
3. Prompts to select default organization
4. Prompts to select default vault
5. Optionally scaffolds a starter `schema.ipl`

### Registration

```bash
# Interactive registration (prompts for email, name, and password)
inferadb register

# Non-interactive registration (prompts for password securely)
inferadb register --email hello@example.com --name "John Doe"

# Fully non-interactive (for scripts - not recommended)
inferadb register --email hello@example.com --name "Jone Done" --password "SecurePass123!"

# After successful registration, prompts:
# "Save as profile? [profile-name]:"
```

Password requirements: minimum 12 characters.

---

## Understanding Profiles

A **profile** is a complete, ready-to-use target environment. Each profile contains everything needed to run commands: the server URL, authentication, organization, and vault.

### The Mental Model

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         Your CLI Configuration                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  PROFILE "prod"                      PROFILE "staging"              │
│  ┌────────────────────────┐          ┌────────────────────────┐     │
│  │ URL: api.inferadb.com  │          │ URL: api.inferadb.com  │     │
│  │ Org: 123456789012345678│          │ Org: 123456789012345678│     │
│  │ Vault: 987654321098765 │          │ Vault: 876543210987654 │     │
│  │ Auth: (in keychain)    │          │ Auth: (in keychain)    │     │
│  └────────────────────────┘          └────────────────────────┘     │
│                                                                     │
│  PROFILE "prod-analytics"            PROFILE "dev"                  │
│  ┌────────────────────────┐          ┌────────────────────────┐     │
│  │ URL: api.inferadb.com  │          │ URL: localhost:3000    │     │
│  │ Org: 123456789012345678│          │ Org: 111222333444555666│     │
│  │ Vault: 555666777888999 │          │ Vault: 222333444555666 │     │
│  │ Auth: (in keychain)    │          │ Auth: (in keychain)    │     │
│  └────────────────────────┘          └────────────────────────┘     │
│                                                                     │
│  Default profile: prod                                              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**One profile = one complete target.** No separate "context" to manage. This matches how the AWS CLI works.

> **Note on "context" terminology:** The CLI uses profiles (not contexts) for targeting environments. The `--context` flag you may see in commands like `check` is for ABAC (Attribute-Based Access Control) - passing runtime attributes like IP address or time for policy evaluation. These are unrelated concepts.

### Beginner: Just Get Started

For most users, you only need this:

```bash
# 1. Run the setup wizard (creates your first profile)
inferadb init

# 2. You're done! Commands now "just work"
inferadb check user:alice can_view document:readme
inferadb relationships list
```

The wizard creates a profile with your server, org, and vault already configured.

### Working with Multiple Environments

Create a profile for each environment you work with:

```bash
# Production vault
inferadb profiles create prod \
  --url https://api.inferadb.com \
  --org 123456789012345678 \
  --vault 987654321098765432

# Staging vault (same server, different vault)
inferadb profiles create staging \
  --url https://api.inferadb.com \
  --org 123456789012345678 \
  --vault 876543210987654321

# Analytics vault (same server, different vault)
inferadb profiles create prod-analytics \
  --url https://api.inferadb.com \
  --org 123456789012345678 \
  --vault 555666777888999000

# Local development
inferadb profiles create dev \
  --url http://localhost:3000 \
  --org 111222333444555666 \
  --vault 222333444555666777

# Log into each (authenticates and stores token in keychain)
inferadb @prod login
inferadb @staging login
inferadb @dev login

# Set your default
inferadb profiles default prod
```

### Using Profiles

```bash
# Commands use your default profile
inferadb check user:alice can_view document:readme

# Use a different profile for one command
inferadb @staging check user:alice can_view document:readme

# Switch your default
inferadb profiles default staging
```

### Quick Reference

```bash
# Create a profile (complete target environment)
inferadb profiles create <name> \
  --url <server-url> \
  --org <org-id> \
  --vault <vault-id>

# List all profiles
inferadb profiles list

# Show profile details
inferadb profiles show <name>

# Set default profile
inferadb profiles default <name>

# Update a profile
inferadb profiles update <name> --vault <new-vault-id>

# Delete a profile
inferadb profiles delete <name>

# Use a profile for one command
inferadb @<name> <command>
```

### Common Patterns

#### Compare prod vs staging

```bash
inferadb @prod check user:alice can_edit document:readme
inferadb @staging check user:alice can_edit document:readme
```

#### Copy data between environments

```bash
inferadb @prod export --output backup.json
inferadb @staging import backup.json --yes
```

#### CI/CD with environment variables

```bash
# In CI, use environment variables instead of profiles
export INFERADB_URL="https://api.inferadb.com"
export INFERADB_ORG="123456789012345678"
export INFERADB_VAULT="987654321098765432"
export INFERADB_TOKEN="$PROD_TOKEN"
inferadb check user:alice can_view document:readme
```

### Why This Design?

- **Simple mental model**: One profile = one target. No layered concepts.
- **Explicit**: Each profile clearly shows where commands will run.
- **Familiar**: Works like AWS CLI profiles that many developers already know.
- **Flexible**: Create as many profiles as you need for different vaults/environments.

---

---

<!-- ═══════════════════════════════════════════════════════════════════════════
     PART 2: CONFIGURATION & AUTHENTICATION
     ═══════════════════════════════════════════════════════════════════════════ -->

## Name Resolution

The CLI supports **name-based references** for resources where names are unique within their scope.

### What Supports Name Resolution

Names can be used for resources that are **unique within their parent scope**:

```bash
# Team names (unique within an organization)
inferadb orgs teams get "Engineering"
inferadb orgs teams delete "Legacy Team" --yes

# Client names (unique within an organization)
inferadb orgs clients get "API Server"
inferadb orgs clients update "Web Dashboard" --name "Web Portal"

# Schema names (unique within a vault)
inferadb schemas get "user-permissions"
```

### What Requires Snowflake IDs

**Organizations and vaults require Snowflake IDs** because their names are not unique:

```bash
# Creating a profile - use Snowflake IDs for org and vault
inferadb profiles create prod \
  --url https://api.inferadb.com \
  --org 123456789012345678 \
  --vault 987654321098765432

# Getting resources - use Snowflake IDs
inferadb orgs get 123456789012345678
inferadb orgs vaults get 987654321098765432
inferadb orgs vaults delete 876543210987654321 --yes
```

Why? Multiple organizations can have the same name (e.g., "Production"), and multiple vaults across different orgs can share names. Snowflake IDs guarantee uniqueness.

### Finding IDs

```bash
# List organizations to find IDs
inferadb orgs list
# ID                   NAME         TIER
# 123456789012345678   Acme Corp    pro
# 234567890123456789   Beta Inc     starter

# List vaults to find IDs
inferadb orgs vaults list --org 123456789012345678
# ID                   NAME         RELATIONSHIPS
# 987654321098765432   Production   15,432
# 876543210987654321   Staging      1,234
```

### @ Prefix for Profiles

The `@` prefix selects which profile to use for a command:

```bash
inferadb @staging check user:alice can_view document:readme
inferadb @prod schemas list
inferadb @dev relationships add user:alice viewer document:readme
```

### Limitations

Name resolution is not available for:

- **Organizations and vaults** (use Snowflake IDs)
- Authorization subjects/resources (e.g., `user:alice`, `document:readme`)
- Relationship tuples (these use your application's identifiers)
- Batch operations (use IDs for reliability)

---

---

<!-- ═══════════════════════════════════════════════════════════════════════════
     PART 3: IDENTITY & DIAGNOSTICS
     ═══════════════════════════════════════════════════════════════════════════ -->

## Identity & Diagnostics

### Who Am I

Display current authenticated user and profile:

```bash
inferadb whoami
```

Output:

```text
User: alice@acme.com (222333444555666777)
Profile: prod
Organization: Acme Corp (123456789012345678)
Vault: Production (987654321098765432)
Org Role: admin
Vault Role: writer
Token expires: 2025-01-16T10:30:00Z (in 23 hours)
```

With JSON output:

```bash
inferadb whoami -o json
```

### Status

```bash
inferadb status
inferadb @prod status
```

Displays:

- Control plane health
- Engine health
- Current profile authentication status
- Active organization and vault from profile

### Ping

```bash
inferadb ping --control    # Latency to Control plane
inferadb ping --engine     # Latency to Engine
inferadb ping              # Both
inferadb ping --count 10   # Multiple pings for statistics
```

Output:

```text
Control plane: https://api.inferadb.com
  Response: 200 OK
  Latency: 23ms (min: 21ms, max: 28ms, avg: 24ms)

Engine: https://engine.inferadb.com
  Response: 200 OK
  Latency: 12ms (min: 10ms, max: 15ms, avg: 12ms)
```

### Doctor

```bash
inferadb doctor
```

Runs comprehensive connectivity diagnostics:

- DNS resolution
- TLS certificate validation
- Authentication token validity and age
- Permission checks for current profile
- Latency measurements
- Proxy configuration verification
- Credential rotation recommendations

Output:

```text
InferaDB Diagnostics

✓ DNS resolution for api.inferadb.com (23ms)
✓ TLS certificate valid (expires in 89 days)
✓ Authentication token valid (expires in 23 hours)
✓ Organization access verified (Acme Corp)
✓ Vault access verified (Production)
✓ Engine connectivity (12ms latency)

All checks passed. Your CLI is properly configured.
```

With credential age warnings:

```text
InferaDB Diagnostics

✓ DNS resolution for api.inferadb.com (23ms)
✓ TLS certificate valid (expires in 89 days)
⚠ Authentication token valid but aging
    Token age: 89 days
    Issued: 2024-10-18T10:30:00Z
    Recommendation: Consider rotating credentials
    Run: inferadb @prod login
✓ Organization access verified (Acme Corp)
✓ Vault access verified (Production)
✓ Engine connectivity (12ms latency)

5 checks passed, 1 warning.

Recommendations:
  ⚠ Token for profile 'prod' is 89 days old
    Long-lived tokens increase security risk if compromised.
    Rotate with: inferadb @prod login
```

Security checks include:

- **Token age**: Warns if token is older than 30 days
- **Token expiry**: Warns if token expires within 24 hours
- **Unused profiles**: Identifies profiles not used in 90+ days
- **Stale profiles**: Warns about profiles pointing to deleted orgs/vaults

### Version

```bash
# Show version
inferadb version

# Check for updates
inferadb version --check

# Show detailed version info
inferadb version --verbose
```

Output:

```text
inferadb 1.2.3 (darwin-arm64)
Built: 2025-01-10T10:30:00Z
Commit: abc1234
```

With `--check`:

```text
inferadb 1.2.3 (darwin-arm64)

Update available: 1.3.0
  Release notes: https://github.com/inferadb/cli/releases/tag/v1.3.0

  To upgrade:
    brew upgrade inferadb      # macOS
    inferadb upgrade           # Self-update
```

### Upgrade

Self-update the CLI to the latest version:

```bash
# Upgrade to latest
inferadb upgrade

# Upgrade to specific version
inferadb upgrade --version 1.3.0

# Check what would be upgraded (dry-run)
inferadb upgrade --dry-run
```

### Stats

Display relationship statistics for the current vault:

```bash
# Show vault statistics
inferadb stats

# Show statistics for specific vault
inferadb stats --vault 987654321098765432

# Output as JSON
inferadb stats -o json

# Include historical trends
inferadb stats --trends
```

Output:

```text
Vault Statistics: Production (987654321098765432)

Relationships:
  Total: 15,432
  Created (last 24h): 234
  Deleted (last 24h): 12

By Subject Type:
  user → document:     8,234 (53.4%)
  group → document:    4,521 (29.3%)
  user → folder:       1,892 (12.3%)
  folder → document:     785 (5.1%)

By Relation:
  viewer:    9,123 (59.1%)
  editor:    4,234 (27.4%)
  owner:     1,892 (12.3%)
  parent:      183 (1.2%)

Schema:
  Active version: 777888999000111222
  Entities: 5
  Relations: 12
  Permissions: 8

Last updated: 2025-01-15T10:30:00Z
```

With `--trends`:

```text
Vault Statistics: Production (987654321098765432)

...

Trends (last 7 days):
  Day         Total    Created    Deleted    Net Change
  2025-01-15  15,432   +234       -12        +222
  2025-01-14  15,210   +189       -8         +181
  2025-01-13  15,029   +156       -23        +133
  2025-01-12  14,896   +201       -15        +186
  2025-01-11  14,710   +178       -9         +169
  2025-01-10  14,541   +145       -11        +134
  2025-01-09  14,407   +167       -14        +153

  7-day average: +168/day
```

### Health

Real-time operational health of the InferaDB service. Unlike `doctor` which checks CLI configuration, `health` checks the actual service status.

```bash
# Quick health check
inferadb health

# Watch mode for monitoring
inferadb health --watch

# Include detailed component status
inferadb health --verbose

# CI/CD mode - exit code reflects health
inferadb health --exit-code

# Check specific components
inferadb health --components api,storage,cache
```

Output:

```text
InferaDB Service Health

COMPONENT          STATUS    LATENCY    DETAILS
API                ✓ up      12ms       v2.1.0
Authentication     ✓ up      8ms        EdDSA keys loaded
Storage            ✓ up      3ms        FoundationDB cluster healthy
Cache              ✓ up      1ms        Redis 7.2, 94% hit rate
Schema Engine      ✓ up      2ms        3 schemas loaded

Overall: ✓ healthy

Last checked: 2025-01-15T14:30:00Z
```

With issues:

```text
InferaDB Service Health

COMPONENT          STATUS    LATENCY    DETAILS
API                ✓ up      12ms       v2.1.0
Authentication     ✓ up      8ms        EdDSA keys loaded
Storage            ⚠ degraded 145ms     1/3 nodes slow
Cache              ✗ down    timeout    Connection refused
Schema Engine      ✓ up      2ms        3 schemas loaded

Overall: ⚠ degraded

Issues:
  ⚠ Storage: Node fdb-3 responding slowly (145ms vs 3ms avg)
     Impact: Increased latency for relationship queries
     ETA: Auto-recovery in progress

  ✗ Cache: Redis connection refused
     Impact: All queries hitting storage directly
     Workaround: Queries still work but slower
     Action: Check Redis status or contact support

Last checked: 2025-01-15T14:30:00Z
Status page: https://status.inferadb.com
```

Watch mode (updates every 5 seconds):

```bash
inferadb health --watch
```

```text
InferaDB Service Health (watching, Ctrl+C to stop)

14:30:05  ✓ api ✓ auth ✓ storage ✓ cache ✓ engine  (45ms total)
14:30:10  ✓ api ✓ auth ✓ storage ✓ cache ✓ engine  (42ms total)
14:30:15  ✓ api ✓ auth ⚠ storage ✓ cache ✓ engine  (89ms total)
          └─ storage: node fdb-3 slow (85ms)
14:30:20  ✓ api ✓ auth ✓ storage ✓ cache ✓ engine  (41ms total)
          └─ storage: recovered

Session: 4 checks, 1 warning, 0 errors
```

Verbose mode shows additional metrics:

```bash
inferadb health --verbose
```

```text
InferaDB Service Health

COMPONENT          STATUS    LATENCY    DETAILS
API                ✓ up      12ms       v2.1.0
  └─ Requests/sec: 1,234
  └─ Error rate: 0.01%
  └─ P99 latency: 45ms

Storage            ✓ up      3ms        FoundationDB cluster healthy
  └─ Cluster size: 3 nodes
  └─ Data size: 2.4GB
  └─ Replication: 3x

Cache              ✓ up      1ms        Redis 7.2
  └─ Memory: 512MB / 2GB (25%)
  └─ Hit rate: 94.2%
  └─ Keys: 45,231

Schema Engine      ✓ up      2ms
  └─ Active schema: user-permissions v3
  └─ Compiled rules: 156
  └─ Cache entries: 12,345
```

Exit codes for CI/CD:

| Exit Code | Meaning                                       |
| --------- | --------------------------------------------- |
| 0         | All components healthy                        |
| 1         | One or more components degraded               |
| 2         | One or more components down                   |
| 3         | Unable to reach InferaDB (network/auth issue) |

```bash
# CI/CD health gate
inferadb health --exit-code && echo "Healthy" || echo "Issues detected"

# With specific threshold
inferadb health --exit-code --fail-on degraded
```

---

## What Changed

Quick situational awareness for teams and CI pipelines. Answers: "What happened since I last looked?"

### Basic Usage

```bash
# What changed since yesterday
inferadb what-changed --since yesterday

# What changed in the last hour
inferadb what-changed --since 1h

# What changed since a specific time
inferadb what-changed --since "2025-01-15T09:00:00Z"

# What changed since last deployment
inferadb what-changed --since-event last-deploy
```

Output:

```text
Changes since 2025-01-15 09:00:00 UTC (4 hours ago)

SCHEMAS
  └─ user-permissions v3 activated (was v2)
     └─ Breaking: removed permission 'document:archive'
     └─ Added: permission 'document:soft_delete'

RELATIONSHIPS
  └─ +47 added, -3 removed
  └─ Notable: alice@acme.com added as admin to 12 new folders

PERMISSIONS (sampled)
  └─ ~156 permission decisions affected by schema change
  └─ Run 'inferadb schemas diff v2 v3 --impact' for details

CONFIGURATION
  └─ No changes

Summary: 1 schema activation, 50 relationship changes
```

### Focused Views

```bash
# Only schema changes
inferadb what-changed --since 1d --focus schemas

# Only relationship changes with details
inferadb what-changed --since 1h --focus relationships --verbose

# Changes by a specific actor
inferadb what-changed --since 1d --actor alice@acme.com

# Changes affecting a specific resource
inferadb what-changed --since 1d --resource document:report-2025
```

### CI/CD Integration

```bash
# Machine-readable output for CI
inferadb what-changed --since "$LAST_DEPLOY_TIME" -o json

# Exit with error if breaking changes detected
inferadb what-changed --since "$LAST_DEPLOY_TIME" --fail-on-breaking

# Compact summary for notifications
inferadb what-changed --since 1h --compact
```

Output (compact):

```text
4h: 1 schema (breaking), +47/-3 relationships
```

### Event Markers

Track deployments and other events for easier time references:

```bash
# Mark current state as a deployment
inferadb what-changed mark-event deploy --name "v2.1.0"

# List event markers
inferadb what-changed list-events

# Show changes between events
inferadb what-changed --since-event "v2.0.0" --until-event "v2.1.0"
```

Output (list-events):

```text
EVENT MARKERS

NAME       TYPE      TIMESTAMP                 AGO
v2.1.0     deploy    2025-01-15T14:00:00Z     2h
v2.0.0     deploy    2025-01-10T09:00:00Z     5d
hotfix-1   deploy    2025-01-12T16:30:00Z     3d
```

---

## Configuration

The CLI follows the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/latest/):

| Purpose | Path | XDG Variable |
|---------|------|--------------|
| Configuration | `~/.config/inferadb/cli.yaml` | `XDG_CONFIG_HOME` |
| State/Logs | `~/.local/state/inferadb/` | `XDG_STATE_HOME` |
| Project config | `.inferadb-cli.yaml` | (current directory) |

### Show Configuration

Display resolved configuration from all sources:

```bash
# Show all configuration
inferadb config show

# Show specific key
inferadb config show default_profile

# Show as JSON
inferadb config show -o json
```

Output:

```text
Configuration sources:
  1. Environment variables
  2. ~/.config/inferadb/cli.yaml (user)
  3. .inferadb-cli.yaml (project)

Resolved configuration:
  default_profile: prod
  profiles:
    prod:
      url: https://api.inferadb.com
      org: 123456789012345678
      vault: 987654321098765432
    dev:
      url: http://localhost:3000
```

### Edit Configuration

Open configuration file in editor:

```bash
# Edit global config
inferadb config edit

# Edit with specific editor
inferadb config edit --editor code
```

### Configuration Path

Show path to configuration file:

```bash
inferadb config path
# /Users/alice/.config/inferadb/cli.yaml

inferadb config path --dir
# /Users/alice/.config/inferadb/
```

---

## Authentication

The CLI associates and stores authentication details with individual profiles. A user can be logged into production with one profile, local development with another, and so on.

### Login

Uses OAuth PKCE flow (opens browser):

```bash
inferadb login
inferadb @prod login
```

### Logout

```bash
inferadb logout
inferadb @prod logout
```

---

---

<!-- ═══════════════════════════════════════════════════════════════════════════
     PART 4: ACCOUNT & ORGANIZATION MANAGEMENT
     ═══════════════════════════════════════════════════════════════════════════ -->

## Account Management

Manage the authenticated user's account.

### Show Account Details

```bash
inferadb account show
```

### Update Account

```bash
inferadb account update --name "New Name"
```

### Delete Account

```bash
inferadb account delete
inferadb account delete --yes  # Skip confirmation
```

### Email Management

```bash
# List emails (shows ID, email, primary status, verification status)
inferadb account emails list

# Add an email (sends verification)
inferadb account emails add newemail@example.com
inferadb account emails add newemail@example.com --primary  # Set as primary after verification

# Verify an email (using token from verification email)
inferadb account emails verify --token {verification-token}

# Remove an email (by ID from list command)
inferadb account emails remove 123456789012345678

# Set primary email (by ID)
inferadb account emails set-primary 123456789012345678
```

Note: Use `inferadb account emails list` to get the email IDs needed for remove/set-primary operations.

### Session Management

```bash
# List all active sessions
inferadb account sessions list

# Revoke a specific session
inferadb account sessions revoke 123456789012345678

# Revoke all other sessions (keep current)
inferadb account sessions revoke-others
```

### Password Management

```bash
# Request password reset (sends email)
inferadb account password reset --request --email user@example.com

# Confirm password reset (using token from email)
inferadb account password reset --confirm --token {reset-token}
# Prompts securely for new password, or provide inline (not recommended):
inferadb account password reset --confirm --token {reset-token} --new-password "NewSecurePass123!"
```

---

## Organization Management

### List Organizations

```bash
inferadb orgs list
```

### Create Organization

```bash
inferadb orgs create "Acme Corp"
inferadb orgs create "Acme Corp" --tier pro
```

Returns the Snowflake ID of the new organization.

### Get Organization Details

```bash
inferadb orgs get 123456789012345678
inferadb orgs get  # Uses org from current profile
```

### Update Organization

```bash
inferadb orgs update 123456789012345678 --name "Acme Corporation"
inferadb orgs update --name "Acme Corporation"  # Uses org from current profile
```

### Delete Organization

```bash
inferadb orgs delete 123456789012345678
inferadb orgs delete 123456789012345678 --yes  # Skip confirmation
```

Confirmation shows impact:

```text
Delete organization "Acme Corp" (123456789012345678)?

This will permanently delete:
  • 5 vaults (including all schemas and relationships)
  • 23 members (membership records, not user accounts)
  • 3 teams
  • 2 clients

This action cannot be undone.

Type "Acme Corp" to confirm: _
```

### Suspend Organization

```bash
inferadb orgs suspend 123456789012345678
```

### Resume Organization

```bash
inferadb orgs resume 123456789012345678
```

### Leave Organization

Removes your membership (unless you are the sole owner).

```bash
inferadb orgs leave 123456789012345678
inferadb orgs leave 123456789012345678 --yes  # Skip confirmation
```

---

## Organization Members

### List Members

```bash
inferadb orgs members list
inferadb orgs members list --org 123456789012345678

# With pagination
inferadb orgs members list --page 2 --per-page 50
```

### Update Member Role

```bash
inferadb orgs members update-role 111222333444555666 admin
inferadb orgs members update-role 111222333444555666 member --org 123456789012345678
```

Roles: `owner`, `admin`, `member`

### Remove Member

```bash
inferadb orgs members remove 111222333444555666
inferadb orgs members remove 111222333444555666 --yes
```

---

## Organization Invitations

### List Pending Invitations

```bash
inferadb orgs invitations list
inferadb orgs invitations list --org 123456789012345678
```

### Create Invitation

```bash
inferadb orgs invitations create user@example.com --role member
inferadb orgs invitations create user@example.com --role admin --org 123456789012345678
```

### Delete Invitation

```bash
inferadb orgs invitations delete 111222333444555666
```

### Accept Invitation

```bash
inferadb orgs invitations accept {invitation-token}
```

---

## Teams

### List Teams

```bash
inferadb orgs teams list
inferadb orgs teams list --org 123456789012345678
```

### Create Team

```bash
inferadb orgs teams create "Engineering"
inferadb orgs teams create "Engineering" --description "Core engineering team"
```

### Get Team

```bash
inferadb orgs teams get 111222333444555666
```

### Update Team

```bash
inferadb orgs teams update 111222333444555666 --name "Platform Engineering"
```

### Delete Team

```bash
inferadb orgs teams delete 111222333444555666
inferadb orgs teams delete 111222333444555666 --yes
```

### Team Members

```bash
# List team members
inferadb orgs teams members list 111222333444555666

# Add member to team
inferadb orgs teams members add 111222333444555666 222333444555666777
inferadb orgs teams members add 111222333444555666 222333444555666777 --role maintainer

# Update member role
inferadb orgs teams members update-role 111222333444555666 222333444555666777 member

# Remove member from team
inferadb orgs teams members remove 111222333444555666 222333444555666777
```

Team roles: `maintainer`, `member`

### Team Permissions

```bash
# List team permissions
inferadb orgs teams permissions list 111222333444555666

# Grant permission to team
inferadb orgs teams permissions grant 111222333444555666 OrgPermVaultCreate

# Revoke permission from team
inferadb orgs teams permissions revoke 111222333444555666 OrgPermVaultCreate
```

### Team Vault Grants

```bash
# List team grants for a vault
inferadb orgs teams grants list 111222333444555666

# Create team grant
inferadb orgs teams grants create 111222333444555666 --vault 987654321098765432 --role writer

# Update team grant
inferadb orgs teams grants update 333444555666777888 --role reader

# Delete team grant
inferadb orgs teams grants delete 333444555666777888
```

---

## Organization Roles

Assign roles to users at the organization level.

```bash
# List role assignments
inferadb orgs roles list

# Grant a role to a user
inferadb orgs roles grant 222333444555666777 admin

# Update a user's role
inferadb orgs roles update 222333444555666777 member

# Revoke a user's role
inferadb orgs roles revoke 222333444555666777
```

Org roles: `owner`, `admin`, `member`

---

---

<!-- ═══════════════════════════════════════════════════════════════════════════
     PART 5: VAULT & CLIENT MANAGEMENT
     ═══════════════════════════════════════════════════════════════════════════ -->

## Vault Management

### List Vaults

```bash
inferadb orgs vaults list
inferadb orgs vaults list --org 123456789012345678

# With pagination
inferadb orgs vaults list --page 2 --per-page 50
```

### Create Vault

```bash
inferadb orgs vaults create "Production"
inferadb orgs vaults create "Production" --description "Production authorization policies"
```

Returns the Snowflake ID of the new vault.

### Get Vault

```bash
inferadb orgs vaults get 987654321098765432
inferadb orgs vaults get  # Uses vault from current profile
```

### Update Vault

```bash
inferadb orgs vaults update 987654321098765432 --name "Prod" --description "Updated description"
```

### Delete Vault

```bash
inferadb orgs vaults delete 987654321098765432
inferadb orgs vaults delete 987654321098765432 --yes
```

Confirmation shows impact:

```text
Delete vault "Production" (987654321098765432)?

This will permanently delete:
  • 3 schema versions (1 active)
  • 15,432 relationships
  • 2 vault tokens

This action cannot be undone.

Type "Production" to confirm: _
```

### Vault Roles (User)

```bash
# List user role assignments for vault
inferadb orgs vaults roles list --vault 987654321098765432

# Grant a role to a user
inferadb orgs vaults roles grant 222333444555666777 writer --vault 987654321098765432

# Update a user's role
inferadb orgs vaults roles update 333444555666777888 reader

# Revoke a user's role
inferadb orgs vaults roles revoke 333444555666777888
```

Vault roles: `admin`, `manager`, `editor`, `writer`, `reader`

### Vault Team Roles

```bash
# List team role assignments for vault
inferadb orgs vaults team-roles list --vault 987654321098765432

# Grant a role to a team
inferadb orgs vaults team-roles grant 111222333444555666 writer --vault 987654321098765432

# Update a team's role
inferadb orgs vaults team-roles update 333444555666777888 reader

# Revoke a team's role
inferadb orgs vaults team-roles revoke 333444555666777888
```

### Vault Tokens

```bash
# List vault tokens
inferadb orgs vaults tokens list --vault 987654321098765432

# Generate vault token
inferadb orgs vaults tokens generate --vault 987654321098765432
inferadb orgs vaults tokens generate --vault 987654321098765432 --ttl 1h    # Human-friendly: 1h, 30m, 1d
inferadb orgs vaults tokens generate --vault 987654321098765432 --ttl 3600  # Or seconds (60-86400)

# Revoke a specific vault token
inferadb orgs vaults tokens revoke 444555666777888999

# Revoke ALL tokens for a vault (requires confirmation)
inferadb orgs vaults tokens revoke-all --vault 987654321098765432
inferadb orgs vaults tokens revoke-all --vault 987654321098765432 --yes
```

---

## Client Management

### List Clients

```bash
inferadb orgs clients list
inferadb orgs clients list --org 123456789012345678

# With pagination
inferadb orgs clients list --page 2 --per-page 50
```

### Create Client

```bash
inferadb orgs clients create "API Server" --vault 987654321098765432
```

### Get Client

```bash
inferadb orgs clients get 555666777888999000
```

### Update Client

```bash
inferadb orgs clients update 555666777888999000 --name "Production API Server"
inferadb orgs clients update 555666777888999000 --vault 987654321098765432  # Change default vault
```

### Delete Client

```bash
inferadb orgs clients delete 555666777888999000
inferadb orgs clients delete 555666777888999000 --yes
```

### Deactivate Client

Immediately revokes all certificates and tokens:

```bash
inferadb orgs clients deactivate 555666777888999000
```

### Client Certificates

```bash
# List certificates for a client
inferadb orgs clients certificates list 555666777888999000

# Create certificate (private key shown ONCE)
inferadb orgs clients certificates create 555666777888999000 --name "Prod Cert 2025"

# Get certificate details
inferadb orgs clients certificates get 555666777888999000 666777888999000111

# Revoke certificate
inferadb orgs clients certificates revoke 555666777888999000 666777888999000111

# Delete certificate
inferadb orgs clients certificates delete 555666777888999000 666777888999000111
```

---

## Audit Logs

```bash
# List audit logs for organization
inferadb orgs audit-logs

# Filter by actor
inferadb orgs audit-logs --actor 222333444555666777

# Filter by action
inferadb orgs audit-logs --action vault.create

# Filter by time range
inferadb orgs audit-logs --from 2025-01-01T00:00:00Z --to 2025-01-31T23:59:59Z

# Filter by resource type
inferadb orgs audit-logs --resource-type vault

# Combine filters
inferadb orgs audit-logs --actor 222333444555666777 --action client.delete --from 2025-01-01

# With pagination
inferadb orgs audit-logs --page 2 --per-page 100
```

---

---

<!-- ═══════════════════════════════════════════════════════════════════════════
     PART 6: SCHEMA DEVELOPMENT
     ═══════════════════════════════════════════════════════════════════════════ -->

## Schema Management

Manage IPL authorization schemas with full lifecycle support: validate, test, diff, and deploy.

### Initialize a Schema Project

```bash
# Create a new schema project in current directory
inferadb schemas init

# Create with a specific template
inferadb schemas init --template rbac          # Role-based access control
inferadb schemas init --template document      # Document/folder hierarchy
inferadb schemas init --template multi-tenant  # Multi-tenant SaaS
inferadb schemas init --template blank         # Empty schema

# Initialize in a specific directory
inferadb schemas init ./authorization
```

Creates:

```text
./
├── schema.ipl              # Main schema file
├── tests/
│   └── schema.test.yaml    # Authorization test cases
└── .inferadb-cli.yaml      # Project configuration
```

### List Schema Versions

```bash
inferadb schemas list
inferadb schemas list --vault 987654321098765432

# Show all versions including inactive
inferadb schemas list --all

# With pagination
inferadb schemas list --limit 20 --cursor eyJvZmZzZXQiOjIwfQ==
```

### Get Schema

```bash
# Get a specific version
inferadb schemas get 777888999000111222

# Get the currently active schema
inferadb schemas get --active

# Output to file
inferadb schemas get 777888999000111222 > schema.ipl
inferadb schemas get 777888999000111222 -o json > schema.json
```

### Preview Schema Changes

Preview the full impact of a schema change before pushing. Combines diff, validation, and impact analysis in one command:

```bash
# Preview changes between local file and active schema
inferadb schemas preview schema.ipl

# Preview against a specific schema version
inferadb schemas preview schema.ipl --base 777888999000111222

# Preview with detailed relationship impact
inferadb schemas preview schema.ipl --impact

# Preview in JSON format for CI/CD pipelines
inferadb schemas preview schema.ipl -o json
```

Output:

```text
Schema Preview: schema.ipl → Active (777888999000111222)

Validation:
  ✓ Syntax valid
  ✓ 5 entities, 14 relations, 9 permissions
  ⚠ 1 warning: Unused relation 'legacy_access' on Document

Changes:
  + Added entity: Team
  + Added relation: Document.team_viewer
  ~ Modified permission: Document.view
    - viewer
    + viewer | team_viewer
  - Removed relation: Document.legacy_access

Breaking Changes:
  ! Removing 'Document.legacy_access' affects 47 existing relationships
    Run: inferadb relationships list --relation legacy_access

Impact Summary:
  Relationships affected: 47 (0.3% of total)
  Permission changes: 156 subjects will gain access via team_viewer
  No subjects will lose existing access

Tests:
  ✓ 6/6 tests pass against new schema

Ready to push? Run:
  inferadb schemas push schema.ipl
  inferadb schemas push schema.ipl --activate  # Push and activate
```

With `--impact` for detailed breakdown:

```text
...

Detailed Impact Analysis:

Relationships to be orphaned (47):
  user:legacy1 legacy_access document:old1
  user:legacy2 legacy_access document:old2
  ... (45 more)

  Export for review:
    inferadb relationships list --relation legacy_access -o json > orphaned.json

Subjects gaining access via 'team_viewer' (156):
  user:alice (via team:engineering#member → document:readme)
  user:bob (via team:engineering#member → document:readme)
  ... (154 more)

Subjects with changed permissions (0):
  No subjects will have reduced access.

Recommendation:
  ✓ Safe to proceed. No breaking permission changes detected.
```

### Push Schema

Upload a new schema version:

```bash
# Push schema (creates new version, does not activate)
inferadb schemas push schema.ipl

# Push with explicit vault
inferadb schemas push schema.ipl --vault 987654321098765432

# Push and activate immediately
inferadb schemas push schema.ipl --activate

# Push with a description/changelog
inferadb schemas push schema.ipl --message "Added team hierarchy support"

# Dry-run: validate against server without pushing
inferadb schemas push schema.ipl --dry-run

# Interactive push with guided workflow
inferadb schemas push schema.ipl --interactive
```

#### Interactive Push

Use `--interactive` for guided safety when pushing to production:

```bash
inferadb schemas push schema.ipl --interactive
```

Output:

```text
Interactive Schema Push

Step 1: Select Target Vault
  Current context: Production (987654321098765432)

  Target vault: [Production ▼]
    > Production
      Staging
      Development

  Selected: Production

Step 2: Validation
  ✓ Syntax valid
  ✓ 5 entities, 14 relations, 9 permissions
  ⚠ 1 warning: Unused relation 'legacy_access'

Step 3: Impact Analysis
  Comparing against active version...

  Changes:
    + Added relation: Document.team_viewer
    ~ Modified permission: Document.view
    - Removed relation: Document.legacy_access

  Impact:
    ✓ 2,341 relationships remain valid
    ! 47 relationships use removed relation
    ? 12 permission grants need review

Step 4: Choose Action

  Breaking changes detected. What would you like to do?

    > Review affected relationships
      Push without activating (safe)
      Push and use canary deployment
      Push and activate immediately
      Abort

  Selection: Push without activating (safe)

Step 5: Confirm

  Push schema to Production?
  - Version will be created but NOT activated
  - Run tests before activating

  Proceed? [Y/n]: y

✓ Schema pushed successfully
  ID: 888999000111222333

Next steps:
  • Run tests: inferadb schemas test --schema 888999000111222333
  • Preview activation: inferadb schemas pre-flight 888999000111222333
  • Activate: inferadb schemas activate 888999000111222333
```

### Activate Schema

Make a schema version active (serves authorization requests):

```bash
inferadb schemas activate 777888999000111222

# Activate with safety check (compares with current)
inferadb schemas activate 777888999000111222 --diff

# Skip confirmation
inferadb schemas activate 777888999000111222 --yes

# Interactive activation with guided workflow
inferadb schemas activate 777888999000111222 --interactive
```

#### Interactive Activation

Use `--interactive` for a guided activation workflow:

```bash
inferadb schemas activate 777888999000111222 --interactive
```

Output:

```text
Interactive Schema Activation

Schema: 777888999000111222
Vault: Production (987654321098765432)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Step 1: Pre-flight Checks
  ✓ Schema syntax valid
  ✓ All tests passing (6/6)
  ⚠ Breaking changes detected
  ✓ No orphaned relationships (immediate)

Step 2: Breaking Change Review

  The following changes may affect authorization:

  1. Removed relation 'Document.legacy_viewer'
     → 47 relationships use this relation
     → Run: inferadb relationships list --relation legacy_viewer

  2. Permission 'Document.share' now requires 'edit AND viewer'
     → 12 subjects may lose access

  Review these changes? [Y/n]: y

  [Opens pager with detailed relationship list]

Step 3: Choose Activation Strategy

  How would you like to activate?

    > Use canary deployment (recommended)
      Activate immediately (all traffic)
      Abort

  Selection: Use canary deployment (recommended)

Step 4: Canary Configuration

  Canary percentage: [10] %
  Monitoring duration: [15] minutes
  Auto-promote if healthy? [y/N]: n

Step 5: Confirm

  Activate schema 777888999000111222 with:
  - Canary: 10% of traffic
  - Duration: 15 minutes
  - Manual promotion required

  Proceed? [Y/n]: y

✓ Canary deployment started

  Monitor with: inferadb schemas canary status
  Promote with: inferadb schemas canary promote
  Rollback with: inferadb schemas canary rollback
```

#### Impact Analysis

Before activating, analyze how the schema change affects existing relationships:

```bash
# Show detailed impact analysis
inferadb schemas activate 777888999000111222 --impact-analysis
```

Output:

```text
Schema Impact Analysis: 777888999000111222

Breaking changes detected:
  ! Removed relation 'Document.legacy_viewer'
    → 47 existing relationships will become invalid
    → Affected resources: document:1, document:2, ... (45 more)

  ! Permission 'Document.share' now requires 'edit AND viewer'
    → 12 subjects may lose access

Permission changes:
  ~ 'Document.view' expanded to include 'team_viewer'
    → 156 additional subjects will gain access

Relationship compatibility:
  ✓ 2,341 relationships remain valid
  ! 47 relationships will be orphaned
  ? 12 permission grants need review

Recommendations:
  • Run 'inferadb relationships list --relation legacy_viewer' to review affected relationships
  • Consider migrating relationships before activation:
    inferadb relationships add --file migration.json

Proceed with activation? [y/N]
```

#### Canary Deployment

Gradually roll out schema changes with canary support:

```bash
# Activate for a percentage of traffic
inferadb schemas activate 777888999000111222 --canary 10

# Wait for canary to be fully deployed before returning
inferadb schemas activate 777888999000111222 --canary 10 --wait

# Check canary metrics
inferadb schemas canary status

# Promote canary to full deployment
inferadb schemas canary promote

# Promote and wait for completion
inferadb schemas canary promote --wait

# Rollback canary
inferadb schemas canary rollback
```

Canary status output:

```text
Canary Deployment Status

Schema: 777888999000111222 (10% traffic)
Baseline: 666777888999000111 (90% traffic)
Duration: 15 minutes

Metrics (last 5 min):
                    Canary    Baseline    Delta
  Allow rate:       94.2%     94.1%       +0.1%
  Deny rate:        5.8%      5.9%        -0.1%
  Avg latency:      2.3ms     2.2ms       +0.1ms
  Error rate:       0.01%     0.01%       0.00%

No anomalies detected. Safe to promote.
```

### Pre-flight Check

Run a comprehensive pre-activation safety check that combines validation, diff, impact analysis, and tests in one command:

```bash
# Pre-flight check for a schema version
inferadb schemas pre-flight 777888999000111222

# Pre-flight for local file (validates and compares to active)
inferadb schemas pre-flight schema.ipl

# Output as JSON (for CI/CD integration)
inferadb schemas pre-flight 777888999000111222 -o json

# Skip specific checks
inferadb schemas pre-flight 777888999000111222 --skip-tests
```

Output:

```text
Pre-flight Activation Check: 777888999000111222

Comparing against: active version (666777888999000111)
Vault: Production (987654321098765432)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✓ SCHEMA SYNTAX
  Valid IPL, compiles successfully
  5 entities, 14 relations, 9 permissions

⚠ COMPATIBILITY
  Breaking changes detected:

  1. Removed relation 'Document.legacy_viewer'
     → 47 existing relationships will become invalid
     → Subjects affected: user:alice, user:bob, ... (45 more)
     → Run: inferadb relationships list --relation legacy_viewer

  2. Permission 'Document.share' requirements changed
     → From: edit
     → To: edit AND viewer
     → 12 subjects may lose access

  Non-breaking changes:
  + Added relation: Document.team_viewer
  ~ Modified: Document.view now includes team_viewer

✓ RELATIONSHIPS
  Existing: 2,341
  Remain valid: 2,294 (98.0%)
  Will be orphaned: 47 (2.0%)
  Immediate errors: 0

? PERMISSION IMPACT
  Access changes:
  + 156 subjects will gain access (via team_viewer)
  - 12 subjects may lose access (share permission change)
  = 2,173 subjects unchanged

  Review changes:
    inferadb schemas pre-flight 777888999000111222 --show-access-changes

✓ TESTS
  Running 6 test cases...
  ✓ 6/6 tests passed (142ms)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

STATUS: ⚠ CAUTION REQUIRED

Summary:
  • 2 breaking changes detected
  • 47 relationships will be orphaned
  • 12 subjects may lose access
  • All tests passing

Recommended approach:
  1. Review affected relationships:
     inferadb relationships list --relation legacy_viewer -o json > legacy.json

  2. Migrate relationships before activation:
     # Edit legacy.json to change relation from 'legacy_viewer' to 'viewer'
     inferadb relationships add --file legacy.json

  3. Use canary deployment for safety:
     inferadb schemas activate 777888999000111222 --canary 10

  4. Monitor and promote:
     inferadb schemas canary status
     inferadb schemas canary promote

Alternative: Activate immediately (not recommended with breaking changes)
  inferadb schemas activate 777888999000111222 --yes

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Proceed with activation? [y/N]: _
```

Pre-flight with no issues:

```text
Pre-flight Activation Check: 888999000111222333

...

STATUS: ✓ READY TO ACTIVATE

Summary:
  • No breaking changes
  • All 2,341 relationships remain valid
  • No subjects will lose access
  • All tests passing

This schema is safe to activate:
  inferadb schemas activate 888999000111222333

Or use canary for extra safety:
  inferadb schemas activate 888999000111222333 --canary 10
```

### Rollback Schema

Revert to a previous schema version:

```bash
# Rollback to a specific version
inferadb schemas rollback 777888999000111222

# Rollback to the previous active version
inferadb schemas rollback --previous

# Dry-run: show what would change
inferadb schemas rollback 777888999000111222 --dry-run
```

### Validate Schema

Validate IPL syntax and semantics:

```bash
# Local validation (no server required)
inferadb schemas validate schema.ipl

# Validate against a specific schema version for compatibility
inferadb schemas validate schema.ipl --base 777888999000111222

# Validate with strict mode (warnings are errors)
inferadb schemas validate schema.ipl --strict

# Validate multiple files (modular schemas)
inferadb schemas validate schemas/*.ipl

# Server-side validation (checks against existing relationships)
inferadb schemas validate schema.ipl --server

# Use custom lint configuration
inferadb schemas validate schema.ipl --config .inferadb-lint.yaml
```

Lint configuration file (`.inferadb-lint.yaml`):

```yaml
# Lint rules configuration
rules:
  # Treat unused relations as errors (default: warning)
  unused-relation: error

  # Disable shadowing warnings
  shadowing: off

  # Warn about deep permission inheritance
  max-depth:
    level: warning
    value: 5

  # Require all entities to have at least one permission
  entity-has-permissions: error

  # Warn about redundant relations
  redundant-relation: warning

# Ignore specific patterns
ignore:
  - "legacy_*" # Ignore relations starting with legacy_
```

Output:

```text
✓ schema.ipl is valid

Entities: 5
Relations: 12
Permissions: 8
WASM modules: 1

Warnings:
  schema.ipl:42: Unused relation 'legacy_viewer' on entity 'Document'
```

With `--strict`:

```text
✗ schema.ipl has errors

Errors:
  schema.ipl:42:3: Unused relation 'legacy_viewer' on entity 'Document' [unused-relation]

1 error, 0 warnings
```

### Format Schema

Auto-format IPL files for consistent style:

```bash
# Preview formatted output (no changes)
inferadb schemas format schema.ipl

# Write changes back to file
inferadb schemas format schema.ipl --write

# Check if file is formatted (exit code 1 if not)
inferadb schemas format schema.ipl --check

# Format all IPL files in directory
inferadb schemas format schemas/ --write

# Format with specific options
inferadb schemas format schema.ipl --write --indent 2 --sort-entities
```

### Diff Schemas

Compare schema versions to understand changes:

```bash
# Compare two schema versions by ID
inferadb schemas diff 777888999000111222 888999000111222333

# Compare local file to active schema
inferadb schemas diff schema.ipl --active

# Compare local file to specific version
inferadb schemas diff schema.ipl 777888999000111222

# Compare two local files
inferadb schemas diff old-schema.ipl new-schema.ipl

# Output as JSON for tooling
inferadb schemas diff 777888999000111222 888999000111222333 -o json
```

Output:

```text
Schema Diff: 777888999000111222 → 888999000111222333

+ Added entity: Team
+ Added relation: Document.team_viewer
~ Modified permission: Document.view
  - viewer
  + viewer | team_viewer
- Removed relation: Document.legacy_access

Breaking changes: 1
  - Removing 'Document.legacy_access' may break existing relationships

Non-breaking changes: 3
```

### Analyze Schema

Perform deep analysis on schemas for issues, redundancies, and optimization opportunities:

```bash
# Analyze a schema file
inferadb schemas analyze schema.ipl

# Analyze with specific checks
inferadb schemas analyze schema.ipl --checks unused,cycles,shadowing

# Compare permission changes between versions
inferadb schemas analyze --compare 777888999000111222 888999000111222333
```

Output:

```text
Schema Analysis: schema.ipl

✓ No circular dependencies detected
✓ All relations are reachable
! 2 potential issues found

Issues:
  1. [shadowing] Permission 'view' on Document shadows inherited permission from Folder
     schema.ipl:45

  2. [redundant] Relation 'editor' already implies 'viewer' access
     schema.ipl:32

Suggestions:
  - Consider using 'viewer from editor' instead of duplicating the relation

Complexity metrics:
  Max relation depth: 4
  Entities with WASM: 1
  Estimated evaluation cost: low
```

### Test Schema

Run authorization test cases against your schema. Inspired by [OpenFGA model testing](https://openfga.dev/docs/modeling/testing) and [SpiceDB assertions](https://authzed.com/docs/spicedb/modeling/validation-testing-debugging).

```bash
# Run all tests
inferadb schemas test

# Run tests from specific file
inferadb schemas test tests/schema.test.yaml

# Run tests matching a pattern
inferadb schemas test tests/*.test.yaml

# Run specific test by name
inferadb schemas test --name "editors can edit documents"

# Run tests against a local schema (no server)
inferadb schemas test --schema schema.ipl

# Run tests against live vault
inferadb schemas test --vault 987654321098765432

# Verbose output showing each assertion
inferadb schemas test --verbose

# Output as JSON (for CI integration)
inferadb schemas test -o json
```

Test file format (`schema.test.yaml`):

```yaml
# Test suite name
name: Document permissions

# Schema to test (optional if using --schema flag)
schema: schema.ipl

# Seed relationships for all tests
tuples:
  - user:alice owner document:readme
  - user:bob viewer document:readme
  - user:charlie member group:engineering
  - group:engineering viewer document:internal

# Test cases
tests:
  - name: owners can delete their documents
    check:
      - subject: user:alice
        resource: document:readme
        permission: delete
        expect: allow

      - subject: user:bob
        resource: document:readme
        permission: delete
        expect: deny

  - name: viewers can view but not edit
    check:
      - subject: user:bob
        resource: document:readme
        permission: view
        expect: allow

      - subject: user:bob
        resource: document:readme
        permission: edit
        expect: deny

  - name: group members inherit permissions
    check:
      - subject: user:charlie
        resource: document:internal
        permission: view
        expect: allow

  - name: list documents alice can edit
    list_resources:
      subject: user:alice
      resource_type: document
      permission: edit
      expect:
        - document:readme

  - name: list viewers of internal doc
    list_subjects:
      resource: document:internal
      relation: viewer
      expect:
        - user:bob
        - group:engineering#member

  - name: ABAC conditions are evaluated
    context:
      ip_address: "10.0.1.50"
      time_of_day: "14:00"
    check:
      - subject: user:alice
        resource: document:confidential
        permission: view
        expect: allow

  - name: ABAC denies outside network
    context:
      ip_address: "203.0.113.50"
    check:
      - subject: user:alice
        resource: document:confidential
        permission: view
        expect: deny
```

Output:

```text
Running 6 tests from tests/schema.test.yaml

✓ owners can delete their documents (2 assertions)
✓ viewers can view but not edit (2 assertions)
✓ group members inherit permissions (1 assertion)
✓ list documents alice can edit (1 assertion)
✓ list viewers of internal doc (1 assertion)
✗ ABAC conditions are evaluated (1 assertion)
  Expected: allow
  Actual: deny
  Context: {"ip_address": "10.0.1.50", "time_of_day": "14:00"}

  Hint: Check that 'time_of_day' is being parsed correctly in your WASM module

5 passed, 1 failed
```

#### Watch Mode for Tests

Automatically re-run tests when schema or test files change:

```bash
# Watch and re-run tests on file changes
inferadb schemas test --watch

# Watch specific files
inferadb schemas test --watch --schema schema.ipl --tests tests/*.test.yaml

# Watch with verbose output
inferadb schemas test --watch --verbose

# Watch with sound notification on failure
inferadb schemas test --watch --notify
```

Watch mode output:

```text
InferaDB Test Watch Mode
Schema: schema.ipl
Tests: tests/schema.test.yaml
Press Ctrl+C to exit, 'r' to re-run, 'q' to quit

──────────────────────────────────────────────────────────────────────
[10:30:15] Watching for changes...

[10:30:22] File changed: schema.ipl
Running 6 tests...
  ✓ 6/6 tests passed (142ms)

[10:31:05] File changed: tests/schema.test.yaml
Running 7 tests...
  ✗ 6/7 tests passed (156ms)

  FAIL: new permission test case
    Expected: allow
    Actual: deny

    Hint: Review the permission definition for 'can_share'

[10:31:45] File changed: schema.ipl
Running 7 tests...
  ✓ 7/7 tests passed (148ms)

──────────────────────────────────────────────────────────────────────
```

### Visualize Schema

Generate visual representations of your schema:

```bash
# Output as Mermaid diagram
inferadb schemas visualize schema.ipl -o mermaid > schema.mmd

# Output as DOT (Graphviz)
inferadb schemas visualize schema.ipl -o dot > schema.dot

# Output as ASCII (terminal-friendly)
inferadb schemas visualize schema.ipl -o ascii

# Show only specific entity
inferadb schemas visualize schema.ipl --entity Document

# Show permission inheritance
inferadb schemas visualize schema.ipl --show-permissions
```

ASCII output:

```text
Schema: inferadb v1.0

┌─────────────────────────────────────────────────────┐
│ User                                                │
├─────────────────────────────────────────────────────┤
│ attributes: id, email, department, roles            │
│ methods: has_clearance(level)                       │
└─────────────────────────────────────────────────────┘
         │
         │ owner/editor/viewer
         ▼
┌─────────────────────────────────────────────────────┐
│ Document                                            │
├─────────────────────────────────────────────────────┤
│ relations: owner→User, editor→User|Group#member,   │
│            viewer→User|Group#member|editor,        │
│            parent_folder→Folder                    │
│ permissions: delete←owner, edit←editor,            │
│              view←viewer, share←edit∧viewer        │
└─────────────────────────────────────────────────────┘
         │
         │ parent_folder
         ▼
┌─────────────────────────────────────────────────────┐
│ Folder                                              │
└─────────────────────────────────────────────────────┘
```

### Watch Schema Changes

Watch for schema file changes and auto-validate:

```bash
# Watch and validate on change
inferadb schemas watch schema.ipl

# Watch and run tests on change
inferadb schemas watch schema.ipl --test

# Watch directory
inferadb schemas watch schemas/
```

### Develop (Unified Dev Workflow)

A single command that combines watch, validate, test, and visualization for rapid schema iteration:

```bash
# Start development mode
inferadb schemas develop

# With specific schema file
inferadb schemas develop schema.ipl

# With test file
inferadb schemas develop schema.ipl --tests tests/schema.test.yaml

# Auto-push after tests pass
inferadb schemas develop schema.ipl --auto-push

# Auto-activate if no breaking changes
inferadb schemas develop schema.ipl --auto-push --auto-activate-if-safe

# Target a specific vault for auto-push
inferadb schemas develop schema.ipl --auto-push --vault 987654321098765432
```

This starts an interactive development session:

```text
InferaDB Schema Development Mode
Schema: schema.ipl
Tests: tests/schema.test.yaml
Press 'q' to quit, 'r' to reload, 't' to run tests, 'v' to visualize

──────────────────────────────────────────────────────────────────────
[10:30:15] Watching schema.ipl for changes...

[10:30:22] File changed: schema.ipl
  ✓ Syntax valid
  ✓ 5 entities, 12 relations, 8 permissions
  ⚠ 1 warning: Unused relation 'legacy_viewer'

[10:30:22] Running tests...
  ✓ 6/6 tests passed

[10:30:45] File changed: schema.ipl
  ✗ Syntax error at line 42:
    Expected '}' but found 'permission'

[10:31:02] File changed: schema.ipl
  ✓ Syntax valid
  ✓ Schema ready

──────────────────────────────────────────────────────────────────────
Commands:
  r  Reload and revalidate
  t  Run tests
  v  Show visualization (ASCII)
  d  Show diff from active schema
  p  Push to vault (draft)
  q  Quit
```

With `--auto-push`:

```text
InferaDB Schema Development Mode
Schema: schema.ipl
Tests: tests/schema.test.yaml
Auto-push: enabled (vault: Production)
Press 'q' to quit, 'r' to reload, 'a' to toggle auto-push

──────────────────────────────────────────────────────────────────────
[10:30:22] File changed: schema.ipl
  ✓ Syntax valid
  ✓ 5 entities, 12 relations, 8 permissions

[10:30:22] Running tests...
  ✓ 6/6 tests passed

[10:30:23] 🚀 Auto-pushing schema...
  ✓ Schema pushed (id: 777888999000111222)

[10:30:24] What would you like to do?
  [a] Activate now
  [c] Activate with canary (10%)
  [n] Don't activate (keep as draft)
  [q] Quit

Selection: c
  ✓ Canary deployment started (10% traffic)
  Monitor: inferadb schemas canary status

──────────────────────────────────────────────────────────────────────
```

With `--auto-activate-if-safe`:

```text
InferaDB Schema Development Mode
Schema: schema.ipl
Auto-push: enabled
Auto-activate: if safe (no breaking changes)

──────────────────────────────────────────────────────────────────────
[10:30:22] File changed: schema.ipl
  ✓ Syntax valid
  ✓ 6/6 tests passed

[10:30:23] 🚀 Auto-pushing schema...
  ✓ Schema pushed (id: 777888999000111222)

[10:30:24] Checking for breaking changes...
  ✓ No breaking changes detected
  ✓ All relationships remain valid

[10:30:25] ✓ Auto-activating schema...
  ✓ Schema 777888999000111222 is now active

──────────────────────────────────────────────────────────────────────

[10:35:45] File changed: schema.ipl
  ✓ Syntax valid
  ✓ 6/6 tests passed

[10:35:46] 🚀 Auto-pushing schema...
  ✓ Schema pushed (id: 888999000111222333)

[10:35:47] Checking for breaking changes...
  ⚠ Breaking changes detected:
    - Removed relation 'Document.legacy_viewer' (47 relationships)

[10:35:47] ⏸ Auto-activation skipped (breaking changes)
  Schema pushed but not activated.

  To activate manually:
    inferadb schemas activate 888999000111222333 --canary 10

──────────────────────────────────────────────────────────────────────
```

Key features:

- **Auto-reload**: Validates on every file save
- **Live test results**: Re-runs tests automatically
- **Quick actions**: Single-key commands for common operations
- **Diff preview**: See changes vs. active schema before pushing
- **Draft push**: Push without activating for review
- **Auto-push**: Automatically push after tests pass
- **Auto-activate-if-safe**: Only activate when no breaking changes detected

### Copy Schema

Copy a schema from one vault to another:

```bash
# Copy active schema from source vault to target vault
inferadb schemas copy --from-vault 111222333444555666 --to-vault 222333444555666777

# Copy specific schema version
inferadb schemas copy 777888999000111222 --to-vault 222333444555666777

# Copy and activate immediately
inferadb schemas copy 777888999000111222 --to-vault 222333444555666777 --activate

# Copy across organizations (requires access to both)
inferadb schemas copy 777888999000111222 \
  --from-org 111222333444555666 \
  --to-org 222333444555666777 \
  --to-vault 333444555666777888

# Dry-run
inferadb schemas copy 777888999000111222 --to-vault 222333444555666777 --dry-run
```

### Migrate Schema (Helper)

Generate migration commands for schema changes:

```bash
# Generate migration plan between two versions
inferadb schemas migrate --from 777888999000111222 --to 888999000111222333

# Generate migration from active schema to local file
inferadb schemas migrate --to schema.ipl
```

Output:

```text
Schema Migration Plan: 777888999000111222 → 888999000111222333

Breaking changes detected:
  1. Removed relation 'Document.legacy_viewer'
     → 47 relationships use this relation

  2. Renamed relation 'Document.can_view' → 'Document.viewer'
     → 156 relationships need updating

Generated migration commands:

# Step 1: Export affected relationships
inferadb relationships list --relation legacy_viewer -o json > legacy_viewer.json
inferadb relationships list --relation can_view --resource-type document -o json > can_view.json

# Step 2: Transform relationships (edit files to change relation names)
# legacy_viewer.json → delete these or migrate to 'viewer'
# can_view.json → change relation from 'can_view' to 'viewer'

# Step 3: Add new relationships
inferadb relationships add --file can_view_migrated.json

# Step 4: Delete old relationships
inferadb relationships delete --filter --relation legacy_viewer --yes
inferadb relationships delete --filter --relation can_view --resource-type document --yes

# Step 5: Activate new schema
inferadb schemas activate 888999000111222333

Save this plan? [migration-2025-01-15.md]:
```

---

## JWKS (JSON Web Key Sets)

Retrieve public keys for token verification (useful for debugging and external integrations).

### Get Service JWKS

```bash
# Get the service-level JWKS (Control plane signing keys)
inferadb jwks

# Output as formatted JSON
inferadb jwks -o json
```

### Get Organization JWKS

```bash
# Get organization-specific JWKS (for vault tokens)
inferadb jwks --org 123456789012345678

# Using org from current profile
inferadb jwks --org
```

Output:

```json
{
  "keys": [
    {
      "kty": "OKP",
      "crv": "Ed25519",
      "kid": "ctrl-2025-01",
      "use": "sig",
      "x": "..."
    }
  ]
}
```

---

---

<!-- ═══════════════════════════════════════════════════════════════════════════
     PART 7: AUTHORIZATION & RELATIONSHIPS
     ═══════════════════════════════════════════════════════════════════════════ -->

## Authorization Queries

All authorization commands use the vault from your current profile by default, or accept `--vault` flag to override.

### Understanding Argument Order

Authorization commands use different argument orders depending on the *question being asked*:

| Question                        | Command                                       | Order                       |
| ------------------------------- | --------------------------------------------- | --------------------------- |
| "Can Alice view this document?" | `check user:alice can_view document:readme`   | subject permission resource |
| "Who can view this document?"   | `list-subjects document:readme viewer`        | resource relation           |
| "What can Alice view?"          | `list-resources user:alice can_view document` | subject permission type     |
| "Show all viewers"              | `expand document:readme viewer`               | resource relation           |

**The principle**: The *focus* of your question comes first.

- **Subject-centric** ("Can Alice..."): subject first → `check`, `list-resources`
- **Resource-centric** ("Who can access..."): resource first → `expand`, `list-subjects`

#### Natural Language Aliases

For more intuitive commands, use these aliases:

```bash
# These are equivalent:
inferadb list-subjects document:readme viewer
inferadb who-can document:readme viewer          # Natural English alias

# These are equivalent:
inferadb list-resources user:alice can_view document
inferadb what-can user:alice can_view document   # Natural English alias
```

#### Cognitive Map

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Authorization Query Decision Tree                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  "I want to know..."                                                        │
│      │                                                                      │
│      ├─► "if a SPECIFIC USER has access"                                    │
│      │       └─► check user:X permission resource:Y                         │
│      │           (subject permission resource)                              │
│      │                                                                      │
│      ├─► "what a USER can access"                                           │
│      │       └─► list-resources user:X permission type                      │
│      │           └─► alias: what-can user:X permission type                 │
│      │                                                                      │
│      ├─► "who can access a RESOURCE"                                        │
│      │       └─► list-subjects resource:Y relation                          │
│      │           └─► alias: who-can resource:Y relation                     │
│      │                                                                      │
│      └─► "the full access tree for a RESOURCE"                              │
│              └─► expand resource:Y relation                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Check Permission

```bash
# Basic check: subject permission resource
inferadb check user:alice can_view document:readme

# With explicit vault
inferadb check user:alice can_view document:readme --vault 987654321098765432

# With debug trace
inferadb check user:alice can_view document:readme --trace

# With ABAC context (JSON for attribute-based conditions)
# Note: This is authorization context (IP, time, etc.), not a CLI profile/context
inferadb check user:alice can_view document:readme --context '{"ip_address": "192.168.1.1"}'

# Output formats
inferadb check user:alice can_view document:readme -o json
inferadb check user:alice can_view document:readme -o yaml

# Check multiple permissions at once
inferadb check user:alice can_view,can_edit,can_delete document:readme

# Silent mode with exit code (useful for scripts)
inferadb check user:alice can_view document:readme --quiet --exit-code
# Exit 0 = allowed, Exit 1 = denied

# Explain why access was denied (human-friendly)
inferadb check user:alice can_view document:readme --explain
```

Output:

```text
✓ ALLOW: user:alice can_view document:readme
```

With `--trace`:

```text
✓ ALLOW: user:alice can_view document:readme

Evaluation trace:
  document:readme#can_view
  ├── document:readme#viewer (MATCH)
  │   └── user:alice (direct grant)
  └── document:readme#editor
      └── (no matches)

Stats:
  Duration: 2.3ms
  Relationships read: 3
  Relations evaluated: 2
```

With `--explain` (on denial):

```text
✗ DENY: user:alice can_view document:secret

Why access was denied:
  • user:alice is not a direct 'viewer' of document:secret
  • user:alice is not an 'editor' of document:secret
  • user:alice is not a member of any group with 'viewer' access
  • No inherited permissions from parent resources

To grant access, you could:
  inferadb relationships add user:alice viewer document:secret
```

### Batch Check

```bash
# From file
inferadb check --batch checks.json

# From stdin
cat checks.json | inferadb check --batch -

# With parallel execution
inferadb check --batch checks.json --parallel 10

# With progress bar (for large batches)
inferadb check --batch checks.json --progress
```

`checks.json` format:

```json
{
  "checks": [
    {
      "subject": "user:alice",
      "permission": "can_view",
      "resource": "document:1"
    },
    {
      "subject": "user:alice",
      "permission": "can_view",
      "resource": "document:2"
    },
    {
      "subject": "user:bob",
      "permission": "can_edit",
      "resource": "document:1"
    }
  ]
}
```

#### Progressive Output

For large batch operations, use `--progress` to see real-time status:

```bash
inferadb check --batch checks.json --progress
```

Output:

```text
[████████████░░░░░░░░░░░░░░░░░░] 42% (420/1000) | 12ms avg | 15 denied

Summary:
  ✓ 405 allowed
  ✗ 15 denied
  ⏳ 580 remaining

Estimated time: 7s
```

On completion:

```text
[██████████████████████████████] 100% (1000/1000)

Batch check complete:
  ✓ 962 allowed (96.2%)
  ✗ 38 denied (3.8%)

Duration: 12.3s
Throughput: 81 checks/second
```

Failed checks are listed at the end:

```text
Denied checks:
  user:bob can_edit document:secret
  user:charlie can_delete folder:root
  ... (36 more)

Use --output denied.json to export denied checks.
```

### Simulate (What-If Testing)

Test authorization with ephemeral relationships:

```bash
# Inline relationships: subject permission resource, with ephemeral tuples
inferadb simulate user:alice can_view document:secret \
  --with "user:alice viewer document:secret" \
  --with "document:secret parent folder:confidential"

# From file
inferadb simulate --scenario scenario.yaml
```

`scenario.yaml` format:

```yaml
context_relationships:
  - subject: user:alice
    relation: viewer
    resource: document:secret
  - subject: document:secret
    relation: parent
    resource: folder:confidential
check:
  subject: user:alice
  permission: can_view
  resource: document:secret
```

### Expand Userset

Show all subjects who have a relation on a resource.

**Argument Order**: `resource relation` — This differs from check/relationship commands because expand answers "who has access to this resource?" rather than "does this subject have access?"

```bash
# resource relation (note: resource comes first)
inferadb expand document:readme viewer

# With depth limit
inferadb expand document:readme viewer --depth 3

# Limit graph traversal for performance
inferadb expand document:readme viewer --max-depth 5

# Output as tree (default)
inferadb expand document:readme viewer -o tree

# Output as JSON
inferadb expand document:readme viewer -o json
```

Output:

```text
document:readme#viewer
├── user:alice (direct)
├── user:bob (direct)
└── group:engineering#member
    ├── user:charlie
    └── user:dave
```

**Why the different order?** The `expand` command walks the graph starting from a resource and discovers all subjects. The argument order reflects this: you specify "this resource, this relation" and get back all subjects. In contrast, `check` asks "does subject X have permission Y on resource Z?" — a subject-centric question.

### List Resources

Find all resources a subject can access:

```bash
# subject permission resource_type
inferadb list-resources user:alice can_view document

# With pagination
inferadb list-resources user:alice can_view document --limit 100
inferadb list-resources user:alice can_view document --limit 100 --cursor eyJvZmZzZXQiOjEwMH0=

# Limit traversal depth for performance
inferadb list-resources user:alice can_view document --max-depth 5

# Output as JSON
inferadb list-resources user:alice can_view document -o json
```

### List Subjects

Find all subjects who have access to a resource.

**Argument Order**: `resource relation` — Like `expand`, this is resource-centric: "who can access this resource?"

```bash
# resource relation (note: resource comes first)
inferadb list-subjects document:readme viewer

# Filter by subject type
inferadb list-subjects document:readme viewer --subject-type user

# Limit traversal depth
inferadb list-subjects document:readme viewer --max-depth 5

# With pagination
inferadb list-subjects document:readme viewer --limit 100
inferadb list-subjects document:readme viewer --limit 100 --cursor eyJvZmZzZXQiOjEwMH0=
```

### Explain Permission

Deep-dive into why a permission exists. Goes beyond `check --explain` to provide full schema context, alternative paths, and actionable insights.

```bash
# Why does alice have view access?
inferadb explain-permission user:alice can_view document:readme

# Why can't bob edit this document?
inferadb explain-permission user:bob can_edit document:confidential-report

# Show all possible paths (not just the first match)
inferadb explain-permission user:alice can_view document:readme --all-paths

# Visual graph output (ASCII tree diagram)
inferadb explain-permission user:alice can_view document:readme --graph
```

Output (permission granted):

```text
PERMISSION EXPLANATION

user:alice CAN can_view document:readme

RESOLUTION PATH (fastest match):
  document:readme
    └─ viewer: user:alice (direct relationship) ✓

ALTERNATIVE PATHS (2 more):
  document:readme
    └─ editor: user:alice ✓
       └─ (editor implies viewer per schema)

  document:readme
    └─ parent_folder: folder:engineering
       └─ viewer: group:eng#member
          └─ member: user:alice ✓

SCHEMA CONTEXT:
  Permission 'can_view' is defined in: user-permissions v3
  Definition: viewer when { ... conditions ... }

  entity Document {
    permissions {
      view: viewer when {
        match resource.classification {
          "public" => true
          "internal" => principal.is_employee
          "confidential" => { ... }
        }
      }
    }
  }

ATTRIBUTE CONDITIONS:
  resource.classification = "internal"
  principal.is_employee = true (required, satisfied)

TIMING:
  Resolution: 3.2ms
  Paths explored: 4
  Cache: miss (first query)

TIP: To remove this access, delete: document:readme#viewer@user:alice
     Command: inferadb relationships remove user:alice viewer document:readme
```

Output (permission denied):

```text
PERMISSION EXPLANATION

user:bob CANNOT can_edit document:confidential-report

WHY DENIED:
  ✗ No 'editor' relationship found for user:bob on document:confidential-report
  ✗ No group membership that grants editor access
  ✗ No inherited access from parent folder

CLOSEST MATCHES:
  user:bob HAS viewer on document:confidential-report
    └─ viewer does NOT imply editor (one-way relationship)

  user:bob IS member of group:marketing
    └─ group:marketing has NO access to document:confidential-report

WHAT WOULD GRANT ACCESS:
  Option 1: Direct relationship
    inferadb relationships add user:bob editor document:confidential-report

  Option 2: Via group (if bob should have same access as other editors)
    inferadb relationships add group:editors#member user:bob
    # (if group:editors already has editor access)

  Option 3: Via folder inheritance
    # Add to parent folder:
    inferadb relationships add user:bob editor folder:confidential

SCHEMA REQUIREMENTS:
  Permission 'can_edit' requires:
    - editor relationship, AND
    - resource.classification != "confidential" OR principal.has_clearance("confidential")

  user:bob clearance_levels = ["internal"]  (missing "confidential")

  ⚠ Even with editor relationship, bob would need "confidential" clearance
    to edit this document.
```

Interactive mode for complex investigations:

```bash
inferadb explain-permission user:alice can_view document:readme --interactive
```

```text
PERMISSION EXPLANATION

user:alice CAN can_view document:readme (via editor)

Interactive mode - enter commands or 'quit':

explain> what if we remove editor?
  Simulating: remove user:alice editor document:readme
  Result: user:alice would STILL have can_view
  Reason: inherited from folder:engineering#viewer

explain> what if we remove both?
  Simulating: remove user:alice editor document:readme
             AND remove folder:engineering viewer group:eng
  Result: user:alice would LOSE can_view
  Warning: This would also affect 12 other users

explain> show all affected users
  Users who would lose access:
    - user:alice (direct + group)
    - user:bob (group only)
    - user:charlie (group only)
    ... 9 more

explain> quit
```

Graph visualization mode (`--graph`):

```bash
inferadb explain-permission user:alice can_view document:readme --graph
```

```text
ACCESS GRAPH: user:alice → can_view → document:readme

user:alice
  │
  ├─── direct ─────────────────────────────────────────────┐
  │    └─ viewer on document:readme ✓                      │
  │                                                        │
  ├─── via role ───────────────────────────────────────────┤
  │    └─ editor on document:readme ✓                      │
  │       └─ (editor implies viewer)                       │
  │                                                        │
  └─── via group ──────────────────────────────────────────┤
       └─ member of group:engineering                      │
          └─ viewer on folder:docs                         │
             └─ parent of document:readme ✓                │
                                                           │
                                                           ▼
                                              document:readme [can_view ✓]

Legend: ✓ = grants access, ✗ = blocked, ─ = relationship path
```

For denied permissions, the graph shows where paths fail:

```bash
inferadb explain-permission user:bob can_edit document:secret --graph
```

```text
ACCESS GRAPH: user:bob → can_edit → document:secret

user:bob
  │
  ├─── direct ─────────────────────────────────────────────┐
  │    └─ (no editor relationship) ✗                       │
  │                                                        │
  ├─── via role ───────────────────────────────────────────┤
  │    └─ viewer on document:secret                        │
  │       └─ (viewer does NOT imply editor) ✗              │
  │                                                        │
  └─── via group ──────────────────────────────────────────┤
       └─ member of group:marketing                        │
          └─ (no access to document:secret) ✗              │
                                                           │
                                                           ▼
                                              document:secret [can_edit ✗]

BLOCKED BY: No valid path grants 'editor' relationship
SUGGESTION: inferadb relationships add user:bob editor document:secret
```

---

## Relationship Management

**Argument Order**: All relationship commands follow **Zanzibar tuple order**: `subject relation resource`.

### List Relationships

```bash
# List all relationships in vault
inferadb relationships list

# Filter by resource
inferadb relationships list --resource document:readme

# Filter by relation
inferadb relationships list --relation viewer

# Filter by subject
inferadb relationships list --subject user:alice

# Combine filters
inferadb relationships list --resource document:readme --relation viewer

# With pagination
inferadb relationships list --limit 100 --cursor eyJvZmZzZXQiOjEwMH0=

# Filter by time (useful for auditing and sync)
inferadb relationships list --changed-since 2025-01-15T00:00:00Z
inferadb relationships list --changed-since 24h  # Relative time
```

### Relationship History

View the change history for relationships (requires audit logging enabled):

```bash
# History for a specific relationship (subject relation resource)
inferadb relationships history user:alice viewer document:readme

# History for a resource
inferadb relationships history --resource document:readme

# History for a subject
inferadb relationships history --subject user:alice

# Filter by time range
inferadb relationships history --resource document:readme \
  --from 2025-01-01T00:00:00Z \
  --to 2025-01-31T23:59:59Z

# Include who made the change
inferadb relationships history --resource document:readme --show-actor
```

Output:

```text
Relationship history for document:readme

TIME                      ACTION   SUBJECT      RELATION   ACTOR
2025-01-15T10:30:00Z      CREATE   user:alice   viewer     user:admin
2025-01-14T09:15:00Z      DELETE   user:bob     editor     user:admin
2025-01-10T14:22:00Z      CREATE   user:bob     editor     system:import
2025-01-05T11:00:00Z      CREATE   user:alice   owner      user:alice
```

### Add Relationships

```bash
# Add single relationship (subject relation resource)
inferadb relationships add user:alice viewer document:readme

# Add multiple relationships
inferadb relationships add \
  "user:alice viewer document:readme" \
  "user:bob editor document:readme"

# From file
inferadb relationships add --file relationships.json

# From stdin
cat relationships.json | inferadb relationships add --file -

# Idempotent add (no error if already exists)
inferadb relationships add user:alice viewer document:readme --if-not-exists

# With preview confirmation (default for interactive terminals)
inferadb relationships add user:alice viewer document:readme --preview

# Skip preview (for scripts)
inferadb relationships add user:alice viewer document:readme --no-preview
```

#### Relationship Preview

By default, the CLI shows a preview before adding relationships in interactive mode:

```bash
inferadb relationships add user:alice viewer document:readme
```

Output:

```text
Relationship Preview

  Subject:  user:alice
  Relation: viewer
  Resource: document:readme

This grants:
  • user:alice can view document:readme
  • Inherits: can_view permission (via viewer relation)

Schema validation: ✓ Valid
  Entity: Document
  Relation: viewer accepts User | Group#member

Add this relationship? [Y/n]: y
✓ Relationship added
```

For batch additions:

```bash
inferadb relationships add --file relationships.json
```

Output:

```text
Batch Relationship Preview

Source: relationships.json
Count: 47 relationships

Resource breakdown:
  document: 23 relationships
  folder:   18 relationships
  team:     6 relationships

Relation breakdown:
  viewer: 25
  editor: 15
  owner:  7

Sample relationships:
  user:alice owner document:readme
  user:bob viewer document:readme
  group:engineering member team:platform
  ... (44 more)

Schema validation: ✓ All valid

Add all 47 relationships? [Y/n]: y

Adding relationships...
[██████████████████████████████] 100% (47/47)

✓ 47 relationships added
  Duration: 1.2s
  Throughput: 39 relationships/second
```

Suppress preview in scripts:

```bash
# Skip preview
inferadb relationships add user:alice viewer document:readme --no-preview

# Or use --yes for batch
inferadb relationships add --file relationships.json --yes
```

`relationships.json` format:

```json
{
  "relationships": [
    {
      "subject": "user:alice",
      "relation": "viewer",
      "resource": "document:readme"
    },
    {
      "subject": "user:bob",
      "relation": "editor",
      "resource": "document:readme"
    }
  ]
}
```

### Delete Relationships

```bash
# Delete single relationship (subject relation resource)
inferadb relationships delete user:alice viewer document:readme

# Idempotent delete (no error if doesn't exist)
inferadb relationships delete user:alice viewer document:readme --if-exists

# Delete with filter (requires confirmation)
inferadb relationships delete --filter --subject user:alice
inferadb relationships delete --filter --resource document:readme
inferadb relationships delete --filter --relation viewer

# Dry-run (show what would be deleted)
inferadb relationships delete --filter --subject user:alice --dry-run

# Safety limit (default: 1000, set 0 for unlimited)
inferadb relationships delete --filter --subject user:alice --limit 500

# Skip confirmation
inferadb relationships delete --filter --subject user:alice --yes

# From file
inferadb relationships delete --file relationships.json
```

Dry-run output:

```text
Would delete 47 relationships:
  user:alice viewer document:1
  user:alice viewer document:2
  user:alice editor document:3
  ... (44 more)

Run with --yes to confirm deletion.
```

### Validate Relationships

Validate that relationships conform to the active schema before adding them:

```bash
# Validate a single relationship against the schema
inferadb relationships validate user:alice viewer document:readme

# Validate multiple relationships
inferadb relationships validate \
  "user:alice viewer document:readme" \
  "user:bob invalid_relation document:readme"

# Validate from file
inferadb relationships validate --file relationships.json

# Validate against a specific schema version (not active)
inferadb relationships validate user:alice viewer document:readme --schema 777888999000111222
```

Output (success):

```text
✓ user:alice viewer document:readme
  Entity: Document
  Relation: viewer (User | Group#member | editor)
```

Output (failure):

```text
✗ user:bob invalid_relation document:readme
  Error: Relation 'invalid_relation' not defined on entity 'Document'

  Available relations for Document:
    • owner (User)
    • editor (User | Group#member)
    • viewer (User | Group#member | editor)
    • parent_folder (Folder)

✗ group:engineering member user:alice
  Error: Relation 'member' on 'Group' expects resource type 'User', got 'user:alice'

  Hint: The relation direction may be reversed. Did you mean:
    user:alice member group:engineering
```

Batch validation output:

```text
Validating 100 relationships...

✓ 97 valid
✗ 3 invalid

Errors:
  Line 23: user:bob invalid_relation document:readme
           Relation 'invalid_relation' not defined on entity 'Document'

  Line 45: folder:123 parent folder:123
           Self-referential relationship not allowed for 'parent'

  Line 78: user:charlie editor document:missing_type
           Unknown entity type 'missing_type'
```

---

## Stream

Stream real-time relationship changes:

```bash
# Stream all changes
inferadb stream

# Stream specific resource types
inferadb stream --resource-types document,folder

# Resume from cursor
inferadb stream --cursor eyJyZXZpc2lvbiI6MTIzfQ==

# Output as JSON lines
inferadb stream -o jsonl
```

Output:

```text
[2025-01-15T10:30:00Z] CREATE user:alice viewer document:readme
[2025-01-15T10:30:01Z] DELETE user:bob editor document:readme
[2025-01-15T10:30:02Z] CREATE group:engineering member user:charlie
```

---

## Token Management

Generate and manage vault access tokens for use in scripts or other tools.

### Generate Token

```bash
# Generate token for vault in current profile
inferadb tokens generate

# With specific role
inferadb tokens generate --role writer

# With custom TTL
inferadb tokens generate --ttl 1h

# Output as environment variable
inferadb tokens generate --output env
# export INFERADB_TOKEN="eyJhbGciOiJFZERTQSI..."

# Output just the token
inferadb tokens generate --output token
```

### List Tokens

```bash
inferadb tokens list
inferadb tokens list --vault 987654321098765432
```

### Revoke Token

```bash
inferadb tokens revoke 888999000111222333
```

### Refresh Token

```bash
# Refresh an access token using a refresh token
inferadb tokens refresh --refresh-token {refresh-token}
```

### Inspect Token

Decode and display token contents for debugging (does not verify signature):

```bash
# Inspect a token
inferadb tokens inspect eyJhbGciOiJFZERTQSI...

# Inspect from environment variable
inferadb tokens inspect $INFERADB_TOKEN

# Inspect from file
inferadb tokens inspect --file token.txt

# Output as JSON
inferadb tokens inspect eyJhbGciOiJFZERTQSI... -o json

# Verify signature against JWKS (requires network)
inferadb tokens inspect eyJhbGciOiJFZERTQSI... --verify

# Check if token is valid (for scripts) - exit 0 if valid, 1 if expired/revoked
inferadb tokens inspect eyJhbGciOiJFZERTQSI... --check-valid

# Auto-refresh if close to expiry
inferadb tokens inspect eyJhbGciOiJFZERTQSI... --refresh
```

Output:

```text
Token Inspection

Token Details:
  Format: JWT (EdDSA)
  Key ID: ctrl-2025-01
  Subject: user:222333444555666777
  Issued: 2025-01-15T08:30:00Z (5 hours ago)
  Expires: 2025-01-16T08:30:00Z (in 19 hours)
  Issuer: https://api.inferadb.com

Scope:
  Organization: 123456789012345678 (Acme Corp)
  Vault: 987654321098765432 (Production)
  Role: writer

Permissions:
  • read relationships
  • write relationships
  • read schemas

Claimed Identity:
  User: alice@acme.com
  Email verified: yes

Security:
  ✓ Token is not expired
  ✓ Token is valid for vault in current profile
  ✓ Not revoked
  ⚠ Token age: 5 hours (rotate recommended after 24h)
```

With `--verify` (requires network):

```text
Token Inspection

...

Signature Verification:
  ✓ Signature valid
  ✓ Signed by: ctrl-2025-01 (Control Plane)
  ✓ Key not revoked
  ✓ Certificate chain valid

Usage Statistics:
  Last used: 2025-01-15T13:25:00Z (5 minutes ago)
  Requests: 147
  First seen: 2025-01-15T08:32:00Z
```

On expired/invalid token:

```text
Token Inspection

...

Security:
  ✗ Token expired at 2025-01-14T08:30:00Z (2 days ago)

Suggestions:
  • Generate a new token: inferadb tokens generate
  • Re-authenticate: inferadb login
```

On token expiring soon:

```text
Token Inspection

...

Security:
  ⚠ Token expires in 25 minutes
  ✓ Not revoked

Suggestions:
  • Refresh now: inferadb tokens refresh
  • Or re-run with --refresh to auto-refresh
```

---

## Bulk Operations

### Export

```bash
# Export all relationships from vault
inferadb export > relationships.json

# Export to file
inferadb export --output relationships.json

# Export specific resource types
inferadb export --resource-types document,folder

# Export with schema
inferadb export --include-schema

# Export changes since a point in time (incremental backup)
inferadb export --changed-since 2025-01-15T00:00:00Z

# Export with metadata (created_at, updated_by, etc.)
inferadb export --include-metadata
```

### Import

```bash
# Import relationships (dry-run by default, shows preview)
inferadb import relationships.json

# Actually import
inferadb import relationships.json --yes

# Import from stdin
cat relationships.json | inferadb import -

# Import modes
inferadb import relationships.json --mode merge --yes    # Add new, skip existing (default)
inferadb import relationships.json --mode replace --yes  # Clear vault first, then import
inferadb import relationships.json --mode upsert --yes   # Add new, update existing

# Conflict resolution
inferadb import relationships.json --on-conflict skip    # Skip conflicts (default)
inferadb import relationships.json --on-conflict error   # Fail on first conflict
inferadb import relationships.json --on-conflict report  # Continue but report all conflicts

# Transaction behavior (see Atomicity Guarantees below)
inferadb import relationships.json --atomic --yes        # All-or-nothing (rollback on error)
inferadb import relationships.json --continue-on-error   # Import as many as possible
inferadb import relationships.json --skip-duplicates     # Treat duplicates as success (idempotent)

# Progress and reporting
inferadb import relationships.json --yes --progress      # Show progress bar
inferadb import relationships.json --yes --report        # Generate detailed report

# Wait for async import to complete (for large imports)
inferadb import relationships.json --yes --async         # Returns job ID immediately
inferadb import relationships.json --yes --async --wait  # Wait for completion
```

Import dry-run output:

```text
Import preview for relationships.json

Source: 1,247 relationships
Vault: 987654321098765432 (Production)

Changes:
  + 892 new relationships
  ~ 0 updates (mode: merge)
  - 0 deletions
  = 355 unchanged (skipped)

Conflicts: 0

Run with --yes to apply these changes.
Run with --mode replace to clear existing data first.
```

Import report (with `--report`):

```text
Import complete

Duration: 2.3s
Relationships processed: 1,247
  Created: 892
  Skipped: 355
  Failed: 0

Throughput: 542 relationships/second
```

### Interactive Conflict Resolution

For complex imports with conflicts, use interactive mode to resolve issues one-by-one:

```bash
# Interactive mode - resolve conflicts as they arise
inferadb import relationships.json --interactive
```

Output:

```text
Import preview for relationships.json

Source: 1,247 relationships
Vault: 987654321098765432 (Production)

Initial scan complete:
  + 892 new relationships (will create)
  = 347 unchanged (will skip)
  ⚠ 8 conflicts detected

Starting interactive conflict resolution...

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
CONFLICT 1/8: Duplicate relationship with different attributes

EXISTING:
  user:alice editor document:readme
  Created: 2025-01-10T09:00:00Z by admin@acme.com
  Condition: context.ip_address in_cidr "10.0.0.0/8"

IMPORTING:
  user:alice editor document:readme
  Condition: (none)

Difference: Existing has IP restriction, import would remove it

? How do you want to resolve this conflict?
  [k] Keep existing (skip this import)
  [r] Replace with import
  [m] Merge (keep existing condition)
  [e] Edit manually
  [s] Skip all similar conflicts
  [a] Abort import
> _
```

User selects `m` (merge):

```text
✓ Kept existing relationship with conditions intact

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
CONFLICT 2/8: Schema mismatch

IMPORTING:
  user:charlie admin document:secret

Problem: Relation 'admin' does not exist in active schema 'user-permissions v3'
         Similar relations: 'owner', 'editor'

? How do you want to resolve this conflict?
  [1] Map 'admin' → 'owner' for this relationship
  [2] Map 'admin' → 'editor' for this relationship
  [3] Map 'admin' → 'owner' for ALL relationships in this import
  [4] Skip this relationship
  [a] Abort import
> _
```

User selects `3` (map all):

```text
✓ Will map 'admin' → 'owner' for 3 relationships

Remaining conflicts: 5
  ⚠ Applying 'admin' → 'owner' mapping resolved 2 additional conflicts

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
CONFLICT 3/5: Invalid subject reference

IMPORTING:
  user:departed-employee viewer document:budget

Problem: Subject 'user:departed-employee' does not exist in vault

? How do you want to resolve this conflict?
  [c] Create subject and import relationship
  [s] Skip this relationship
  [r] Replace subject (enter new ID)
  [a] Abort import
> _
```

After resolving all conflicts:

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
All conflicts resolved!

Resolution summary:
  Kept existing:     2
  Merged:            1
  Mapped relations:  3 ('admin' → 'owner')
  Skipped:           2

Ready to import:
  + 892 new relationships
  ~ 1 merged
  = 354 unchanged

? Proceed with import? [Y/n] _
```

Final result:

```text
Import complete

Duration: 4.7s (including conflict resolution)
Relationships processed: 1,247
  Created: 892
  Merged: 1
  Mapped: 3
  Skipped: 354 (347 unchanged + 7 conflicts)
  Failed: 0

Conflict resolution log saved to: ~/.local/state/inferadb/import-2025-01-15-143022.log
Replay this import with: inferadb import relationships.json --resolution-file ~/.local/state/inferadb/import-2025-01-15-143022.log
```

### Saved Resolution Strategies

Save and reuse conflict resolution strategies:

```bash
# Save resolutions to a file for replay
inferadb import relationships.json --interactive --save-resolutions strategy.yaml

# Replay with saved resolutions (non-interactive)
inferadb import relationships.json --resolution-file strategy.yaml --yes

# Preview what resolutions would apply
inferadb import relationships.json --resolution-file strategy.yaml --dry-run
```

Resolution file format:

```yaml
# strategy.yaml
version: 1
created: 2025-01-15T14:30:22Z
resolutions:
  relation_mapping:
    admin: owner
    superuser: owner

  on_duplicate:
    default: keep_existing
    exceptions:
      - subject: "user:alice"
        resource: "document:readme"
        action: merge

  on_invalid_reference:
    default: skip
    auto_create_subjects: false

  on_schema_mismatch:
    default: error
```

### Batch Conflict Reporting

For CI/CD pipelines, get a full conflict report before deciding:

```bash
# Generate conflict report without interactive prompts
inferadb import relationships.json --analyze-conflicts -o json > conflicts.json

# Then resolve programmatically and import
inferadb import relationships.json --resolution-file resolutions.yaml --yes
```

Conflict report output:

```json
{
  "source_file": "relationships.json",
  "total_relationships": 1247,
  "analysis": {
    "will_create": 892,
    "unchanged": 347,
    "conflicts": 8
  },
  "conflicts": [
    {
      "type": "duplicate_with_different_attributes",
      "line": 45,
      "importing": {
        "subject": "user:alice",
        "relation": "editor",
        "resource": "document:readme"
      },
      "existing": {
        "subject": "user:alice",
        "relation": "editor",
        "resource": "document:readme",
        "condition": "context.ip_address in_cidr \"10.0.0.0/8\""
      },
      "suggested_resolution": "keep_existing"
    }
  ]
}
```

---

### Atomicity Guarantees

Understanding transaction behavior is critical for production use:

| Flag                | Behavior                                   | Use When                              |
| ------------------- | ------------------------------------------ | ------------------------------------- |
| (default)           | Best-effort: apply what you can            | One-off imports, duplicates expected  |
| `--atomic`          | All-or-nothing: rollback on any failure    | Critical data sync, migrations        |
| `--continue-on-error` | Apply as many as possible, report failures | Large migrations with known issues  |
| `--skip-duplicates` | Treat duplicates as success (idempotent)   | Re-running operations safely          |

**Examples by use case:**

```bash
# Production import (atomic - safest)
# If anything fails, nothing is applied
inferadb import data.json --atomic --yes

# Migration from legacy system (best-effort)
# Import what you can, report what failed
inferadb import legacy.json --continue-on-error --report --yes

# Idempotent operation (safe to re-run)
# Won't error on duplicates, won't create duplicates
inferadb relationships add --file tuples.json --skip-duplicates --yes

# Atomic with duplicate handling
# All-or-nothing, but duplicates don't count as failures
inferadb import data.json --atomic --skip-duplicates --yes
```

**Rollback behavior with `--atomic`:**

```text
Importing relationships.json (atomic mode)...
[████████████████░░░░░░░░░░░░░░] 54% (675/1247)

Error at relationship 676:
  Invalid relation 'superadmin' for entity Document

Rolling back 675 applied relationships...
✓ Rollback complete - vault unchanged

No changes were made. Fix the error and retry:
  Line 676: "user:bob superadmin document:secret"
  Available relations for Document: owner, editor, viewer
```

---

<!-- ═══════════════════════════════════════════════════════════════════════════
     PART 8: CLI REFERENCE
     ═══════════════════════════════════════════════════════════════════════════ -->

## Global Flags

These flags are available on all commands:

| Flag        | Short | Description                                    |
| ----------- | ----- | ---------------------------------------------- |
| `@<name>`   |       | Use specific profile (e.g., `@prod`)           |
| `--org`     |       | Override organization from profile             |
| `--vault`   | `-v`  | Override vault from profile                    |
| `--output`  | `-o`  | Output format: `json`, `yaml`, `table`, `tree` |
| `--quiet`   | `-q`  | Suppress non-essential output                  |
| `--verbose` |       | Enable verbose output                          |
| `--debug`   |       | Enable debug logging                           |
| `--timeout` |       | Operation timeout (e.g., `30s`, `5m`)          |
| `--help`    | `-h`  | Show help                                      |
| `--version` |       | Show version                                   |

### Scripting Flags

Additional flags for automation and scripting:

| Flag               | Description                                                 |
| ------------------ | ----------------------------------------------------------- |
| `--yes` / `-y`     | Skip confirmation prompts                                   |
| `--if-exists`      | No error if resource doesn't exist (for delete operations)  |
| `--if-not-exists`  | No error if resource already exists (for create operations) |
| `--wait`           | Wait for async operations to complete                       |
| `--wait-timeout`   | Timeout when waiting (default: 5m)                          |
| `--exit-code`      | Return meaningful exit codes (for check commands)           |
| `--wait-for-ready` | Wait for service to be available before running             |

### Scripting Examples

```bash
# Idempotent relationship creation
inferadb relationships add user:alice viewer document:readme --if-not-exists

# Idempotent deletion
inferadb relationships delete user:alice viewer document:readme --if-exists

# Wait for service before running (useful in CI/CD)
inferadb --wait-for-ready schemas push schema.ipl --activate

# Timeout for slow operations
inferadb import large-dataset.json --yes --timeout 10m

# Check with exit code for shell scripts
if inferadb check user:alice can_view document:readme --quiet --exit-code; then
  echo "Access granted"
else
  echo "Access denied"
fi
```

### Common Flags by Command

Quick reference for which commands support common flags:

| Command                  | `--dry-run` | `--yes` | `--if-exists` | `--if-not-exists` | `--wait` |
| ------------------------ | :---------: | :-----: | :-----------: | :---------------: | :------: |
| `relationships add`      |             |         |               |         ✓         |          |
| `relationships delete`   |      ✓      |    ✓    |       ✓       |                   |          |
| `schemas push`           |      ✓      |         |               |                   |          |
| `schemas activate`       |             |    ✓    |               |                   |    ✓     |
| `schemas canary promote` |             |    ✓    |               |                   |    ✓     |
| `schemas rollback`       |      ✓      |    ✓    |               |                   |          |
| `schemas copy`           |      ✓      |         |               |                   |          |
| `import`                 |  (default)  |    ✓    |               |                   |    ✓     |
| `orgs delete`            |             |    ✓    |               |                   |          |
| `orgs vaults delete`     |             |    ✓    |               |                   |          |
| `orgs teams delete`      |             |    ✓    |               |                   |          |
| `orgs clients delete`    |             |    ✓    |               |                   |          |
| `account delete`         |             |    ✓    |               |                   |          |
| `profiles delete`        |             |    ✓    |               |                   |          |
| `tokens revoke-all`      |             |    ✓    |               |                   |          |
| `upgrade`                |      ✓      |         |               |                   |          |

Notes:

- `--dry-run`: Preview changes without applying them
- `--yes`: Skip interactive confirmation prompts
- `--if-exists`: Succeed silently if resource already exists (idempotent delete)
- `--if-not-exists`: Succeed silently if resource doesn't exist (idempotent create)
- `--wait`: Block until async operation completes

---

## Configuration File

Profiles are stored in `~/.config/inferadb/cli.yaml`:

```yaml
default_profile: prod

profiles:
  prod:
    url: https://api.inferadb.com
    org: "123456789012345678"
    vault: "987654321098765432"
    # Auth tokens stored separately in OS keychain

  staging:
    url: https://api.inferadb.com
    org: "123456789012345678"
    vault: "876543210987654321"

  dev:
    url: http://localhost:3000
    org: "111222333444555666"
    vault: "666555444333222111"
```

---

## Environment Variables

| Variable            | Description                             |
| ------------------- | --------------------------------------- |
| `INFERADB_PROFILE`  | Default profile to use                  |
| `INFERADB_URL`      | Override service URL                    |
| `INFERADB_ORG`      | Override organization ID                |
| `INFERADB_VAULT`    | Override vault ID                       |
| `INFERADB_TOKEN`    | Bearer token for authentication         |
| `INFERADB_DEBUG`    | Enable debug logging (`1` or `true`)    |
| `INFERADB_PROXY`    | HTTP/SOCKS5 proxy URL                   |
| `INFERADB_NO_PROXY` | Hosts to bypass proxy (comma-separated) |
| `NO_COLOR`          | Disable colored output                  |

---

## Value Formats

### Duration / TTL

Duration values accept human-friendly formats:

| Format  | Meaning           | Example       |
| ------- | ----------------- | ------------- |
| `30s`   | Seconds           | `--ttl 30s`   |
| `5m`    | Minutes           | `--ttl 5m`    |
| `2h`    | Hours             | `--ttl 2h`    |
| `1d`    | Days              | `--ttl 1d`    |
| `1w`    | Weeks             | `--ttl 1w`    |
| `1h30m` | Combined          | `--ttl 1h30m` |
| `3600`  | Seconds (numeric) | `--ttl 3600`  |

Examples:

```bash
# Token TTL
inferadb tokens generate --ttl 1h
inferadb tokens generate --ttl 30m
inferadb tokens generate --ttl 86400  # 24 hours in seconds

# Timeout
inferadb import data.json --timeout 10m
inferadb --wait-for-ready --timeout 2m schemas push schema.ipl
```

### Timestamps

Timestamp values accept ISO 8601 format and relative times:

| Format    | Example                             |
| --------- | ----------------------------------- |
| ISO 8601  | `2025-01-15T10:30:00Z`              |
| Date only | `2025-01-15` (assumes 00:00:00 UTC) |
| Relative  | `24h` (24 hours ago)                |
| Relative  | `7d` (7 days ago)                   |

Examples:

```bash
# Audit logs since date
inferadb orgs audit-logs --from 2025-01-01
inferadb orgs audit-logs --from 2025-01-01T00:00:00Z --to 2025-01-31T23:59:59Z

# Relationships changed recently
inferadb relationships list --changed-since 24h
inferadb export --changed-since 7d
```

### Pagination Cursors

Cursor values are opaque, base64-encoded strings returned by the API:

```bash
# First page
inferadb relationships list --limit 100
# ... results ...
# Next cursor: eyJvZmZzZXQiOjEwMH0=

# Next page
inferadb relationships list --limit 100 --cursor eyJvZmZzZXQiOjEwMH0=
```

Notes:

- Cursors are stateless and can be reused
- Cursors may expire after extended periods
- Store cursors for resumable iteration over large datasets

---

## Tuple Format

Authorization tuples follow a consistent format throughout the CLI. Understanding this format is essential for working with relationships and authorization checks.

### Basic Format

```text
<subject_type>:<subject_id> <relation_or_permission> <resource_type>:<resource_id>
```

Examples:

```bash
user:alice viewer document:readme
group:engineering member user:charlie
folder:root parent document:readme
```

### Subject Sets (Computed Usersets)

Reference all members of a relation on another object using the `#` syntax:

```text
<type>:<id>#<relation>
```

Examples:

```bash
# All members of the engineering group
group:engineering#member

# All editors of the parent folder
folder:root#editor

# Use in relationships
inferadb relationships add "group:engineering#member viewer document:readme"
```

This grants `viewer` access to `document:readme` for anyone who is a `member` of `group:engineering`.

### Valid Characters

| Component       | Allowed Characters                 | Max Length |
| --------------- | ---------------------------------- | ---------- |
| Type name       | `a-z`, `A-Z`, `0-9`, `_`           | 64         |
| ID              | `a-z`, `A-Z`, `0-9`, `_`, `-`, `.` | 256        |
| Relation name   | `a-z`, `A-Z`, `0-9`, `_`           | 64         |
| Permission name | `a-z`, `A-Z`, `0-9`, `_`           | 64         |

Notes:

- Type names are case-sensitive (`User` ≠ `user`)
- IDs are case-sensitive
- Relation and permission names typically use `snake_case`
- IDs can contain `.` for namespacing (e.g., `tenant.user123`)

### Examples by Command

```bash
# Check permission: subject permission resource
inferadb check user:alice can_view document:readme

# Add relationship: subject relation resource
inferadb relationships add user:alice viewer document:readme

# Add with subject set
inferadb relationships add "group:engineering#member viewer document:readme"

# Expand userset: resource relation
inferadb expand document:readme viewer

# List resources: subject permission resource_type
inferadb list-resources user:alice can_view document

# List subjects: resource relation
inferadb list-subjects document:readme viewer
```

### Quoting Rules

When using subject sets or special characters in shells, quote the tuple:

```bash
# Subject sets require quotes (shell interprets # as comment)
inferadb relationships add "group:engineering#member viewer document:readme"

# Multiple tuples
inferadb relationships add \
  "user:alice viewer document:readme" \
  "group:engineering#member editor document:readme"
```

---

## Exit Codes

| Code | Meaning                    | Examples                                    | Next Steps                                       |
| ---- | -------------------------- | ------------------------------------------- | ------------------------------------------------ |
| 0    | Success                    | Command completed                           | None                                             |
| 1    | General error              | Missing required arg                        | Check `--help`                                   |
| 2    | Invalid arguments          | `--vault invalid-id`                        | Use `--vault` with Snowflake ID                  |
| 3    | Authentication required    | Token expired, not logged in                | `inferadb @prod login`                           |
| 4    | Permission denied          | User lacks vault access                     | Contact org admin                                |
| 5    | Resource not found         | Vault doesn't exist, schema version missing | Check resource ID with `list` commands           |
| 6    | Conflict                   | Profile name already exists                 | Use `--force` or `delete` first                  |
| 7    | Rate limited               | Too many requests                           | Back off and retry with exponential delay        |
| 10   | Network error              | Connection refused, DNS failure             | Run `inferadb doctor`                            |
| 11   | Server error               | 5xx response                                | Check [status page](https://status.inferadb.com) |

### Scripting with Exit Codes

Use exit codes to build robust automation scripts with proper error handling:

```bash
#!/bin/bash
# Script pattern: retry on transient errors, fail fast on permanent errors

set +e  # Don't exit on error, we'll handle it

for i in {1..3}; do
  inferadb check user:alice can_view document:readme --quiet --exit-code
  CODE=$?

  case $CODE in
    0)
      echo "Access granted"
      exit 0
      ;;
    3)
      echo "Auth error - re-authenticating..."
      inferadb @prod login
      ;;
    4)
      echo "Permission denied - not retrying"
      exit 4
      ;;
    7)
      echo "Rate limited - backing off ${i}s..."
      sleep $((2**i))
      ;;
    10)
      echo "Network error - retrying in 1s..."
      sleep 1
      ;;
    11)
      echo "Server error - retrying in ${i}s..."
      sleep $i
      ;;
    *)
      echo "Unexpected error: $CODE"
      exit $CODE
      ;;
  esac
done

echo "Failed after 3 attempts"
exit 1
```

Common patterns:

```bash
# Simple check with fallback
if inferadb check user:alice can_view document:readme --quiet --exit-code; then
  echo "Allowed"
else
  echo "Denied or error"
fi

# Distinguish between denial and error
inferadb check user:alice can_view document:readme --quiet --exit-code
case $? in
  0)  echo "Allowed" ;;
  4)  echo "Denied" ;;       # Permission denied = authorization decision
  *)  echo "Error" ;;        # Everything else = infrastructure issue
esac

# CI/CD gate with timeout
timeout 30s inferadb check user:$CI_USER can_deploy app:$APP_NAME --exit-code \
  || { echo "Authorization check failed"; exit 1; }
```

### Error Code Lookup

Look up detailed information about an exit code:

```bash
# Look up error code
inferadb help error 4

# List all error codes
inferadb help error --list
```

Output for `inferadb help error 4`:

```text
Exit Code 4: Permission Denied

Description:
  The authenticated user does not have permission to perform
  the requested action on the specified resource.

Common causes:
  • User lacks the required role (admin, writer, etc.)
  • Resource belongs to a different organization
  • Team membership doesn't grant sufficient permissions
  • Vault-level permissions are insufficient

Troubleshooting:
  1. Check your current role: inferadb whoami
  2. Verify organization access: inferadb orgs get
  3. Check vault permissions: inferadb orgs vaults roles list
  4. Contact an administrator to request access

Related documentation:
  https://docs.inferadb.com/errors/permission-denied
```

Output for `inferadb help error --list`:

```text
InferaDB CLI Exit Codes

Code  Meaning                 Description
────  ──────────────────────  ──────────────────────────────────────────
0     Success                 Operation completed successfully
1     General error           Unexpected error (check --debug output)
2     Invalid arguments       Command syntax or parameter error
3     Authentication required Token missing, expired, or invalid
4     Permission denied       Insufficient permissions for operation
5     Resource not found      Requested resource does not exist
6     Conflict                Resource already exists or state conflict
7     Rate limited            Too many requests, retry after delay
10    Network error           Connection, DNS, or TLS failure
11    Server error            Service returned 5xx error

Use 'inferadb help error <code>' for detailed information.
```

---

## Error Handling & Diagnostics

The CLI provides actionable error messages with recovery suggestions. Every error includes context and next steps.

### Error Message Format

```text
Error: <concise description>

Details:
  <additional context>

Suggestions:
  • <recovery action 1>
  • <recovery action 2>

Run 'inferadb doctor' for connectivity diagnostics.
```

### Common Error Scenarios

Error messages include the exact command attempted, what failed, and step-by-step recovery instructions.

#### Network Connectivity

```text
Error: Unable to connect to https://api.inferadb.com

Command attempted:
  $ inferadb check user:alice can_view document:readme

Details:
  Connection timed out after 30s
  Last successful connection: 2 hours ago
  Profile: prod

Your current setup:
  URL: https://api.inferadb.com
  Profile: prod
  Proxy: (not set)

Troubleshooting steps (in order of likelihood):

  1. Check network connectivity:
     $ ping -c 1 api.inferadb.com

  2. Verify your profile URL:
     $ inferadb config show | grep url
     # Expected: https://api.inferadb.com

  3. Check service status:
     https://status.inferadb.com

  4. If using a proxy, verify INFERADB_PROXY:
     $ echo $INFERADB_PROXY
     # Should be set if you're behind a corporate proxy

  5. Run full diagnostics:
     $ inferadb doctor

Still having issues?
  Community: https://inferadb.community/troubleshooting
  Support: support@inferadb.com

Exit code: 10
```

#### Authentication Expired

```text
Error: Authentication token expired

Command attempted:
  $ inferadb relationships list --subject user:alice

Details:
  Token expired at 2025-01-15T10:30:00Z (2 hours ago)
  Profile: prod
  Token type: user (OAuth)

Quick fix:
  $ inferadb @prod login

Alternative (for CI/CD):
  # Generate a new service token
  $ inferadb tokens generate --ttl 7d

  # Or use environment variable
  $ export INFERADB_TOKEN="your-new-token"

Exit code: 3
```

#### Permission Denied

```text
Error: Permission denied

Command attempted:
  $ inferadb orgs vaults create "New Vault"

Details:
  Action: vault.create
  Organization: 123456789012345678 (Acme Corp)
  Your role: member
  Required role: admin

Your current access:
  $ inferadb whoami
  User: alice@acme.com
  Org Role: member
  Vault Role: (n/a - creating new vault)

Troubleshooting:

  1. Check what you can do:
     $ inferadb whoami --permissions

  2. Request elevated access from an owner:
     Organization owners: bob@acme.com, carol@acme.com

  3. Or ask an admin to create the vault for you

Exit code: 4
```

#### Resource Not Found

```text
Error: Resource not found

Command attempted:
  $ inferadb orgs vaults get 999888777666555444

Details:
  Resource type: vault
  ID: 999888777666555444
  Organization: 123456789012345678 (Acme Corp)

Possible causes:
  • The vault ID may be incorrect
  • The vault may have been deleted
  • The vault may be in a different organization

Troubleshooting:

  1. List available vaults:
     $ inferadb orgs vaults list

  2. Check your current profile:
     $ inferadb whoami
     # Profile: prod, Org: Acme Corp (123456789012345678)

  3. Search across organizations:
     $ inferadb orgs list
     $ inferadb orgs vaults list --org <other-org-id>

Exit code: 5
```

#### Validation Error

```text
Error: Schema validation failed

Command attempted:
  $ inferadb schemas push schema.ipl

File: schema.ipl

Errors:

  Line 42, Column 5:
    permission view: viewr
                     ^^^^^
    Unknown relation 'viewr'. Did you mean 'viewer'?

  Line 67, Column 12:
    owner: User | Goup
                  ^^^^
    Unknown entity type 'Goup'. Did you mean 'Group'?

Summary:
  2 errors, 0 warnings

Quick fixes:
  • Line 42: Change 'viewr' to 'viewer'
  • Line 67: Change 'Goup' to 'Group'

Auto-fix available:
  $ inferadb schemas format schema.ipl --fix-typos

Exit code: 2
```

#### Rate Limited

```text
Error: Rate limit exceeded

Command attempted:
  $ inferadb check --batch checks.json

Details:
  Limit: 100 requests/minute
  Current usage: 100/100
  Reset in: 45 seconds
  Endpoint: POST /v1/check

Your request:
  Batch size: 500 checks
  Effective rate: 500 requests (exceeds limit)

Solutions:

  1. Wait and retry:
     $ sleep 45 && inferadb check --batch checks.json

  2. Use batch endpoint (counts as 1 request):
     # Already using batch - reduce batch size
     $ inferadb check --batch checks.json --chunk-size 50

  3. Request limit increase:
     Current tier: Free (100 req/min)
     Upgrade: https://inferadb.com/pricing

Exit code: 7
```

### Retry Behavior

The CLI automatically retries transient failures with exponential backoff:

```bash
# Configure retry behavior
inferadb check user:alice can_view document:readme \
  --retries 3 \           # Number of retries (default: 3)
  --retry-delay 1s \      # Initial delay (default: 1s)
  --retry-max-delay 30s   # Maximum delay (default: 30s)

# Disable retries
inferadb check user:alice can_view document:readme --retries 0
```

Retried errors (5xx, timeouts, connection errors) show progress:

```text
Retrying... attempt 2/3 (waiting 2s)
Retrying... attempt 3/3 (waiting 4s)
Error: Server unavailable after 3 attempts
```

### Proxy Support

```bash
# Via environment variable
export INFERADB_PROXY="http://proxy.example.com:8080"

# With authentication
export INFERADB_PROXY="http://user:pass@proxy.example.com:8080"

# SOCKS5 proxy
export INFERADB_PROXY="socks5://proxy.example.com:1080"

# Bypass proxy for specific hosts
export INFERADB_NO_PROXY="localhost,127.0.0.1,.internal.corp"
```

### Debug Mode

Enable verbose logging for troubleshooting:

```bash
# Via flag
inferadb check user:alice can_view document:readme --debug

# Via environment variable
export INFERADB_DEBUG=1
inferadb check user:alice can_view document:readme
```

Debug output includes:

```text
[DEBUG] Loading profile: prod
[DEBUG] URL: https://api.inferadb.com
[DEBUG] Auth: Bearer token (expires in 23h)
[DEBUG] POST /v1/vaults/987654321098765432/check
[DEBUG] Request-ID: req_abc123def456
[DEBUG] Request: {"subject":"user:alice","permission":"can_view","resource":"document:readme"}
[DEBUG] Response: 200 OK (23ms)
[DEBUG] {"allowed":true,"resolution_path":["viewer"]}
✓ ALLOW: user:alice can_view document:readme
```

### Request Tracking

Every request includes a unique request ID for support and debugging:

```bash
# Show request ID in output
inferadb check user:alice can_view document:readme --verbose
```

Output:

```text
✓ ALLOW: user:alice can_view document:readme
Request-ID: req_abc123def456
```

The request ID is useful for:

- Correlating with server-side logs
- Support ticket references
- Debugging specific failures

### Timing Information

Get detailed timing breakdown for performance analysis:

```bash
inferadb check user:alice can_view document:readme --timing
```

Output:

```text
✓ ALLOW: user:alice can_view document:readme

Timing breakdown:
  DNS lookup:     5ms
  TLS handshake:  12ms
  Request send:   2ms
  Server time:    18ms
  Response read:  3ms
  ─────────────────────
  Total:          40ms

Request-ID: req_abc123def456
```

For batch operations:

```bash
inferadb check --batch checks.json --timing
```

```text
Batch check complete: 100 checks

Timing statistics:
  Total time:     1.2s
  Avg per check:  12ms
  Min:            8ms
  Max:            45ms
  P50:            11ms
  P95:            23ms
  P99:            38ms

Request-ID: req_xyz789ghi012
```

---

---

<!-- ═══════════════════════════════════════════════════════════════════════════
     PART 9: DEVELOPER EXPERIENCE
     ═══════════════════════════════════════════════════════════════════════════ -->

## Shell Completion

Generate shell completion scripts for tab completion of commands, flags, and arguments.

```bash
# Bash
inferadb --generate-completion bash > ~/.local/share/bash-completion/completions/inferadb
# Or add to ~/.bashrc:
# eval "$(inferadb --generate-completion bash)"

# Zsh
inferadb --generate-completion zsh > ~/.zfunc/_inferadb
# Or add to ~/.zshrc:
# eval "$(inferadb --generate-completion zsh)"

# Fish
inferadb --generate-completion fish > ~/.config/fish/completions/inferadb.fish

# PowerShell
inferadb --generate-completion powershell >> $PROFILE
```

Completions include:

- All commands and subcommands
- Flag names and values
- Profile names from `~/.config/inferadb/cli.yaml`
- Organization and vault IDs from current profile
- Resource types from active schema
- Common permission names

---

## Output Formatting

### Format Selection

```bash
# Table (default for lists)
inferadb orgs list -o table

# JSON (default for single resources)
inferadb orgs get 123456789012345678 -o json

# YAML
inferadb relationships list -o yaml

# Compact JSON (single line, for piping)
inferadb check user:alice document:readme can_view -o json --compact
```

### Column Selection

Select specific columns for table output:

```bash
# Show only specific columns
inferadb orgs list --columns name,id,tier

# Available columns vary by resource type
inferadb orgs vaults list --columns name,id,created_at,relationship_count
```

### Table Display Options

Control table appearance for different use cases:

```bash
# Use full terminal width (wraps long content)
inferadb relationships list -o table --wide

# No headers (useful for piping to other tools)
inferadb relationships list -o table --no-headers

# Combine for scripting
inferadb relationships list --columns subject,relation,resource --no-headers | while read s r o; do
  echo "Subject $s has $r on $o"
done

# Control column alignment
inferadb orgs list -o table --align left      # Left-align all columns
inferadb orgs list -o table --align right     # Right-align all columns

# Truncate long values (default: 40 chars)
inferadb relationships list -o table --truncate 80

# No truncation (show full values)
inferadb relationships list -o table --no-truncate

# Sort output
inferadb relationships list -o table --sort created_at
inferadb relationships list -o table --sort created_at --desc
```

### Field Filtering with JSONPath

Filter and transform JSON output using JSONPath expressions:

```bash
# Extract specific field
inferadb orgs get 123456789012345678 -o json --query '$.name'
# "Acme Corp"

# Filter array elements
inferadb orgs list -o json --query '$[?(@.tier=="pro")]'

# Extract nested fields
inferadb schemas get --active -o json --query '$.entities[*].name'
# ["User", "Document", "Folder"]
```

### Go Template Output

Use Go templates for custom formatting:

```bash
# Custom format
inferadb orgs list -o template --template '{{.name}} ({{.id}})'
# Acme Corp (123456789012345678)
# Beta Inc (234567890123456789)

# Multi-line templates
inferadb orgs get 123456789012345678 -o template --template '
Organization: {{.name}}
Tier: {{.tier}}
Created: {{.created_at | date "2006-01-02"}}
Members: {{.member_count}}
'

# Template from file
inferadb orgs list -o template --template-file ./org-report.tmpl
```

### Machine-Readable Output

For scripting and automation:

```bash
# Just the IDs
inferadb orgs list --ids-only
# 123456789012345678
# 234567890123456789

# Tab-separated values
inferadb orgs list -o tsv

# CSV with headers
inferadb orgs list -o csv

# JSON Lines (one object per line)
inferadb stream -o jsonl
```

### Compact Output Mode

The `--compact` flag provides minimal, information-dense output for quick scanning and CI/CD logs:

```bash
# Normal output
inferadb check user:alice can_edit document:readme
```

```text
CHECK RESULT

Decision: allow
Subject:  user:alice
Relation: can_edit
Resource: document:readme

Resolution path:
  document:readme#editor → user:alice ✓

Evaluation time: 2.1ms
```

```bash
# Compact output
inferadb check user:alice can_edit document:readme --compact
```

```text
✓ user:alice can_edit document:readme (via editor, 2.1ms)
```

Compact mode works across all commands:

```bash
# Relationships
inferadb relationships list --limit 5 --compact
```

```text
user:alice editor document:readme
user:bob viewer document:readme
group:eng#member user:alice
group:eng#member user:bob
folder:root owner user:admin
```

```bash
# Schema operations
inferadb schemas list --compact
```

```text
v3 active  user-permissions  2h ago
v2 -       user-permissions  5d ago
v1 -       user-permissions  2w ago
```

```bash
# Stats
inferadb stats --compact
```

```text
entities:3 relations:47,832 schemas:3(v3 active) latency:p99=12ms
```

```bash
# Expand
inferadb expand document:readme viewer --compact
```

```text
viewer: user:alice(editor) user:bob(viewer) group:eng#member(viewer)
```

```bash
# Health
inferadb health --compact
```

```text
✓ api ✓ auth ✓ storage ✓ cache (all healthy, 45ms)
```

Compact + JSON for minimal machine parsing:

```bash
inferadb check user:alice can_edit document:readme -o json --compact
```

```json
{
  "decision": "allow",
  "subject": "user:alice",
  "relation": "can_edit",
  "resource": "document:readme",
  "via": "editor",
  "ms": 2.1
}
```

---

## Security

### Token Protection

Sensitive tokens are hidden by default:

```bash
# Token is masked by default
inferadb tokens generate
# Token generated successfully
# Token ID: 888999000111222333
# Expires: 2025-01-16T10:30:00Z
#
# Token: ****...****(hidden)
# Use --reveal to display the token

# Explicitly reveal token
inferadb tokens generate --reveal
# Token: eyJhbGciOiJFZERTQSI...

# Copy directly to clipboard (macOS/Linux)
inferadb tokens generate --copy
# Token copied to clipboard

# Output for scripts (no formatting, just token)
inferadb tokens generate --raw
```

### Credential Storage

Authentication tokens are stored securely:

- **macOS**: Keychain Access
- **Linux**: libsecret / GNOME Keyring / KWallet
- **Windows**: Windows Credential Manager

```bash
# Check where credentials are stored
inferadb config show credential_store
# credential_store: keychain (macOS)

# Force file-based storage (less secure)
inferadb config set credential_store file
```

### Token Expiry Warnings

The CLI warns when tokens are nearing expiry:

```text
inferadb check user:alice can_view document:readme

⚠ Warning: Your authentication token expires in 2 hours.
  Run 'inferadb login' to refresh.

✓ ALLOW: user:alice can_view document:readme
```

Suppress warnings:

```bash
inferadb check user:alice can_view document:readme --no-warnings
```

### Local Audit Log

Track CLI operations locally for security review:

```bash
# Enable local audit logging
inferadb config set audit.enabled true
inferadb config set audit.path ~/.local/state/inferadb/audit.log

# View local audit log
inferadb audit local
inferadb audit local --from 2025-01-15 --to 2025-01-16

# Clear local audit log
inferadb audit local --clear
```

Local audit log format:

```text
2025-01-15T10:30:00Z check user:alice can_view document:readme [ALLOW]
2025-01-15T10:30:15Z relationships add user:bob viewer document:readme [OK]
2025-01-15T10:31:00Z schemas push schema.ipl [OK] schema_id=777888999000111222
2025-01-15T10:32:00Z schemas activate 777888999000111222 [OK]
```

### Credential Rotation

**When to Rotate:**

- Token age > 30 days (recommended every 7-30 days depending on sensitivity)
- After suspected compromise
- After team member leaves the organization
- Per compliance policy (SOC2, HIPAA, etc.)
- When `inferadb doctor` or `tokens inspect` shows rotation warnings

**Rotating Personal Tokens (CLI Auth):**

```bash
# Re-authenticate to get a fresh token
inferadb @prod login

# Check current token age
inferadb tokens inspect --verify
# Shows: ⚠ Token age: 25 days (rotate recommended after 24h)
```

**Rotating Vault Tokens (CI/CD Integrations):**

```bash
# 1. Generate new token
NEW_TOKEN=$(inferadb tokens generate --ttl 30d --raw)

# 2. Update CI/CD secrets
# GitHub Actions: gh secret set INFERADB_TOKEN --body "$NEW_TOKEN"
# GitLab: Update variable in CI/CD settings
# AWS: aws secretsmanager put-secret-value --secret-id inferadb-token --secret-string "$NEW_TOKEN"

# 3. List existing tokens to find old one
inferadb tokens list
# TOKEN_ID          DESCRIPTION      CREATED              LAST_USED
# 888999000111222   CI/CD Pipeline   2024-12-15T10:00:00Z 2025-01-14T23:45:00Z
# 999000111222333   CI/CD Pipeline   2025-01-15T10:00:00Z never

# 4. Revoke old token after confirming new one works
inferadb tokens revoke 888999000111222
```

**Rotating Client Certificates:**

```bash
# 1. Generate new certificate for client
inferadb orgs clients certificates create 444555666777888999 \
  --name "Production Cert 2025-Q1" \
  --ttl 90d

# 2. Deploy new certificate to application
# ... update app configuration with new cert ...

# 3. Verify new cert is working
inferadb orgs clients certificates list 444555666777888999

# 4. Revoke old certificate
inferadb orgs clients certificates revoke 444555666777888999 OLD_CERT_ID
```

**Automated Rotation Reminder:**

```bash
# Add to cron or CI pipeline to check credential age
inferadb doctor --check credentials
# Exit code 1 if any credentials need rotation
```

### Sensitive Data Handling

The CLI avoids logging sensitive data:

- Passwords are never logged or stored in command history
- Token values are redacted in debug output
- Context data with sensitive fields is masked

```bash
# Password prompt (not echoed)
inferadb register --email user@example.com
Password: ████████████

# Context with sensitive data
inferadb check user:alice can_view document:readme \
  --context '{"api_key": "secret123"}'
# Debug log shows: {"api_key": "***REDACTED***"}
```

---

## Built-in Help & Examples

### Command Examples

Every command supports `--examples` to show common usage patterns:

```bash
inferadb check --examples
```

Output:

```text
Examples for 'inferadb check':

  # Basic permission check
  inferadb check user:alice can_view document:readme

  # Check with trace to see resolution path
  inferadb check user:alice can_view document:readme --trace

  # Check multiple permissions at once
  inferadb check user:alice can_view,can_edit document:readme

  # Check with ABAC context
  inferadb check user:alice can_view document:secret \
    --context '{"ip_address": "10.0.1.50"}'

  # Batch check from file
  inferadb check --batch checks.json

  # Silent check for scripts (exit code only)
  inferadb check user:alice can_view document:readme --quiet --exit-code

For more information, see:
  https://docs.inferadb.com/cli/check
```

### Cheatsheet

Quick reference card for common operations. The cheatsheet adapts to different roles and can be exported in various formats.

```bash
# Default cheatsheet
inferadb cheatsheet

# Role-specific cheatsheet
inferadb cheatsheet --role developer     # Schema dev, testing, debugging
inferadb cheatsheet --role devops        # Deployment, monitoring, tokens
inferadb cheatsheet --role admin         # User management, audit, security

# Profile-aware (shows commands for current profile's vault)
inferadb cheatsheet --show-profile

# Export formats
inferadb cheatsheet --format markdown > CLI-QUICKSTART.md
inferadb cheatsheet --format json        # For documentation tooling
inferadb cheatsheet --format man         # Man page format
```

Default output:

```text
InferaDB CLI Cheatsheet

SETUP & AUTH
  inferadb init                              First-run setup wizard
  inferadb login                             Authenticate with browser
  inferadb whoami                            Show current user and profile

PROFILES
  inferadb profiles list                     List all profiles
  inferadb profiles create <name> --url ...  Create new profile
  inferadb profiles default <name>           Set default profile
  inferadb @<profile> <command>              Use profile for one command

AUTHORIZATION CHECKS  (subject permission resource)
  inferadb check user:X can_Y resource:Z     Check permission
  inferadb check ... --trace                 Show resolution path
  inferadb check ... --explain               Explain denial reason

RELATIONSHIPS  (subject relation resource)
  inferadb relationships list                List all relationships
  inferadb relationships add S R O           Add relationship
  inferadb relationships delete S R O        Delete relationship

SCHEMAS
  inferadb schemas validate file.ipl         Validate locally
  inferadb schemas push file.ipl             Upload new version
  inferadb schemas activate <id>             Make version active
  inferadb schemas test                      Run authorization tests

COMMON FLAGS
  -o json|yaml|table                         Output format
  @<name>                                    Use specific profile
  --vault <id>                               Override vault
  --yes                                      Skip confirmations
  --debug                                    Enable debug output

Full documentation: https://docs.inferadb.com/cli
```

With `--role developer`:

```text
InferaDB CLI Cheatsheet (Developer)

SCHEMA DEVELOPMENT
  inferadb schemas init                      Create new schema project
  inferadb schemas develop                   Start development mode
  inferadb schemas validate file.ipl         Validate locally
  inferadb schemas format file.ipl --write   Auto-format schema
  inferadb schemas test                      Run authorization tests
  inferadb schemas test --watch              Watch mode for tests

DEBUGGING AUTHORIZATION
  inferadb check user:X perm resource:Y      Check permission
  inferadb check ... --trace                 Show resolution path
  inferadb check ... --explain               Explain denial reason
  inferadb expand resource:Y relation        Show all subjects with access
  inferadb explain-permission resource perm  Show permission hierarchy

WHAT-IF TESTING
  inferadb simulate user:X perm resource:Y \
    --with "user:X relation resource:Y"      Test with temp relationships
  inferadb schemas preview schema.ipl        Preview schema impact

RELATIONSHIPS
  inferadb relationships list --subject X    See user's relationships
  inferadb relationships add S R O           Add relationship
  inferadb relationships validate S R O      Validate before adding

QUICK CHECKS
  inferadb what-can user:X perm type         What can user access?
  inferadb who-can resource:Y relation       Who has access?
```

With `--role devops`:

```text
InferaDB CLI Cheatsheet (DevOps)

DEPLOYMENT
  inferadb schemas pre-flight <id>           Pre-activation safety check
  inferadb schemas activate <id> --canary 10 Canary deployment
  inferadb schemas canary status             Monitor canary metrics
  inferadb schemas canary promote            Promote to full deployment
  inferadb schemas rollback --previous       Emergency rollback

MONITORING
  inferadb health                            Cluster health dashboard
  inferadb stats                             Relationship statistics
  inferadb what-changed                      Recent vault changes
  inferadb ping --count 10                   Latency measurement

TOKENS & ACCESS
  inferadb tokens generate --ttl 7d          Generate vault token
  inferadb tokens list                       List active tokens
  inferadb tokens inspect <token>            Decode token
  inferadb orgs audit-logs --action schema.* Audit schema changes

BACKUP & RESTORE
  inferadb export > backup.json              Export relationships
  inferadb import backup.json --atomic       Import with rollback
  inferadb schemas copy --to-vault <id>      Copy schema between vaults

TROUBLESHOOTING
  inferadb doctor                            Run diagnostics
  inferadb --debug <command>                 Debug mode
```

### Command Help

Detailed help for any command:

```bash
inferadb check --help
inferadb schemas push --help
```

### Templates

Copy-paste-ready workflow templates for common tasks:

```bash
# List available templates
inferadb templates list

# Show a specific template
inferadb templates show user-offboarding

# Show template with your values substituted
inferadb templates show user-offboarding --subject user:departing-employee

# Copy template to clipboard
inferadb templates show user-offboarding --copy

# Output as script
inferadb templates show user-offboarding --format script > offboard.sh
```

Available templates:

```text
inferadb templates list

Available Templates

COMMON WORKFLOWS
  user-offboarding       Remove all access for a departing user
  batch-check            Check multiple permissions efficiently
  export-backup          Create a complete vault backup
  permission-audit       Audit who has access to what

SCHEMA OPERATIONS
  schema-migration       Safely migrate schema with breaking changes
  canary-deploy          Deploy schema with canary rollout
  schema-rollback        Emergency rollback procedure

ADMINISTRATION
  new-vault-setup        Set up a new vault with common patterns
  token-rotation         Rotate vault tokens safely
  audit-report           Generate compliance audit report

DEBUGGING
  debug-denial           Investigate why access was denied
  compare-access         Compare access between two users

Use 'inferadb templates show <name>' for details.
```

Example template output:

```bash
inferadb templates show user-offboarding
```

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
User Offboarding Workflow
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Use this workflow to safely remove all access for a departing user.

VARIABLES:
  DEPARTING_USER = user:departing-employee  (customize with --subject)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

# Step 1: Review what the user has access to
inferadb relationships list --subject user:departing-employee -o json > departing-access.json

# Step 2: Audit the relationships (human review)
cat departing-access.json | jq -r '.[] | "\(.relation) on \(.resource)"'

# Step 3: Backup current state (for rollback if needed)
inferadb export > pre-offboarding-backup-$(date +%Y%m%d).json

# Step 4: Dry-run deletion (see what will be removed)
inferadb relationships delete --filter --subject user:departing-employee --dry-run

# Step 5: Delete all relationships (requires confirmation)
inferadb relationships delete --filter --subject user:departing-employee --yes

# Step 6: Verify removal
inferadb relationships list --subject user:departing-employee
# Should return: No relationships found

# Step 7: Audit log entry (optional)
inferadb orgs audit-logs --actor $USER --action relationships.delete --since 1h

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

NOTES:
  • This removes relationships, not the user account
  • Relationships in other vaults are not affected
  • Consider notifying the user's manager before proceeding

RELATED:
  • inferadb guide user-management
  • inferadb templates show permission-audit
```

With variables substituted:

```bash
inferadb templates show user-offboarding --subject user:alice@acme.com
```

All occurrences of `user:departing-employee` are replaced with `user:alice@acme.com`.

### Guide

Opinionated workflow guides for best practices:

```bash
# List available guides
inferadb guide list

# View a guide
inferadb guide schema-deployment

# Open guide in browser
inferadb guide schema-deployment --web
```

Available guides:

```text
inferadb guide list

Available Guides

GETTING STARTED
  quickstart             First-time setup and basic usage
  concepts               Core concepts: subjects, resources, relations

SCHEMA DEVELOPMENT
  schema-best-practices  Writing maintainable schemas
  schema-deployment      Safe deployment workflow
  schema-testing         Writing effective tests

OPERATIONS
  production-checklist   Pre-production readiness checklist
  incident-response      Handling authorization incidents
  performance-tuning     Optimizing for scale

SECURITY
  security-best-practices Token management, audit, access control
  compliance             SOC2, GDPR, HIPAA considerations

Use 'inferadb guide <name>' for details.
```

Example guide:

```bash
inferadb guide schema-deployment
```

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Schema Deployment Guide
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

This guide covers best practices for safely deploying schema changes to
production. Following this workflow minimizes the risk of breaking changes
affecting your users.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

PHASE 1: DEVELOPMENT

  1. Start with development mode:
     $ inferadb schemas develop schema.ipl

  2. Write tests alongside your schema changes:
     $ inferadb schemas test --watch

  3. Validate before pushing:
     $ inferadb schemas validate schema.ipl --strict

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

PHASE 2: STAGING (Optional but Recommended)

  1. Push to staging vault first:
     $ inferadb @staging schemas push schema.ipl --activate

  2. Run integration tests:
     $ your-integration-tests --target staging

  3. Verify with sample checks:
     $ inferadb @staging check user:test can_view document:test

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

PHASE 3: PRODUCTION PRE-FLIGHT

  1. Preview the impact:
     $ inferadb schemas preview schema.ipl

  2. Push without activating:
     $ inferadb schemas push schema.ipl

  3. Run pre-flight checks:
     $ inferadb schemas pre-flight <new-schema-id>

  4. Review breaking changes carefully:
     - Are orphaned relationships expected?
     - Will any users lose access unexpectedly?
     - Are all tests passing?

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

PHASE 4: CANARY DEPLOYMENT

  1. Start with 10% traffic:
     $ inferadb schemas activate <id> --canary 10

  2. Monitor for 15 minutes:
     $ watch inferadb schemas canary status

  3. Check for anomalies:
     - Allow rate should be stable
     - Latency should not increase significantly
     - Error rate should be ~0%

  4. If healthy, promote:
     $ inferadb schemas canary promote

  5. If issues, rollback:
     $ inferadb schemas canary rollback

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

EMERGENCY ROLLBACK

  If issues are detected after full deployment:

  $ inferadb schemas rollback --previous

  This immediately reverts to the last active schema.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

RELATED COMMANDS:
  inferadb schemas preview      Preview changes before push
  inferadb schemas pre-flight   Run pre-activation checks
  inferadb schemas canary       Manage canary deployments
  inferadb what-changed         See recent changes

RELATED TEMPLATES:
  inferadb templates show canary-deploy
  inferadb templates show schema-rollback

Full documentation: https://docs.inferadb.com/guides/schema-deployment
```

---

## Interactive Mode

### Interactive Check

For exploratory authorization queries:

```bash
inferadb check --interactive
```

Opens an interactive session with fuzzy search and autocomplete:

```text
InferaDB Interactive Check
Vault: Production (987654321098765432)
Press Ctrl+C to exit

Subject: user:█
  > user:alice
    user:bob
    user:charlie
    group:engineering#member

Subject: user:alice
Permission: █
  > can_view
    can_edit
    can_delete
    can_share

Permission: can_view
Resource: document:█
  > document:readme
    document:secret
    document:public

Checking: user:alice can_view document:readme

✓ ALLOW

Check another? [Y/n]: _
```

### Interactive Relationship Builder

```bash
inferadb relationships add --interactive
```

Guides through relationship creation with validation:

```text
Add Relationship

Subject type: █
  > user
    group
    team
    document

Subject type: user
Subject ID: alice

Relation: █
  Available for target type 'document':
  > owner
    editor
    viewer

Relation: viewer

Resource type: document
Resource ID: readme

Preview: user:alice viewer document:readme

Add this relationship? [Y/n]: y
✓ Relationship added

Add another? [Y/n]: _
```

### Interactive Shell (REPL)

For extended sessions with persistent context:

```bash
inferadb shell
```

Opens a REPL with command history and shortcuts:

```text
InferaDB Shell v1.2.3
Profile: prod | Org: Acme Corp | Vault: Production
Type 'help' for commands, 'exit' to quit

inferadb> check user:alice can_view document:readme
✓ ALLOW

inferadb> relationships list --subject user:alice
SUBJECT      RELATION   RESOURCE
user:alice   owner      document:readme
user:alice   viewer     document:secret

inferadb> .profile staging
Switched to profile: staging (vault: 111222333444555666)

inferadb> .history
  1  check user:alice can_view document:readme
  2  relationships list --subject user:alice
  3  .profile staging

inferadb> exit
```

Shell commands (prefixed with `.`):

| Command      | Description            |
| ------------ | ---------------------- |
| `.profile`   | Switch profile         |
| `.history`   | Show command history   |
| `.clear`     | Clear screen           |
| `.help`      | Show shell help        |
| `.exit`      | Exit shell             |

#### Smart Auto-Correction

The shell detects common argument order mistakes and offers corrections:

```text
inferadb> expand viewer document:readme
🤔 Did you mean: expand document:readme viewer?

Arguments appear to be reversed. The 'expand' command uses:
  expand <resource> <relation>

  [y] Yes, fix it  [n] No, run as-is  [?] Explain

Selection: y

document:readme#viewer
├── user:alice (direct)
├── user:bob (direct)
└── group:engineering#member
    ├── user:charlie
    └── user:dave
```

Auto-correction triggers when:

- Arguments match expected types but in wrong positions
- Resource-like values appear where relations expected (and vice versa)
- Subject-like values appear where resources expected

Disable auto-correction:

```bash
inferadb shell --no-autocorrect
```

#### Intelligent Suggestions

After executing a command, the shell suggests relevant follow-up actions:

```text
inferadb> check user:alice can_view document:readme
✓ ALLOW

💡 Related commands:
   • expand document:readme viewer        (see all viewers)
   • what-can user:alice can_view document  (see alice's access)
   • check user:alice can_view document:readme --trace  (see resolution path)

inferadb> relationships delete user:bob viewer document:readme --yes
✓ Relationship deleted

💡 You might want to:
   • Verify deletion: relationships list --subject user:bob
   • Check bob's remaining access: what-can user:bob can_view document
   • Audit this change: orgs audit-logs --actor $USER --action relationships.delete

inferadb> # Disable suggestions
inferadb> .suggestions off
Suggestions disabled. Use '.suggestions on' to re-enable.
```

Suggestions are context-aware and based on:

- The command just executed
- Common workflow patterns
- Recent command history

---

---

<!-- ═══════════════════════════════════════════════════════════════════════════
     PART 10: APPENDIX
     ═══════════════════════════════════════════════════════════════════════════ -->

## Planned Features

### Local Development Environment

```bash
# Start local InferaDB cluster (Engine + Control)
inferadb dev up

# Stop cluster (preserves data)
inferadb dev down

# View logs
inferadb dev logs
inferadb dev logs --follow
inferadb dev logs --service engine

# Check status
inferadb dev status

# Import seed data
inferadb dev import seed-data.json

# Export data
inferadb dev export > backup.json

# Open dashboard in browser
inferadb dev dashboard

# Reset all data
inferadb dev reset --yes
```

### Service Operator Commands

Available only to service administrators:

```bash
# List service operators
inferadb sop accounts list

# Add service operator
inferadb sop accounts add 222333444555666777

# Remove service operator
inferadb sop accounts remove 222333444555666777

# Run performance benchmark
inferadb bench --concurrency 32 --duration 60s --scenario scenarios/org-rbac.yaml

# View SLOs
inferadb slos

# Check burn rate
inferadb slos burn-rate --window 6h
```

### Command Aliases

```bash
# Set alias
inferadb alias set c check
inferadb alias set lr list-resources

# List aliases
inferadb alias list

# Remove alias
inferadb alias remove c

# Use alias
inferadb c user:alice document:readme can_view  # Expands to: inferadb check ...
```

---

## Troubleshooting

Quick fixes for common issues. For more details, run `inferadb help error <code>`.

### "No such profile: prod"

```bash
# Check available profiles
inferadb profiles list

# Create the missing profile
inferadb profiles create prod \
  --url https://api.inferadb.com \
  --org 123456789012345678 \
  --vault 987654321098765432

# Or set as default after creating
inferadb profiles default prod
```

### "Permission denied" (Exit Code 4)

```bash
# Check your current identity and role
inferadb whoami

# Check who has admin access to request help
inferadb orgs members list --role admin

# Verify you're using the right profile/vault
inferadb profiles show
```

### "Authentication required" (Exit Code 3)

```bash
# Re-authenticate
inferadb @prod login

# Or generate a new vault token
inferadb tokens generate --ttl 7d

# Check token status
inferadb tokens inspect --verify
```

### "Timeout waiting for response"

```bash
# Run diagnostics
inferadb doctor

# Try with explicit timeout
inferadb --timeout 30s check user:alice can_view document:readme

# Check service health
inferadb health --verbose
```

### "Network error" (Exit Code 10)

```bash
# Full network diagnostics
inferadb doctor --check network

# Test connectivity
inferadb ping --count 5

# Check DNS and TLS
inferadb doctor --verbose
```

### "Schema activate failed: breaking changes"

```bash
# Review what would break
inferadb schemas pre-flight SCHEMA_ID

# Use canary deployment for safer rollout
inferadb schemas activate SCHEMA_ID --canary 10

# Or force (dangerous!)
inferadb schemas activate SCHEMA_ID --force --yes
```

### "Rate limited" (Exit Code 7)

```bash
# Check current rate limit status
inferadb whoami --show-limits

# Wait and retry with backoff
sleep 60 && inferadb check user:alice can_view document:readme

# For batch operations, use slower throughput
inferadb import data.json --rate-limit 10/s
```

### "Resource not found" (Exit Code 5)

```bash
# Verify the resource exists
inferadb relationships list --resource document:readme

# Check you're in the right vault
inferadb profiles show

# List available resources of that type
inferadb list-resources document --limit 10
```

### Debug mode for any issue

```bash
# Maximum verbosity
inferadb --debug check user:alice can_view document:readme

# Save debug output to file
inferadb --debug check user:alice can_view document:readme 2> debug.log

# Include timing information
inferadb --debug --timing check user:alice can_view document:readme
```

---

## Examples

### Common Workflows

#### Set up a new project

```bash
# Initialize and authenticate
inferadb init

# Create a vault for the project
inferadb orgs vaults create "My Project" --description "Authorization for my app"
# Returns: Vault ID 987654321098765432

# Update profile to use the new vault
inferadb profiles update dev --vault 987654321098765432

# Push initial schema
inferadb schemas push schema.ipl --activate

# Add some relationships (subject relation resource)
inferadb relationships add \
  "user:alice owner document:readme" \
  "user:bob viewer document:readme"

# Test authorization (subject permission resource)
inferadb check user:bob can_view document:readme
```

#### Debug authorization issues

```bash
# Check with trace (subject permission resource)
inferadb check user:alice can_view document:secret --trace

# Get explanation for denial
inferadb check user:alice can_view document:secret --explain

# Expand to see who has access
inferadb expand document:secret viewer

# List relationships for the resource
inferadb relationships list --resource document:secret

# Simulate adding a relationship
inferadb simulate user:alice can_view document:secret \
  --with "user:alice viewer document:secret"
```

#### User offboarding

```bash
# See what the user has access to
inferadb relationships list --subject user:departing-employee

# Dry-run deletion
inferadb relationships delete --filter --subject user:departing-employee --dry-run

# Delete all relationships
inferadb relationships delete --filter --subject user:departing-employee --yes
```

#### CI/CD integration

```bash
# Validate schema in CI
inferadb schemas validate schema.ipl

# Run tests with exit code for CI
inferadb schemas test --exit-code

# Run batch authorization tests
inferadb check --batch test-cases.json -o json | jq '.results[] | select(.decision != "allow")'

# Export for backup before deployment
inferadb export --output backup-$(date +%Y%m%d).json

# Deploy new schema
inferadb schemas push schema.ipl --activate
```

#### Cross-environment comparison

```bash
# Compare schema differences
inferadb @prod schemas show --active -o yaml > prod-schema.yaml
inferadb @staging schemas show --active -o yaml > staging-schema.yaml
diff prod-schema.yaml staging-schema.yaml

# Compare stats
echo "=== Production ===" && inferadb @prod stats
echo "=== Staging ===" && inferadb @staging stats

# Check if a permission works the same in both environments
inferadb @prod check user:alice can_view document:readme
inferadb @staging check user:alice can_view document:readme
```

#### Audit who has access to a resource

```bash
# Who can view this document?
inferadb list-subjects document:secret viewer

# Get full access tree with inheritance
inferadb expand document:secret viewer --show-paths

# Understand why a specific user has access
inferadb explain-permission user:alice can_view document:secret --graph

# Find all users with a specific permission (across all resources)
inferadb relationships list --relation owner -o table --columns subject,resource
```

#### Safe schema deployment workflow

```bash
# Step 1: Preview changes
inferadb schemas diff schema.ipl --active

# Step 2: Check for breaking changes
inferadb schemas preview schema.ipl --impact

# Step 3: Push as draft (doesn't activate)
SCHEMA_ID=$(inferadb schemas push schema.ipl -o json | jq -r '.id')

# Step 4: Run pre-flight checks
inferadb schemas pre-flight $SCHEMA_ID

# Step 5: Canary deployment (10% traffic)
inferadb schemas activate $SCHEMA_ID --canary 10

# Step 6: Monitor and promote
inferadb schemas canary status
inferadb schemas canary promote  # When ready for 100%
```

#### Migrate relationships from legacy system

```bash
# Export from legacy system (adapt to your format)
legacy-export > legacy-data.json

# Transform to InferaDB format
jq '[.[] | {subject: .user_id, relation: .role, resource: .resource_id}]' \
  legacy-data.json > relationships.json

# Preview import
inferadb import relationships.json --dry-run

# Import atomically
inferadb import relationships.json --atomic --yes

# Verify migration
inferadb stats
inferadb relationships list --limit 10
```

#### Multi-environment development

```bash
# Set up profiles for each environment
inferadb profiles create local --url http://localhost:3000 --org DEV_ORG --vault DEV_VAULT
inferadb profiles create staging --url https://staging.inferadb.com --org ORG --vault STAGING_VAULT
inferadb profiles create prod --url https://api.inferadb.com --org ORG --vault PROD_VAULT

# Use @profile shorthand
inferadb @local schemas push schema.ipl --activate
inferadb @staging check user:alice can_view document:readme
inferadb @prod health

# Or set default
inferadb profiles default staging
inferadb check user:alice can_view document:readme  # Uses staging
```

---

## Sources

- [Command line tool (kubectl) - Kubernetes](https://kubernetes.io/docs/reference/kubectl/)
- [AWS CLI User Guide](https://docs.aws.amazon.com/cli/latest/userguide/)
- [GitHub CLI Manual](https://cli.github.com/manual/)
- [OpenFGA CLI](https://openfga.dev/docs/getting-started/cli)
