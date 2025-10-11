//! Internal initialization message for the KeyboardHandler actor.

use acton_reactive::prelude::*;

/// Message sent after the keyboard handler starts to complete initialization.
///
/// The `KeyboardHandlerStarted` message is an internal message used by the
/// KeyboardHandler actor to trigger its initialization sequence after the actor
/// has been spawned and started. This message sets up the keyboard event loop
/// and prepares the actor to begin processing keyboard events.
///
/// # Purpose
///
/// This message serves as a two-phase initialization signal:
///
/// - **Phase 1 (after_start)**: Enable terminal raw mode and notify Console
/// - **Phase 2 (this message)**: Set up the keyboard event reader loop
///
/// # Visibility
///
/// This message is private to the keyboard_handler module because:
///
/// - It's an internal implementation detail of actor initialization
/// - External actors should not send this message
/// - The KeyboardHandler::spawn method sends it automatically
/// - Exposing it would create confusion about intended usage
///
/// # Usage
///
/// This message is sent automatically by `KeyboardHandler::spawn()` immediately
/// after the actor is started. External code should never need to send this message.
///
/// ## Example (Internal to KeyboardHandler)
///
/// ```rust,ignore
/// // Inside KeyboardHandler::spawn
/// let handle = builder.start().await;
/// handle.send(KeyboardHandlerStarted).await;  // Trigger initialization
/// ```
///
/// # Design Notes
///
/// This is a fieldless struct because:
///
/// - It's a pure lifecycle signal with no parameters
/// - All initialization state is already in the actor model
/// - Keeping messages small reduces message passing overhead
/// - Simplifies the initialization sequence
#[acton_message]
pub(crate) struct KeyboardHandlerStarted;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that KeyboardHandlerStarted can be created and cloned.
    #[test]
    fn test_keyboard_handler_started_creation() {
        let msg = KeyboardHandlerStarted;
        let _cloned = msg.clone();
        // If we get here without panicking, Clone is properly implemented
    }

    /// Verify that KeyboardHandlerStarted implements Debug.
    #[test]
    fn test_keyboard_handler_started_debug() {
        let msg = KeyboardHandlerStarted;
        let debug_string = format!("{:?}", msg);
        assert!(
            !debug_string.is_empty(),
            "Debug output should not be empty"
        );
    }

    /// Verify that KeyboardHandlerStarted is Send + Sync.
    #[test]
    fn test_keyboard_handler_started_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<KeyboardHandlerStarted>();
        assert_sync::<KeyboardHandlerStarted>();
    }
}
