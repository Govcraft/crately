//! Server stop message for Crately actors.

use acton_reactive::prelude::*;

/// Message requesting the ServerActor to gracefully stop the HTTP server.
///
/// The `StopServer` message is sent to the `ServerActor` to initiate a graceful
/// shutdown of the HTTP server. The actor will stop accepting new connections,
/// drain existing requests, and then signal completion.
///
/// # Purpose
///
/// This message enables controlled server shutdown:
///
/// - **Graceful Shutdown**: Allows in-flight requests to complete before stopping
/// - **Clean Termination**: Ensures resources are properly released
/// - **Testability**: Tests can control server lifecycle precisely
/// - **Hot Reload Support**: Server can be stopped and restarted with new configuration
///
/// # Usage
///
/// Send this message to the ServerActor handle to stop the server:
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::StopServer;
///
/// async fn shutdown_application(server_handle: &AgentHandle) {
///     server_handle.send(StopServer).await;
/// }
/// ```
///
/// # Behavior
///
/// When the ServerActor receives this message:
/// 1. Signals the server shutdown handle
/// 2. Waits for in-flight requests to complete
/// 3. Cleans up server resources
/// 4. Updates internal state to reflect server is stopped
///
/// # Hot Reload Pattern
///
/// This message is used in conjunction with `StartServer` to implement hot reload:
///
/// ```text
/// ConfigChanged → StopServer → Update Config → StartServer
/// ```
#[acton_message]
pub struct StopServer;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that StopServer can be created and cloned.
    #[test]
    fn test_stop_server_creation() {
        let msg = StopServer;
        let _cloned = msg.clone();
    }

    /// Verify that StopServer implements Debug.
    #[test]
    fn test_stop_server_debug() {
        let msg = StopServer;
        let debug_string = format!("{:?}", msg);
        assert!(!debug_string.is_empty());
    }

    /// Verify that StopServer is Send + Sync.
    #[test]
    fn test_stop_server_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<StopServer>();
        assert_sync::<StopServer>();
    }
}
