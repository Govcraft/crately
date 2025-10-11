//! Event broadcast when the database is fully initialized and ready.

use acton_reactive::prelude::*;

/// Event broadcast when DatabaseActor initialization completes.
///
/// This message is broadcast via the broker after the DatabaseActor successfully:
/// - Connects to the embedded RocksDB database
/// - Selects the namespace and database
/// - Executes all schema definitions
///
/// Subscribers can use this signal to know when database-dependent operations
/// can safely begin.
///
/// # Example
///
/// ```no_run
/// use crately::messages::DatabaseReady;
///
/// // In Console actor - subscribe to the event
/// builder.act_on::<DatabaseReady>(|_actor, _envelope| {
///     println!("Database initialized and ready");
///     AgentReply::immediate()
/// });
///
/// // In DatabaseActor - broadcast when ready
/// // broker.broadcast(DatabaseReady).await;
/// ```
///
/// # Message Flow
///
/// 1. DatabaseActor spawns and begins initialization
/// 2. DatabaseActor connects to RocksDB database
/// 3. DatabaseActor executes schema setup
/// 4. DatabaseActor broadcasts DatabaseReady signal
/// 5. Dependent actors can now begin database operations
///
/// # Subscribers
///
/// Any actor that needs to wait for database initialization before
/// starting operations should subscribe to this event.
///
/// # Design Notes
///
/// This is a unit-like struct (no fields) because:
/// - It serves as a pure signal/trigger
/// - Database connection details are internal to DatabaseActor
/// - Subscribers only need to know initialization is complete
/// - Keeps the message lightweight and focused
#[acton_message(raw)]
pub struct DatabaseReady;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that DatabaseReady can be created and cloned.
    #[test]
    fn test_database_ready_creation() {
        let event = DatabaseReady;
        let _cloned = event.clone();
        // If we get here without panicking, Clone is properly implemented
    }

    /// Verify that DatabaseReady implements Debug.
    #[test]
    fn test_database_ready_debug() {
        let event = DatabaseReady;
        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty(), "Debug output should not be empty");
    }

    /// Verify that DatabaseReady is Send + Sync (required for actor message passing).
    #[test]
    fn test_database_ready_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<DatabaseReady>();
        assert_sync::<DatabaseReady>();
    }
}
