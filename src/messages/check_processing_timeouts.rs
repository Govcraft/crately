//! CheckProcessingTimeouts message type
//!
//! Internal message sent periodically by the CrateCoordinatorActor's timeout
//! detection background task to trigger timeout checks for stuck processing states.

use acton_reactive::prelude::*;

/// Internal message to trigger processing timeout checks
///
/// This message is sent periodically by the CrateCoordinatorActor's background
/// task to check for crates that have been stuck in a non-terminal state for
/// longer than the configured timeout threshold.
#[acton_message]
pub struct CheckProcessingTimeouts;
