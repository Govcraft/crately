//! Progress event broadcast when documentation chunks are being persisted to the database.

use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::*;

/// Progress event broadcast during documentation chunk persistence.
///
/// This message is broadcast via the broker by CrateCoordinatorActor after receiving
/// each `DocChunkPersisted` event. It provides real-time progress information about
/// chunk persistence, enabling UI actors (like Console) to display accurate progress
/// indicators to users.
///
/// # Actor Communication Pattern
///
/// **Publisher**: CrateCoordinatorActor (after tracking each DocChunkPersisted event)
/// **Subscribers**: Console (for progress display), any future progress tracking actors
///
/// # Design Rationale
///
/// The CrateCoordinatorActor is the single source of truth for persistence progress
/// because it maintains the processing state including both `expected_chunks` and
/// `persisted_chunks` counts. Rather than having downstream actors try to reconstruct
/// this state, the coordinator broadcasts complete progress information after each
/// chunk persistence event.
///
/// # Fields
///
/// * `specifier` - The crate being processed
/// * `chunk_index` - Zero-based index of the chunk just persisted
/// * `persisted_count` - How many chunks have been successfully persisted so far
/// * `total_chunks` - Total expected chunks for this crate's documentation
///
/// # Example
///
/// ```no_run
/// use crately::messages::ChunkPersistenceProgress;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let progress = ChunkPersistenceProgress {
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     chunk_index: 14,
///     persisted_count: 15,
///     total_chunks: 20,
/// };
/// // broker.broadcast(progress).await;
/// ```
///
/// # Message Flow
///
/// 1. DatabaseActor persists a documentation chunk
/// 2. DatabaseActor broadcasts `DocChunkPersisted` with chunk details
/// 3. CrateCoordinatorActor receives `DocChunkPersisted` and updates state
/// 4. CrateCoordinatorActor broadcasts `ChunkPersistenceProgress` with complete progress info
/// 5. Console actor receives progress and displays "Persisting chunk X/Y for crate@version"
///
/// # Progress Display
///
/// Subscribers can display progress as:
/// - "Persisting chunk 15/20 for serde@1.0.0" (using persisted_count/total_chunks)
/// - Progress bar: 75% complete (15/20)
/// - Remaining chunks: 5 chunks left
///
/// # Invariants
///
/// - `persisted_count` ≤ `total_chunks` (coordinator ensures this)
/// - `chunk_index` is zero-based, `persisted_count` starts at 1
/// - When `persisted_count` == `total_chunks`, chunking is complete
/// - Message is broadcast for EVERY chunk persistence, not just completion
#[acton_message]
pub struct ChunkPersistenceProgress {
    /// The crate this progress update is for
    pub specifier: CrateSpecifier,

    /// Zero-based index of the chunk just persisted
    pub chunk_index: usize,

    /// How many chunks have been persisted so far (1-indexed count)
    pub persisted_count: u32,

    /// Total expected chunks for this crate
    pub total_chunks: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Verify that ChunkPersistenceProgress can be created and cloned.
    #[test]
    fn test_chunk_persistence_progress_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let progress = ChunkPersistenceProgress {
            specifier: specifier.clone(),
            chunk_index: 5,
            persisted_count: 6,
            total_chunks: 10,
        };
        let cloned = progress.clone();
        assert_eq!(progress.specifier, cloned.specifier);
        assert_eq!(progress.chunk_index, cloned.chunk_index);
        assert_eq!(progress.persisted_count, cloned.persisted_count);
        assert_eq!(progress.total_chunks, cloned.total_chunks);
    }

    /// Verify that ChunkPersistenceProgress implements Debug.
    #[test]
    fn test_chunk_persistence_progress_debug() {
        let progress = ChunkPersistenceProgress {
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            chunk_index: 0,
            persisted_count: 1,
            total_chunks: 20,
        };
        let debug_str = format!("{:?}", progress);
        assert!(!debug_str.is_empty());
    }

    /// Verify that ChunkPersistenceProgress is Send + Sync (required for actor message passing).
    #[test]
    fn test_chunk_persistence_progress_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<ChunkPersistenceProgress>();
        assert_sync::<ChunkPersistenceProgress>();
    }

    /// Verify that fields contain expected data.
    #[test]
    fn test_chunk_persistence_progress_fields() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let progress = ChunkPersistenceProgress {
            specifier: specifier.clone(),
            chunk_index: 9,
            persisted_count: 10,
            total_chunks: 15,
        };
        assert_eq!(progress.specifier, specifier);
        assert_eq!(progress.chunk_index, 9);
        assert_eq!(progress.persisted_count, 10);
        assert_eq!(progress.total_chunks, 15);
    }

    /// Verify first chunk progress (chunk_index 0, persisted_count 1).
    #[test]
    fn test_chunk_persistence_progress_first_chunk() {
        let progress = ChunkPersistenceProgress {
            specifier: CrateSpecifier::from_str("tracing@0.1.0").unwrap(),
            chunk_index: 0,
            persisted_count: 1,
            total_chunks: 5,
        };
        assert_eq!(progress.chunk_index, 0);
        assert_eq!(progress.persisted_count, 1);
    }

    /// Verify last chunk progress (counts match).
    #[test]
    fn test_chunk_persistence_progress_last_chunk() {
        let progress = ChunkPersistenceProgress {
            specifier: CrateSpecifier::from_str("hyper@1.0.0").unwrap(),
            chunk_index: 19,
            persisted_count: 20,
            total_chunks: 20,
        };
        assert_eq!(progress.chunk_index, 19);
        assert_eq!(progress.persisted_count, 20);
        assert_eq!(progress.total_chunks, 20);
        // Last chunk: persisted_count equals total_chunks
        assert_eq!(progress.persisted_count, progress.total_chunks);
    }

    /// Verify progress invariant: persisted_count ≤ total_chunks.
    #[test]
    fn test_chunk_persistence_progress_invariant() {
        let progress = ChunkPersistenceProgress {
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            chunk_index: 14,
            persisted_count: 15,
            total_chunks: 20,
        };
        // Invariant: persisted never exceeds total
        assert!(progress.persisted_count <= progress.total_chunks);
    }
}
