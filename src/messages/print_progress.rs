//! Progress printing message for the Console actor.

use acton_reactive::prelude::*;

/// Message to print a progress message with the progress symbol (⋯).
///
/// This message instructs the Console actor to display a progress message to the user
/// with appropriate formatting and the progress symbol. The Console actor handles
/// terminal mode considerations (raw mode vs normal mode) for proper line endings.
///
/// # Purpose
///
/// Progress messages keep users informed about ongoing operations. They should:
///
/// - **Be Informative**: Describe what operation is in progress
/// - **Be Timely**: Provide updates at meaningful stages of operations
/// - **Be Consistent**: Use the progress symbol (⋯) for visual distinction
/// - **Be Non-Blocking**: All output goes through the Console actor for proper formatting
///
/// # Usage
///
/// Send this message to the Console actor to display a progress update. The Console actor
/// will handle terminal mode and formatting automatically.
///
/// ## Example
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::PrintProgress;
///
/// # async fn example(console: AgentHandle) {
/// // Report download progress
/// console.send(PrintProgress("Downloading crate serde 1.0.0".to_string())).await;
///
/// // Report processing progress
/// console.send(PrintProgress("Extracting documentation".to_string())).await;
///
/// // Report indexing progress
/// console.send(PrintProgress("Building search index".to_string())).await;
/// # }
/// ```
///
/// # Design Notes
///
/// This is a tuple struct wrapping a `String` because:
///
/// - **Simplicity**: Single-field messages use tuple struct pattern for brevity
/// - **Flexibility**: String allows arbitrary progress messages without enum variants
/// - **Efficiency**: Direct string ownership avoids lifetime complications
/// - **Consistency**: Matches the pattern used by other Print* messages
///
/// The `#[acton_message(raw)]` attribute is used because this is a simple wrapper
/// around primitive data without custom derives beyond what acton_message provides.
#[acton_message(raw)]
pub struct PrintProgress(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that PrintProgress can be created with a string.
    #[test]
    fn test_print_progress_creation() {
        let progress = PrintProgress("Test progress message".to_string());
        assert_eq!(progress.0, "Test progress message");
    }

    /// Verify that PrintProgress can be cloned (required by ActonMessage).
    #[test]
    fn test_print_progress_clone() {
        let progress = PrintProgress("Original message".to_string());
        let cloned = progress.clone();
        assert_eq!(progress.0, cloned.0);
    }

    /// Verify that PrintProgress implements Debug (required by ActonMessage).
    #[test]
    fn test_print_progress_debug() {
        let progress = PrintProgress("Debug test".to_string());
        let debug_string = format!("{:?}", progress);
        assert!(!debug_string.is_empty(), "Debug output should not be empty");
    }

    /// Verify that PrintProgress is Send + Sync (required for actor message passing).
    #[test]
    fn test_print_progress_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<PrintProgress>();
        assert_sync::<PrintProgress>();
    }

    /// Verify that PrintProgress can be created with an empty string.
    #[test]
    fn test_print_progress_empty_message() {
        let progress = PrintProgress(String::new());
        assert_eq!(progress.0, "");
    }

    /// Verify that PrintProgress can be created with a long message.
    #[test]
    fn test_print_progress_long_message() {
        let long_message = "a".repeat(1000);
        let progress = PrintProgress(long_message.clone());
        assert_eq!(progress.0.len(), 1000);
        assert_eq!(progress.0, long_message);
    }

    /// Verify that PrintProgress can be created with special characters.
    #[test]
    fn test_print_progress_special_characters() {
        let progress = PrintProgress("Processing: файл обрабатывается 处理中".to_string());
        assert!(progress.0.contains("файл"));
        assert!(progress.0.contains("处理中"));
    }
}
