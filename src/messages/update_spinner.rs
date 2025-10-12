//! Message to update the status message of an active spinner.

use acton_reactive::prelude::*;

/// Message to update the status message of an active spinner.
///
/// This message is sent to the Console actor to change the text displayed
/// alongside a currently running spinner. This is useful for showing progress
/// through multi-step operations (e.g., "Processing chunk 5/18").
///
/// # Actor Communication Pattern
///
/// Sender: Any actor with an active spinner
/// Receiver: Console actor
///
/// # Fields
///
/// * `message` - New status message to display (e.g., "Processing chunk 10/18")
///
/// # Example
///
/// ```no_run
/// use crately::messages::UpdateSpinner;
///
/// let update_msg = UpdateSpinner {
///     message: "Processing chunk 10/18".to_string(),
/// };
/// // console.send(update_msg).await;
/// ```
///
/// # Message Flow
///
/// 1. Actor with active spinner sends UpdateSpinner with new message
/// 2. Console actor updates the displayed status text
/// 3. Spinner animation continues with new message
///
/// # Design Notes
///
/// If no spinner is currently active, this message is silently ignored to
/// simplify error handling in sending actors. The spinner animation continues
/// running; only the text is updated.
#[acton_message]
pub struct UpdateSpinner {
    /// The new status message to display
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that UpdateSpinner can be created and cloned.
    #[test]
    fn test_update_spinner_creation() {
        let msg = UpdateSpinner {
            message: "Step 2/5".to_string(),
        };
        let cloned = msg.clone();
        assert_eq!(msg.message, cloned.message);
    }

    /// Verify that UpdateSpinner implements Debug.
    #[test]
    fn test_update_spinner_debug() {
        let msg = UpdateSpinner {
            message: "Downloading...".to_string(),
        };
        let debug_str = format!("{:?}", msg);
        assert!(!debug_str.is_empty());
    }

    /// Verify that UpdateSpinner is Send + Sync (required for actor message passing).
    #[test]
    fn test_update_spinner_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<UpdateSpinner>();
        assert_sync::<UpdateSpinner>();
    }

    /// Verify message field contains expected text.
    #[test]
    fn test_update_spinner_message_field() {
        let msg = UpdateSpinner {
            message: "Processing chunk 5/18".to_string(),
        };
        assert_eq!(msg.message, "Processing chunk 5/18");
    }

    /// Verify progress format with numbers.
    #[test]
    fn test_update_spinner_progress_format() {
        let msg = UpdateSpinner {
            message: "Vectorizing 12/24 with text-embedding-3-small".to_string(),
        };
        assert!(msg.message.contains("12/24"));
        assert!(msg.message.contains("text-embedding-3-small"));
    }

    /// Verify empty message is valid.
    #[test]
    fn test_update_spinner_empty_message() {
        let msg = UpdateSpinner {
            message: String::new(),
        };
        assert_eq!(msg.message, "");
    }
}
