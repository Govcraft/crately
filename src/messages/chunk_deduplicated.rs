//! ChunkDeduplicated - Event broadcast when duplicate content is detected
//!
//! This message is broadcast when the deduplication system detects that a chunk's
//! content already exists in the database, allowing downstream actors to track
//! storage efficiency metrics.

use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::*;

/// Event broadcast when a documentation chunk is identified as duplicate content
///
/// This message indicates that content with the same hash already exists in the
/// database. The system reuses the existing content node by creating a reference
/// edge from the build to the existing content, rather than creating a duplicate
/// content node.
///
/// # Deduplication Benefits
///
/// - **Storage efficiency**: Identical content stored once, referenced multiple times
/// - **Processing efficiency**: Skip redundant vectorization for duplicate content
/// - **Query efficiency**: Fewer unique content nodes improve graph traversal performance
///
/// # Fields
///
/// * `specifier` - Crate attempting to store this content
/// * `content_hash` - Hash matching existing content (64-char BLAKE3 hex)
/// * `reuse_count` - Number of builds that reference this content
///
/// # Example
///
/// ```no_run
/// use crately::messages::ChunkDeduplicated;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let event = ChunkDeduplicated {
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     content_hash: "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262".to_string(),
///     reuse_count: 3,
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. DatabaseActor receives PersistDocChunk
/// 2. Computes BLAKE3 hash and broadcasts ChunkHashComputed
/// 3. Attempts UPDATE on doc_chunk table by content_hash
/// 4. If rows returned, chunk existed - broadcast ChunkDeduplicated
/// 5. Create edge: `build -> existing_chunk` (skip content creation)
/// 6. MetricsActor increments deduplication counter
///
/// # Subscribers
///
/// - **MetricsActor**: Tracks deduplication rates and storage savings
/// - **Console**: Displays deduplication notification
#[acton_message]
pub struct ChunkDeduplicated {
    /// Crate that attempted to store this content
    pub specifier: CrateSpecifier,

    /// Content hash that matched existing content (64-char BLAKE3 hex)
    pub content_hash: String,

    /// Number of builds that reference this content
    pub reuse_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_chunk_deduplicated_new() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let content_hash = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262".to_string();

        let event = ChunkDeduplicated {
            specifier: specifier.clone(),
            content_hash: content_hash.clone(),
            reuse_count: 2,
        };

        assert_eq!(event.specifier, specifier);
        assert_eq!(event.content_hash, content_hash);
        assert_eq!(event.reuse_count, 2);
    }

    #[test]
    fn test_chunk_deduplicated_hash_length() {
        let event = ChunkDeduplicated {
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            content_hash: "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24".to_string(),
            reuse_count: 5,
        };

        assert_eq!(event.content_hash.len(), 64, "BLAKE3 hash should be 64 hex characters");
    }

    #[test]
    fn test_chunk_deduplicated_reuse_count() {
        let spec = CrateSpecifier::from_str("axum@0.7.0").unwrap();

        let event1 = ChunkDeduplicated {
            specifier: spec.clone(),
            content_hash: "a".repeat(64),
            reuse_count: 1,
        };

        let event2 = ChunkDeduplicated {
            specifier: spec.clone(),
            content_hash: "a".repeat(64),
            reuse_count: 10,
        };

        assert_eq!(event1.reuse_count, 1);
        assert_eq!(event2.reuse_count, 10);
        assert_eq!(event1.content_hash, event2.content_hash);
    }

    #[test]
    fn test_chunk_deduplicated_cross_crate_deduplication() {
        let hash = "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24".to_string();

        let event1 = ChunkDeduplicated {
            specifier: CrateSpecifier::from_str("crate_a@1.0.0").unwrap(),
            content_hash: hash.clone(),
            reuse_count: 2,
        };

        let event2 = ChunkDeduplicated {
            specifier: CrateSpecifier::from_str("crate_b@2.0.0").unwrap(),
            content_hash: hash.clone(),
            reuse_count: 3,
        };

        // Different crates can deduplicate against same content
        assert_ne!(event1.specifier, event2.specifier);
        assert_eq!(event1.content_hash, event2.content_hash);
    }

    #[test]
    fn test_chunk_deduplicated_clone() {
        let original = ChunkDeduplicated {
            specifier: CrateSpecifier::from_str("tracing@0.1.0").unwrap(),
            content_hash: "b".repeat(64),
            reuse_count: 7,
        };

        let cloned = original.clone();
        assert_eq!(original.specifier, cloned.specifier);
        assert_eq!(original.content_hash, cloned.content_hash);
        assert_eq!(original.reuse_count, cloned.reuse_count);
    }

    #[test]
    fn test_chunk_deduplicated_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ChunkDeduplicated>();
        assert_sync::<ChunkDeduplicated>();
    }

    #[test]
    fn test_chunk_deduplicated_debug() {
        let event = ChunkDeduplicated {
            specifier: CrateSpecifier::from_str("serde_json@1.0.0").unwrap(),
            content_hash: "c".repeat(64),
            reuse_count: 4,
        };

        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("ChunkDeduplicated"));
    }

    #[test]
    fn test_chunk_deduplicated_zero_reuse_count() {
        // Edge case: first time seeing this content (reuse_count starts at 1 typically)
        let event = ChunkDeduplicated {
            specifier: CrateSpecifier::from_str("test@1.0.0").unwrap(),
            content_hash: "d".repeat(64),
            reuse_count: 1,
        };

        assert_eq!(event.reuse_count, 1);
    }

    #[test]
    fn test_chunk_deduplicated_high_reuse_count() {
        // Test with high reuse count (popular documentation pattern)
        let event = ChunkDeduplicated {
            specifier: CrateSpecifier::from_str("common@1.0.0").unwrap(),
            content_hash: "e".repeat(64),
            reuse_count: 1000,
        };

        assert_eq!(event.reuse_count, 1000);
    }
}
