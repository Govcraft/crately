//! Keyboard handler stop message for Crately actors.

use acton_reactive::prelude::*;

/// Message to stop the keyboard handler gracefully.
///
/// The `StopKeyboardHandler` message is sent to the `KeyboardHandler` actor to initiate
/// a graceful shutdown of the keyboard event monitoring system. The actor will stop
/// listening for keyboard events, disable terminal raw mode, and clean up resources.
///
/// # Purpose
///
/// This message enables controlled keyboard handler shutdown:
///
/// - **Graceful Termination**: Signals the event loop to stop cleanly
/// - **Raw Mode Cleanup**: Ensures terminal raw mode is properly disabled
/// - **Resource Release**: Cleans up event stream and channels
/// - **Testability**: Tests can control keyboard handler lifecycle precisely
///
/// # Usage
///
/// Send this message to the KeyboardHandler handle to stop keyboard monitoring:
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::StopKeyboardHandler;
///
/// async fn shutdown_keyboard(keyboard_handle: &AgentHandle) {
///     keyboard_handle.send(StopKeyboardHandler).await;
/// }
/// ```
///
/// # Behavior
///
/// When the KeyboardHandler receives this message:
/// 1. Signals the keyboard event loop to stop via the internal channel
/// 2. Notifies the Console actor that raw mode is being disabled
/// 3. Disables terminal raw mode
/// 4. Cleans up event stream resources
///
/// # Shutdown Sequence
///
/// This message is typically used during application shutdown:
///
/// ```text
/// Application Shutdown → StopKeyboardHandler → Disable Raw Mode → Cleanup
/// ```
#[acton_message]
pub struct StopKeyboardHandler;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that StopKeyboardHandler can be created and cloned.
    #[test]
    fn test_stop_keyboard_handler_creation() {
        let msg = StopKeyboardHandler;
        let _cloned = msg.clone();
    }

    /// Verify that StopKeyboardHandler implements Debug.
    #[test]
    fn test_stop_keyboard_handler_debug() {
        let msg = StopKeyboardHandler;
        let debug_string = format!("{:?}", msg);
        assert!(!debug_string.is_empty());
    }

    /// Verify that StopKeyboardHandler is Send + Sync.
    #[test]
    fn test_stop_keyboard_handler_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<StopKeyboardHandler>();
        assert_sync::<StopKeyboardHandler>();
    }
}
