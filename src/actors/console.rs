//! Console output formatting utilities for Crately.
//!
//! This module provides consistent console output formatting with symbols
//! and styling for the Crately service. All output is written to stderr
//! using `eprintln!` as is standard for status messages.

// Allow dead code since this is a utility module that will be used in the future
#![allow(dead_code)]

use acton_reactive::prelude::*;
use crossterm::tty::IsTty;
use std::io::stdout;

use crate::actors::config::ConfigLoaded;
use crate::colors::{format_error, format_progress, format_success, format_warning, ColorConfig};
use crate::messages::{
    ChunksPersistenceComplete, ConfigReloadFailed, CrateListResponse, CrateProcessingComplete,
    CratePersisted, CrateQueryResponse, DatabaseError, DatabaseWarning, DocChunkPersisted,
    EmbeddingPersisted, EmbeddingsPersistenceComplete, Init, PrintError, PrintProgress,
    PrintSeparator, PrintSpinner, PrintSuccess, PrintWarning, ServerReloaded, ServerStarted,
    SetRawMode, StopSpinner, UpdateSpinner,
};
use tracing::info;

/// Success symbol (✓)
pub const SUCCESS: &str = "✓";

/// Error symbol (✗)
pub const ERROR: &str = "✗";

/// Progress symbol (⋯) - indicates ongoing operations
pub const PROGRESS: &str = "⋯";

/// Location symbol (→) - shows relationships, context, and location
pub const LOCATION: &str = "→";

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

/// Commands for controlling the spinner task
enum SpinnerCommand {
    Start(String),
    Update(String),
    Stop,
}

