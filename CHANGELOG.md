# Changelog

All notable changes to the InferaDB CLI will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Authentication commands: `login`, `logout`, `register`, `init`
- Identity commands: `whoami`, `status`, `ping`, `doctor`, `health`
- Authorization commands: `check`, `simulate`, `expand`, `explain-permission`
- Lookup commands: `list-resources`, `list-subjects`
- Relationship management: `relationships list/add/remove`, `export`, `import`
- Schema management: `schemas list/show/apply/validate`
- Profile management: `profiles list/create/update/delete/default`
- Configuration management: `config show/edit/path/explain`
- Organization management: `orgs list/show/create`
- Token management: `tokens list/create/revoke`
- Account management: `account show/update/delete`
- Utility commands: `stats`, `what-changed`, `stream`, `cheatsheet`, `templates`, `guide`
- Local development environment commands: `dev start`, `dev stop`, `dev status`, `dev doctor`, `dev install`, `dev logs`
- Shell completion generation for bash, zsh, fish, and PowerShell
- Multiple output formats: table, json, yaml, jsonl
- Profile-based configuration with `@profile` prefix syntax
- OAuth PKCE authentication flow
- Secure credential storage via OS keychain
- Styled terminal output with `ProgressBox` for multi-step operations
- Tailscale device cleanup during `dev stop --destroy`
- Support for `--commit` flag in `dev install` to clone specific versions
- Dev environment Tailscale devices use `inferadb-dev-` prefix to avoid conflicts with staging/production

[Unreleased]: https://github.com/inferadb/cli/commits/main
