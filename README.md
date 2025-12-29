<div align="center">
    <p><a href="https://inferadb.com"><img src=".github/inferadb.png" width="100" /></a></p>
    <h1>InferaDB CLI</h1>
    <p>Complete authorization management without leaving your terminal</p>
</div>

<br />

The [InferaDB](https://inferadb.com) CLI gives you instant visibility into authorization decisions, lets you test policy changes before deploying, and provides complete control over tenants, schemas, and relationships. From local development to production debugging, everything is one command away.

## Installation

### From Source

```bash
git clone https://github.com/inferadb/cli.git
cd cli
cargo build --release
```

The binary will be available at `target/release/inferadb`.

### Shell Completions

```bash
inferadb completion bash > ~/.local/share/bash-completion/completions/inferadb
inferadb completion zsh > ~/.zfunc/_inferadb
inferadb completion fish > ~/.config/fish/completions/inferadb.fish
```

## Quick Start

```bash
# Authenticate
inferadb login

# Check who you are
inferadb whoami

# Check authorization
inferadb check user:alice can_view document:readme

# List what a user can access
inferadb list-resources user:alice can_view

# Manage relationships
inferadb relationships add document:readme#viewer@user:bob
inferadb relationships list --resource document:readme
```

## Command Reference

### Authentication & Identity

| Command             | Description                   |
| ------------------- | ----------------------------- |
| `inferadb login`    | Authenticate with InferaDB    |
| `inferadb logout`   | Remove authentication         |
| `inferadb register` | Create a new account          |
| `inferadb whoami`   | Show current user and profile |

### Authorization Queries

| Command                                                         | Description                                   |
| --------------------------------------------------------------- | --------------------------------------------- |
| `inferadb check <subject> <permission> <resource>`              | Check if authorized                           |
| `inferadb simulate <subject> <permission> <resource>`           | Simulate with hypothetical changes            |
| `inferadb expand <resource> <relation>`                         | Show userset expansion tree                   |
| `inferadb explain-permission <subject> <permission> <resource>` | Explain permission computation                |
| `inferadb list-resources <subject> <permission>`                | List accessible resources (alias: `what-can`) |
| `inferadb list-subjects <resource> <permission>`                | List subjects with access (alias: `who-can`)  |

### Relationship Management

| Command                                 | Description                          |
| --------------------------------------- | ------------------------------------ |
| `inferadb relationships list`           | List relationships                   |
| `inferadb relationships add <tuple>`    | Add a relationship                   |
| `inferadb relationships delete <tuple>` | Remove a relationship                |
| `inferadb relationships history`        | Show relationship history            |
| `inferadb relationships validate`       | Validate relationship tuples         |
| `inferadb export`                       | Export relationships to file         |
| `inferadb import <file>`                | Import relationships from file       |
| `inferadb stream`                       | Watch real-time relationship changes |

### Schema Management

| Command                               | Description                     |
| ------------------------------------- | ------------------------------- |
| `inferadb schemas init`               | Initialize a new schema project |
| `inferadb schemas list`               | List schema versions            |
| `inferadb schemas get <version>`      | Show schema details             |
| `inferadb schemas preview`            | Preview schema changes          |
| `inferadb schemas push`               | Push schema to vault            |
| `inferadb schemas activate <version>` | Activate a schema version       |
| `inferadb schemas rollback`           | Rollback to previous version    |
| `inferadb schemas diff`               | Compare schema versions         |
| `inferadb schemas validate <file>`    | Validate schema file            |
| `inferadb schemas format <file>`      | Format schema file              |
| `inferadb schemas analyze`            | Analyze schema for issues       |
| `inferadb schemas test`               | Run authorization test cases    |
| `inferadb schemas visualize`          | Generate schema diagrams        |
| `inferadb schemas watch`              | Auto-validate on file change    |
| `inferadb schemas develop`            | Unified dev workflow            |
| `inferadb schemas copy`               | Copy schema between vaults      |
| `inferadb schemas migrate`            | Generate migration helpers      |
| `inferadb schemas canary status`      | Check canary deployment status  |
| `inferadb schemas canary promote`     | Promote canary to production    |
| `inferadb schemas canary rollback`    | Rollback canary deployment      |

### Account Management

| Command                                   | Description               |
| ----------------------------------------- | ------------------------- |
| `inferadb account show`                   | Show account details      |
| `inferadb account update`                 | Update account settings   |
| `inferadb account delete`                 | Delete account            |
| `inferadb account emails list`            | List email addresses      |
| `inferadb account emails add`             | Add email address         |
| `inferadb account emails verify`          | Verify email address      |
| `inferadb account emails remove`          | Remove email address      |
| `inferadb account emails set-primary`     | Set primary email         |
| `inferadb account sessions list`          | List active sessions      |
| `inferadb account sessions revoke`        | Revoke a session          |
| `inferadb account sessions revoke-others` | Revoke all other sessions |
| `inferadb account password reset`         | Reset password            |

### Organization Management

| Command                             | Description               |
| ----------------------------------- | ------------------------- |
| `inferadb orgs list`                | List organizations        |
| `inferadb orgs create <name>`       | Create organization       |
| `inferadb orgs get <org>`           | Show organization details |
| `inferadb orgs update <org>`        | Update organization       |
| `inferadb orgs delete <org>`        | Delete organization       |
| `inferadb orgs suspend <org>`       | Suspend organization      |
| `inferadb orgs resume <org>`        | Resume organization       |
| `inferadb orgs leave <org>`         | Leave organization        |
| `inferadb orgs members list`        | List organization members |
| `inferadb orgs members update-role` | Update member role        |
| `inferadb orgs members remove`      | Remove member             |
| `inferadb orgs invitations list`    | List pending invitations  |
| `inferadb orgs invitations create`  | Create invitation         |
| `inferadb orgs invitations delete`  | Delete invitation         |
| `inferadb orgs invitations accept`  | Accept invitation         |
| `inferadb orgs roles list`          | List role assignments     |
| `inferadb orgs roles grant`         | Grant role to user        |
| `inferadb orgs roles revoke`        | Revoke role from user     |

### Team Management

| Command                                  | Description           |
| ---------------------------------------- | --------------------- |
| `inferadb orgs teams list`               | List teams            |
| `inferadb orgs teams create`             | Create team           |
| `inferadb orgs teams get <team>`         | Show team details     |
| `inferadb orgs teams update <team>`      | Update team           |
| `inferadb orgs teams delete <team>`      | Delete team           |
| `inferadb orgs teams members list`       | List team members     |
| `inferadb orgs teams members add`        | Add team member       |
| `inferadb orgs teams members remove`     | Remove team member    |
| `inferadb orgs teams permissions list`   | List team permissions |
| `inferadb orgs teams permissions grant`  | Grant permission      |
| `inferadb orgs teams permissions revoke` | Revoke permission     |

### Vault Management

| Command                                | Description           |
| -------------------------------------- | --------------------- |
| `inferadb orgs vaults list`            | List vaults           |
| `inferadb orgs vaults create`          | Create vault          |
| `inferadb orgs vaults get <vault>`     | Show vault details    |
| `inferadb orgs vaults update <vault>`  | Update vault          |
| `inferadb orgs vaults delete <vault>`  | Delete vault          |
| `inferadb orgs vaults roles list`      | List vault user roles |
| `inferadb orgs vaults roles grant`     | Grant vault role      |
| `inferadb orgs vaults roles revoke`    | Revoke vault role     |
| `inferadb orgs vaults team-roles list` | List vault team roles |
| `inferadb orgs vaults tokens list`     | List vault tokens     |
| `inferadb orgs vaults tokens generate` | Generate vault token  |
| `inferadb orgs vaults tokens revoke`   | Revoke vault token    |

### Client Management

| Command                                     | Description         |
| ------------------------------------------- | ------------------- |
| `inferadb orgs clients list`                | List clients        |
| `inferadb orgs clients create`              | Create client       |
| `inferadb orgs clients get <client>`        | Show client details |
| `inferadb orgs clients update <client>`     | Update client       |
| `inferadb orgs clients delete <client>`     | Delete client       |
| `inferadb orgs clients deactivate <client>` | Deactivate client   |
| `inferadb orgs clients certificates list`   | List certificates   |
| `inferadb orgs clients certificates create` | Create certificate  |
| `inferadb orgs clients certificates revoke` | Revoke certificate  |

### Token Management

| Command                           | Description           |
| --------------------------------- | --------------------- |
| `inferadb tokens generate`        | Generate access token |
| `inferadb tokens list`            | List tokens           |
| `inferadb tokens revoke <token>`  | Revoke token          |
| `inferadb tokens refresh`         | Refresh token         |
| `inferadb tokens inspect <token>` | Inspect token claims  |

### Diagnostics & Monitoring

| Command                    | Description                   |
| -------------------------- | ----------------------------- |
| `inferadb status`          | Check service status          |
| `inferadb ping`            | Measure latency to service    |
| `inferadb doctor`          | Run connectivity diagnostics  |
| `inferadb health`          | Show service health dashboard |
| `inferadb stats`           | Vault relationship statistics |
| `inferadb what-changed`    | Recent vault changes summary  |
| `inferadb orgs audit-logs` | View audit logs               |
| `inferadb jwks`            | JWKS debugging operations     |

### Profile Management

| Command                                | Description                       |
| -------------------------------------- | --------------------------------- |
| `inferadb profiles list`               | List all profiles                 |
| `inferadb profiles create <name>`      | Create a new profile              |
| `inferadb profiles show [name]`        | Show profile details              |
| `inferadb profiles update <name>`      | Update profile                    |
| `inferadb profiles rename <old> <new>` | Rename profile                    |
| `inferadb profiles delete <name>`      | Delete profile                    |
| `inferadb profiles default [name]`     | Get/set default profile           |
| `inferadb @<profile> <command>`        | Run command with specific profile |

### Configuration

| Command                | Description                  |
| ---------------------- | ---------------------------- |
| `inferadb config show` | Show current configuration   |
| `inferadb config edit` | Edit configuration file      |
| `inferadb config path` | Show configuration file path |

### Help & Learning

| Command               | Description                   |
| --------------------- | ----------------------------- |
| `inferadb cheatsheet` | Quick reference card          |
| `inferadb templates`  | Copy-paste workflow templates |
| `inferadb guide`      | Opinionated workflow guides   |
| `inferadb shell`      | Interactive REPL              |

### Local Development

| Command                       | Description                   |
| ----------------------------- | ----------------------------- |
| `inferadb dev doctor`         | Check development environment |
| `inferadb dev start`          | Start local Talos cluster     |
| `inferadb dev stop`           | Pause local cluster           |
| `inferadb dev stop --destroy` | Destroy local cluster         |
| `inferadb dev status`         | Show cluster status           |
| `inferadb dev logs`           | View cluster logs             |
| `inferadb dev dashboard`      | Open dashboard in browser     |
| `inferadb dev reset`          | Reset all cluster data        |
| `inferadb dev import <file>`  | Import data into cluster      |
| `inferadb dev export <file>`  | Export data from cluster      |

## Global Flags

| Flag       | Short | Description                                     |
| ---------- | ----- | ----------------------------------------------- |
| `@<name>`  |       | Use specific profile (e.g., `@prod`)            |
| `--org`    |       | Override organization from profile              |
| `--vault`  | `-v`  | Override vault from profile                     |
| `--output` | `-o`  | Output format: `json`, `yaml`, `table`, `jsonl` |
| `--color`  |       | Color output: `auto`, `always`, `never`         |
| `--quiet`  | `-q`  | Suppress non-essential output                   |
| `--debug`  |       | Enable debug logging                            |
| `--lang`   |       | Language for CLI output (e.g., `en-US`)         |
| `--yes`    | `-y`  | Skip confirmation prompts                       |
| `--help`   | `-h`  | Show help                                       |
| `--version`| `-V`  | Show version                                    |

### Scripting Flags

These flags are available on specific commands to support scripting:

| Flag              | Description                                      |
| ----------------- | ------------------------------------------------ |
| `--if-exists`     | No error if resource doesn't exist (for delete)  |
| `--if-not-exists` | No error if resource already exists (for create) |

## Configuration

### File Locations

| File                          | Purpose                     |
| ----------------------------- | --------------------------- |
| `~/.config/inferadb/cli.yaml` | User configuration          |
| `.inferadb-cli.yaml`          | Project configuration       |
| OS Keychain                   | Credentials (via `keyring`) |

### Environment Variables

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

### Profile Configuration

```yaml
# ~/.config/inferadb/cli.yaml
default_profile: production

profiles:
  production:
    url: https://api.inferadb.com
    org: org_abc123
    vault: vault_xyz789

  staging:
    url: https://staging-api.inferadb.com
    org: org_staging
```

## Output Formats

The CLI supports multiple output formats via the `--output` flag:

```bash
inferadb --output json relationships list
inferadb --output yaml check user:alice can_view doc:1
inferadb --output table relationships list  # default
inferadb --output jsonl export              # JSON Lines
```

## Exit Codes

### General Exit Codes

| Code | Meaning                 | Next Steps                    |
| ---- | ----------------------- | ----------------------------- |
| 0    | Success                 | None                          |
| 1    | General error           | Check `--help`                |
| 2    | Invalid arguments       | Fix argument format           |
| 3    | Authentication required | Run `inferadb login`          |
| 4    | Permission denied       | Contact org admin             |
| 5    | Resource not found      | Verify resource exists        |
| 6    | Conflict                | Use `--force` or delete first |
| 7    | Rate limited            | Retry with backoff            |
| 10   | Network error           | Run `inferadb doctor`         |
| 11   | Server error            | Check status page             |

### Authorization Decision Exit Codes

The `check` command uses dedicated exit codes to distinguish authorization decisions from errors:

| Code | Meaning       | Description                                          |
| ---- | ------------- | ---------------------------------------------------- |
| 0    | Allowed       | Authorization granted                                |
| 20   | Denied        | Authorization denied (policy decision, not an error) |
| 21   | Indeterminate | Could not determine (missing data, policy error)     |

```bash
# Use in scripts
inferadb check user:alice can_view document:readme
case $? in
  0)  echo "Access granted" ;;
  20) echo "Access denied" ;;
  21) echo "Could not determine" ;;
  *)  echo "Error occurred" ;;
esac
```

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo +nightly fmt
```

See [CLAUDE.md](CLAUDE.md) for architecture details.

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.
