# InferaDB CLI - Code Style & Conventions

## Rust Edition & Toolchain

- **Edition**: Rust 2021
- **MSRV**: 1.92
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

## Builder Pattern Conventions

This codebase uses **two builder pattern libraries** for different purposes:

### bon (v3.8+) - Internal Types
Use `bon` for InferaDB CLI's own types:
- **Structs**: `#[derive(bon::Builder)]` for simple data types
- **Impl blocks**: `#[bon]` on impl + `#[builder]` on constructor for types with complex initialization logic
- **Async functions**: `#[builder]` directly on functions with many parameters

**bon patterns:**
- `#[builder(default)]` for optional fields with `Default::default()` value
- `#[builder(default = value)]` for specific non-Default defaults
- `#[builder(into)]` for `impl Into<String>` ergonomics
- `Option<T>` fields are automatically optional (no annotation needed)
- Builder finisher: `.build()` for structs/impl blocks, `.call()` for functions

**Examples:**
- `Context::builder().profile_name(...).build()`
- `InstallStep::builder().name("step").executor(Arc::new(||...)).build()`
- `schemas::copy().ctx(ctx).to_vault(v).call().await`

### teapot - Framework Types
**Never replace teapot builders with bon.** Teapot is an upstream TUI framework, and its patterns should be used as-is:
- `TaskProgressView::builder(steps).title(...).auto_start().build()`
- `InputField::new("name").title("Title").required().build()`

Types that wrap teapot components (e.g., `DevStopView`, `DevInstallView`, `DevUninstallView`) should remain thin delegates—just an `inner: TaskProgressView` field with methods that forward to the inner type.

## Dependencies Philosophy

- Use `clap` with derive macros for CLI
- Use `tokio` for async runtime
- Use `serde` for serialization
- Use `thiserror` for error types
- Use `tracing` for logging
- Use `keyring` for secure credential storage
- Use `teapot` TUI framework for interactive features
- Use `bon` for builder pattern generation (internal types only—see Builder Pattern Conventions above)
