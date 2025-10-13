//! BuildChunksQueryResponse - Response containing chunks for a build with edge metadata
//!
//! This message is broadcast by DatabaseActor in response to QueryBuildChunks requests.
//! It contains all documentation chunks for a build with relationship metadata from the
//! graph structure.

use crate::types::BuildId;
use acton_reactive::prelude::*;
use serde::{Deserialize, Serialize};

/// Response containing documentation chunks for a build with graph edge metadata
///
/// This response provides all content blocks associated with a specific build,
/// ordered by chunk_index, with relationship metadata from the `contains` edge.
///
/// # Edge Metadata
///
/// Each chunk includes metadata from the `build->contains->content_block` relationship:
/// - `chunk_index`: Position in documentation sequence
/// - `source_file`: Origin file path
///
/// # Example
///
/// ```no_run
/// use crately::messages::{BuildChunksQueryResponse, BuildChunkData};
/// use crately::types::BuildId;
///
/// // Example response handler
/// # /*
/// builder.act_on::<BuildChunksQueryResponse>(|agent, envelope| {
///     let response = envelope.message();
///     for chunk in &response.chunks {
///         println!(
///             "Chunk {} from {}: {} chars",
///             chunk.chunk_index,
///             chunk.source_file,
///             chunk.char_count
///         );
///     }
///     AgentReply::immediate()
/// });
/// # */
/// ```
#[acton_message]
pub struct BuildChunksQueryResponse {
    /// The BuildId these chunks belong to
    pub build_id: BuildId,

    /// Documentation chunks with edge metadata, ordered by chunk_index ascending
    pub chunks: Vec<BuildChunkData>,
}

/// Data for a single documentation chunk with graph relationship context
///
/// Combines content_block node data with edge metadata from the `contains`
/// relationship. This provides all necessary information in a single query
/// without additional lookups.
///
/// # Fields
///
/// ## Content Block Data (from node)
/// - `content_hash`: BLAKE3 hash for content deduplication
/// - `content`: The actual documentation text
/// - `content_type`: Format (markdown, html, etc.)
/// - `char_count`: Character count for sizing
/// - `token_count`: Token count for LLM context management
///
/// ## Edge Metadata (from `contains` relationship)
/// - `chunk_index`: Position in documentation sequence
/// - `source_file`: Origin file path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildChunkData {
    // Content block node data
    pub content_hash: String,
    pub content: String,
    pub content_type: String,
    pub char_count: i64,
    pub token_count: i64,

    // Edge metadata from 'contains' relationship
    pub chunk_index: i64,
    pub source_file: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_chunk(index: i64) -> BuildChunkData {
        BuildChunkData {
            content_hash: format!("hash_{}", index),
            content: format!("Content for chunk {}", index),
            content_type: "markdown".to_string(),
            char_count: 100 + index,
            token_count: 20 + index,
            chunk_index: index,
            source_file: format!("file{}.rs", index),
        }
    }

    #[test]
    fn test_build_chunks_query_response_basic() {
        let build_id = BuildId::from_str("build_abc123def456abc123def456abc12345").unwrap();
        let chunks = vec![create_test_chunk(0), create_test_chunk(1)];

        let response = BuildChunksQueryResponse {
            build_id: build_id.clone(),
            chunks: chunks.clone(),
        };

        assert_eq!(response.build_id, build_id);
        assert_eq!(response.chunks.len(), 2);
    }

    #[test]
    fn test_build_chunks_query_response_empty() {
        let build_id = BuildId::from_str("build_abc123def456abc123def456abc12345").unwrap();
        let response = BuildChunksQueryResponse {
            build_id,
            chunks: vec![],
        };

        assert_eq!(response.chunks.len(), 0);
    }

    #[test]
    fn test_build_chunk_data_ordering() {
        let chunks = vec![
            create_test_chunk(2),
            create_test_chunk(0),
            create_test_chunk(1),
        ];

        // Verify chunk_index values
        assert_eq!(chunks[0].chunk_index, 2);
        assert_eq!(chunks[1].chunk_index, 0);
        assert_eq!(chunks[2].chunk_index, 1);
    }

    #[test]
    fn test_build_chunk_data_edge_metadata() {
        let chunk = create_test_chunk(5);

        // Verify edge metadata
        assert_eq!(chunk.chunk_index, 5);
        assert_eq!(chunk.source_file, "file5.rs");
    }

    #[test]
    fn test_build_chunk_data_content_node() {
        let chunk = create_test_chunk(3);

        // Verify content block node data
        assert_eq!(chunk.content_hash, "hash_3");
        assert_eq!(chunk.content, "Content for chunk 3");
        assert_eq!(chunk.content_type, "markdown");
        assert_eq!(chunk.char_count, 103);
        assert_eq!(chunk.token_count, 23);
    }

    #[test]
    fn test_build_chunks_query_response_clone() {
        let build_id = BuildId::from_str("build_abc123def456abc123def456abc12345").unwrap();
        let original = BuildChunksQueryResponse {
            build_id: build_id.clone(),
            chunks: vec![create_test_chunk(0)],
        };

        let cloned = original.clone();
        assert_eq!(original.build_id, cloned.build_id);
        assert_eq!(original.chunks.len(), cloned.chunks.len());
    }

    #[test]
    fn test_build_chunk_data_serde_roundtrip() {
        let original = create_test_chunk(7);
        let json = serde_json::to_string(&original).expect("Should serialize");
        let deserialized: BuildChunkData =
            serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(original.content_hash, deserialized.content_hash);
        assert_eq!(original.chunk_index, deserialized.chunk_index);
        assert_eq!(original.source_file, deserialized.source_file);
    }

    #[test]
    fn test_build_chunks_query_response_debug() {
        let build_id = BuildId::from_str("build_abc123def456abc123def456abc12345").unwrap();
        let response = BuildChunksQueryResponse {
            build_id,
            chunks: vec![],
        };

        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("BuildChunksQueryResponse"));
    }

    #[test]
    fn test_build_chunks_query_response_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<BuildChunksQueryResponse>();
        assert_sync::<BuildChunksQueryResponse>();
    }
}
