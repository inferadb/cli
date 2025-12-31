//! Async spinner utilities for CLI operations.
//!
//! Provides easy-to-use spinners for long-running async operations.
//!
//! # Example
//!
//! ```rust,ignore
//! use inferadb_cli::tui;
//!
//! // Simple spinner that runs during an async operation
//! let result = tui::spin("Pushing schema...", async {
//!     client.schemas().push(&content).await
//! }).await;
//! ```

use std::future::Future;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use teapot::components::{Spinner, SpinnerStyle};
use teapot::output::{is_ci, is_tty};
use teapot::style::{Color, CLEAR_LINE};
use teapot::Model;

/// Handle to control a running spinner.
pub struct SpinnerHandle {
    running: Arc<AtomicBool>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl SpinnerHandle {
    /// Stop the spinner.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }

    /// Stop with a success message.
    pub fn success(mut self, message: &str) {
        self.stop();
        clear_line();
        teapot::output::success(message);
    }

    /// Stop with an error message.
    pub fn error(mut self, message: &str) {
        self.stop();
        clear_line();
        teapot::output::error(message);
    }

    /// Stop with a failure message (alias for error).
    pub fn failure(mut self, message: &str) {
        self.stop();
        clear_line();
        teapot::output::error(message);
    }

    /// Stop with a warning message.
    pub fn warning(mut self, message: &str) {
        self.stop();
        clear_line();
        teapot::output::warning(message);
    }

    /// Stop with an info message.
    pub fn info(mut self, message: &str) {
        self.stop();
        clear_line();
        teapot::output::info(message);
    }

    /// Stop without any message (clears the line).
    pub fn clear(mut self) {
        self.stop();
        clear_line();
    }
}

impl Drop for SpinnerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start a spinner that can be manually controlled.
///
/// Returns a handle that must be stopped when the operation completes.
///
/// # Example
///
/// ```rust,ignore
/// let mut handle = tui::spinner::start("Loading...");
/// let result = do_something().await;
/// if result.is_ok() {
///     handle.success("Done!");
/// } else {
///     handle.error("Failed!");
/// }
/// ```
pub fn start(message: impl Into<String>) -> SpinnerHandle {
    let message = message.into();
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // In non-interactive mode, just print the message
    if !is_tty() || is_ci() {
        teapot::output::info(&message);
        return SpinnerHandle {
            running,
            join_handle: None,
        };
    }

    let join_handle = std::thread::spawn(move || {
        let mut spinner = Spinner::new()
            .style(SpinnerStyle::Dots)
            .color(Color::Cyan)
            .message(&message);

        let sleep_duration = SpinnerStyle::Dots.interval();

        while running_clone.load(Ordering::SeqCst) {
            // Clear line and print spinner
            eprint!("\r{}{}", CLEAR_LINE, spinner.view());
            let _ = io::stderr().flush();

            spinner.tick();
            std::thread::sleep(sleep_duration);
        }

        // Clear the spinner line when done
        eprint!("\r{}", CLEAR_LINE);
        let _ = io::stderr().flush();
    });

    SpinnerHandle {
        running,
        join_handle: Some(join_handle),
    }
}

/// Run an async operation with a spinner.
///
/// Shows a spinner while the future executes, then returns the result.
/// In non-interactive mode, prints the message and runs without animation.
///
/// # Example
///
/// ```rust,ignore
/// let result = tui::spin("Fetching data...", async {
///     client.fetch().await
/// }).await;
/// ```
pub async fn spin<F, T>(message: impl Into<String>, future: F) -> T
where
    F: Future<Output = T>,
{
    let handle = start(message);
    let result = future.await;
    handle.clear();
    result
}

/// Run an async operation with a spinner, showing success/error on completion.
///
/// If the operation returns `Ok`, shows a success message.
/// If it returns `Err`, shows an error message.
///
/// # Example
///
/// ```rust,ignore
/// let result = tui::spin_result(
///     "Pushing schema...",
///     "Schema pushed successfully",
///     async { client.schemas().push(&content).await }
/// ).await;
/// ```
pub async fn spin_result<F, T, E>(
    in_progress_message: impl Into<String>,
    success_message: impl Into<String>,
    future: F,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let success_msg = success_message.into();
    let handle = start(in_progress_message);

    match future.await {
        Ok(value) => {
            handle.success(&success_msg);
            Ok(value)
        }
        Err(e) => {
            handle.error(&e.to_string());
            Err(e)
        }
    }
}

/// Clear the current line.
fn clear_line() {
    if is_tty() && !is_ci() {
        eprint!("\r{}", CLEAR_LINE);
        let _ = io::stderr().flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_spinner_creation() {
        // Just test that we can create a spinner without panicking
        let handle = start("Test");
        std::thread::sleep(Duration::from_millis(100));
        drop(handle);
    }

    #[tokio::test]
    async fn test_spin_async() {
        let result = spin("Testing...", async { 42 }).await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_spin_result_ok() {
        let result: Result<i32, String> = spin_result("Testing...", "Done", async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_spin_result_err() {
        let result: Result<i32, String> =
            spin_result("Testing...", "Done", async { Err("failed".to_string()) }).await;
        assert!(result.is_err());
    }
}
