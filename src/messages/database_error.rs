//! Message broadcast when database operations fail.

use acton_reactive::prelude::*;

/// Message broadcast when a database operation encounters an error.
///
/// This message is broadcast via the broker when the DatabaseActor encounters
/// any error during initialization, query, or persistence operations. Subscribers
/// (like the Console actor) can listen for these messages to display errors to users.
///
/// # Fields
///
/// * `operation` - Description of the operation that failed
/// * `error` - The error message describing what went wrong
///
/// # Example
///
/// ```no_run
/// use crately::messages::DatabaseError;
///
/// let error_msg = DatabaseError {
///     operation: "schema initialization".to_string(),
///     error: "Failed to create index: ...".to_string(),
/// };
/// // broker.broadcast(error_msg).await;
/// ```
#[acton_message]
pub struct DatabaseError {
    /// Description of the operation that failed
    pub operation: String,
    /// The error message
    pub error: String,
}
