//! ChunkDeduplicated - Event broadcast when duplicate content is detected
//!
//! This message is broadcast when the deduplication system detects that a chunk's
//! content already exists in the database, allowing downstream actors to skip
//! redundant persistence operations.

use crate::types::{BuildId, ContentHash};
use acton_reactive::prelude::*;

/// Event broadcast when a documentation chunk is identified as duplicate content
///
/// This message indicates that content with the same hash already exists in the
/// database, eliminating the need for redundant storage. The system can create
/// a reference edge from the build to the existing content node instead of
/// creating a new content node.
///
/// # Deduplication Benefits
///
/// - **Storage efficiency**: Identical content stored once, referenced multiple times
/// - **Processing efficiency**: Skip redundant vectorization for duplicate content
/// - **Query efficiency**: Fewer unique content nodes improve graph traversal performance
///
/// # Fields
///
/// * `build_id` - Build attempting to store this content
/// * `chunk_id` - Chunk identifier within the build
/// * `content_hash` - Hash matching existing content
/// * `existing_content_id` - Database record ID of existing content node
///
/// # Example
///
/// ```no_run
/// use crately::messages::ChunkDeduplicated;
/// use crately::types::{BuildId, ContentHash};
///
/// let event = ChunkDeduplicated {
///     build_id: BuildId::new(),
///     chunk_id: "chunk_005".to_string(),
///     content_hash: ContentHash::from_content(b"duplicate content"),
///     existing_content_id: "content:af1349b9".to_string(),
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. DatabaseActor receives ChunkHashComputed
/// 2. DatabaseActor queries: `SELECT id FROM content WHERE hash = $hash`
/// 3. If found, broadcast ChunkDeduplicated with existing_content_id
/// 4. Create edge: `build -> existing_content_id` (skip content creation)
/// 5. MetricsActor increments deduplication counter
/// 6. Skip PersistDocChunk and vectorization for this chunk
///
/// # Subscribers
///
/// - **CoordinatorActor**: Updates build progress, skips chunk persistence
/// - **MetricsActor**: Tracks deduplication rates and storage savings
/// - **Console**: Displays deduplication notification
#[acton_message]
pub struct ChunkDeduplicated {
    /// Build that attempted to store this content
    pub build_id: BuildId,

    /// Chunk identifier within the build
    pub chunk_id: String,

    /// Content hash that matched existing content
    pub content_hash: ContentHash,

    /// Database record ID of the existing content node
    /// Format: "content:<hash>" or SurrealDB Thing ID
    pub existing_content_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_deduplicated_new() {
        let build_id = BuildId::new();
        let content_hash = ContentHash::from_content(b"duplicate content");

        let event = ChunkDeduplicated {
            build_id: build_id.clone(),
            chunk_id: "chunk_005".to_string(),
            content_hash: content_hash.clone(),
            existing_content_id: "content:af1349b9".to_string(),
        };

        assert_eq!(event.build_id, build_id);
        assert_eq!(event.chunk_id, "chunk_005");
        assert_eq!(event.content_hash, content_hash);
        assert_eq!(event.existing_content_id, "content:af1349b9");
    }

    #[test]
    fn test_chunk_deduplicated_various_content_ids() {
        let build_id = BuildId::new();
        let content_hash = ContentHash::from_content(b"test");

        let id_formats = vec![
            "content:abc123",
            "content:af1349b9f5f9a1a6",
            "doc_chunk:12345",
        ];

        for content_id in id_formats {
            let event = ChunkDeduplicated {
                build_id: build_id.clone(),
                chunk_id: "chunk_000".to_string(),
                content_hash: content_hash.clone(),
                existing_content_id: content_id.to_string(),
            };
            assert_eq!(event.existing_content_id, content_id);
        }
    }

    #[test]
    fn test_chunk_deduplicated_different_chunks_same_content() {
        let build_id = BuildId::new();
        let content = b"common documentation pattern";
        let content_hash = ContentHash::from_content(content);

        let chunk1 = ChunkDeduplicated {
            build_id: build_id.clone(),
            chunk_id: "chunk_000".to_string(),
            content_hash: content_hash.clone(),
            existing_content_id: "content:xyz789".to_string(),
        };

        let chunk2 = ChunkDeduplicated {
            build_id: build_id.clone(),
            chunk_id: "chunk_042".to_string(),
            content_hash: content_hash.clone(),
            existing_content_id: "content:xyz789".to_string(),
        };

        assert_eq!(chunk1.content_hash, chunk2.content_hash);
        assert_eq!(chunk1.existing_content_id, chunk2.existing_content_id);
        assert_ne!(chunk1.chunk_id, chunk2.chunk_id);
    }

    #[test]
    fn test_chunk_deduplicated_cross_build_deduplication() {
        let build_id1 = BuildId::new();
        let build_id2 = BuildId::new();
        let content_hash = ContentHash::from_content(b"shared documentation");

        let event1 = ChunkDeduplicated {
            build_id: build_id1.clone(),
            chunk_id: "chunk_001".to_string(),
            content_hash: content_hash.clone(),
            existing_content_id: "content:shared123".to_string(),
        };

        let event2 = ChunkDeduplicated {
            build_id: build_id2.clone(),
            chunk_id: "chunk_003".to_string(),
            content_hash: content_hash.clone(),
            existing_content_id: "content:shared123".to_string(),
        };

        assert_ne!(event1.build_id, event2.build_id);
        assert_eq!(event1.content_hash, event2.content_hash);
        assert_eq!(event1.existing_content_id, event2.existing_content_id);
    }

    #[test]
    fn test_chunk_deduplicated_clone() {
        let original = ChunkDeduplicated {
            build_id: BuildId::new(),
            chunk_id: "chunk_010".to_string(),
            content_hash: ContentHash::from_content(b"clone test"),
            existing_content_id: "content:clone123".to_string(),
        };

        let cloned = original.clone();
        assert_eq!(original.build_id, cloned.build_id);
        assert_eq!(original.chunk_id, cloned.chunk_id);
        assert_eq!(original.content_hash, cloned.content_hash);
        assert_eq!(original.existing_content_id, cloned.existing_content_id);
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
            build_id: BuildId::new(),
            chunk_id: "chunk_debug".to_string(),
            content_hash: ContentHash::from_content(b"debug"),
            existing_content_id: "content:debug456".to_string(),
        };

        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("ChunkDeduplicated"));
    }

    #[test]
    fn test_chunk_deduplicated_empty_chunk_id() {
        let event = ChunkDeduplicated {
            build_id: BuildId::new(),
            chunk_id: String::new(),
            content_hash: ContentHash::from_content(b"test"),
            existing_content_id: "content:empty".to_string(),
        };

        assert!(event.chunk_id.is_empty());
    }

    #[test]
    fn test_chunk_deduplicated_hash_uniqueness() {
        let build_id = BuildId::new();

        let event1 = ChunkDeduplicated {
            build_id: build_id.clone(),
            chunk_id: "chunk_001".to_string(),
            content_hash: ContentHash::from_content(b"content A"),
            existing_content_id: "content:aaa".to_string(),
        };

        let event2 = ChunkDeduplicated {
            build_id: build_id.clone(),
            chunk_id: "chunk_002".to_string(),
            content_hash: ContentHash::from_content(b"content B"),
            existing_content_id: "content:bbb".to_string(),
        };

        assert_ne!(event1.content_hash, event2.content_hash);
        assert_ne!(event1.existing_content_id, event2.existing_content_id);
    }
}
