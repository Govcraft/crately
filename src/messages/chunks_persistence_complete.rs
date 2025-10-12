//! Event broadcast when all documentation chunks for a crate have been successfully persisted.

use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::*;

/// Event broadcast when all documentation chunks for a crate have been persisted to the database.
///
/// This message is broadcast via the broker by CrateCoordinatorActor after tracking that all
/// expected chunks have been persisted (via DocChunkPersisted events). This enables the
/// DatabaseActor to safely update the crate status to "chunked" knowing that all data is
/// already in the database.
///
/// # Actor Communication Pattern
///
/// Publisher: CrateCoordinatorActor (after tracking all DocChunkPersisted events)
/// Subscribers: DatabaseActor (for status update after confirmed persistence)
///
/// # Purpose
///
/// This message solves a race condition where DatabaseActor previously attempted to validate
/// chunk existence immediately upon receiving DocumentationChunked, before persistence was
/// complete. By waiting for this confirmation message, we ensure status transitions happen
/// only after data is safely stored.
///
/// # Fields
///
/// * `specifier` - The crate for which all chunks have been persisted
/// * `chunk_count` - Total number of chunks that were persisted
///
/// # Example
///
/// ```no_run
/// use crately::messages::ChunksPersistenceComplete;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let event = ChunksPersistenceComplete {
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     chunk_count: 18,
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. ProcessorActor broadcasts DocumentationChunked + multiple PersistDocChunk messages
/// 2. DatabaseActor processes each PersistDocChunk → broadcasts DocChunkPersisted
/// 3. CrateCoordinatorActor tracks DocChunkPersisted events
/// 4. When all expected chunks are persisted, CrateCoordinatorActor broadcasts ChunksPersistenceComplete
/// 5. DatabaseActor receives this message and updates status to "chunked"
///
/// # Subscribers
///
/// - **DatabaseActor**: Updates crate status to "chunked" after receiving this confirmation
#[acton_message]
pub struct ChunksPersistenceComplete {
    /// The crate for which all chunks have been persisted
    pub specifier: CrateSpecifier,

    /// Total number of chunks that were persisted
    pub chunk_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Verify that ChunksPersistenceComplete can be created and cloned.
    #[test]
    fn test_chunks_persistence_complete_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let event = ChunksPersistenceComplete {
            specifier: specifier.clone(),
            chunk_count: 25,
        };
        let cloned = event.clone();
        assert_eq!(event.specifier, cloned.specifier);
        assert_eq!(event.chunk_count, cloned.chunk_count);
    }

    /// Verify that ChunksPersistenceComplete implements Debug.
    #[test]
    fn test_chunks_persistence_complete_debug() {
        let event = ChunksPersistenceComplete {
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            chunk_count: 18,
        };
        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
    }

    /// Verify that ChunksPersistenceComplete is Send + Sync (required for actor message passing).
    #[test]
    fn test_chunks_persistence_complete_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<ChunksPersistenceComplete>();
        assert_sync::<ChunksPersistenceComplete>();
    }

    /// Verify that ChunksPersistenceComplete fields contain expected data.
    #[test]
    fn test_chunks_persistence_complete_fields() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let event = ChunksPersistenceComplete {
            specifier: specifier.clone(),
            chunk_count: 42,
        };
        assert_eq!(event.specifier, specifier);
        assert_eq!(event.chunk_count, 42);
    }

    /// Verify zero chunks is valid (edge case for crates with no documentation).
    #[test]
    fn test_chunks_persistence_complete_zero_chunks() {
        let event = ChunksPersistenceComplete {
            specifier: CrateSpecifier::from_str("empty-crate@0.1.0").unwrap(),
            chunk_count: 0,
        };
        assert_eq!(event.chunk_count, 0);
    }

    /// Verify large chunk counts are supported.
    #[test]
    fn test_chunks_persistence_complete_large_count() {
        let event = ChunksPersistenceComplete {
            specifier: CrateSpecifier::from_str("massive-crate@1.0.0").unwrap(),
            chunk_count: 10_000,
        };
        assert_eq!(event.chunk_count, 10_000);
    }
}
