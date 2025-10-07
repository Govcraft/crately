//! XDG-compliant logging configuration module
//!
//! This module provides functionality to initialize tracing with file-based logging
//! following the XDG Base Directory Specification. Logs are written to
//! `$XDG_DATA_HOME/crately/logs/` with daily rotation.

use std::fs;
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use xdg::BaseDirectories;

/// Error types that can occur during logging initialization
#[derive(Debug)]
pub enum LoggingError {
    /// HOME directory could not be determined (XDG data home is None)
    HomeDirectoryNotFound,
    /// Failed to create log directory
    DirectoryCreation(std::io::Error),
    /// Failed to set global default tracing subscriber
    SubscriberInit(String),
}

impl std::fmt::Display for LoggingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HomeDirectoryNotFound => {
                write!(f, "HOME directory not found - cannot determine XDG data directory")
            }
            Self::DirectoryCreation(e) => write!(f, "Failed to create log directory: {}", e),
            Self::SubscriberInit(e) => write!(f, "Failed to initialize tracing subscriber: {}", e),
        }
    }
}

impl std::error::Error for LoggingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HomeDirectoryNotFound => None,
            Self::DirectoryCreation(e) => Some(e),
            Self::SubscriberInit(_) => None,
        }
    }
}

/// Gets the XDG-compliant log directory path for the application
///
/// Returns a path following the pattern: `$XDG_DATA_HOME/crately/logs/`
/// where `$XDG_DATA_HOME` defaults to `~/.local/share` if not set.
///
/// # Errors
///
/// Returns `LoggingError::HomeDirectoryNotFound` if the HOME directory cannot be determined
fn get_log_directory() -> Result<PathBuf, LoggingError> {
    // Use with_prefix to automatically append "crately" to all paths
    let base_dirs = BaseDirectories::with_prefix("crately");

    // Get the data home directory with the prefix already applied
    // This will be something like ~/.local/share/crately
    let data_home = base_dirs
        .get_data_home()
        .ok_or(LoggingError::HomeDirectoryNotFound)?;

    // Append "logs" subdirectory
    let log_dir = data_home.join("logs");

    Ok(log_dir)
}

/// Ensures the log directory exists, creating it if necessary
///
/// # Errors
///
/// Returns `LoggingError::DirectoryCreation` if the directory cannot be created
fn ensure_log_directory_exists(log_dir: &PathBuf) -> Result<(), LoggingError> {
    if !log_dir.exists() {
        fs::create_dir_all(log_dir).map_err(LoggingError::DirectoryCreation)?;
    }
    Ok(())
}

/// Initializes the tracing subscriber with XDG-compliant file logging
///
/// This function sets up a rolling file appender that:
/// - Writes logs to `$XDG_DATA_HOME/crately/logs/`
/// - Rotates log files daily
/// - Uses the filename prefix "crately"
/// - Applies the filter: "crately=debug,tower_http=off,axum=off"
///
/// The function returns a `WorkerGuard` that must be kept alive for the duration
/// of the application. When the guard is dropped, the logging worker thread will
/// flush and shut down.
///
/// # Returns
///
/// Returns a `WorkerGuard` that should be stored for the application lifetime
///
/// # Errors
///
/// Returns `LoggingError` if:
/// - XDG base directories cannot be determined
/// - Log directory cannot be created
/// - Tracing subscriber cannot be initialized
///
/// # Example
///
/// ```no_run
/// let _guard = logging::init().expect("Failed to initialize logging");
/// // _guard must be kept alive for logging to work
/// ```
pub fn init() -> Result<WorkerGuard, LoggingError> {
    // Get XDG-compliant log directory
    let log_dir = get_log_directory()?;

    // Ensure the directory exists
    ensure_log_directory_exists(&log_dir)?;

    // Create a rolling file appender with daily rotation
    let file_appender = tracing_appender::rolling::daily(&log_dir, "crately");

    // Wrap in non-blocking appender to prevent blocking the application on log writes
    let (non_blocking_appender, guard) = tracing_appender::non_blocking(file_appender);

    // Configure the environment filter
    // Default: crately=debug,tower_http=off,axum=off
    // Can be overridden with RUST_LOG environment variable
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "crately=debug,tower_http=off,axum=off".into());

    // Initialize the tracing subscriber with file output
    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking_appender)
                .with_ansi(false), // Disable ANSI color codes in log files
        )
        .try_init()
        .map_err(|e| LoggingError::SubscriberInit(e.to_string()))?;

    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_log_directory_returns_valid_path() {
        let result = get_log_directory();
        assert!(result.is_ok());

        let log_dir = result.unwrap();
        assert!(log_dir.to_string_lossy().contains("crately"));
        assert!(log_dir.to_string_lossy().contains("logs"));
    }

    #[test]
    fn test_ensure_log_directory_exists_creates_directory() {
        use std::env;

        // Create a temporary directory for testing
        let temp_dir = env::temp_dir().join("crately_test_logs");

        // Ensure it doesn't exist before the test
        let _ = fs::remove_dir_all(&temp_dir);

        // Test directory creation
        let result = ensure_log_directory_exists(&temp_dir);
        assert!(result.is_ok());
        assert!(temp_dir.exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ensure_log_directory_exists_handles_existing_directory() {
        use std::env;

        let temp_dir = env::temp_dir().join("crately_test_logs_existing");

        // Create the directory first
        let _ = fs::create_dir_all(&temp_dir);

        // Test that it handles existing directory
        let result = ensure_log_directory_exists(&temp_dir);
        assert!(result.is_ok());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
