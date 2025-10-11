//! Warning output message for the Console actor.

use acton_reactive::prelude::*;

/// Message to print a warning message with the warning symbol (⚠).
///
/// This message is sent to the Console actor to display a warning message
/// formatted with the warning symbol. The Console actor handles the actual
/// output formatting and ensures proper line endings based on terminal mode.
///
/// # Purpose
///
/// Warning messages indicate potential issues or non-critical problems that
/// users should be aware of but don't prevent operation from continuing.
///
/// # Usage
///
/// The tuple field contains the warning message text to display. The Console
/// actor will prepend the warning symbol (⚠) and handle line ending formatting.
///
/// ## Example
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::PrintWarning;
///
/// async fn warn_user(console: AgentHandle) {
///     console.send(PrintWarning("Configuration reload failed".to_string())).await;
/// }
/// ```
///
/// # Design Notes
///
/// This is a tuple struct with a single String field because:
///
/// - Simple warning messages need only the text content
/// - The Console actor owns the formatting logic (symbols, line endings)
/// - Keeping the message small reduces overhead in the actor system
/// - Single-field tuple structs are idiomatic for wrapper types
///
/// The `#[acton_message(raw)]` attribute ensures efficient message passing
/// without additional wrapper overhead in the acton-reactive framework.
#[acton_message(raw)]
pub struct PrintWarning(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that PrintWarning can be created with a String.
    #[test]
    fn test_print_warning_creation() {
        let message = PrintWarning("Test warning".to_string());
        assert_eq!(message.0, "Test warning");
    }

    /// Verify that PrintWarning can be cloned (required by ActonMessage).
    #[test]
    fn test_print_warning_clone() {
        let message = PrintWarning("Test warning".to_string());
        let cloned = message.clone();
        assert_eq!(message.0, cloned.0);
    }

    /// Verify that PrintWarning implements Debug (required by ActonMessage).
    #[test]
    fn test_print_warning_debug() {
        let message = PrintWarning("Test warning".to_string());
        let debug_string = format!("{:?}", message);
        assert!(!debug_string.is_empty(), "Debug output should not be empty");
        // With #[acton_message(raw)], Debug format may be PrintWarning("Test warning") or similar
        // Just verify it's not empty - the exact format doesn't matter for functionality
    }

    /// Verify that PrintWarning is Send + Sync (required for actor message passing).
    #[test]
    fn test_print_warning_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<PrintWarning>();
        assert_sync::<PrintWarning>();
    }

    /// Verify that PrintWarning works with empty strings.
    #[test]
    fn test_print_warning_empty_string() {
        let message = PrintWarning(String::new());
        assert_eq!(message.0, "");
    }

    /// Verify that PrintWarning works with long strings.
    #[test]
    fn test_print_warning_long_string() {
        let long_text = "a".repeat(1000);
        let message = PrintWarning(long_text.clone());
        assert_eq!(message.0.len(), 1000);
        assert_eq!(message.0, long_text);
    }

    /// Verify that PrintWarning works with Unicode characters.
    #[test]
    fn test_print_warning_unicode() {
        let message = PrintWarning("警告: エラーが発生しました 🚨".to_string());
        assert!(message.0.contains("警告"));
        assert!(message.0.contains("🚨"));
    }
}
