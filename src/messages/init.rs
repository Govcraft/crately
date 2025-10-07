//! Initialization message for Crately actors.

use acton_reactive::prelude::*;

/// Message sent to actors during initialization phase.
///
/// The `Init` message is used to signal that an actor should perform its initialization
/// sequence. This is typically sent after actor creation but before the actor begins
/// processing domain-specific messages.
///
/// # Purpose
///
/// Initialization messages serve several important purposes:
///
/// - **Resource Allocation**: Allow actors to set up required resources (connections, caches, etc.)
/// - **State Preparation**: Initialize internal state before processing work messages
/// - **Dependency Verification**: Confirm that external dependencies are available
/// - **Configuration Loading**: Load and validate configuration specific to the actor
///
/// # Usage
///
/// The `Init` message has no fields because initialization parameters should be passed
/// during actor construction. This message simply triggers the initialization sequence.
///
/// ## Example
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::Init;
///
/// // In an actor's message handler
/// async fn handle_init(init: Init) {
///     // Perform initialization tasks
///     println!("Received init message: {:?}", init);
///     // Set up resources, load configuration, etc.
/// }
/// ```
///
/// # Design Notes
///
/// This is a fieldless struct (unit-like struct) because:
///
/// - Initialization parameters should be provided during actor construction
/// - The message serves as a pure signal/trigger
/// - Keeping messages small reduces overhead in the actor system
/// - Separates configuration (constructor) from lifecycle events (messages)
///
/// If initialization requires dynamic parameters, consider either:
/// - Passing them through the actor's constructor
/// - Creating a separate parameterized initialization message type
#[acton_message]
pub struct Init;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that Init can be created and cloned (required by ActonMessage).
    #[test]
    fn test_init_creation() {
        let init = Init;
        let _cloned = init.clone();
        // If we get here without panicking, Clone is properly implemented
    }

    /// Verify that Init implements Debug (required by ActonMessage).
    #[test]
    fn test_init_debug() {
        let init = Init;
        let debug_string = format!("{:?}", init);
        assert!(!debug_string.is_empty(), "Debug output should not be empty");
    }

    /// Verify that Init is Send + Sync (required for actor message passing).
    #[test]
    fn test_init_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<Init>();
        assert_sync::<Init>();
    }
}
