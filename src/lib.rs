//! InferaDB CLI Library
//!
//! This crate provides the command-line interface for InferaDB,
//! built on top of the `inferadb` Rust SDK.
//!
//! ## Usage
//!
//! The CLI can be invoked as `inferadb` with various subcommands:
//!
//! ```bash
//! inferadb login                                    # Authenticate
//! inferadb check user:alice can_view doc:readme    # Check authorization
//! inferadb relationships add user:alice viewer doc:readme
//! ```
//!
//! ## Profiles
//!
//! The CLI supports multiple profiles for different environments:
//!
//! ```bash
//! inferadb @prod check user:alice can_view doc:readme
//! inferadb @staging relationships list
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod cli;
pub mod client;
pub mod commands;
pub mod config;
pub mod error;
pub mod output;

pub use cli::Cli;
pub use error::{Error, Result};

/// CLI version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Run the CLI with the given arguments.
///
/// This is the main entry point for the CLI, parsing arguments and
/// dispatching to the appropriate command handler.
pub async fn run(args: Vec<String>) -> Result<()> {
    use clap::Parser;

    // Parse @profile prefix before clap
    let (profile_override, args) = cli::parse_profile_prefix(args);

    // Parse CLI arguments
    let mut cli_args = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            // Print clap error (includes help/version)
            e.print().ok();
            // Exit successfully for help/version, otherwise return error
            use clap::error::ErrorKind;
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => return Ok(()),
                _ => return Err(Error::other("")),
            }
        }
    };

    // Apply profile override from @prefix
    if cli_args.profile.is_none() {
        cli_args.profile = profile_override;
    }

    // Initialize logging if debug mode
    if cli_args.debug {
        init_logging();
    }

    // Create context
    let ctx = client::Context::new(
        cli_args.profile,
        cli_args.org,
        cli_args.vault,
        cli_args.output,
        cli_args.color,
        cli_args.quiet,
        cli_args.yes,
        cli_args.debug,
    )?;

    // Execute command
    commands::execute(&ctx, &cli_args.command).await
}

/// Initialize tracing/logging for debug mode.
fn init_logging() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("inferadb_cli=debug,inferadb=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();
}
