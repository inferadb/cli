//! Progress bar utilities for bulk operations.
//!
//! Provides easy-to-use progress tracking for operations with known counts.
//!
//! # Example
//!
//! ```rust,ignore
//! use inferadb_cli::tui;
//!
//! let mut progress = tui::progress("Importing relationships", total_count);
//! for item in items {
//!     import_item(item).await?;
//!     progress.inc();
//! }
//! progress.finish("Import complete");
//! ```

use std::{
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use teapot::{
    Model,
    components::Progress as TeapotProgress,
    output::{is_ci, is_tty},
    style::{CLEAR_LINE, CURSOR_UP, Color},
};

/// A progress bar handle for tracking operation progress.
pub struct ProgressBar {
    current: Arc<AtomicU64>,
    total: u64,
    message: String,
    running: Arc<AtomicBool>,
    join_handle: Option<std::thread::JoinHandle<()>>,
    start_time: Instant,
}

impl ProgressBar {
    /// Increment progress by one.
    pub fn inc(&self) {
        self.inc_by(1);
    }

    /// Increment progress by a specific amount.
    pub fn inc_by(&self, amount: u64) {
        self.current.fetch_add(amount, Ordering::SeqCst);
    }

    /// Set progress to a specific value.
    pub fn set(&self, value: u64) {
        self.current.store(value.min(self.total), Ordering::SeqCst);
    }

    /// Get current progress value.
    pub fn current(&self) -> u64 {
        self.current.load(Ordering::SeqCst)
    }

    /// Get total count.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Update the message.
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    /// Get elapsed time.
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Finish with a success message.
    pub fn finish(mut self, message: &str) {
        self.stop();
        clear_line();
        let elapsed = format_duration(self.elapsed());
        teapot::output::success(&format!("{} ({})", message, elapsed));
    }

    /// Finish with an error message.
    pub fn error(mut self, message: &str) {
        self.stop();
        clear_line();
        teapot::output::error(message);
    }

    /// Finish silently (just clear the progress bar).
    pub fn finish_silent(mut self) {
        self.stop();
        clear_line();
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ProgressBar {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Create a progress bar for a bulk operation.
///
/// # Example
///
/// ```rust,ignore
/// let mut progress = tui::progress("Processing items", 100);
/// for i in 0..100 {
///     process_item(i).await?;
///     progress.inc();
/// }
/// progress.finish("Processing complete");
/// ```
pub fn progress(message: impl Into<String>, total: u64) -> ProgressBar {
    let message = message.into();
    let current = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));
    let start_time = Instant::now();

    // In non-interactive mode, just print the message
    if !is_tty() || is_ci() {
        teapot::output::info(&format!("{} (0/{})", message, total));
        return ProgressBar { current, total, message, running, join_handle: None, start_time };
    }

    let current_clone = current.clone();
    let running_clone = running.clone();
    let message_clone = message.clone();

    let join_handle = std::thread::spawn(move || {
        let mut last_current = 0u64;

        while running_clone.load(Ordering::SeqCst) {
            let current_val = current_clone.load(Ordering::SeqCst);

            // Only redraw if progress changed
            if current_val != last_current {
                last_current = current_val;

                let progress = TeapotProgress::new()
                    .total(total)
                    .current(current_val)
                    .message(&message_clone)
                    .width(30)
                    .show_percentage(true)
                    .filled_color(Color::Cyan)
                    .empty_color(Color::BrightBlack);

                eprint!("\r{}{}", CLEAR_LINE, progress.view());
                let _ = io::stderr().flush();
            }

            std::thread::sleep(Duration::from_millis(50));
        }

        // Clear line when done
        eprint!("\r{}", CLEAR_LINE);
        let _ = io::stderr().flush();
    });

    ProgressBar { current, total, message, running, join_handle: Some(join_handle), start_time }
}

/// Create a multi-progress tracker for parallel operations.
///
/// # Example
///
/// ```rust,ignore
/// let mut multi = tui::multi_progress("Deployment");
/// multi.add("validate", "Validating schema", 1);
/// multi.add("push", "Pushing changes", 100);
/// multi.add("activate", "Activating", 1);
///
/// // Complete tasks
/// multi.complete("validate");
/// // ... or update progress
/// multi.set_progress("push", 50);
/// ```
pub struct MultiProgressBar {
    tasks: Vec<TaskHandle>,
    #[allow(dead_code)] // Used for rendering title in future
    title: String,
    running: Arc<AtomicBool>,
    join_handle: Option<std::thread::JoinHandle<()>>,
    start_time: Instant,
}

struct TaskHandle {
    id: String,
    #[allow(dead_code)] // Used for rendering task name in future
    message: String,
    current: Arc<AtomicU64>,
    total: u64,
    status: Arc<std::sync::Mutex<TaskState>>,
}

#[derive(Clone)]
enum TaskState {
    Pending,
    InProgress,
    Completed,
    Failed(#[allow(dead_code)] String),
}

impl MultiProgressBar {
    /// Add a task to the multi-progress.
    pub fn add(&mut self, id: impl Into<String>, message: impl Into<String>, total: u64) {
        self.tasks.push(TaskHandle {
            id: id.into(),
            message: message.into(),
            current: Arc::new(AtomicU64::new(0)),
            total,
            status: Arc::new(std::sync::Mutex::new(TaskState::Pending)),
        });
    }

    /// Start a task (mark as in-progress).
    pub fn start(&self, id: &str) {
        if let Some(task) = self.tasks.iter().find(|t| t.id == id) {
            *task.status.lock().unwrap() = TaskState::InProgress;
        }
    }

    /// Set progress for a task.
    pub fn set_progress(&self, id: &str, current: u64) {
        if let Some(task) = self.tasks.iter().find(|t| t.id == id) {
            task.current.store(current.min(task.total), Ordering::SeqCst);
            *task.status.lock().unwrap() = TaskState::InProgress;
        }
    }

    /// Increment progress for a task.
    pub fn inc(&self, id: &str) {
        if let Some(task) = self.tasks.iter().find(|t| t.id == id) {
            task.current.fetch_add(1, Ordering::SeqCst);
            *task.status.lock().unwrap() = TaskState::InProgress;
        }
    }

    /// Mark a task as completed.
    pub fn complete(&self, id: &str) {
        if let Some(task) = self.tasks.iter().find(|t| t.id == id) {
            task.current.store(task.total, Ordering::SeqCst);
            *task.status.lock().unwrap() = TaskState::Completed;
        }
    }

    /// Mark a task as failed.
    pub fn fail(&self, id: &str, error: impl Into<String>) {
        if let Some(task) = self.tasks.iter().find(|t| t.id == id) {
            *task.status.lock().unwrap() = TaskState::Failed(error.into());
        }
    }

    /// Finish all tasks and show summary.
    pub fn finish(mut self, message: &str) {
        self.stop();
        let elapsed = format_duration(self.elapsed());

        // Count completed and failed
        let completed = self
            .tasks
            .iter()
            .filter(|t| matches!(*t.status.lock().unwrap(), TaskState::Completed))
            .count();
        let failed = self
            .tasks
            .iter()
            .filter(|t| matches!(*t.status.lock().unwrap(), TaskState::Failed(_)))
            .count();

        if failed > 0 {
            teapot::output::warning(&format!(
                "{} ({}/{} tasks, {} failed, {})",
                message,
                completed,
                self.tasks.len(),
                failed,
                elapsed
            ));
        } else {
            teapot::output::success(&format!("{} ({} tasks, {})", message, completed, elapsed));
        }
    }

    /// Get elapsed time.
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
        clear_lines(self.tasks.len() + 2); // Clear all task lines + title + summary
    }
}

impl Drop for MultiProgressBar {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Create a multi-progress tracker.
pub fn multi_progress(title: impl Into<String>) -> MultiProgressBar {
    MultiProgressBar {
        tasks: Vec::new(),
        title: title.into(),
        running: Arc::new(AtomicBool::new(true)),
        join_handle: None,
        start_time: Instant::now(),
    }
}

/// Format a duration for display.
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Clear the current line.
fn clear_line() {
    if is_tty() && !is_ci() {
        eprint!("\r{}", CLEAR_LINE);
        let _ = io::stderr().flush();
    }
}

/// Clear multiple lines.
fn clear_lines(count: usize) {
    if is_tty() && !is_ci() {
        for _ in 0..count {
            eprint!("{}{}", CURSOR_UP, CLEAR_LINE);
        }
        let _ = io::stderr().flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_creation() {
        let progress = progress("Test", 100);
        assert_eq!(progress.total(), 100);
        assert_eq!(progress.current(), 0);
        drop(progress);
    }

    #[test]
    fn test_progress_inc() {
        let progress = progress("Test", 100);
        progress.inc();
        assert_eq!(progress.current(), 1);
        progress.inc_by(5);
        assert_eq!(progress.current(), 6);
        drop(progress);
    }

    #[test]
    fn test_progress_set() {
        let progress = progress("Test", 100);
        progress.set(50);
        assert_eq!(progress.current(), 50);
        // Can't exceed total
        progress.set(200);
        assert_eq!(progress.current(), 100);
        drop(progress);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(5)), "5s");
        assert_eq!(format_duration(Duration::from_secs(65)), "1m 5s");
        assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m");
    }
}
