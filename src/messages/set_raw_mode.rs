//! Message to notify the Console actor about terminal raw mode state changes.
//!
//! Terminal raw mode requires different line ending handling:
//! - Normal mode: `\n` (line feed) is sufficient
//! - Raw mode: `\r\n` (carriage return + line feed) is required
//!
//! The KeyboardHandler actor sends this message to coordinate proper
//! console output formatting when raw mode is enabled or disabled.

use acton_reactive::prelude::*;

/// Message to notify Console of terminal raw mode state changes.
///
/// This message is sent by the KeyboardHandler actor when it enables or
/// disables terminal raw mode. The Console actor uses this information to
/// format output correctly:
///
/// - When raw mode is active, use `\r\n` for line endings
/// - When raw mode is inactive, use standard `\n` line endings
///
/// # Example
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::SetRawMode;
///
/// async fn enable_raw_mode_and_notify(console_handle: AgentHandle) {
///     // ... enable terminal raw mode ...
///
///     // Notify console that raw mode is active
///     console_handle.send(SetRawMode(true)).await;
/// }
/// ```
#[acton_message(raw)]
pub struct SetRawMode(pub bool);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_raw_mode_creates_with_true() {
        let msg = SetRawMode(true);
        assert!(msg.0);
    }

    #[test]
    fn test_set_raw_mode_creates_with_false() {
        let msg = SetRawMode(false);
        assert!(!msg.0);
    }
}
