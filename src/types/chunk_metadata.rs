//! ChunkMetadata - Metadata for documentation chunks
//!
//! Provides structured metadata for documentation chunks including content type,
//! line ranges, token counts, and structural information for documentation processing.

use serde::{Deserialize, Serialize};

/// Metadata describing a documentation chunk
///
/// Contains information about the chunk's content type, location in source,
/// size metrics, and structural context within the crate's module hierarchy.
///
/// # Examples
///
/// ```
/// use crately::types::ChunkMetadata;
///
/// let metadata = ChunkMetadata {
///     content_type: "markdown".to_string(),
///     start_line: Some(10),
///     end_line: Some(50),
///     token_count: 256,
///     char_count: 1024,
///     parent_module: Some("std::collections".to_string()),
///     item_type: Some("struct".to_string()),
///     item_name: Some("HashMap".to_string()),
/// };
///
/// assert_eq!(metadata.content_type, "markdown");
/// assert_eq!(metadata.token_count, 256);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Content type of the chunk (markdown, rust, toml, text)
    pub content_type: String,

    /// Starting line number in source file (if applicable)
    pub start_line: Option<usize>,

    /// Ending line number in source file (if applicable)
    pub end_line: Option<usize>,

    /// Number of tokens in the chunk (for embedding model)
    pub token_count: usize,

    /// Number of characters in the chunk
    pub char_count: usize,

    /// Parent module path in crate hierarchy
    pub parent_module: Option<String>,

    /// Type of item (struct, enum, function, module, etc.)
    pub item_type: Option<String>,

    /// Name of the item
    pub item_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_metadata_creation() {
        let metadata = ChunkMetadata {
            content_type: "rust".to_string(),
            start_line: Some(100),
            end_line: Some(200),
            token_count: 512,
            char_count: 2048,
            parent_module: Some("tokio::runtime".to_string()),
            item_type: Some("function".to_string()),
            item_name: Some("spawn".to_string()),
        };

        assert_eq!(metadata.content_type, "rust");
        assert_eq!(metadata.start_line, Some(100));
        assert_eq!(metadata.end_line, Some(200));
        assert_eq!(metadata.token_count, 512);
        assert_eq!(metadata.char_count, 2048);
        assert_eq!(metadata.parent_module, Some("tokio::runtime".to_string()));
        assert_eq!(metadata.item_type, Some("function".to_string()));
        assert_eq!(metadata.item_name, Some("spawn".to_string()));
    }

    #[test]
    fn test_chunk_metadata_optional_fields() {
        let metadata = ChunkMetadata {
            content_type: "markdown".to_string(),
            start_line: None,
            end_line: None,
            token_count: 128,
            char_count: 512,
            parent_module: None,
            item_type: None,
            item_name: None,
        };

        assert!(metadata.start_line.is_none());
        assert!(metadata.parent_module.is_none());
        assert!(metadata.item_type.is_none());
    }

    #[test]
    fn test_chunk_metadata_serde_roundtrip() {
        let original = ChunkMetadata {
            content_type: "toml".to_string(),
            start_line: Some(1),
            end_line: Some(10),
            token_count: 64,
            char_count: 256,
            parent_module: Some("config".to_string()),
            item_type: Some("table".to_string()),
            item_name: Some("dependencies".to_string()),
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ChunkMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(original.content_type, deserialized.content_type);
        assert_eq!(original.start_line, deserialized.start_line);
        assert_eq!(original.token_count, deserialized.token_count);
        assert_eq!(original.parent_module, deserialized.parent_module);
    }

    #[test]
    fn test_chunk_metadata_clone() {
        let metadata = ChunkMetadata {
            content_type: "text".to_string(),
            start_line: Some(42),
            end_line: Some(84),
            token_count: 256,
            char_count: 1024,
            parent_module: Some("docs".to_string()),
            item_type: Some("readme".to_string()),
            item_name: Some("introduction".to_string()),
        };

        let cloned = metadata.clone();
        assert_eq!(metadata.content_type, cloned.content_type);
        assert_eq!(metadata.token_count, cloned.token_count);
    }

    #[test]
    fn test_chunk_metadata_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ChunkMetadata>();
        assert_sync::<ChunkMetadata>();
    }
}
