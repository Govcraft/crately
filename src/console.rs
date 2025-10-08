//! Console output formatting utilities for Crately.
//!
//! This module provides consistent console output formatting with symbols
//! and styling for the Crately service. All output is written to stderr
//! using `eprintln!` as is standard for status messages.

// Allow dead code since this is a utility module that will be used in the future
#![allow(dead_code)]

use acton_reactive::prelude::*;

use crate::config::ConfigLoaded;
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

/// Message to print a success message with the success symbol (✓)
#[acton_message(raw)]
pub struct PrintSuccess(pub String);

/// Message to print an error message with the error symbol (✗)
#[acton_message(raw)]
pub struct PrintError(pub String);

/// Message to print a progress message with the progress symbol (→)
#[acton_message(raw)]
pub struct PrintProgress(pub String);

/// Message to print a warning message with the warning symbol (⚠)
#[acton_message(raw)]
pub struct PrintWarning(pub String);

/// Message to print a horizontal separator line
#[acton_message(raw)]
pub struct PrintSeparator;

impl Console {
    /// Spawns, configures, and starts a new Console actor
    ///
    /// This is the standard factory method for creating Console actors.
    /// The Console actor handles visual output formatting for application
    /// events including startup banner, configuration loading, and status messages.
    ///
    /// This follows the simple actor pattern where only the handle is returned,
    /// as the Console actor has no startup data to provide to the application.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    ///
    /// # Returns
    ///
    /// Returns `AgentHandle` to the started Console actor for message passing.
    ///
    /// # When to Use
    ///
    /// Call this early in application startup to enable visual output
    /// and event notifications throughout the application lifecycle.
    ///
    /// # Errors
    ///
    /// Returns an error if actor creation or initialization fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use acton_reactive::prelude::*;
    /// use crately::console::Console;
    /// use crately::messages::Init;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut runtime = ActonApp::launch();
    ///
    ///     // Spawn the Console actor for visual output
    ///     let console = Console::spawn(&mut runtime).await?;
    ///
    ///     // Trigger initialization sequence
    ///     console.send(Init).await;
    ///
    ///     runtime.shutdown_all().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn spawn(runtime: &mut AgentRuntime) -> anyhow::Result<AgentHandle> {
        let mut builder = runtime
            .new_agent_with_name::<Console>("console".to_string())
            .await;

        builder
            .before_start(|_| {
                print_banner(env!("CARGO_PKG_VERSION"));

                AgentReply::immediate()
            })
            .act_on::<Init>(|_actor, _context| AgentReply::immediate())
            .act_on::<PrintSuccess>(|_actor, envelope| {
                let message = envelope.message();
                print_success(&message.0);
                AgentReply::immediate()
            })
            .act_on::<PrintError>(|_actor, envelope| {
                let message = envelope.message();
                print_error(&message.0);
                AgentReply::immediate()
            })
            .act_on::<PrintProgress>(|_actor, envelope| {
                let message = envelope.message();
                print_progress(&message.0);
                AgentReply::immediate()
            })
            .act_on::<PrintWarning>(|_actor, envelope| {
                let message = envelope.message();
                print_warning(&message.0);
                AgentReply::immediate()
            })
            .act_on::<PrintSeparator>(|_actor, _envelope| {
                print_separator();
                AgentReply::immediate()
            })
            .mutate_on::<ConfigLoaded>(|_agent, envelope| {
                let message = envelope.message();
                print_success(&format!(
                    "Configuration loaded {} {}",
                    PROGRESS,
                    message.config_path.display()
                ));
                AgentReply::immediate()
            });

        // Subscribe to ConfigLoaded messages before starting
        builder.handle().subscribe::<ConfigLoaded>().await;

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
/// This is a private helper function used by the Console actor's message handlers.
/// External code should send `PrintSuccess` messages to the Console actor instead.
///
/// # Arguments
///
/// * `message` - The success message to display
fn print_success(message: &str) {
    eprintln!("{} {}", SUCCESS, message);
}

/// Prints an error message with the error symbol (✗).
///
/// This is a private helper function used by the Console actor's message handlers.
/// External code should send `PrintError` messages to the Console actor instead.
///
/// # Arguments
///
/// * `message` - The error message to display
fn print_error(message: &str) {
    eprintln!("{} {}", ERROR, message);
}

/// Prints a progress message with the progress symbol (→).
///
/// This is a private helper function used by the Console actor's message handlers.
/// External code should send `PrintProgress` messages to the Console actor instead.
///
/// # Arguments
///
/// * `message` - The progress message to display
fn print_progress(message: &str) {
    eprintln!("{} {}", PROGRESS, message);
}

/// Prints a warning message with the warning symbol (⚠).
///
/// This is a private helper function used by the Console actor's message handlers.
/// External code should send `PrintWarning` messages to the Console actor instead.
///
/// # Arguments
///
/// * `message` - The warning message to display
fn print_warning(message: &str) {
    eprintln!("{} {}", WARNING, message);
}

/// Prints a horizontal separator line.
///
/// This is a private helper function used by the Console actor's message handlers.
/// External code should send `PrintSeparator` messages to the Console actor instead.
fn print_separator() {
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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_print_success_message() {
        let mut runtime = ActonApp::launch();
        let console = Console::spawn(&mut runtime).await.unwrap();

        // Send message and verify it doesn't panic
        console.send(PrintSuccess("Test message".to_string())).await;

        // Cleanup
        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_print_error_message() {
        let mut runtime = ActonApp::launch();
        let console = Console::spawn(&mut runtime).await.unwrap();

        console.send(PrintError("Test error".to_string())).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_print_progress_message() {
        let mut runtime = ActonApp::launch();
        let console = Console::spawn(&mut runtime).await.unwrap();

        console.send(PrintProgress("Test progress".to_string())).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_print_warning_message() {
        let mut runtime = ActonApp::launch();
        let console = Console::spawn(&mut runtime).await.unwrap();

        console.send(PrintWarning("Test warning".to_string())).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_print_separator_message() {
        let mut runtime = ActonApp::launch();
        let console = Console::spawn(&mut runtime).await.unwrap();

        console.send(PrintSeparator).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }
}
