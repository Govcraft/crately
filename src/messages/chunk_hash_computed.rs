//! ChunkHashComputed - Event broadcast when documentation chunk content is hashed
//!
//! This message is broadcast after a documentation chunk's content hash is computed,
//! enabling deduplication detection before persistence.

use crate::types::{BuildId, ContentHash};
use acton_reactive::prelude::*;

/// Event broadcast when a documentation chunk's content hash is computed
///
/// This message enables the deduplication system to detect when content has been
/// seen before. Multiple actors can subscribe to perform different actions:
/// - DatabaseActor checks if content already exists
/// - MetricsActor tracks deduplication rates
/// - Console displays progress updates
///
/// # Deduplication Flow
///
/// 1. Content is hashed using BLAKE3
/// 2. ChunkHashComputed is broadcast with hash and metadata
/// 3. DatabaseActor queries for existing content with this hash
/// 4. If found, ChunkDeduplicated is broadcast (skip persistence)
/// 5. If not found, proceed with full persistence
///
/// # Fields
///
/// * `build_id` - Build this chunk belongs to
/// * `chunk_id` - Unique identifier for this chunk within the build
/// * `content_hash` - BLAKE3 hash of chunk content (for deduplication)
/// * `content_length` - Byte length of original content
/// * `source_file` - File path where content was extracted
///
/// # Example
///
/// ```no_run
/// use crately::messages::ChunkHashComputed;
/// use crately::types::{BuildId, ContentHash};
///
/// let event = ChunkHashComputed {
///     build_id: BuildId::new(),
///     chunk_id: "chunk_000".to_string(),
///     content_hash: ContentHash::from_content(b"documentation content"),
///     content_length: 21,
///     source_file: "src/lib.rs".to_string(),
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. DocumentationChunker processes documentation
/// 2. For each chunk, compute BLAKE3 hash
/// 3. Broadcast ChunkHashComputed with hash and metadata
/// 4. DatabaseActor checks if content_hash exists in database
/// 5. If duplicate found, broadcast ChunkDeduplicated
/// 6. If unique, proceed with PersistDocChunk
///
/// # Subscribers
///
/// - **DatabaseActor**: Queries for existing content by hash
/// - **DeduplicationActor**: Tracks deduplication statistics
/// - **Console**: Displays chunk processing progress
#[acton_message]
pub struct ChunkHashComputed {
    /// Build this chunk belongs to
    pub build_id: BuildId,

    /// Unique identifier for this chunk within the build
    pub chunk_id: String,

    /// BLAKE3 content hash for deduplication
    pub content_hash: ContentHash,

    /// Length of content in bytes
    pub content_length: usize,

    /// Source file path where chunk was extracted
    pub source_file: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_hash_computed_new() {
        let build_id = BuildId::new();
        let content = b"test documentation content";
        let content_hash = ContentHash::from_content(content);

        let event = ChunkHashComputed {
            build_id: build_id.clone(),
            chunk_id: "chunk_000".to_string(),
            content_hash: content_hash.clone(),
            content_length: content.len(),
            source_file: "src/lib.rs".to_string(),
        };

        assert_eq!(event.build_id, build_id);
        assert_eq!(event.chunk_id, "chunk_000");
        assert_eq!(event.content_hash, content_hash);
        assert_eq!(event.content_length, 26);
        assert_eq!(event.source_file, "src/lib.rs");
    }

    #[test]
    fn test_chunk_hash_computed_deterministic() {
        let content = b"deterministic content";

        let hash1 = ContentHash::from_content(content);
        let hash2 = ContentHash::from_content(content);

        assert_eq!(hash1, hash2, "Same content should produce same hash");
    }

    #[test]
    fn test_chunk_hash_computed_different_content() {
        let content1 = b"content A";
        let content2 = b"content B";

        let hash1 = ContentHash::from_content(content1);
        let hash2 = ContentHash::from_content(content2);

        assert_ne!(hash1, hash2, "Different content should produce different hash");
    }

