//! Console output formatting utilities for Crately.
//!
//! This module provides consistent console output formatting with symbols
//! and styling for the Crately service. All output is written to stderr
//! using `eprintln!` as is standard for status messages.

// Allow dead code since this is a utility module that will be used in the future
#![allow(dead_code)]

/// Success symbol (✓)
pub const SUCCESS: &str = "✓";

/// Error symbol (✗)
pub const ERROR: &str = "✗";

/// Progress symbol (→)
pub const PROGRESS: &str = "→";

/// Warning symbol (⚠)
pub const WARNING: &str = "⚠";

/// Prints the startup banner with version information.
///
/// # Arguments
///
/// * `version` - The application version string
///
/// # Example
///
/// ```no_run
/// crately::console::print_banner("0.1.0");
/// ```
pub fn print_banner(version: &str) {
    eprintln!();
    eprintln!("╔═══════════════════════════════════════════════════════════╗");
    eprintln!("║                        CRATELY                            ║");
    eprintln!("║          Crate Documentation & Search Service             ║");
    eprintln!(
        "║                     Version {}                         ║",
        version
    );
    eprintln!("╚═══════════════════════════════════════════════════════════╝");
    eprintln!();
}

/// Prints a success message with the success symbol (✓).
///
/// # Arguments
///
/// * `message` - The success message to display
///
/// # Example
///
/// ```no_run
/// crately::console::print_success("Server started on 127.0.0.1:3000");
/// ```
pub fn print_success(message: &str) {
    eprintln!("{} {}", SUCCESS, message);
}

/// Prints an error message with the error symbol (✗).
///
/// # Arguments
///
/// * `message` - The error message to display
///
/// # Example
///
/// ```no_run
/// crately::console::print_error("Failed to connect to database");
/// ```
pub fn print_error(message: &str) {
    eprintln!("{} {}", ERROR, message);
}

/// Prints a progress message with the progress symbol (→).
///
/// # Arguments
///
/// * `message` - The progress message to display
///
/// # Example
///
/// ```no_run
/// crately::console::print_progress("Initializing runtime");
/// ```
pub fn print_progress(message: &str) {
    eprintln!("{} {}", PROGRESS, message);
}

/// Prints a warning message with the warning symbol (⚠).
///
/// # Arguments
///
/// * `message` - The warning message to display
///
/// # Example
///
/// ```no_run
/// crately::console::print_warning("Using default configuration");
/// ```
pub fn print_warning(message: &str) {
    eprintln!("{} {}", WARNING, message);
}

/// Prints a horizontal separator line.
///
/// # Example
///
/// ```no_run
/// crately::console::print_separator();
/// ```
pub fn print_separator() {
    eprintln!("───────────────────────────────────────────────────────────");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_are_defined() {
        assert_eq!(SUCCESS, "✓");
        assert_eq!(ERROR, "✗");
        assert_eq!(PROGRESS, "→");
        assert_eq!(WARNING, "⚠");
    }

    #[test]
    fn test_print_banner_executes_without_panic() {
        // This test verifies the function executes successfully
        print_banner("0.1.0");
    }

    #[test]
    fn test_print_success_executes_without_panic() {
        print_success("Test success message");
    }

    #[test]
    fn test_print_error_executes_without_panic() {
        print_error("Test error message");
    }

    #[test]
    fn test_print_progress_executes_without_panic() {
        print_progress("Test progress message");
    }

    #[test]
    fn test_print_warning_executes_without_panic() {
        print_warning("Test warning message");
    }

    #[test]
    fn test_print_separator_executes_without_panic() {
        print_separator();
    }
}
