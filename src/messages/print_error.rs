//! Error printing message for the Console actor.

use acton_reactive::prelude::*;

/// Message to print an error message with the error symbol (✗).
///
/// This message instructs the Console actor to display an error message to the user
/// with appropriate formatting and the error symbol. The Console actor handles
/// terminal mode considerations (raw mode vs normal mode) for proper line endings.
///
/// # Purpose
///
/// Error messages are critical for user feedback when operations fail. They should:
///
/// - **Be Clear**: Describe what went wrong in understandable terms
/// - **Be Actionable**: When possible, suggest what the user can do
/// - **Be Visible**: Use the error symbol (✗) for visual distinction
/// - **Be Consistent**: All errors go through the Console actor for uniform formatting
///
/// # Usage
///
/// Send this message to the Console actor to display an error. The Console actor
/// will handle terminal mode and formatting automatically.
///
/// ## Example
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::PrintError;
///
/// # async fn example(console: AgentHandle) {
/// // Report a download failure
/// console.send(PrintError("Failed to download crate: network error".to_string())).await;
///
/// // Report a validation error
/// console.send(PrintError("Invalid crate version format".to_string())).await;
/// # }
/// ```
///
/// # Design Notes
///
/// This is a tuple struct wrapping a `String` because:
///
/// - **Simplicity**: Single-field messages use tuple struct pattern for brevity
/// - **Flexibility**: String allows arbitrary error messages without enum variants
/// - **Efficiency**: Direct string ownership avoids lifetime complications
/// - **Consistency**: Matches the pattern used by other Print* messages
///
/// The `#[acton_message(raw)]` attribute is used because this is a simple wrapper
/// around primitive data without custom derives beyond what acton_message provides.
#[acton_message(raw)]
pub struct PrintError(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that PrintError can be created with a string.
    #[test]
    fn test_print_error_creation() {
        let error = PrintError("Test error message".to_string());
        assert_eq!(error.0, "Test error message");
    }

    /// Verify that PrintError can be cloned (required by ActonMessage).
    #[test]
    fn test_print_error_clone() {
        let error = PrintError("Original message".to_string());
        let cloned = error.clone();
        assert_eq!(error.0, cloned.0);
    }

    /// Verify that PrintError implements Debug (required by ActonMessage).
    #[test]
    fn test_print_error_debug() {
        let error = PrintError("Debug test".to_string());
        let debug_string = format!("{:?}", error);
        assert!(!debug_string.is_empty(), "Debug output should not be empty");
    }

    /// Verify that PrintError is Send + Sync (required for actor message passing).
    #[test]
    fn test_print_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<PrintError>();
        assert_sync::<PrintError>();
    }

    /// Verify that PrintError can be created with an empty string.
    #[test]
    fn test_print_error_empty_message() {
        let error = PrintError(String::new());
        assert_eq!(error.0, "");
    }

    /// Verify that PrintError can be created with a long message.
    #[test]
    fn test_print_error_long_message() {
        let long_message = "a".repeat(1000);
        let error = PrintError(long_message.clone());
        assert_eq!(error.0.len(), 1000);
        assert_eq!(error.0, long_message);
    }

    /// Verify that PrintError can be created with special characters.
    #[test]
    fn test_print_error_special_characters() {
        let error = PrintError("Error: файл не найден 文件未找到".to_string());
        assert!(error.0.contains("файл"));
        assert!(error.0.contains("文件"));
    }
}
