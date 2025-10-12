//! Message broadcast when database validation warnings occur.

use acton_reactive::prelude::*;

/// Message broadcast when a database validation operation produces a warning.
///
/// This message is broadcast via the broker when the DatabaseActor encounters
/// non-fatal validation issues that don't prevent continued operation. These
/// warnings are logged at WARN level and should be displayed differently from
/// errors in the UI.
///
/// # Fields
///
/// * `operation` - Description of the operation that produced the warning
/// * `warning` - The warning message describing the validation issue
///
/// # Example
///
/// ```no_run
/// use crately::messages::DatabaseWarning;
///
/// let warning_msg = DatabaseWarning {
///     operation: "validate chunking for serde@1.0.0".to_string(),
///     warning: "No doc_chunks found in database (expected 18)".to_string(),
/// };
/// // broker.broadcast(warning_msg).await;
/// ```
///
/// # See Also
///
/// * [`DatabaseError`](crate::messages::DatabaseError) - For fatal database errors
#[acton_message]
pub struct DatabaseWarning {
    /// Description of the operation that produced the warning
    pub operation: String,
    /// The warning message
    pub warning: String,
}