#[acton_actor]
pub struct Console {
    /// Whether terminal raw mode is currently active.
    ///
    /// When raw mode is active, console output must use `\r\n` for line endings
    /// instead of just `\n` to ensure proper cursor positioning.
    raw_mode_active: bool,
    /// Color configuration for terminal output
    color_config: ColorConfig,
    /// Channel sender for spinner commands
    spinner_tx: Option<tokio::sync::mpsc::UnboundedSender<SpinnerCommand>>,
}

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
    /// * `color_config` - Color configuration for terminal output
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
    /// use crately::colors::ColorConfig;
    /// use crately::messages::Init;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut runtime = ActonApp::launch();
    ///     let color_config = ColorConfig::new(false);
    ///
    ///     // Spawn the Console actor for visual output
    ///     let console = Console::spawn(&mut runtime, color_config).await?;
    ///
    ///     // Trigger initialization sequence
    ///     console.send(Init).await;
    ///
    ///     runtime.shutdown_all().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn spawn(
        runtime: &mut AgentRuntime,
        color_config: ColorConfig,
    ) -> anyhow::Result<AgentHandle> {
        let mut builder = runtime
            .new_agent_with_name::<Console>("console".to_string())
            .await;

        // Initialize with raw mode inactive by default
        builder.model = Console {
            raw_mode_active: false,
            color_config,
            spinner_tx: None,
        };

        builder
            .before_start(|_| {
                print_banner(env!("CARGO_PKG_VERSION"), false);

                AgentReply::immediate()
            })
            .act_on::<Init>(|_actor, _context| AgentReply::immediate())
            .mutate_on::<SetRawMode>(|agent, envelope| {
                let message = envelope.message();
                agent.model.raw_mode_active = message.0;
                AgentReply::immediate()
            })
            .act_on::<PrintSuccess>(|actor, envelope| {
                let message = envelope.message();
                print_success(
                    &message.0,
                    actor.model.raw_mode_active,
                    actor.model.color_config,
                );
                AgentReply::immediate()
            })
            .act_on::<PrintError>(|actor, envelope| {
                let message = envelope.message();
                print_error(
                    &message.0,
                    actor.model.raw_mode_active,
                    actor.model.color_config,
                );
                AgentReply::immediate()
            })
            .act_on::<PrintProgress>(|actor, envelope| {
                let message = envelope.message();
                print_progress(
                    &message.0,
                    actor.model.raw_mode_active,
                    actor.model.color_config,
                );
                AgentReply::immediate()
            })
            .act_on::<PrintWarning>(|actor, envelope| {
                let message = envelope.message();
                print_warning(
                    &message.0,
                    actor.model.raw_mode_active,
                    actor.model.color_config,
                );
                AgentReply::immediate()
            })
            .act_on::<PrintSeparator>(|actor, _envelope| {
                print_separator(actor.model.raw_mode_active);
                AgentReply::immediate()
            })
            .mutate_on::<ConfigLoaded>(|agent, envelope| {
                let message = envelope.message();
                print_success(
                    &format!(
                        "Configuration loaded {} {}",
                        LOCATION,
                        message.config_path.display()
                    ),
                    agent.model.raw_mode_active,
                    agent.model.color_config,
                );
                AgentReply::immediate()
            })
            .act_on::<ConfigReloadFailed>(|actor, envelope| {
                let message = envelope.message();
                let raw_mode = actor.model.raw_mode_active;
                let color_config = actor.model.color_config;
                print_error(
                    &format!("Configuration reload failed {} {}", LOCATION, message.error),
                    raw_mode,
                    color_config,
                );
                print_warning(
                    "Server continues with previous configuration",
                    raw_mode,
                    color_config,
                );
                AgentReply::immediate()
            })
            .act_on::<ServerReloaded>(|actor, envelope| {
                let message = envelope.message();
                let raw_mode = actor.model.raw_mode_active;
                let color_config = actor.model.color_config;
                print_success(
                    &format!("Server reloaded {} http://127.0.0.1:{}", LOCATION, message.port),
                    raw_mode,
                    color_config,
                );
                AgentReply::immediate()
            })
            .act_on::<ServerStarted>(|actor, _envelope| {
                let raw_mode = actor.model.raw_mode_active;
                print_newline(raw_mode);
                print_line(
                    "Server is running. Press 'q' or Ctrl+C to shutdown gracefully",
                    raw_mode,
                );
                print_line("Press 'r' to reload configuration", raw_mode);
                print_newline(raw_mode);
                AgentReply::immediate()
            })
            .act_on::<CratePersisted>(|actor, envelope| {
                let msg = envelope.message();
                print_success(
                    &format!(
                        "Persisted crate: {}@{} (record: {})",
                        msg.name, msg.version, msg.record_id
                    ),
                    actor.model.raw_mode_active,
                    actor.model.color_config,
                );
                AgentReply::immediate()
            })
            .act_on::<DatabaseError>(|actor, envelope| {
                let msg = envelope.message();
                print_error(
                    &format!("Database error during {}: {}", msg.operation, msg.error),
                    actor.model.raw_mode_active,
                    actor.model.color_config,
                );
                AgentReply::immediate()
            })
            .act_on::<DatabaseWarning>(|actor, envelope| {
                let msg = envelope.message();
                print_warning(
                    &format!("Database warning during {}: {}", msg.operation, msg.warning),
                    actor.model.raw_mode_active,
                    actor.model.color_config,
                );
                AgentReply::immediate()
            })
            .act_on::<CrateQueryResponse>(|actor, envelope| {
                let msg = envelope.message();
                let raw_mode = actor.model.raw_mode_active;
                let color_config = actor.model.color_config;

                if let Some(specifier) = &msg.specifier {
                    // Crate found - display success with details
                    print_success(
                        &format!(
                            "Found crate: {}@{}",
                            specifier.name(),
                            specifier.version()
                        ),
                        raw_mode,
                        color_config,
                    );

                    // Display additional details
                    print_line(
                        &format!("  {} Status: {}", LOCATION, msg.status),
                        raw_mode,
                    );

                    if !msg.features.is_empty() {
                        print_line(
                            &format!("  {} Features: {}", LOCATION, msg.features.join(", ")),
                            raw_mode,
                        );
                    }

                    if !msg.created_at.is_empty() {
                        print_line(
                            &format!("  {} Created: {}", LOCATION, msg.created_at),
                            raw_mode,
                        );
                    }
                } else {
                    // Crate not found
                    print_warning("Crate not found in database", raw_mode, color_config);
                }

                AgentReply::immediate()
            })
            .act_on::<CrateListResponse>(|actor, envelope| {
                let msg = envelope.message();
                let raw_mode = actor.model.raw_mode_active;
                let color_config = actor.model.color_config;

                if msg.crates.is_empty() {
                    print_warning("No crates found in database", raw_mode, color_config);
                } else {
                    print_success(
                        &format!(
                            "Found {} crate{} (total: {})",
                            msg.crates.len(),
                            if msg.crates.len() == 1 { "" } else { "s" },
                            msg.total_count
                        ),
                        raw_mode,
                        color_config,
                    );

                    // Display each crate summary
                    for crate_summary in &msg.crates {
                        print_line(
                            &format!(
                                "  {} {}@{} [{}]",
                                LOCATION,
                                crate_summary.name,
                                crate_summary.version,
                                crate_summary.status
                            ),
                            raw_mode,
                        );
                    }
                }

                AgentReply::immediate()
            })
            .mutate_on::<PrintSpinner>(|agent, envelope| {
                let message = envelope.message();
                let raw_mode = agent.model.raw_mode_active;
                let color_config = agent.model.color_config;

                // Create a new spinner task if one doesn't exist
                if agent.model.spinner_tx.is_none() {
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SpinnerCommand>();
                    agent.model.spinner_tx = Some(tx);

                    tokio::spawn(async move {
                        let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                        let mut frame_index = 0;
                        let mut current_message = String::new();
                        let mut running = false;

                        loop {
                            tokio::select! {
                                Some(cmd) = rx.recv() => {
                                    match cmd {
                                        SpinnerCommand::Start(msg) => {
                                            current_message = msg;
                                            running = true;
                                        }
                                        SpinnerCommand::Update(msg) => {
                                            current_message = msg;
                                        }
                                        SpinnerCommand::Stop => {
                                            running = false;
                                            // Clear the spinner line
                                            eprint!("\r{}", " ".repeat(80));
                                            eprint!("\r");
                                            if raw_mode {
                                                eprint!("\r\n");
                                            }
                                        }
                                    }
                                }
                                _ = tokio::time::sleep(tokio::time::Duration::from_millis(80)), if running => {
                                    // Print spinner frame with message
                                    let formatted = format_progress(&current_message, color_config);
                                    eprint!("\r{} {}", frames[frame_index], formatted);
                                    frame_index = (frame_index + 1) % frames.len();
                                }
                            }
                        }
                    });
                }

                // Send start command
                if let Some(tx) = &agent.model.spinner_tx {
                    let _ = tx.send(SpinnerCommand::Start(message.message.clone()));
                }

                AgentReply::immediate()
            })
            .mutate_on::<UpdateSpinner>(|agent, envelope| {
                let message = envelope.message();

                // Send update command to spinner task
                if let Some(tx) = &agent.model.spinner_tx {
                    let _ = tx.send(SpinnerCommand::Update(message.message.clone()));
                }

                AgentReply::immediate()
            })
            .mutate_on::<StopSpinner>(|agent, envelope| {
                let message = envelope.message();
                let raw_mode = agent.model.raw_mode_active;
                let color_config = agent.model.color_config;

                // Send stop command to spinner task
                if let Some(tx) = &agent.model.spinner_tx {
                    let _ = tx.send(SpinnerCommand::Stop);
                }

                // Display final message if provided
                if let Some(final_msg) = &message.final_message {
                    if message.success {
                        print_success(final_msg, raw_mode, color_config);
                    } else {
                        print_error(final_msg, raw_mode, color_config);
                    }
                }

                AgentReply::immediate()
            })
            .act_on::<DocChunkPersisted>(|agent, envelope| {
                let msg = envelope.message();

                // Use chunk_index to show progress
                let progress_msg = format!(
                    "Persisting chunk {}/? for {}@{}",
                    msg.chunk_index + 1, // Display as 1-indexed
                    msg.specifier.name(),
                    msg.specifier.version()
                );

                print_progress(
                    &progress_msg,
                    agent.model.raw_mode_active,
                    agent.model.color_config,
                );

                AgentReply::immediate()
            })
            .act_on::<EmbeddingPersisted>(|agent, envelope| {
                let msg = envelope.message();

                // Use model_name to show which embedding model is being used
                let progress_msg = format!(
                    "Vectorized chunk for {}@{} with {}",
                    msg.specifier.name(),
                    msg.specifier.version(),
                    msg.model_name
                );

                print_progress(
                    &progress_msg,
                    agent.model.raw_mode_active,
                    agent.model.color_config,
                );

                AgentReply::immediate()
            })
            .act_on::<ChunksPersistenceComplete>(|agent, envelope| {
                let msg = envelope.message();

                let success_msg = format!(
                    "All {} chunks persisted for {}@{}",
                    msg.chunk_count,
                    msg.specifier.name(),
                    msg.specifier.version()
                );

                print_success(
                    &success_msg,
                    agent.model.raw_mode_active,
                    agent.model.color_config,
                );

                AgentReply::immediate()
            })
            .act_on::<EmbeddingsPersistenceComplete>(|agent, envelope| {
                let msg = envelope.message();

                let success_msg = format!(
                    "All {} embeddings persisted for {}@{} using {}",
                    msg.vector_count,
                    msg.specifier.name(),
                    msg.specifier.version(),
                    msg.embedding_model
                );

                print_success(
                    &success_msg,
                    agent.model.raw_mode_active,
                    agent.model.color_config,
                );

                AgentReply::immediate()
            })
            .act_on::<CrateProcessingComplete>(|agent, envelope| {
                let msg = envelope.message();

                let success_msg = format!(
                    "Crate processing complete: {}@{} ({} stages, {}ms)",
                    msg.specifier.name(),
                    msg.specifier.version(),
                    msg.stages_completed,
                    msg.total_duration_ms
                );

                print_success(
                    &success_msg,
                    agent.model.raw_mode_active,
                    agent.model.color_config,
                );

                AgentReply::immediate()
            });

        // Subscribe to broadcast messages before starting
        builder.handle().subscribe::<ConfigLoaded>().await;
        builder.handle().subscribe::<ConfigReloadFailed>().await;
        builder.handle().subscribe::<ServerReloaded>().await;
        builder.handle().subscribe::<ServerStarted>().await;
        builder.handle().subscribe::<CratePersisted>().await;
        builder.handle().subscribe::<DatabaseError>().await;
        builder.handle().subscribe::<DatabaseWarning>().await;
        builder.handle().subscribe::<CrateQueryResponse>().await;
        builder.handle().subscribe::<CrateListResponse>().await;
        builder.handle().subscribe::<DocChunkPersisted>().await;
        builder.handle().subscribe::<EmbeddingPersisted>().await;
        builder
            .handle()
            .subscribe::<ChunksPersistenceComplete>()
            .await;
        builder
            .handle()
            .subscribe::<EmbeddingsPersistenceComplete>()
            .await;
        builder
            .handle()
            .subscribe::<CrateProcessingComplete>()
            .await;

        Ok(builder.start().await)
    }
}
/// Checks if stdout is connected to a TTY (terminal).
///
/// This is used to determine whether console output should be displayed.
/// When not connected to a TTY (e.g., CI/CD, background jobs, log files),
/// console output is suppressed to avoid filling mailboxes with broadcast messages.
///
/// # Returns
///
/// Returns `true` if stdout is connected to a TTY, `false` otherwise
#[inline]
fn is_tty() -> bool {
    stdout().is_tty()
}

