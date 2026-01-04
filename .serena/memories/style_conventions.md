# InferaDB CLI - Code Style & Conventions

## Rust Edition & Toolchain

- **Edition**: Rust 2021
- **MSRV**: 1.88
- **Toolchain**: Stable (with nightly for rustfmt)
- **Components**: clippy, rust-analyzer, rust-src, rustfmt

## Formatting (.rustfmt.toml)

The project uses rustfmt with the following notable settings:

```toml
style_edition = "2024"
group_imports = "StdExternalCrate"     # Group std, then external, then crate
imports_granularity = "Crate"          # Merge imports by crate
comment_width = 100
wrap_comments = true
normalize_comments = true
match_block_trailing_comma = true
merge_derives = false
newline_style = "Unix"
use_small_heuristics = "MAX"
```

**Key formatting rules:**
- Imports grouped: std → external crates → local crate
- Imports merged at crate level
- Comments wrapped at 100 chars
- Unix newlines

## Code Quality

### Lints

The crate enables strict linting:

```rust
#![warn(missing_docs)]
#![warn(clippy::all)]
```

### Error Handling

- Use `thiserror` for error type definitions
- Use `anyhow` for context propagation in application code
- Custom `Error` enum in `src/error.rs` with semantic exit codes
- All errors implement structured exit codes (see error.rs)

### Naming Conventions

- Standard Rust naming: `snake_case` for functions/variables, `PascalCase` for types
- CLI subcommands use `kebab-case` (e.g., `list-resources`)
- Profile names: `@profile` prefix syntax

### Documentation

- Doc comments (`///`) for public APIs
- Module-level docs (`//!`) at the top of each module
- Examples in doc comments where helpful

## Project Structure

```
src/
├── main.rs          # Entry point
├── lib.rs           # Library exports, run() function
├── cli.rs           # Clap command definitions
├── error.rs         # Error types with exit codes
├── client/          # API client and auth context
├── commands/        # Command implementations
│   ├── mod.rs       # Command dispatch
│   ├── check.rs     # Authorization queries
│   ├── auth.rs      # login/logout/whoami
│   └── ...
├── config/          # Configuration and profiles
├── i18n/            # Localization (Project Fluent)
├── output/          # Output formatting (table/json/yaml)
└── tui/             # Terminal UI components
```

## Dependencies Philosophy

- Use `clap` with derive macros for CLI
- Use `tokio` for async runtime
- Use `serde` for serialization
- Use `thiserror` for error types
- Use `tracing` for logging
- Use `keyring` for secure credential storage
- Use `teapot` TUI framework for interactive features
