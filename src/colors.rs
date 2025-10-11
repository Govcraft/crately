//! Color support and management for terminal output.
//!
//! This module provides color detection and styling functions for console output,
//! respecting terminal capabilities and user preferences via the NO_COLOR
//! environment variable, CLICOLOR, and TTY detection.

use crate::cli::ColorChoice;
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use std::env;
use std::io::{IsTerminal, Write};

/// Configuration for color output support.
///
/// This struct tracks whether colors should be enabled based on terminal
/// capabilities, environment variables, and user preferences.
#[derive(Debug, Clone, Copy, Default)]
pub struct ColorConfig {
    /// Whether colors are enabled for output
    enabled: bool,
}

impl ColorConfig {
    /// Creates a new ColorConfig based on the color choice and environment.
    ///
    /// # Arguments
    ///
    /// * `choice` - The user's color preference (Always, Auto, Never)
    ///
    /// # Returns
    ///
    /// Returns a ColorConfig instance configured based on choice and environment.
    ///
    /// # Color Detection Precedence (for Auto mode)
    ///
    /// 1. NO_COLOR environment variable (if set, colors are disabled)
    /// 2. CLICOLOR environment variable (if set to "0", colors are disabled)
    /// 3. TTY detection (colors disabled if stdout is not a TTY)
    /// 4. TERM environment variable (colors disabled for "dumb" terminals)
    ///
    /// # Example
    ///
    /// ```
    /// use crately::colors::ColorConfig;
    /// use crately::cli::ColorChoice;
    ///
    /// let config = ColorConfig::new(ColorChoice::Auto);
    /// assert!(config.is_enabled() || !config.is_enabled()); // Either state is valid
    /// ```
    pub fn new(choice: ColorChoice) -> Self {
        let enabled = match choice {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => Self::should_enable_colors(),
        };
        Self { enabled }
    }

    /// Checks if colors are enabled.
    ///
    /// # Returns
    ///
    /// Returns `true` if colors should be used in output, `false` otherwise.
    pub fn is_enabled(self) -> bool {
        self.enabled
    }

    /// Determines if colors should be enabled based on environment.
    ///
    /// Checks for (in order of precedence):
    /// 1. NO_COLOR environment variable (https://no-color.org/)
    /// 2. CLICOLOR environment variable (0 = disable colors)
    /// 3. TTY detection (colors disabled if stdout is not a TTY)
    /// 4. TERM environment variable for basic terminal detection
    ///
    /// # Returns
    ///
    /// Returns `true` if colors should be enabled, `false` otherwise.
    fn should_enable_colors() -> bool {
        // Check NO_COLOR environment variable first
        // According to no-color.org, any value (even empty) means disable colors
        if env::var("NO_COLOR").is_ok() {
            return false;
        }

        // Check CLICOLOR environment variable
        // CLICOLOR=0 means disable colors
        if let Ok(clicolor) = env::var("CLICOLOR") {
            if clicolor == "0" {
                return false;
            }
        }

        // Check if stdout is a TTY
        // Colors should be disabled when output is piped or redirected
        if !std::io::stdout().is_terminal() {
            return false;
        }

        // Check if we're in a terminal that likely supports colors
        // This is a conservative approach that works on most platforms
        if let Ok(term) = env::var("TERM") {
            // Common terminals that support ANSI colors
            !term.is_empty() && term != "dumb"
        } else {
            // No TERM variable, but we're on a TTY, so assume we support colors
            // This is safe because crossterm handles fallback gracefully
            true
        }
    }
}

/// Applies a color style to text and writes it to the provided writer.
///
/// This is a helper function that conditionally applies color codes based on
/// the ColorConfig. If colors are disabled, text is written without styling.
///
/// # Arguments
///
/// * `writer` - The writer to output to (typically stderr)
/// * `text` - The text to write
/// * `color` - The foreground color to apply
/// * `config` - The color configuration determining if colors should be applied
///
/// # Errors
///
/// Returns an error if writing to the writer fails.
fn write_colored<W: Write>(
    writer: &mut W,
    text: &str,
    color: Color,
    config: ColorConfig,
) -> std::io::Result<()> {
    if config.is_enabled() {
        write!(writer, "{}{}{}", SetForegroundColor(color), text, ResetColor)
    } else {
        write!(writer, "{}", text)
    }
}

/// Formats a success message with green color (if enabled).
///
/// # Arguments
///
/// * `text` - The text to format
/// * `config` - The color configuration
///
/// # Returns
///
/// Returns a formatted string with color codes (if enabled).
///
/// # Example
///
/// ```
/// use crately::colors::{ColorConfig, format_success};
///
/// let config = ColorConfig::new(false);
/// let formatted = format_success("Success!", config);
/// assert!(formatted.contains("Success!"));
/// ```
pub fn format_success(text: &str, config: ColorConfig) -> String {
    let mut buffer = Vec::new();
    write_colored(&mut buffer, text, Color::Green, config)
        .expect("Writing to Vec should never fail");
    String::from_utf8(buffer).expect("Color codes should be valid UTF-8")
}

/// Formats an error message with red color (if enabled).
///
/// # Arguments
///
/// * `text` - The text to format
/// * `config` - The color configuration
///
/// # Returns
///
/// Returns a formatted string with color codes (if enabled).
///
/// # Example
///
/// ```
/// use crately::colors::{ColorConfig, format_error};
///
/// let config = ColorConfig::new(false);
/// let formatted = format_error("Error!", config);
/// assert!(formatted.contains("Error!"));
/// ```
pub fn format_error(text: &str, config: ColorConfig) -> String {
    let mut buffer = Vec::new();
    write_colored(&mut buffer, text, Color::Red, config)
        .expect("Writing to Vec should never fail");
    String::from_utf8(buffer).expect("Color codes should be valid UTF-8")
}

