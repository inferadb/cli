<div align="center">
    <p><a href="https://inferadb.com"><img src=".github/inferadb.png" width="100" /></a></p>
    <h1>InferaDB CLI</h1>
    <p>
        <a href="https://discord.gg/inferadb"><img src="https://img.shields.io/badge/Discord-Join%20us-5865F2?logo=discord&logoColor=white" alt="Discord" /></a>
        <a href="#license"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License" /></a>
    </p>
    <p>Complete authorization management from your terminal</p>
</div>

> [!IMPORTANT]
> Under active development. Not production-ready.

## Installation

```bash
git clone https://github.com/inferadb/cli.git
cd cli
cargo build --release
# Binary: target/release/inferadb
```

Shell completions:

```bash
inferadb completion bash > ~/.local/share/bash-completion/completions/inferadb
inferadb completion zsh > ~/.zfunc/_inferadb
inferadb completion fish > ~/.config/fish/completions/inferadb.fish
```

## Quick Start

```bash
inferadb login                                      # Authenticate
inferadb whoami                                     # Check identity
inferadb check user:alice can_view document:readme  # Check authorization
inferadb list-resources user:alice can_view         # List accessible resources
inferadb relationships add document:readme#viewer@user:bob
```

## Commands

Run `inferadb --help` for the full command list. Key command groups:

| Group | Commands |
|-------|----------|
| **Auth** | `login`, `logout`, `register`, `whoami` |
| **Queries** | `check`, `simulate`, `expand`, `explain-permission`, `list-resources`, `list-subjects` |
| **Data** | `relationships`, `export`, `import`, `stream`, `stats`, `what-changed` |
| **Schema** | `schemas` (init, push, validate, test, diff, visualize, etc.) |
| **Admin** | `account`, `orgs`, `tokens` |
| **Diagnostics** | `status`, `ping`, `doctor`, `health`, `jwks` |
| **Help** | `cheatsheet`, `templates`, `guide`, `shell` |
| **Dev** | `dev` (doctor, start, stop, status, logs, reset) |
| **Config** | `profiles`, `config`, `completion` |

## Global Flags

| Flag | Description |
|------|-------------|
| `@<profile>` | Use specific profile (e.g., `@prod check ...`) |
| `--org` | Override organization |
| `-v, --vault` | Override vault |
| `-o, --output` | Format: `table`, `json`, `yaml`, `jsonl` |
| `-q, --quiet` | Suppress non-essential output |
| `-y, --yes` | Skip confirmation prompts |
| `--debug` | Enable debug logging |

## Configuration

| Location | Purpose |
|----------|---------|
| `~/.config/inferadb/cli.yaml` | User configuration |
| `.inferadb-cli.yaml` | Project configuration |
| OS Keychain | Credentials |

```yaml
# ~/.config/inferadb/cli.yaml
default_profile: production
profiles:
  production:
    url: https://api.inferadb.com
    org: org_abc123
    vault: vault_xyz789
```

Environment variables: `INFERADB_PROFILE`, `INFERADB_URL`, `INFERADB_ORG`, `INFERADB_VAULT`, `INFERADB_TOKEN`, `INFERADB_DEBUG`, `NO_COLOR`

## Exit Codes

| Code | Meaning | Code | Meaning |
|------|---------|------|---------|
| 0 | Success | 5 | Not found |
| 1 | General error | 6 | Conflict |
| 2 | Invalid arguments | 7 | Rate limited |
| 3 | Auth required | 10 | Network error |
| 4 | Permission denied | 11 | Server error |

Authorization-specific (`check` command):

| Code | Meaning |
|------|---------|
| 0 | Allowed |
| 20 | Denied |
| 21 | Indeterminate |

## Development

```bash
just        # List available commands
just ci     # Run all checks (fmt, lint, test, doc)
just test   # Run tests
just lint   # Run clippy
just fmt    # Format code
```

See [CLAUDE.md](CLAUDE.md) for architecture details.

## Community

Join us on [Discord](https://discord.gg/inferadb) for questions and discussions.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE).
