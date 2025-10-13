//! Event broadcast when a documentation chunk is successfully persisted to the database.

use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::*;

/// Event broadcast when a documentation chunk is successfully persisted.
///
/// This message is broadcast via the broker by DatabaseActor after successfully
/// storing a documentation chunk in the database. This enables the CrateCoordinatorActor
/// to track completion of chunk persistence and coordinate proper state transitions.
///
/// # Actor Communication Pattern
///
/// Publisher: DatabaseActor (after successful PersistDocChunk operation)
/// Subscribers: CrateCoordinatorActor (for completion tracking)
///
/// # Fields
///
/// * `chunk_id` - Unique identifier for the persisted chunk
/// * `specifier` - The crate this chunk belongs to
/// * `chunk_index` - Zero-based index of this chunk within the crate's documentation
/// * `source_file` - Source file path this chunk was extracted from
///
/// # Example
///
/// ```no_run
/// use crately::messages::DocChunkPersisted;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let event = DocChunkPersisted {
///     chunk_id: "serde_1.0.0_chunk_005".to_string(),
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     chunk_index: 5,
///     source_file: "src/lib.rs".to_string(),
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. DatabaseActor receives PersistDocChunk message
/// 2. DatabaseActor stores chunk in SurrealDB
/// 3. DatabaseActor broadcasts DocChunkPersisted with chunk details
/// 4. CrateCoordinatorActor updates chunk persistence state
///
/// # Subscribers
///
/// - **CrateCoordinatorActor**: Tracks chunk persistence completion
#[acton_message]
pub struct DocChunkPersisted {
    /// Unique identifier for the persisted chunk
    pub chunk_id: String,

    /// The crate this chunk belongs to
    pub specifier: CrateSpecifier,

    /// Zero-based index of this chunk within the crate's documentation
    pub chunk_index: usize,

    /// Source file path this chunk was extracted from
    pub source_file: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Verify that DocChunkPersisted can be created and cloned.
    #[test]
    fn test_doc_chunk_persisted_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let event = DocChunkPersisted {
            chunk_id: "tokio_1.35.0_chunk_003".to_string(),
            specifier: specifier.clone(),
            chunk_index: 3,
            source_file: "src/runtime/mod.rs".to_string(),
        };
        let cloned = event.clone();
        assert_eq!(event.chunk_id, cloned.chunk_id);
        assert_eq!(event.specifier, cloned.specifier);
        assert_eq!(event.chunk_index, cloned.chunk_index);
        assert_eq!(event.source_file, cloned.source_file);
    }

    /// Verify that DocChunkPersisted implements Debug.
    #[test]
    fn test_doc_chunk_persisted_debug() {
        let event = DocChunkPersisted {
            chunk_id: "serde_1.0.0_chunk_000".to_string(),
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            chunk_index: 0,
            source_file: "src/lib.rs".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
    }

    /// Verify that DocChunkPersisted is Send + Sync (required for actor message passing).
    #[test]
    fn test_doc_chunk_persisted_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<DocChunkPersisted>();
        assert_sync::<DocChunkPersisted>();
    }

    /// Verify that DocChunkPersisted fields contain expected data.
    #[test]
    fn test_doc_chunk_persisted_fields() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let event = DocChunkPersisted {
            chunk_id: "axum_0.7.0_chunk_010".to_string(),
            specifier: specifier.clone(),
            chunk_index: 10,
            source_file: "src/routing/mod.rs".to_string(),
        };
        assert_eq!(event.chunk_id, "axum_0.7.0_chunk_010");
        assert_eq!(event.specifier, specifier);
        assert_eq!(event.chunk_index, 10);
        assert_eq!(event.source_file, "src/routing/mod.rs");
    }

    /// Verify chunk_index starts at zero.
    #[test]
    fn test_doc_chunk_persisted_first_chunk() {
        let event = DocChunkPersisted {
            chunk_id: "tracing_0.1.0_chunk_000".to_string(),
            specifier: CrateSpecifier::from_str("tracing@0.1.0").unwrap(),
            chunk_index: 0,
            source_file: "src/lib.rs".to_string(),
        };
        assert_eq!(event.chunk_index, 0);
    }
}
