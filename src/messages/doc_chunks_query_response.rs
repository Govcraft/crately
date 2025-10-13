//! Response message containing documentation chunk data from database query.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;
use serde::{Deserialize, Serialize};

/// Response containing documentation chunks for a queried crate.
///
/// This message is broadcast by the DatabaseActor in response to QueryDocChunks.
/// It contains all chunk records for the requested crate, ordered by chunk_index.
///
/// # Fields
///
/// * `specifier` - The crate these chunks belong to
/// * `chunks` - Vector of chunk data records, ordered by chunk_index
///
/// # Example
///
/// ```no_run
/// use crately::messages::{DocChunksQueryResponse, DocChunkData};
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
/// let chunks = vec![
///     DocChunkData {
///         chunk_id: "serde_1_0_0_000".to_string(),
///         chunk_index: 0,
///         content: "Serde documentation...".to_string(),
///         content_type: "markdown".to_string(),
///         source_file: "README.md".to_string(),
///         token_count: 150,
///         char_count: 500,
///     },
/// ];
/// let response = DocChunksQueryResponse { specifier, chunks };
/// ```
#[acton_message]
pub struct DocChunksQueryResponse {
    /// The crate these chunks belong to
    pub specifier: CrateSpecifier,
    /// The chunk data, ordered by chunk_index
    pub chunks: Vec<DocChunkData>,
}

/// Individual documentation chunk data record.
///
/// This struct represents a single chunk of documentation text that can be
/// vectorized and used for semantic search. Each chunk has metadata about
/// its content, location, and size.
///
/// # Fields
///
/// * `chunk_id` - Unique chunk identifier (e.g., "serde_1_0_0_000")
/// * `chunk_index` - Zero-based chunk index for ordering
/// * `content` - The actual documentation content to vectorize
/// * `content_type` - Content type (markdown, rust, toml, text)
/// * `source_file` - Source file path
/// * `token_count` - Token count estimate
/// * `char_count` - Character count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocChunkData {
    /// Unique chunk identifier (e.g., "serde_1_0_0_000")
    pub chunk_id: String,
    /// Zero-based chunk index for ordering
    pub chunk_index: u32,
    /// The actual documentation content to vectorize
    pub content: String,
    /// Content type (markdown, rust, toml, text)
    pub content_type: String,
    /// Source file path
    pub source_file: String,
    /// Token count estimate
    pub token_count: u32,
    /// Character count
    pub char_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Verify that DocChunksQueryResponse can be created with chunks.
    #[test]
    fn test_doc_chunks_query_response_creation() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let chunks = vec![
            DocChunkData {
                chunk_id: "serde_1_0_0_000".to_string(),
                chunk_index: 0,
                content: "First chunk".to_string(),
                content_type: "markdown".to_string(),
                source_file: "README.md".to_string(),
                token_count: 50,
                char_count: 200,
            },
            DocChunkData {
                chunk_id: "serde_1_0_0_001".to_string(),
                chunk_index: 1,
                content: "Second chunk".to_string(),
                content_type: "rust".to_string(),
                source_file: "lib.rs".to_string(),
                token_count: 75,
                char_count: 300,
            },
        ];

        let response = DocChunksQueryResponse {
            specifier: specifier.clone(),
            chunks: chunks.clone(),
        };

        assert_eq!(response.specifier.name(), "serde");
        assert_eq!(response.specifier.version().to_string(), "1.0.0");
        assert_eq!(response.chunks.len(), 2);
        assert_eq!(response.chunks[0].chunk_id, "serde_1_0_0_000");
        assert_eq!(response.chunks[1].chunk_index, 1);
    }

    /// Verify that DocChunksQueryResponse can be created with empty chunks.
    #[test]
    fn test_doc_chunks_query_response_empty() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let response = DocChunksQueryResponse {
            specifier: specifier.clone(),
            chunks: vec![],
        };

        assert_eq!(response.specifier.name(), "tokio");
        assert_eq!(response.chunks.len(), 0);
    }

    /// Verify that DocChunksQueryResponse can be cloned.
    #[test]
    fn test_doc_chunks_query_response_clone() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.75").unwrap();
        let chunks = vec![
            DocChunkData {
                chunk_id: "anyhow_1_0_75_000".to_string(),
                chunk_index: 0,
                content: "Test content".to_string(),
                content_type: "text".to_string(),
                source_file: "test.txt".to_string(),
                token_count: 25,
                char_count: 100,
            },
        ];

        let response = DocChunksQueryResponse {
            specifier: specifier.clone(),
            chunks: chunks.clone(),
        };
        let cloned = response.clone();

        assert_eq!(response.specifier.name(), cloned.specifier.name());
        assert_eq!(response.chunks.len(), cloned.chunks.len());
        assert_eq!(response.chunks[0].chunk_id, cloned.chunks[0].chunk_id);
    }

    /// Verify that DocChunksQueryResponse implements Debug.
    #[test]
    fn test_doc_chunks_query_response_debug() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let response = DocChunksQueryResponse {
            specifier,
            chunks: vec![],
        };
        let debug_str = format!("{:?}", response);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("DocChunksQueryResponse"));
    }

    /// Verify that DocChunksQueryResponse is Send + Sync (required for actor message passing).
    #[test]
    fn test_doc_chunks_query_response_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<DocChunksQueryResponse>();
        assert_sync::<DocChunksQueryResponse>();
    }

    /// Verify that DocChunkData can be serialized and deserialized.
    #[test]
    fn test_doc_chunk_data_serde() {
        let chunk = DocChunkData {
            chunk_id: "test_1_0_0_000".to_string(),
            chunk_index: 0,
            content: "Test content for serialization".to_string(),
            content_type: "markdown".to_string(),
            source_file: "docs.md".to_string(),
            token_count: 42,
            char_count: 200,
        };

        let json = serde_json::to_string(&chunk).unwrap();
        let deserialized: DocChunkData = serde_json::from_str(&json).unwrap();

        assert_eq!(chunk.chunk_id, deserialized.chunk_id);
        assert_eq!(chunk.chunk_index, deserialized.chunk_index);
        assert_eq!(chunk.content, deserialized.content);
        assert_eq!(chunk.token_count, deserialized.token_count);
    }

    /// Verify that DocChunkData is Send + Sync.
    #[test]
    fn test_doc_chunk_data_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<DocChunkData>();
        assert_sync::<DocChunkData>();
    }
}
