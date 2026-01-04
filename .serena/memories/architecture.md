# InferaDB CLI - Architecture Overview

## Purpose

InferaDB CLI is a command-line tool for managing authorization in InferaDB. It provides:
- Authentication (OAuth PKCE flow, credential storage)
- Authorization queries (check, simulate, expand, list-resources)
- Relationship management (CRUD operations on authorization tuples)
- Schema management (push, activate, validate)
- Organization/vault/team administration
- Local development environment (`dev` commands with Docker/Kubernetes)

## Tech Stack

- **Language**: Rust 2021, MSRV 1.88
- **CLI Framework**: `clap` 4.x with derive macros
- **Async Runtime**: `tokio` (full features)
- **API Client**: `inferadb` SDK crate (REST with rustls)
- **TUI Framework**: `teapot` for interactive components
- **Credential Storage**: `keyring` (OS keychain integration)
- **OAuth**: `oauth2` crate with PKCE flow
- **Localization**: Project Fluent (`fluent-bundle`)

## Module Architecture

### Entry Points
- `main.rs` - Binary entry, calls `lib::run()`
- `lib.rs` - Library entry, argument parsing, i18n init, command dispatch

### Core Modules

**cli.rs** - Command definitions using clap derive
- `Cli` struct with global flags
- `Commands` enum with all subcommands
- Localization support for help text

**error.rs** - Error handling
- `Error` enum with semantic variants
- Exit code mapping (0=success, 20=denied, 21=indeterminate, etc.)
- Localized error messages

**client/** - API communication
- `Context` - Request context with auth, org, vault
- `auth.rs` - OAuth flow, token management

**commands/** - Command implementations
- `mod.rs` - `execute()` dispatcher
- Per-command modules: `check.rs`, `auth.rs`, `relationships.rs`, etc.
- `dev/` - Local development commands (Docker, Kubernetes)

**config/** - Configuration
- `mod.rs` - Config loading, file locations
- `profile.rs` - Profile management, defaults

**i18n/** - Internationalization
- `bundle.rs` - Fluent bundle management
- `locales/` - Translation files (en-US.ftl)

**output/** - Output formatting
- Table, JSON, YAML, JSONL formats
- Color and width handling

**tui/** - Terminal UI
- Spinner, progress, forms
- Interactive views (install, status, etc.)

## Design Patterns

### Command Pattern
Each subcommand is a function in `src/commands/` that:
1. Receives `&Context` and parsed args
2. Performs API calls via the InferaDB SDK
3. Formats and outputs results
4. Returns `Result<()>`

### Profile System
- Profiles stored in `~/.config/inferadb/cli.yaml`
- `@profile` prefix syntax for per-command override
- Environment variables for CI/automation

### Output Strategy
- `OutputFormat` enum (Table, Json, Yaml, Jsonl)
- `--output` flag on all commands
- Uses teapot's Table component for terminal tables

### Error Handling Strategy
- `Error` enum with `thiserror` derivation
- Semantic exit codes for scripting
- `localized_message()` for i18n support
