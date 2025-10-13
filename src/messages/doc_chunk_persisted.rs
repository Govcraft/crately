//! Event broadcast when a documentation chunk is successfully persisted to the database.

use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::*;

/// Event broadcast when a documentation chunk is successfully persisted.
///
/// This message is broadcast via the broker by DatabaseActor after successfully
/// storing a documentation chunk in the database and creating the build->chunk
/// relationship. This enables the CrateCoordinatorActor to track completion of
/// chunk persistence and coordinate proper state transitions.
///
/// # Actor Communication Pattern
///
/// Publisher: DatabaseActor (after successful PersistDocChunk operation)
/// Subscribers: CrateCoordinatorActor (for completion tracking)
///
/// # Fields
///
/// * `specifier` - The crate this chunk belongs to
/// * `chunk_index` - Zero-based index of this chunk within the crate's documentation
/// * `content_hash` - BLAKE3 hash of chunk content (64-char hex string)
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
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     chunk_index: 5,
///     content_hash: "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262".to_string(),
///     source_file: "src/lib.rs".to_string(),
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. DatabaseActor receives PersistDocChunk message
/// 2. DatabaseActor computes BLAKE3 hash and broadcasts ChunkHashComputed
/// 3. DatabaseActor upserts doc_chunk keyed by content_hash
/// 4. DatabaseActor creates build->chunk relationship with context on edge
/// 5. DatabaseActor broadcasts DocChunkPersisted with content hash
/// 6. CrateCoordinatorActor updates chunk persistence state
///
/// # Subscribers
///
/// - **CrateCoordinatorActor**: Tracks chunk persistence completion
#[acton_message]
pub struct DocChunkPersisted {
    /// The crate this chunk belongs to
    pub specifier: CrateSpecifier,

    /// Zero-based index of this chunk within the crate's documentation
    pub chunk_index: usize,

    /// BLAKE3 content hash (64-character hex string)
    pub content_hash: String,

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
        let content_hash = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262".to_string();
        let event = DocChunkPersisted {
            specifier: specifier.clone(),
            chunk_index: 3,
            content_hash: content_hash.clone(),
            source_file: "src/runtime/mod.rs".to_string(),
        };
        let cloned = event.clone();
        assert_eq!(event.specifier, cloned.specifier);
        assert_eq!(event.chunk_index, cloned.chunk_index);
        assert_eq!(event.content_hash, cloned.content_hash);
        assert_eq!(event.source_file, cloned.source_file);
    }

    /// Verify that DocChunkPersisted implements Debug.
    #[test]
    fn test_doc_chunk_persisted_debug() {
        let event = DocChunkPersisted {
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            chunk_index: 0,
            content_hash: "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24".to_string(),
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
        let content_hash = "abc123def456789abc123def456789abc123def456789abc123def456789abc1".to_string();
        let event = DocChunkPersisted {
            specifier: specifier.clone(),
            chunk_index: 10,
            content_hash: content_hash.clone(),
            source_file: "src/routing/mod.rs".to_string(),
        };
        assert_eq!(event.specifier, specifier);
        assert_eq!(event.chunk_index, 10);
        assert_eq!(event.content_hash, content_hash);
        assert_eq!(event.source_file, "src/routing/mod.rs");
    }

    /// Verify chunk_index starts at zero.
    #[test]
    fn test_doc_chunk_persisted_first_chunk() {
        let event = DocChunkPersisted {
            specifier: CrateSpecifier::from_str("tracing@0.1.0").unwrap(),
            chunk_index: 0,
            content_hash: "a".repeat(64),
            source_file: "src/lib.rs".to_string(),
        };
        assert_eq!(event.chunk_index, 0);
    }
}