/// Returns the appropriate line ending based on raw mode state.
///
/// # Arguments
///
/// * `raw_mode` - Whether terminal raw mode is active
///
/// # Returns
///
/// Returns `"\r\n"` when raw mode is active, `"\n"` otherwise
#[inline]
fn line_ending(raw_mode: bool) -> &'static str {
    if raw_mode {
        "\r\n"
    } else {
        "\n"
    }
}

/// Prints a line with the appropriate line ending for the current mode.
///
/// This function checks if stdout is connected to a TTY before printing.
/// If not in a TTY (e.g., CI/CD, background jobs), output is suppressed.
///
/// # Arguments
///
/// * `text` - The text to print
/// * `raw_mode` - Whether terminal raw mode is active
fn print_line(text: &str, raw_mode: bool) {
    if !is_tty() {
        return;
    }
    eprint!("{}{}", text, line_ending(raw_mode));
}

/// Prints a newline with the appropriate line ending for the current mode.
///
/// This function checks if stdout is connected to a TTY before printing.
/// If not in a TTY (e.g., CI/CD, background jobs), output is suppressed.
///
/// # Arguments
///
/// * `raw_mode` - Whether terminal raw mode is active
fn print_newline(raw_mode: bool) {
    if !is_tty() {
        return;
    }
    eprint!("{}", line_ending(raw_mode));
}

