//! Message to start displaying an animated spinner for long-running operations.

use acton_reactive::prelude::*;

/// Message to start displaying an animated spinner with a status message.
///
/// This message is sent to the Console actor to begin displaying an animated
/// spinner indicating a long-running operation is in progress. The spinner
/// cycles through animation frames (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏) until stopped with `StopSpinner`.
///
/// # Actor Communication Pattern
///
/// Sender: Any actor initiating a long-running operation
/// Receiver: Console actor
///
/// # Fields
///
/// * `message` - Status message to display alongside the spinner (e.g., "Downloading serde@1.0.0...")
///
/// # Example
///
/// ```no_run
/// use crately::messages::PrintSpinner;
///
/// let spinner_msg = PrintSpinner {
///     message: "Downloading crate...".to_string(),
/// };
/// // console.send(spinner_msg).await;
/// ```
///
/// # Message Flow
///
/// 1. Actor initiating work sends PrintSpinner with operation description
/// 2. Console actor starts spinner animation in background task
/// 3. Spinner continues until StopSpinner received
/// 4. Console actor clears spinner and displays completion status
///
/// # Design Notes
///
/// The spinner animation runs in a separate tokio task to avoid blocking the
/// Console actor's message processing. The animation respects NO_COLOR environment
/// variable and raw mode state for proper terminal output.
#[acton_message]
pub struct PrintSpinner {
    /// The status message to display alongside the spinner
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that PrintSpinner can be created and cloned.
    #[test]
    fn test_print_spinner_creation() {
        let msg = PrintSpinner {
            message: "Processing...".to_string(),
        };
        let cloned = msg.clone();
        assert_eq!(msg.message, cloned.message);
    }

    /// Verify that PrintSpinner implements Debug.
    #[test]
    fn test_print_spinner_debug() {
        let msg = PrintSpinner {
            message: "Loading data...".to_string(),
        };
        let debug_str = format!("{:?}", msg);
        assert!(!debug_str.is_empty());
    }

    /// Verify that PrintSpinner is Send + Sync (required for actor message passing).
    #[test]
    fn test_print_spinner_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<PrintSpinner>();
        assert_sync::<PrintSpinner>();
    }

    /// Verify message field contains expected text.
    #[test]
    fn test_print_spinner_message_field() {
        let msg = PrintSpinner {
            message: "Downloading serde@1.0.0...".to_string(),
        };
        assert_eq!(msg.message, "Downloading serde@1.0.0...");
    }

    /// Verify empty message is valid.
    #[test]
    fn test_print_spinner_empty_message() {
        let msg = PrintSpinner {
            message: String::new(),
        };
        assert_eq!(msg.message, "");
    }

    /// Verify long message is valid.
    #[test]
    fn test_print_spinner_long_message() {
        let long_msg = "A".repeat(200);
        let msg = PrintSpinner {
            message: long_msg.clone(),
        };
        assert_eq!(msg.message, long_msg);
    }
}
