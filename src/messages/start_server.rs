//! Server start message for Crately actors.

use acton_reactive::prelude::*;

/// Message requesting the ServerActor to start the HTTP server.
///
/// The `StartServer` message is sent to the `ServerActor` to begin serving HTTP
/// requests. The actor will bind to the configured address and port, then begin
/// accepting incoming connections.
///
/// # Purpose
///
/// This message enables controlled server lifecycle management:
///
/// - **Delayed Start**: Server can be prepared but not started until this message is received
/// - **Testability**: Tests can control when the server starts
/// - **Coordination**: Application can ensure all actors are ready before serving traffic
/// - **Explicit Intent**: Server startup is an explicit operation, not a side effect
///
/// # Usage
///
/// Send this message to the ServerActor handle to start the server:
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::StartServer;
///
/// async fn start_application(server_handle: &AgentHandle) {
///     server_handle.send(StartServer).await;
/// }
/// ```
///
/// # Behavior
///
/// When the ServerActor receives this message:
/// 1. Creates an Axum router with configured routes
/// 2. Binds to the configured address and port
/// 3. Stores the shutdown handle for graceful termination
/// 4. Begins accepting and processing HTTP requests
///
/// # Error Handling
///
/// If the server fails to start (e.g., port already in use), the ServerActor
/// will log the error and the application should handle the failure appropriately.
#[acton_message]
pub struct StartServer;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that StartServer can be created and cloned.
    #[test]
    fn test_start_server_creation() {
        let msg = StartServer;
        let _cloned = msg.clone();
    }

    /// Verify that StartServer implements Debug.
    #[test]
    fn test_start_server_debug() {
        let msg = StartServer;
        let debug_string = format!("{:?}", msg);
        assert!(!debug_string.is_empty());
    }

    /// Verify that StartServer is Send + Sync.
    #[test]
    fn test_start_server_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<StartServer>();
        assert_sync::<StartServer>();
    }
}
