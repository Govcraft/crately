//! ChunkHashComputed - Event broadcast when documentation chunk content is hashed
//!
//! This message is broadcast after a documentation chunk's content hash is computed,
//! enabling deduplication detection before persistence.

use crate::crate_specifier::CrateSpecifier;
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
/// * `specifier` - Crate this chunk belongs to
/// * `chunk_index` - Zero-based index of this chunk
/// * `content_hash` - BLAKE3 hash as 64-character hex string
/// * `source_file` - File path where content was extracted
///
/// # Example
///
/// ```no_run
/// use crately::messages::ChunkHashComputed;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let event = ChunkHashComputed {
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     chunk_index: 0,
///     content_hash: "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262".to_string(),
///     source_file: "src/lib.rs".to_string(),
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. PersistDocChunk handler computes BLAKE3 hash
/// 2. Broadcast ChunkHashComputed with hash and metadata
/// 3. DatabaseActor checks if content_hash exists in database
/// 4. If duplicate found, broadcast ChunkDeduplicated
/// 5. If unique, proceed with doc_chunk insertion
///
/// # Subscribers
///
/// - **DatabaseActor**: Queries for existing content by hash (implicit - part of PersistDocChunk handler)
/// - **MetricsActor**: Tracks hash computation performance
/// - **Console**: Displays chunk processing progress
#[acton_message]
pub struct ChunkHashComputed {
    /// Crate this chunk belongs to
    pub specifier: CrateSpecifier,

    /// Zero-based index of this chunk within the crate's documentation
    pub chunk_index: usize,

    /// BLAKE3 content hash (64-character hex string)
    pub content_hash: String,

    /// Source file path where chunk was extracted
    pub source_file: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_chunk_hash_computed_new() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let content_hash = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262".to_string();

        let event = ChunkHashComputed {
            specifier: specifier.clone(),
            chunk_index: 0,
            content_hash: content_hash.clone(),
            source_file: "src/lib.rs".to_string(),
        };

        assert_eq!(event.specifier, specifier);
        assert_eq!(event.chunk_index, 0);
        assert_eq!(event.content_hash, content_hash);
        assert_eq!(event.source_file, "src/lib.rs");
    }

    #[test]
    fn test_chunk_hash_computed_hash_length() {
        let event = ChunkHashComputed {
            specifier: CrateSpecifier::from_str("tokio@1.35.0").unwrap(),
            chunk_index: 5,
            content_hash: "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24".to_string(),
            source_file: "src/runtime.rs".to_string(),
        };

        assert_eq!(event.content_hash.len(), 64, "BLAKE3 hash should be 64 hex characters");
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
                specifier: CrateSpecifier::from_str("test@1.0.0").unwrap(),
                chunk_index: 0,
                content_hash: "a".repeat(64),
                source_file: file.to_string(),
            };
            assert_eq!(event.source_file, file);
        }
    }

    #[test]
    fn test_chunk_hash_computed_clone() {
        let original = ChunkHashComputed {
            specifier: CrateSpecifier::from_str("axum@0.7.0").unwrap(),
            chunk_index: 1,
            content_hash: "b".repeat(64),
            source_file: "test.rs".to_string(),
        };

        let cloned = original.clone();
        assert_eq!(original.specifier, cloned.specifier);
        assert_eq!(original.chunk_index, cloned.chunk_index);
        assert_eq!(original.content_hash, cloned.content_hash);
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
            specifier: CrateSpecifier::from_str("tracing@0.1.0").unwrap(),
            chunk_index: 10,
            content_hash: "c".repeat(64),
            source_file: "debug.rs".to_string(),
        };

        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("ChunkHashComputed"));
    }

    #[test]
    fn test_chunk_hash_computed_multiple_chunks_same_crate() {
        let specifier = CrateSpecifier::from_str("serde_json@1.0.0").unwrap();

        let chunk1 = ChunkHashComputed {
            specifier: specifier.clone(),
            chunk_index: 0,
            content_hash: "aaa".repeat(21) + "a", // 64 chars
            source_file: "src/lib.rs".to_string(),
        };

        let chunk2 = ChunkHashComputed {
            specifier: specifier.clone(),
            chunk_index: 1,
            content_hash: "bbb".repeat(21) + "b", // 64 chars
            source_file: "src/lib.rs".to_string(),
        };

        assert_eq!(chunk1.specifier, chunk2.specifier);
        assert_ne!(chunk1.chunk_index, chunk2.chunk_index);
        assert_ne!(chunk1.content_hash, chunk2.content_hash);
    }

    #[test]
    fn test_chunk_hash_computed_different_crates_same_hash() {
        let hash = "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24".to_string();

        let event1 = ChunkHashComputed {
            specifier: CrateSpecifier::from_str("crate_a@1.0.0").unwrap(),
            chunk_index: 0,
            content_hash: hash.clone(),
            source_file: "src/lib.rs".to_string(),
        };

        let event2 = ChunkHashComputed {
            specifier: CrateSpecifier::from_str("crate_b@1.0.0").unwrap(),
            chunk_index: 5,
            content_hash: hash.clone(),
            source_file: "src/main.rs".to_string(),
        };

        // Different crates can have same content hash (shared documentation)
        assert_ne!(event1.specifier, event2.specifier);
        assert_eq!(event1.content_hash, event2.content_hash);
    }
}
