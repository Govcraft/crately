//! ConfigReloadFailed message type.
//!
//! This message is broadcast when configuration reload fails, allowing
//! the Console actor to display user-facing error messages.

use acton_reactive::prelude::*;

/// Broadcast message when configuration reload fails
///
/// This message is sent by the ConfigManager actor when a configuration
/// reload operation fails. It allows the Console actor to display
/// user-facing error messages and warnings.
///
/// # Fields
///
/// * `error` - A human-readable error message describing what went wrong
///
/// # Example
///
/// ```no_run
/// use crately::messages::ConfigReloadFailed;
/// use acton_reactive::prelude::*;
///
/// async fn handle_reload_failure(console: AgentHandle) {
///     let message = ConfigReloadFailed {
///         error: "Invalid TOML syntax at line 5".to_string(),
///     };
///     console.send(message).await;
/// }
/// ```
#[acton_message(raw)]
pub struct ConfigReloadFailed {
    /// Human-readable error message describing the failure
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_reload_failed_creation() {
        let message = ConfigReloadFailed {
            error: "Test error".to_string(),
        };
        assert_eq!(message.error, "Test error");
    }

    #[test]
    fn test_config_reload_failed_clone() {
        let message1 = ConfigReloadFailed {
            error: "Test error".to_string(),
        };
        let message2 = message1.clone();
        assert_eq!(message1.error, message2.error);
    }

    #[test]
    fn test_config_reload_failed_debug() {
        let message = ConfigReloadFailed {
            error: "Test error".to_string(),
        };
        let debug_str = format!("{:?}", message);
        assert!(!debug_str.is_empty());
    }
}
