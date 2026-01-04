# InferaDB CLI - Development Commands

## Build Commands

```bash
# Standard build
cargo build

# Release build
cargo build --release

# Build with all features
cargo build --workspace --all-features
```

## Testing Commands

```bash
# Run all unit tests
cargo test

# Run unit tests with nextest (faster, used in CI)
cargo nextest run --workspace --lib --no-fail-fast

# Run doc tests
cargo test --workspace --doc

# Run integration tests
cargo test --test integration
```

## Linting & Formatting

```bash
# Format code (requires nightly)
cargo +nightly fmt

# Check formatting
cargo +nightly fmt --all -- --check

# Run clippy lints
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Documentation

```bash
# Build documentation
cargo doc --workspace --no-deps --all-features

# Build with nightly (enables docsrs features)
RUSTDOCFLAGS="-D warnings --cfg docsrs" cargo +nightly doc --workspace --no-deps --all-features
```

## Running the CLI

```bash
# Run from source
cargo run -- <args>

# Examples:
cargo run -- --help
cargo run -- whoami
cargo run -- check user:alice can_view document:readme
```

## System Utilities (macOS/Darwin)

```bash
# Standard Unix utilities available
git, ls, cd, grep, find, cat, head, tail

# Note: Some BSD variants differ from GNU
# Example: `sed -i ''` instead of `sed -i` for in-place edit
```