    #[test]
    fn test_chunk_hash_computed_empty_content() {
        let event = ChunkHashComputed {
            build_id: BuildId::new(),
            chunk_id: "chunk_empty".to_string(),
            content_hash: ContentHash::from_content(b""),
            content_length: 0,
            source_file: "empty.rs".to_string(),
        };

        assert_eq!(event.content_length, 0);
    }

    #[test]
    fn test_chunk_hash_computed_large_content() {
        let large_content = vec![b'X'; 1_000_000]; // 1MB
        let event = ChunkHashComputed {
            build_id: BuildId::new(),
            chunk_id: "chunk_large".to_string(),
            content_hash: ContentHash::from_content(&large_content),
            content_length: large_content.len(),
            source_file: "large.rs".to_string(),
        };

        assert_eq!(event.content_length, 1_000_000);
    }

    #[test]
    fn test_chunk_hash_computed_various_files() {
        let files = vec![
            "src/lib.rs",
            "src/main.rs",
            "examples/example.rs",
            "README.md",
            "Cargo.toml",
        ];

        for file in files {
            let event = ChunkHashComputed {
                build_id: BuildId::new(),
                chunk_id: "chunk_000".to_string(),
                content_hash: ContentHash::from_content(b"test"),
                content_length: 4,
                source_file: file.to_string(),
            };
            assert_eq!(event.source_file, file);
        }
    }

    #[test]
    fn test_chunk_hash_computed_clone() {
        let original = ChunkHashComputed {
            build_id: BuildId::new(),
            chunk_id: "chunk_001".to_string(),
            content_hash: ContentHash::from_content(b"clone test"),
            content_length: 10,
            source_file: "test.rs".to_string(),
        };

        let cloned = original.clone();
        assert_eq!(original.build_id, cloned.build_id);
        assert_eq!(original.chunk_id, cloned.chunk_id);
        assert_eq!(original.content_hash, cloned.content_hash);
        assert_eq!(original.content_length, cloned.content_length);
        assert_eq!(original.source_file, cloned.source_file);
    }

    #[test]
    fn test_chunk_hash_computed_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ChunkHashComputed>();
        assert_sync::<ChunkHashComputed>();
    }

    #[test]
    fn test_chunk_hash_computed_debug() {
        let event = ChunkHashComputed {
            build_id: BuildId::new(),
            chunk_id: "chunk_debug".to_string(),
            content_hash: ContentHash::from_content(b"debug"),
            content_length: 5,
            source_file: "debug.rs".to_string(),
        };

        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("ChunkHashComputed"));
    }

    #[test]
    fn test_chunk_hash_computed_chunk_id_formats() {
        let formats = vec![
            "chunk_000",
            "chunk_999",
            "build_abc123_chunk_0",
            "serde_1.0.0_chunk_005",
        ];

        for chunk_id in formats {
            let event = ChunkHashComputed {
                build_id: BuildId::new(),
                chunk_id: chunk_id.to_string(),
                content_hash: ContentHash::from_content(b"test"),
                content_length: 4,
                source_file: "test.rs".to_string(),
            };
            assert_eq!(event.chunk_id, chunk_id);
        }
    }

    #[test]
    fn test_chunk_hash_computed_multiple_chunks_same_build() {
        let build_id = BuildId::new();

        let chunk1 = ChunkHashComputed {
            build_id: build_id.clone(),
            chunk_id: "chunk_000".to_string(),
            content_hash: ContentHash::from_content(b"content 1"),
            content_length: 9,
            source_file: "src/lib.rs".to_string(),
        };

        let chunk2 = ChunkHashComputed {
            build_id: build_id.clone(),
            chunk_id: "chunk_001".to_string(),
            content_hash: ContentHash::from_content(b"content 2"),
            content_length: 9,
            source_file: "src/lib.rs".to_string(),
        };

        assert_eq!(chunk1.build_id, chunk2.build_id);
        assert_ne!(chunk1.chunk_id, chunk2.chunk_id);
        assert_ne!(chunk1.content_hash, chunk2.content_hash);
    }
}
