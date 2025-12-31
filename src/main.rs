//! InferaDB CLI
//!
//! Command-line interface for the InferaDB authorization engine.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    match inferadb_cli::run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Don't print if it's an empty error (e.g., from clap --help)
            let msg = e.to_string();
            if !msg.is_empty() {
                eprintln!("Error: {}", e);

                // Show hint if relevant
                if e.should_suggest_login() {
                    eprintln!();
                    eprintln!("Run 'inferadb login' to authenticate.");
                }
            }

            // Return appropriate exit code
            let code = e.exit_code();
            ExitCode::from(code as u8)
        },
    }
}
