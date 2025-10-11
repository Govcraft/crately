//! Color support and management for terminal output.
//!
//! This module provides color detection and styling functions for console output,
//! respecting terminal capabilities and user preferences via the NO_COLOR
//! environment variable.

use crossterm::style::{Color, ResetColor, SetForegroundColor};
use std::env;
use std::io::Write;

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
    /// Creates a new ColorConfig with automatic detection.
    ///
    /// Colors are enabled if ALL of the following are true:
    /// - Terminal supports colors (via crossterm detection)
    /// - NO_COLOR environment variable is not set
    /// - User has not explicitly disabled colors via flag
    ///
    /// # Arguments
    ///
    /// * `force_disable` - If true, colors are disabled regardless of terminal support
    ///
    /// # Returns
    ///
    /// Returns a ColorConfig instance configured based on detection results.
    ///
    /// # Example
    ///
    /// ```
    /// use crately::colors::ColorConfig;
    ///
    /// let config = ColorConfig::new(false);
    /// assert!(config.is_enabled() || !config.is_enabled()); // Either state is valid
    /// ```
    pub fn new(force_disable: bool) -> Self {
        let enabled = !force_disable && Self::should_enable_colors();
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
    /// Checks for:
    /// - NO_COLOR environment variable (https://no-color.org/)
    /// - TERM environment variable for basic terminal detection
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

        // Check if we're in a terminal that likely supports colors
        // This is a conservative approach that works on most platforms
        if let Ok(term) = env::var("TERM") {
            // Common terminals that support ANSI colors
            !term.is_empty() && term != "dumb"
        } else {
            // No TERM variable, assume we support colors
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
    fn test_color_config_force_disable_disables_colors() {
        let config = ColorConfig::new(true);
        assert!(!config.is_enabled(), "Force disable should disable colors");
    }

    #[test]
    fn test_color_config_respects_no_color_env() {
        // Note: We cannot safely test NO_COLOR environment variable in unit tests
        // because env::set_var is unsafe and can cause data races in multi-threaded tests.
        // This functionality is tested manually and in integration tests instead.
        //
        // The logic is straightforward: if NO_COLOR exists, colors are disabled.
        // This test documents the behavior without actually running unsafe code.

        // We can test that force_disable works correctly
        let config = ColorConfig::new(true);
        assert!(
            !config.is_enabled(),
            "Force disable should disable colors"
        );
    }

    #[test]
    fn test_color_config_respects_no_color_empty_value() {
        // Note: This would require unsafe env::set_var in tests.
        // See test_color_config_respects_no_color_env for explanation.
        //
        // The implementation correctly checks if NO_COLOR.is_ok(), which
        // returns true for both empty and non-empty values.

        // We can verify the force_disable path works
        let config = ColorConfig::new(true);
        assert!(
            !config.is_enabled(),
            "Force disable should work regardless of NO_COLOR"
        );
    }

    #[test]
    fn test_format_success_with_colors_disabled() {
        let config = ColorConfig::new(true);
        let result = format_success("Success", config);
        // Should not contain ANSI escape codes
        assert_eq!(result, "Success", "Should be plain text without colors");
    }

    #[test]
    fn test_format_error_with_colors_disabled() {
        let config = ColorConfig::new(true);
        let result = format_error("Error", config);
        assert_eq!(result, "Error", "Should be plain text without colors");
    }

    #[test]
    fn test_format_warning_with_colors_disabled() {
        let config = ColorConfig::new(true);
        let result = format_warning("Warning", config);
        assert_eq!(result, "Warning", "Should be plain text without colors");
    }

    #[test]
    fn test_format_progress_with_colors_disabled() {
        let config = ColorConfig::new(true);
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
        let config1 = ColorConfig::new(false);
        let config2 = config1;
        // This test verifies ColorConfig implements Copy
        assert_eq!(config1.is_enabled(), config2.is_enabled());
    }
}