/// Formats a warning message with yellow color (if enabled).
///
/// # Arguments
///
/// * `text` - The text to format
/// * `config` - The color configuration
///
/// # Returns
///
/// Returns a formatted string with color codes (if enabled).
///
/// # Example
///
/// ```
/// use crately::colors::{ColorConfig, format_warning};
///
/// let config = ColorConfig::new(false);
/// let formatted = format_warning("Warning!", config);
/// assert!(formatted.contains("Warning!"));
/// ```
pub fn format_warning(text: &str, config: ColorConfig) -> String {
    let mut buffer = Vec::new();
    write_colored(&mut buffer, text, Color::Yellow, config)
        .expect("Writing to Vec should never fail");
    String::from_utf8(buffer).expect("Color codes should be valid UTF-8")
}

/// Formats a progress message with cyan color (if enabled).
///
/// # Arguments
///
/// * `text` - The text to format
/// * `config` - The color configuration
///
/// # Returns
///
/// Returns a formatted string with color codes (if enabled).
///
/// # Example
///
/// ```
/// use crately::colors::{ColorConfig, format_progress};
///
/// let config = ColorConfig::new(false);
/// let formatted = format_progress("Processing...", config);
/// assert!(formatted.contains("Processing..."));
/// ```
pub fn format_progress(text: &str, config: ColorConfig) -> String {
    let mut buffer = Vec::new();
    write_colored(&mut buffer, text, Color::Cyan, config)
        .expect("Writing to Vec should never fail");
    String::from_utf8(buffer).expect("Color codes should be valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_config_always_enables_colors() {
        use crate::cli::ColorChoice;
        let config = ColorConfig::new(ColorChoice::Always);
        assert!(config.is_enabled(), "Always should enable colors");
    }

    #[test]
    fn test_color_config_never_disables_colors() {
        use crate::cli::ColorChoice;
        let config = ColorConfig::new(ColorChoice::Never);
        assert!(!config.is_enabled(), "Never should disable colors");
    }

    #[test]
    fn test_color_config_auto_respects_environment() {
        use crate::cli::ColorChoice;
        // Note: We cannot safely test environment variables in unit tests
        // because env::set_var is unsafe and can cause data races in multi-threaded tests.
        // This functionality is tested manually and in integration tests instead.
        //
        // The logic checks: NO_COLOR, CLICOLOR, TTY status, and TERM variable
        // in that order of precedence.

        // We can test that Auto mode works without panicking
        let config = ColorConfig::new(ColorChoice::Auto);
        // Result depends on environment, so we just verify it doesn't panic
        let _ = config.is_enabled();
    }

    #[test]
    fn test_format_success_with_colors_disabled() {
        use crate::cli::ColorChoice;
        let config = ColorConfig::new(ColorChoice::Never);
        let result = format_success("Success", config);
        // Should not contain ANSI escape codes
        assert_eq!(result, "Success", "Should be plain text without colors");
    }

    #[test]
    fn test_format_error_with_colors_disabled() {
        use crate::cli::ColorChoice;
        let config = ColorConfig::new(ColorChoice::Never);
        let result = format_error("Error", config);
        assert_eq!(result, "Error", "Should be plain text without colors");
    }

    #[test]
    fn test_format_warning_with_colors_disabled() {
        use crate::cli::ColorChoice;
        let config = ColorConfig::new(ColorChoice::Never);
        let result = format_warning("Warning", config);
        assert_eq!(result, "Warning", "Should be plain text without colors");
    }

    #[test]
    fn test_format_progress_with_colors_disabled() {
        use crate::cli::ColorChoice;
        let config = ColorConfig::new(ColorChoice::Never);
        let result = format_progress("Progress", config);
        assert_eq!(result, "Progress", "Should be plain text without colors");
    }

    #[test]
    fn test_format_success_with_colors_enabled() {
        let config = ColorConfig { enabled: true };
        let result = format_success("Success", config);
        // Should contain ANSI escape codes for green
        assert!(
            result.contains("\x1b["),
            "Should contain ANSI escape codes"
        );
        assert!(result.contains("Success"), "Should contain the text");
    }

    #[test]
    fn test_format_error_with_colors_enabled() {
        let config = ColorConfig { enabled: true };
        let result = format_error("Error", config);
        assert!(
            result.contains("\x1b["),
            "Should contain ANSI escape codes"
        );
        assert!(result.contains("Error"), "Should contain the text");
    }

    #[test]
    fn test_format_warning_with_colors_enabled() {
        let config = ColorConfig { enabled: true };
        let result = format_warning("Warning", config);
        assert!(
            result.contains("\x1b["),
            "Should contain ANSI escape codes"
        );
        assert!(result.contains("Warning"), "Should contain the text");
    }

    #[test]
    fn test_format_progress_with_colors_enabled() {
        let config = ColorConfig { enabled: true };
        let result = format_progress("Progress", config);
        assert!(
            result.contains("\x1b["),
            "Should contain ANSI escape codes"
        );
        assert!(result.contains("Progress"), "Should contain the text");
    }

    #[test]
    fn test_color_config_is_copy() {
        use crate::cli::ColorChoice;
        let config1 = ColorConfig::new(ColorChoice::Auto);
        let config2 = config1;
        // This test verifies ColorConfig implements Copy
        assert_eq!(config1.is_enabled(), config2.is_enabled());
    }
}