/// Prints the startup banner with version information.
///
/// This function uses [`BANNER_TEMPLATE`] to generate a consistent startup banner
/// with the provided version string. The version is left-aligned within a 7-character
/// field to maintain proper box alignment.
///
/// The banner is only displayed when stdout is connected to a TTY. When not in a TTY
/// (e.g., CI/CD, background jobs), the banner is suppressed but still logged via tracing.
///
/// # Arguments
///
/// * `version` - The application version string (max 7 chars for proper alignment)
/// * `raw_mode` - Whether terminal raw mode is active
///
/// # Example
///
/// ```no_run
/// crately::console::print_banner("0.1.0", false);
/// ```
pub fn print_banner(version: &str, raw_mode: bool) {
    let banner_text = BANNER_TEMPLATE.replace("{version:<7}", &format!("{version:<7}"));

    print_newline(raw_mode);
    print_line(&banner_text, raw_mode);
    info!("{}", banner_text);
    print_newline(raw_mode);
}

/// Prints a success message with the success symbol (✓).
///
/// This is a private helper function used by the Console actor's message handlers.
/// External code should send `PrintSuccess` messages to the Console actor instead.
///
/// # Arguments
///
/// * `message` - The success message to display
/// * `raw_mode` - Whether terminal raw mode is active
/// * `color_config` - Color configuration for output
fn print_success(message: &str, raw_mode: bool, color_config: ColorConfig) {
    let formatted = format_success(message, color_config);
    print_line(&format!("{} {}", SUCCESS, formatted), raw_mode);
}

