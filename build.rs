//! Build script for `InferaDB` CLI.
//!
//! This sets Windows-specific linker flags to increase the default stack size.

fn main() {
    // Increase stack size on Windows to 8MB (default is 1MB, Linux/macOS default is 8MB).
    // This is needed because our CLI has deeply nested command enums that use significant
    // stack space during clap parsing.
    //
    // We check CARGO_CFG_TARGET_OS (not #[cfg]) to support cross-compilation.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg=/STACK:8388608");
    }
}
