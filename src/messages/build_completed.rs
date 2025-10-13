use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Broadcast when content graph build completes
///
/// This event provides final statistics about the deduplication process,
/// including storage savings and unique content counts.
#[acton_message(raw)]
#[derive(Clone)]
pub struct BuildCompleted {
    /// The crate that was analyzed
    pub specifier: CrateSpecifier,
    /// Build identifier
    pub build_id: String,
    /// Total chunks analyzed
    pub total_chunks: u32,
    /// Unique chunks after deduplication
    pub unique_chunks: u32,
    /// Number of duplicates found
    pub duplicates_found: u32,
    /// Storage reduction percentage (0.0-100.0)
    pub storage_savings_percent: f64,
    /// Build duration in milliseconds
    pub build_duration_ms: u64,
}