/// Prints an error message with the error symbol (✗).
///
/// This is a private helper function used by the Console actor's message handlers.
/// External code should send `PrintError` messages to the Console actor instead.
///
/// # Arguments
///
/// * `message` - The error message to display
/// * `raw_mode` - Whether terminal raw mode is active
/// * `color_config` - Color configuration for output
fn print_error(message: &str, raw_mode: bool, color_config: ColorConfig) {
    let formatted = format_error(message, color_config);
    print_line(&format!("{} {}", ERROR, formatted), raw_mode);
}

/// Prints a progress message with the progress symbol (⋯).
///
/// This is a private helper function used by the Console actor's message handlers.
/// External code should send `PrintProgress` messages to the Console actor instead.
///
/// # Arguments
///
/// * `message` - The progress message to display
/// * `raw_mode` - Whether terminal raw mode is active
/// * `color_config` - Color configuration for output
fn print_progress(message: &str, raw_mode: bool, color_config: ColorConfig) {
    let formatted = format_progress(message, color_config);
    print_line(&format!("{} {}", PROGRESS, formatted), raw_mode);
}

/// Prints a warning message with the warning symbol (⚠).
///
/// This is a private helper function used by the Console actor's message handlers.
/// External code should send `PrintWarning` messages to the Console actor instead.
///
/// # Arguments
///
/// * `message` - The warning message to display
/// * `raw_mode` - Whether terminal raw mode is active
/// * `color_config` - Color configuration for output
fn print_warning(message: &str, raw_mode: bool, color_config: ColorConfig) {
    let formatted = format_warning(message, color_config);
    print_line(&format!("{} {}", WARNING, formatted), raw_mode);
}

