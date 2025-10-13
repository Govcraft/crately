use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Broadcast when content graph build fails
///
/// This event signals that the deduplication process encountered an error
/// and could not complete successfully.
#[acton_message(raw)]
#[derive(Clone)]
pub struct BuildFailed {
    /// The crate that failed
    pub specifier: CrateSpecifier,
    /// Build identifier
    pub build_id: String,
    /// Error message describing the failure
    pub error: String,
    /// Number of chunks successfully processed before failure
    pub chunks_processed: u32,
}
