# InferaDB CLI - Task Completion Checklist

When completing a coding task, ensure the following checks pass:

## Required Checks

### 1. Format Code
```bash
cargo +nightly fmt
```

### 2. Run Clippy
```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
- All warnings must be fixed (CI uses `-D warnings`)

### 3. Run Tests
```bash
# Unit tests
cargo test

# Or with nextest (faster)
cargo nextest run --workspace --lib --no-fail-fast
```

### 4. Build Successfully
```bash
cargo build --workspace --all-features
```

## Additional Checks (When Applicable)

### If Documentation Changed
```bash
cargo doc --workspace --no-deps --all-features
```

### If Public API Changed
- Ensure doc comments are present (`#![warn(missing_docs)]` is enabled)
- Update README.md command reference if CLI interface changed

### If Adding New Commands
- Add to `Commands` enum in `src/cli.rs`
- Implement handler in appropriate `src/commands/*.rs` file
- Add localization keys in `src/i18n/locales/en-US.ftl`
- Update README.md command table

### If Changing Error Handling
- Review exit codes in `src/error.rs`
- Ensure exit code semantics are maintained

## CI Pipeline Expectations

The CI runs these jobs (all must pass):
1. **fmt** - Formatting check (nightly)
2. **clippy** - Lint check
3. **build** - Workspace build
4. **test** - Unit tests with nextest
5. **docs** - Documentation build (nightly)