/// Prints a horizontal separator line.
///
/// This is a private helper function used by the Console actor's message handlers.
/// External code should send `PrintSeparator` messages to the Console actor instead.
///
/// # Arguments
///
/// * `raw_mode` - Whether terminal raw mode is active
fn print_separator(raw_mode: bool) {
    print_line("───────────────────────────────────────────────────────────", raw_mode);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_are_defined() {
        assert_eq!(SUCCESS, "✓");
        assert_eq!(ERROR, "✗");
        assert_eq!(PROGRESS, "⋯");
        assert_eq!(LOCATION, "→");
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
    fn test_line_ending_normal_mode() {
        assert_eq!(line_ending(false), "\n");
    }

    #[test]
    fn test_line_ending_raw_mode() {
        assert_eq!(line_ending(true), "\r\n");
    }

    #[test]
    fn test_is_tty_does_not_panic() {
        // This test verifies the is_tty function executes without panic
        // The actual return value depends on test environment
        let _ = is_tty();
    }

    #[test]
    fn test_print_banner_executes_without_panic() {
        // This test verifies the function executes successfully
        print_banner("0.1.0", false);
    }

    #[test]
    fn test_print_banner_raw_mode_executes_without_panic() {
        print_banner("0.1.0", true);
    }

    #[test]
    fn test_print_banner_with_short_version() {
        // Short versions should work fine
        print_banner("0.1", false);
    }

    #[test]
    fn test_print_banner_with_max_length_version() {
        // 7 character version should work properly
        print_banner("10.20.3", false);
    }

    #[test]
    fn test_print_banner_with_long_version() {
        // Longer versions may break alignment but should not panic
        print_banner("1.0.0-rc.1", false);
    }

    #[test]
    fn test_print_banner_with_empty_version() {
        // Empty version should not panic
        print_banner("", false);
    }

    #[test]
    fn test_print_success_executes_without_panic() {
        use crate::cli::ColorChoice;
        let color_config = ColorConfig::new(ColorChoice::Never); // Disable colors for tests
        print_success("Test success message", false, color_config);
    }

    #[test]
    fn test_print_success_raw_mode_executes_without_panic() {
        use crate::cli::ColorChoice;
        let color_config = ColorConfig::new(ColorChoice::Never);
        print_success("Test success message", true, color_config);
    }

    #[test]
    fn test_print_error_executes_without_panic() {
        use crate::cli::ColorChoice;
        let color_config = ColorConfig::new(ColorChoice::Never);
        print_error("Test error message", false, color_config);
    }

    #[test]
    fn test_print_error_raw_mode_executes_without_panic() {
        use crate::cli::ColorChoice;
        let color_config = ColorConfig::new(ColorChoice::Never);
        print_error("Test error message", true, color_config);
    }

    #[test]
    fn test_print_progress_executes_without_panic() {
        use crate::cli::ColorChoice;
        let color_config = ColorConfig::new(ColorChoice::Never);
        print_progress("Test progress message", false, color_config);
    }

    #[test]
    fn test_print_progress_raw_mode_executes_without_panic() {
        use crate::cli::ColorChoice;
        let color_config = ColorConfig::new(ColorChoice::Never);
        print_progress("Test progress message", true, color_config);
    }

    #[test]
    fn test_print_warning_executes_without_panic() {
        use crate::cli::ColorChoice;
        let color_config = ColorConfig::new(ColorChoice::Never);
        print_warning("Test warning message", false, color_config);
    }

    #[test]
    fn test_print_warning_raw_mode_executes_without_panic() {
        use crate::cli::ColorChoice;
        let color_config = ColorConfig::new(ColorChoice::Never);
        print_warning("Test warning message", true, color_config);
    }

    #[test]
    fn test_print_separator_executes_without_panic() {
        print_separator(false);
    }

    #[test]
    fn test_print_separator_raw_mode_executes_without_panic() {
        print_separator(true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_print_success_message() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never); // Disable colors for tests
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Send message and verify it doesn't panic
        console.send(PrintSuccess("Test message".to_string())).await;

        // Cleanup
        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_print_error_message() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        console.send(PrintError("Test error".to_string())).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_print_progress_message() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        console.send(PrintProgress("Test progress".to_string())).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_print_warning_message() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        console.send(PrintWarning("Test warning".to_string())).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_print_separator_message() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        console.send(PrintSeparator).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_set_raw_mode_true() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Send SetRawMode(true) message
        console.send(SetRawMode(true)).await;

        // Give it time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Verify it doesn't panic and can still handle messages
        console.send(PrintSuccess("Test after raw mode".to_string())).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_set_raw_mode_false() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Send SetRawMode(false) message
        console.send(SetRawMode(false)).await;

        // Give it time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Verify it doesn't panic and can still handle messages
        console.send(PrintSuccess("Test after normal mode".to_string())).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_mode_transition() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Start in normal mode
        console.send(PrintSuccess("Normal mode 1".to_string())).await;

        // Switch to raw mode
        console.send(SetRawMode(true)).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        console.send(PrintSuccess("Raw mode".to_string())).await;

        // Switch back to normal mode
        console.send(SetRawMode(false)).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        console.send(PrintSuccess("Normal mode 2".to_string())).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_print_spinner_message() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Send spinner start message
        console
            .send(PrintSpinner {
                message: "Test spinner".to_string(),
            })
            .await;

        // Let spinner run briefly
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Stop spinner
        console
            .send(StopSpinner {
                success: true,
                final_message: Some("Spinner completed".to_string()),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_update_spinner_message() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Start spinner
        console
            .send(PrintSpinner {
                message: "Initial message".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Update spinner message
        console
            .send(UpdateSpinner {
                message: "Updated message".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Stop spinner
        console
            .send(StopSpinner {
                success: true,
                final_message: None,
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_stop_spinner_with_success() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Start spinner
        console
            .send(PrintSpinner {
                message: "Working...".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Stop with success
        console
            .send(StopSpinner {
                success: true,
                final_message: Some("Operation succeeded".to_string()),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_stop_spinner_with_failure() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Start spinner
        console
            .send(PrintSpinner {
                message: "Working...".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Stop with failure
        console
            .send(StopSpinner {
                success: false,
                final_message: Some("Operation failed".to_string()),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_stop_spinner_without_message() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Start spinner
        console
            .send(PrintSpinner {
                message: "Working...".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Stop without message
        console
            .send(StopSpinner {
                success: true,
                final_message: None,
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_multiple_spinners_sequentially() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // First spinner
        console
            .send(PrintSpinner {
                message: "First operation".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        console
            .send(StopSpinner {
                success: true,
                final_message: Some("First done".to_string()),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Second spinner
        console
            .send(PrintSpinner {
                message: "Second operation".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        console
            .send(StopSpinner {
                success: true,
                final_message: Some("Second done".to_string()),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_doc_chunk_persisted() {
        use crate::cli::ColorChoice;
        use crate::crate_specifier::CrateSpecifier;
        use std::str::FromStr;

        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Broadcast DocChunkPersisted event
        runtime
            .broker()
            .broadcast(DocChunkPersisted {
                chunk_id: "test_chunk_001".to_string(),
                specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
                chunk_index: 5,
            })
            .await;

        // Allow time for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_embedding_persisted() {
        use crate::cli::ColorChoice;
        use crate::crate_specifier::CrateSpecifier;
        use std::str::FromStr;

        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Broadcast EmbeddingPersisted event
        runtime
            .broker()
            .broadcast(EmbeddingPersisted {
                chunk_id: "test_chunk_001".to_string(),
                specifier: CrateSpecifier::from_str("tokio@1.35.0").unwrap(),
                model_name: "text-embedding-3-small".to_string(),
            })
            .await;

        // Allow time for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_handles_multiple_chunk_progress() {
        use crate::cli::ColorChoice;
        use crate::crate_specifier::CrateSpecifier;
        use std::str::FromStr;

        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();

        // Simulate persisting multiple chunks
        for chunk_idx in 0..5 {
            runtime
                .broker()
                .broadcast(DocChunkPersisted {
                    chunk_id: format!("axum_chunk_{:03}", chunk_idx),
                    specifier: specifier.clone(),
                    chunk_index: chunk_idx,
                })
                .await;

            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        }

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_console_actor_spinner_in_raw_mode() {
        use crate::cli::ColorChoice;
        let mut runtime = ActonApp::launch();
        let color_config = ColorConfig::new(ColorChoice::Never);
        let console = Console::spawn(&mut runtime, color_config).await.unwrap();

        // Enable raw mode
        console.send(SetRawMode(true)).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Start spinner in raw mode
        console
            .send(PrintSpinner {
                message: "Raw mode spinner".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

        // Stop spinner
        console
            .send(StopSpinner {
                success: true,
                final_message: Some("Raw mode complete".to_string()),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        console.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }
}
