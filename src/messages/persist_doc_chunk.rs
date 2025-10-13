//! PersistDocChunk message - request to persist documentation chunk
//!
//! This message is sent to the DatabaseActor to persist a documentation chunk
//! with its associated metadata to the database.

use crate::crate_specifier::CrateSpecifier;
use crate::types::ChunkMetadata;
use acton_reactive::prelude::*;

/// Request to persist a documentation chunk to the database
///
/// This message is sent by documentation processing actors to the DatabaseActor
/// to store a chunk of documentation text along with its metadata for later
/// retrieval and vectorization.
///
/// # Actor Communication Pattern
///
/// Publisher: DocumentationChunker actor
/// Subscriber: DatabaseActor (for persistence)
///
/// # Examples
///
/// ```
/// use crately::messages::PersistDocChunk;
/// use crately::crate_specifier::CrateSpecifier;
/// use crately::types::ChunkMetadata;
/// use std::str::FromStr;
///
/// let metadata = ChunkMetadata {
///     content_type: "markdown".to_string(),
///     start_line: Some(1),
///     end_line: Some(50),
///     token_count: 256,
///     char_count: 1024,
///     parent_module: Some("std::collections".to_string()),
///     item_type: Some("module".to_string()),
///     item_name: Some("HashMap".to_string()),
/// };
///
/// let msg = PersistDocChunk {
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     chunk_index: 0,
///     chunk_id: "serde_1.0.0_chunk_000".to_string(),
///     content: "Serde is a framework for serializing and deserializing...".to_string(),
///     source_file: "src/lib.rs".to_string(),
///     metadata,
///     features: None,
/// };
///
/// assert_eq!(msg.chunk_index, 0);
/// assert_eq!(msg.metadata.content_type, "markdown");
/// ```
#[acton_message]
pub struct PersistDocChunk {
    /// The crate this documentation chunk belongs to
    pub specifier: CrateSpecifier,

    /// Zero-based index of this chunk within the crate's documentation
    pub chunk_index: usize,

    /// Unique identifier for this chunk
    pub _chunk_id: String,

    /// The documentation text content
    pub content: String,

    /// Source file path where this chunk was extracted from
    pub source_file: String,

    /// Additional metadata about the chunk
    pub metadata: ChunkMetadata,

    /// Optional feature flags enabled for this build
    pub features: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_metadata() -> ChunkMetadata {
        ChunkMetadata {
            content_type: "rust".to_string(),
            start_line: Some(100),
            end_line: Some(200),
            token_count: 512,
            char_count: 2048,
            parent_module: Some("tokio::runtime".to_string()),
            item_type: Some("function".to_string()),
            item_name: Some("spawn".to_string()),
        }
    }

    #[test]
    fn test_persist_doc_chunk_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let metadata = create_test_metadata();

        let msg = PersistDocChunk {
            specifier: specifier.clone(),
            chunk_index: 5,
            _chunk_id: "tokio_1.35.0_chunk_005".to_string(),
            content: "Tokio runtime documentation...".to_string(),
            source_file: "src/runtime/mod.rs".to_string(),
            metadata: metadata.clone(),
            features: None,
        };

        assert_eq!(msg.specifier, specifier);
        assert_eq!(msg.chunk_index, 5);
        assert_eq!(msg._chunk_id, "tokio_1.35.0_chunk_005");
        assert_eq!(msg.content, "Tokio runtime documentation...");
        assert_eq!(msg.source_file, "src/runtime/mod.rs");
        assert_eq!(msg.metadata.content_type, "rust");
    }

    #[test]
    fn test_persist_doc_chunk_first_chunk() {
        let msg = PersistDocChunk {
            specifier: CrateSpecifier::from_str("axum@0.7.0").unwrap(),
            chunk_index: 0,
            _chunk_id: "axum_0.7.0_chunk_000".to_string(),
            content: "Axum web framework overview...".to_string(),
            source_file: "src/lib.rs".to_string(),
            metadata: ChunkMetadata {
                content_type: "markdown".to_string(),
                start_line: None,
                end_line: None,
                token_count: 128,
                char_count: 512,
                parent_module: None,
                item_type: Some("crate".to_string()),
                item_name: Some("axum".to_string()),
            },
            features: None,
        };

        assert_eq!(msg.chunk_index, 0);
        assert!(msg.metadata.start_line.is_none());
    }

    #[test]
    fn test_persist_doc_chunk_with_line_numbers() {
        let msg = PersistDocChunk {
            specifier: CrateSpecifier::from_str("serde_json@1.0.0").unwrap(),
            chunk_index: 10,
            _chunk_id: "serde_json_1.0.0_chunk_010".to_string(),
            content: "JSON serialization utilities...".to_string(),
            source_file: "src/ser.rs".to_string(),
            metadata: ChunkMetadata {
                content_type: "rust".to_string(),
                start_line: Some(42),
                end_line: Some(84),
                token_count: 256,
                char_count: 1024,
                parent_module: Some("serde_json::ser".to_string()),
                item_type: Some("impl".to_string()),
                item_name: Some("Serializer".to_string()),
            },
            features: None,
        };

        assert_eq!(msg.metadata.start_line, Some(42));
        assert_eq!(msg.metadata.end_line, Some(84));
    }

    #[test]
    fn test_persist_doc_chunk_clone() {
        let msg = PersistDocChunk {
            specifier: CrateSpecifier::from_str("tracing@0.1.0").unwrap(),
            chunk_index: 3,
            _chunk_id: "tracing_0.1.0_chunk_003".to_string(),
            content: "Tracing subscriber documentation...".to_string(),
            source_file: "src/subscriber.rs".to_string(),
            metadata: create_test_metadata(),
            features: None,
        };

        let cloned = msg.clone();
        assert_eq!(msg.specifier, cloned.specifier);
        assert_eq!(msg.chunk_index, cloned.chunk_index);
        assert_eq!(msg._chunk_id, cloned._chunk_id);
        assert_eq!(msg.metadata.content_type, cloned.metadata.content_type);
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<PersistDocChunk>();
        assert_sync::<PersistDocChunk>();
    }
}
