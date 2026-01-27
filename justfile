# Development commands for inferadb-cli

# Default recipe - show available commands
default:
    @just --list

# Build the project
build:
    cargo build

# Run all tests
test:
    cargo test --all-targets

# Run clippy lints
lint:
    cargo clippy --all-targets -- -D warnings

# Format code (requires nightly)
fmt:
    cargo +nightly fmt

# Check formatting without modifying files
fmt-check:
    cargo +nightly fmt -- --check

# Quick compilation check
check:
    cargo check --all-targets

# Build documentation
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

# Run all CI checks locally
ci: fmt-check lint test doc

# Clean build artifacts
clean:
    cargo clean
