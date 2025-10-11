//! ServerReloaded message type.
//!
//! This message is broadcast when the server has been successfully reloaded
//! with new configuration.

use acton_reactive::prelude::*;

/// Broadcast message when server reload completes successfully
///
/// This message is sent by the ServerActor after it successfully restarts
/// with new configuration. It allows the Console actor to display a
/// success confirmation to the user.
///
/// # Fields
///
/// * `port` - The port number the server is now listening on
///
/// # Example
///
/// ```no_run
/// use crately::messages::ServerReloaded;
/// use acton_reactive::prelude::*;
///
/// async fn notify_reload_complete(broker: BrokerRef, port: u16) {
///     let message = ServerReloaded { port };
///     broker.broadcast(message).await;
/// }
/// ```
#[acton_message]
pub struct ServerReloaded {
    /// The port number the server is listening on after reload
    pub port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_reloaded_creation() {
        let message = ServerReloaded { port: 3000 };
        assert_eq!(message.port, 3000);
    }

    #[test]
    fn test_server_reloaded_clone() {
        let message1 = ServerReloaded { port: 8080 };
        let message2 = message1.clone();
        assert_eq!(message1.port, message2.port);
    }

    #[test]
    fn test_server_reloaded_debug() {
        let message = ServerReloaded { port: 4000 };
        let debug_str = format!("{:?}", message);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_server_reloaded_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ServerReloaded>();
    }
}
