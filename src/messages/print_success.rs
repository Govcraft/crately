//! Success message for console output.

use acton_reactive::prelude::*;

/// Message to print a success message with the success symbol (✓).
///
/// The `PrintSuccess` message is sent to the Console actor to display success
/// messages to the user. The Console actor handles the formatting and adds
/// the appropriate success symbol.
///
/// # Purpose
///
/// This message provides a centralized way to display success feedback:
///
/// - **Consistent Formatting**: All success messages use the same symbol and style
/// - **Terminal Mode Awareness**: Console actor handles raw/normal mode line endings
/// - **Centralized Output**: All console output flows through the Console actor
/// - **Testability**: Success output can be tested by sending messages to test actors
///
/// # Usage
///
/// Send this message to the Console actor to display a success message:
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::PrintSuccess;
///
/// async fn notify_success(console: &AgentHandle) {
///     console.send(PrintSuccess("Operation completed successfully".to_string())).await;
/// }
/// ```
///
/// # Design Notes
///
/// This is a tuple struct (newtype pattern) containing a single `String` field:
///
/// - The `#[acton_message(raw)]` attribute creates a tuple struct
/// - The single field contains the message text to display
/// - The Console actor adds the success symbol (✓) during rendering
/// - Raw mode line ending handling is managed by the Console actor
#[acton_message(raw)]
pub struct PrintSuccess(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that PrintSuccess can be created with a string.
    #[test]
    fn test_print_success_creation() {
        let msg = PrintSuccess("Test message".to_string());
        let _cloned = msg.clone();
    }

    /// Verify that PrintSuccess can access its inner string.
    #[test]
    fn test_print_success_access() {
        let msg = PrintSuccess("Success text".to_string());
        assert_eq!(msg.0, "Success text");
    }

    /// Verify that PrintSuccess implements Debug.
    #[test]
    fn test_print_success_debug() {
        let msg = PrintSuccess("Debug test".to_string());
        let debug_string = format!("{:?}", msg);
        assert!(!debug_string.is_empty());
        assert!(debug_string.contains("Debug test"));
    }

    /// Verify that PrintSuccess is Send + Sync.
    #[test]
    fn test_print_success_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<PrintSuccess>();
        assert_sync::<PrintSuccess>();
    }

    /// Verify that PrintSuccess works with empty strings.
    #[test]
    fn test_print_success_empty_string() {
        let msg = PrintSuccess(String::new());
        assert_eq!(msg.0, "");
    }

    /// Verify that PrintSuccess works with multi-line strings.
    #[test]
    fn test_print_success_multiline() {
        let msg = PrintSuccess("Line 1\nLine 2".to_string());
        assert!(msg.0.contains('\n'));
    }
}
