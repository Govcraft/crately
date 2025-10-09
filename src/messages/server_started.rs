//! Server startup notification message for Crately actors.

use acton_reactive::prelude::*;

/// Message broadcast when the HTTP server has successfully started.
///
/// The `ServerStarted` message is broadcast by the `ServerActor` via the broker
/// after the HTTP server has successfully bound to its address and is ready to
/// accept connections. This follows the pub/sub pattern where subscribers can
/// react to server startup without direct coupling to the ServerActor.
///
/// # Purpose
///
/// This message serves as the architectural boundary for console output:
///
/// - **Separation of Concerns**: ServerActor focuses on server lifecycle, Console actor handles display
/// - **Event-Driven Architecture**: Server startup triggers console display via event broadcasting
/// - **Testability**: Console output can be tested independently from server logic
/// - **Flexibility**: Easy to add multiple subscribers for server startup events
///
/// # Architecture Pattern: Event Broadcasting for Console Display
///
/// ```text
/// ServerActor (after successful bind) → broadcasts ServerStarted
///      |
///      v
/// Console (subscribed) → receives ServerStarted → displays user instructions
/// ```
///
/// # Usage
///
/// Actors subscribe to this message to react to server startup:
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::ServerStarted;
///
/// // In an actor's initialization
/// async fn setup_server_started_subscription(handle: &AgentHandle) {
///     handle.subscribe::<ServerStarted>().await;
/// }
/// ```
///
/// Then handle the message in the actor's builder:
///
/// ```rust,ignore
/// builder.act_on::<ServerStarted>(|_actor, _envelope| {
///     eprintln!();
///     eprintln!("Server is running. Press 'q' or Ctrl+C to shutdown gracefully");
///     eprintln!("Press 'r' to reload configuration");
///     eprintln!();
///     AgentReply::immediate()
/// });
/// ```
///
/// # Design Notes
///
/// This message is a **zero-sized type** (unit struct) because it carries no data.
/// The event itself is the information - the server has started. Subscribers can
/// query the ServerActor or ConfigManager for details if needed.
///
/// # Architectural Principle
///
/// **All console output must flow through a dedicated Console actor.**
///
/// This message enables that principle by:
/// 1. Allowing ServerActor to signal readiness without doing direct I/O
/// 2. Delegating all console display to the Console actor
/// 3. Maintaining proper separation of concerns in the actor model
#[acton_message(raw)]
pub struct ServerStarted;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that ServerStarted can be created and cloned.
    #[test]
    fn test_server_started_creation() {
        let msg = ServerStarted;
        let _cloned = msg.clone();
    }

    /// Verify that ServerStarted implements Debug.
    #[test]
    fn test_server_started_debug() {
        let msg = ServerStarted;
        let debug_string = format!("{:?}", msg);
        assert!(!debug_string.is_empty());
    }

    /// Verify that ServerStarted is Send + Sync.
    #[test]
    fn test_server_started_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<ServerStarted>();
        assert_sync::<ServerStarted>();
    }
}
