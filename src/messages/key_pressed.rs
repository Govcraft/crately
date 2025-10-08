//! Keyboard event message for Crately actors.

use acton_reactive::prelude::*;
use crossterm::event::{KeyCode, KeyModifiers};

/// Message broadcast when a key is pressed in the keyboard handler.
///
/// The `KeyPressed` message is used by the `KeyboardHandler` actor to broadcast
/// keyboard events to all subscribed actors in the system. This follows the pub/sub
/// pattern where the keyboard handler simply broadcasts events without dictating
/// their meaning - subscribing actors decide how to handle each key event.
///
/// # Purpose
///
/// This message serves as a decoupling mechanism:
///
/// - **Separation of Concerns**: Keyboard handler focuses on event detection
/// - **Flexible Handling**: Different actors can respond to the same key differently
/// - **Testability**: Keyboard events can be simulated by sending this message
/// - **Extensibility**: New key handlers can be added without modifying the keyboard actor
///
/// # Architecture Pattern
///
/// ```text
/// KeyboardHandler → broadcasts KeyPressed
///      |
///      v
/// ServerActor (subscribed) → receives KeyPressed → handles if relevant
/// ConfigManager → broadcasts ConfigChanged → ServerActor receives → hot reload
/// ```
///
/// # Usage
///
/// Actors subscribe to this message to receive keyboard events:
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::KeyPressed;
///
/// // In an actor's initialization
/// async fn setup_keyboard_subscription(handle: &AgentHandle) {
///     handle.subscribe::<KeyPressed>().await;
/// }
/// ```
///
/// Then handle the message in the actor's builder:
///
/// ```rust,ignore
/// builder.act_on::<KeyPressed>(|agent, envelope| {
///     let key_event = envelope.message();
///     match key_event.key {
///         KeyCode::Char('r') => {
///             // Handle reload
///         }
///         KeyCode::Char('q') => {
///             // Handle quit
///         }
///         _ => {}
///     }
///     AgentReply::immediate()
/// });
/// ```
///
/// # Design Notes
///
/// The message contains the raw keyboard event data:
/// - `key`: The key code (character, function key, arrow, etc.)
/// - `modifiers`: Modifier keys pressed (Ctrl, Alt, Shift, etc.)
///
/// This allows maximum flexibility for handlers to implement context-specific behavior.
#[acton_message]
pub struct KeyPressed {
    /// The key code that was pressed
    pub key: KeyCode,
    /// The modifier keys that were held during the press
    pub modifiers: KeyModifiers,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that KeyPressed can be created with different key codes.
    #[test]
    fn test_key_pressed_creation() {
        let msg = KeyPressed {
            key: KeyCode::Char('a'),
            modifiers: KeyModifiers::empty(),
        };
        let _cloned = msg.clone();
    }

    /// Verify that KeyPressed works with modifier keys.
    #[test]
    fn test_key_pressed_with_modifiers() {
        let msg = KeyPressed {
            key: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
        };
        assert_eq!(msg.modifiers, KeyModifiers::CONTROL);
    }

    /// Verify that KeyPressed implements Debug.
    #[test]
    fn test_key_pressed_debug() {
        let msg = KeyPressed {
            key: KeyCode::Enter,
            modifiers: KeyModifiers::empty(),
        };
        let debug_string = format!("{:?}", msg);
        assert!(!debug_string.is_empty());
    }

    /// Verify that KeyPressed is Send + Sync.
    #[test]
    fn test_key_pressed_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<KeyPressed>();
        assert_sync::<KeyPressed>();
    }
}
