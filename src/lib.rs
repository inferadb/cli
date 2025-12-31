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
pub mod i18n;
pub mod output;
pub mod tui;

pub use cli::Cli;
pub use error::{Error, Result};

/// CLI version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Run the CLI with the given arguments.
///
/// This is the main entry point for the CLI, parsing arguments and
/// dispatching to the appropriate command handler.
pub async fn run(args: Vec<String>) -> Result<()> {
    // Parse @profile prefix before clap
    let (profile_override, args) = cli::parse_profile_prefix(args);

    // Pre-scan for --lang to initialize i18n before full parse
    let lang = extract_lang_arg(&args);
    let lang_supported = i18n::init(&lang);

    // Show warning if language not supported (after i18n init so we can use translations)
    if !lang_supported && lang != "en-US" {
        eprintln!("Warning: Language '{}' is not supported. Falling back to en-US.", lang);
        eprintln!("Supported languages: {}", i18n::SUPPORTED_LOCALES.join(", "));
    }

    // Parse CLI arguments using localized command
    let mut cli_args = match Cli::command_localized().try_get_matches_from(&args) {
        Ok(matches) => {
            use clap::FromArgMatches;
            Cli::from_arg_matches(&matches).map_err(|e| {
                e.print().ok();
                Error::other("")
            })?
        },
        Err(e) => {
            // Print clap error (includes help/version)
            e.print().ok();
            // Exit successfully for help/version, otherwise return error
            use clap::error::ErrorKind;
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => return Ok(()),
                _ => return Err(Error::other("")),
            }
        },
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
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("inferadb_cli=debug,inferadb=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();
}

/// Extract the --lang argument from args before full parsing.
///
/// This allows us to initialize i18n with the correct language before
/// parsing the full command (which may need localized help text).
fn extract_lang_arg(args: &[String]) -> String {
    // Check for --lang=value or --lang value
    for (i, arg) in args.iter().enumerate() {
        if let Some(lang) = arg.strip_prefix("--lang=") {
            return lang.to_string();
        }
        if arg == "--lang" {
            if let Some(lang) = args.get(i + 1) {
                return lang.to_string();
            }
        }
    }

    // Check INFERADB_LANG environment variable
    if let Ok(lang) = std::env::var("INFERADB_LANG") {
        if !lang.is_empty() {
            return lang;
        }
    }

    // Default to en-US
    "en-US".to_string()
}
