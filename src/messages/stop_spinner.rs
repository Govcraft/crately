//! Message to stop an active spinner and display completion status.

use acton_reactive::prelude::*;

/// Message to stop an active spinner and display completion status.
///
/// This message is sent to the Console actor to stop a running spinner and
/// optionally display a final completion message with success/failure status.
///
/// # Actor Communication Pattern
///
/// Sender: Any actor completing a long-running operation
/// Receiver: Console actor
///
/// # Fields
///
/// * `success` - Whether the operation completed successfully
/// * `final_message` - Optional final status message to display after stopping spinner
///
/// # Example
///
/// ```no_run
/// use crately::messages::StopSpinner;
///
/// // Success case with message
/// let success_msg = StopSpinner {
///     success: true,
///     final_message: Some("Downloaded serde@1.0.0".to_string()),
/// };
/// // console.send(success_msg).await;
///
/// // Failure case
/// let failure_msg = StopSpinner {
///     success: false,
///     final_message: Some("Download failed: network error".to_string()),
/// };
/// // console.send(failure_msg).await;
///
/// // Just stop spinner without message
/// let simple_stop = StopSpinner {
///     success: true,
///     final_message: None,
/// };
/// // console.send(simple_stop).await;
/// ```
///
/// # Message Flow
///
/// 1. Actor completing work sends StopSpinner with success flag and optional message
/// 2. Console actor stops spinner animation task
/// 3. Console actor clears spinner from display
/// 4. If final_message provided, Console displays it with appropriate formatting
///
/// # Design Notes
///
/// If no spinner is currently active, the final message is still displayed to
/// ensure operation feedback reaches the user. The success flag determines
/// whether the message is formatted as success (✓) or error (✗).
#[acton_message]
pub struct StopSpinner {
    /// Whether the operation completed successfully
    pub success: bool,

    /// Optional final status message to display after stopping spinner
    pub final_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that StopSpinner can be created and cloned.
    #[test]
    fn test_stop_spinner_creation() {
        let msg = StopSpinner {
            success: true,
            final_message: Some("Operation complete".to_string()),
        };
        let cloned = msg.clone();
        assert_eq!(msg.success, cloned.success);
        assert_eq!(msg.final_message, cloned.final_message);
    }

    /// Verify that StopSpinner implements Debug.
    #[test]
    fn test_stop_spinner_debug() {
        let msg = StopSpinner {
            success: false,
            final_message: None,
        };
        let debug_str = format!("{:?}", msg);
        assert!(!debug_str.is_empty());
    }

    /// Verify that StopSpinner is Send + Sync (required for actor message passing).
    #[test]
    fn test_stop_spinner_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<StopSpinner>();
        assert_sync::<StopSpinner>();
    }

    /// Verify success with message.
    #[test]
    fn test_stop_spinner_success_with_message() {
        let msg = StopSpinner {
            success: true,
            final_message: Some("Downloaded successfully".to_string()),
        };
        assert!(msg.success);
        assert_eq!(msg.final_message, Some("Downloaded successfully".to_string()));
    }

    /// Verify failure with message.
    #[test]
    fn test_stop_spinner_failure_with_message() {
        let msg = StopSpinner {
            success: false,
            final_message: Some("Operation failed".to_string()),
        };
        assert!(!msg.success);
        assert_eq!(msg.final_message, Some("Operation failed".to_string()));
    }

    /// Verify success without message.
    #[test]
    fn test_stop_spinner_success_without_message() {
        let msg = StopSpinner {
            success: true,
            final_message: None,
        };
        assert!(msg.success);
        assert_eq!(msg.final_message, None);
    }

    /// Verify failure without message.
    #[test]
    fn test_stop_spinner_failure_without_message() {
        let msg = StopSpinner {
            success: false,
            final_message: None,
        };
        assert!(!msg.success);
        assert_eq!(msg.final_message, None);
    }

    /// Verify empty message string is valid.
    #[test]
    fn test_stop_spinner_empty_message_string() {
        let msg = StopSpinner {
            success: true,
            final_message: Some(String::new()),
        };
        assert_eq!(msg.final_message, Some(String::new()));
    }
}
