# InferaDB CLI

Command-line interface for InferaDB authorization engine, built on the `inferadb` Rust SDK.

## Build Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # Run all tests
cargo test --lib               # Unit tests only
cargo test --test integration  # Integration tests only
cargo clippy -- -D warnings    # Lint
cargo +nightly fmt             # Format
```

## Architecture

### Module Structure

```
src/
├── main.rs           # Entry point
├── lib.rs            # Library exports, run() function
├── cli.rs            # Clap command definitions
├── error.rs          # Error types with exit codes
├── config/           # Configuration system
│   ├── mod.rs        # Config loading, XDG paths
│   └── profile.rs    # Profile and credential management
├── output/           # Output formatting
│   ├── mod.rs        # Format selection, Output writer
│   └── table.rs      # Table formatter
├── client/           # API client wrappers
│   ├── mod.rs        # CliClient, Context
│   └── auth.rs       # OAuth PKCE flow
└── commands/         # Command implementations
    ├── mod.rs        # Command dispatch
    ├── auth.rs       # login, logout, register, init
    ├── identity.rs   # whoami, status, ping, doctor
    ├── check.rs      # check, simulate, expand
    ├── profiles.rs   # profile management
    └── relationships.rs
```

### Key Patterns

1. **Profile Prefix**: `@profile` syntax parsed before clap in `cli::parse_profile_prefix`
2. **Context**: `client::Context` holds config, profile, output settings for commands
3. **Exit Codes**: Defined in `error::Error::exit_code()` per CLI spec
4. **Output**: Commands use `ctx.output` for format-aware output (table/json/yaml)

### SDK Integration

Uses `inferadb` crate from `../sdks/rust`:

```rust
use inferadb::{Client, VaultClient, Relationship};

// Check: subject, permission, resource order
vault.check("user:alice", "view", "doc:1").await?

// Relationship: resource, relation, subject order
Relationship::new("doc:1", "viewer", "user:alice")
```

Key: `check()` returns `Ok(false)` for denial, not `Err`. Use exit code 20 for denied.

### Configuration

- User config: `~/.config/inferadb/cli.yaml`
- Project config: `.inferadb-cli.yaml`
- Credentials: OS keychain via `keyring` crate
- Env vars: `INFERADB_*` prefix

### Testing

```bash
cargo test                           # All tests
cargo test test_help                 # Single test
cargo test --test integration        # Integration tests
INFERADB_DEBUG=1 cargo test          # With debug output
```

Integration tests use `assert_cmd` for CLI testing and `tempfile` for isolated configs.

## Important Notes

- Always use `ctx.output` for output, never raw `println!` in commands
- Exit code 20 = authorization denied (not an error, a decision)
- Profile credentials stored in OS keychain, not config file
- `@profile` prefix must come before any other arguments
