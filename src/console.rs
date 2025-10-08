//! Console output formatting utilities for Crately.
//!
//! This module provides consistent console output formatting with symbols
//! and styling for the Crately service. All output is written to stderr
//! using `eprintln!` as is standard for status messages.

// Allow dead code since this is a utility module that will be used in the future
#![allow(dead_code)]

use acton_reactive::prelude::*;

use crate::messages::Init;
use tracing::info;

/// Success symbol (✓)
pub const SUCCESS: &str = "✓";

/// Error symbol (✗)
pub const ERROR: &str = "✗";

/// Progress symbol (→)
pub const PROGRESS: &str = "→";

/// Warning symbol (⚠)
pub const WARNING: &str = "⚠";

/// Banner template for the application startup banner.
///
/// This const contains the ASCII art box drawing for the Crately startup banner.
/// The banner includes placeholders that must be filled in at runtime:
/// - `{version}` placeholder for the version string (7 chars max for proper alignment)
///
/// # Example
///
/// ```
/// let banner = crately::console::BANNER_TEMPLATE;
/// assert!(banner.contains("{version}"));
/// ```
pub const BANNER_TEMPLATE: &str = "\
╔═══════════════════════════════════════════════════════════╗
║                        CRATELY                            ║
║          Crate Documentation & Search Service             ║
║                  License: AGPL-3.0-or-later               ║
║                     Version {version:<7}                       ║
╚═══════════════════════════════════════════════════════════╝";

#[acton_actor]
pub struct Console;
impl Console {
    pub async fn init(runtime: &mut AgentRuntime) -> anyhow::Result<AgentHandle> {
        let mut builder = runtime
            .new_agent_with_name::<Console>("console".to_string())
            .await;

        builder
            .before_start(|_| {
                print_banner(env!("CARGO_PKG_VERSION"));

                AgentReply::immediate()
            })
            .act_on::<Init>(|_actor, _context| AgentReply::immediate())
            .after_start(|_actor| {
                print_success("Runtime initialized");
                AgentReply::immediate()
            });

        Ok(builder.start().await)
    }
}
/// Prints the startup banner with version information.
///
/// This function uses [`BANNER_TEMPLATE`] to generate a consistent startup banner
/// with the provided version string. The version is left-aligned within a 7-character
/// field to maintain proper box alignment.
///
/// # Arguments
///
/// * `version` - The application version string (max 7 chars for proper alignment)
///
/// # Example
///
/// ```no_run
/// crately::console::print_banner("0.1.0");
/// ```
pub fn print_banner(version: &str) {
    eprintln!();
    eprintln!(
        "{}",
        BANNER_TEMPLATE.replace("{version:<7}", &format!("{version:<7}"))
    );
    info!(
        "{}",
        BANNER_TEMPLATE.replace("{version:<7}", &format!("{version:<7}"))
    );
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
    fn test_banner_template_contains_placeholder() {
        assert!(
            BANNER_TEMPLATE.contains("{version:<7}"),
            "BANNER_TEMPLATE should contain version placeholder"
        );
    }

    #[test]
    fn test_banner_template_has_box_drawing_characters() {
        // Verify the banner contains box-drawing characters
        assert!(BANNER_TEMPLATE.contains("╔"));
        assert!(BANNER_TEMPLATE.contains("╗"));
        assert!(BANNER_TEMPLATE.contains("╚"));
        assert!(BANNER_TEMPLATE.contains("╝"));
        assert!(BANNER_TEMPLATE.contains("═"));
        assert!(BANNER_TEMPLATE.contains("║"));
    }

    #[test]
    fn test_banner_template_has_expected_content() {
        assert!(BANNER_TEMPLATE.contains("CRATELY"));
        assert!(BANNER_TEMPLATE.contains("Crate Documentation & Search Service"));
        assert!(BANNER_TEMPLATE.contains("License: AGPL-3.0-or-later"));
        assert!(BANNER_TEMPLATE.contains("Version"));
    }

    #[test]
    fn test_print_banner_executes_without_panic() {
        // This test verifies the function executes successfully
        print_banner("0.1.0");
    }

    #[test]
    fn test_print_banner_with_short_version() {
        // Short versions should work fine
        print_banner("0.1");
    }

    #[test]
    fn test_print_banner_with_max_length_version() {
        // 7 character version should work properly
        print_banner("10.20.3");
    }

    #[test]
    fn test_print_banner_with_long_version() {
        // Longer versions may break alignment but should not panic
        print_banner("1.0.0-rc.1");
    }

    #[test]
    fn test_print_banner_with_empty_version() {
        // Empty version should not panic
        print_banner("");
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
