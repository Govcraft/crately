//! CodeSampleMetadata - Metadata for code samples
//!
//! Provides structured metadata for code samples extracted from documentation
//! including source location, context, categorization, and complexity metrics.

use serde::{Deserialize, Serialize};

/// Metadata describing a code sample
///
/// Contains information about the code sample's source location, documentation
/// context, classification, and metrics for quality and complexity assessment.
///
/// # Examples
///
/// ```
/// use crately::types::CodeSampleMetadata;
///
/// let metadata = CodeSampleMetadata {
///     source_file: "examples/basic.rs".to_string(),
///     doc_context: Some("Basic usage example".to_string()),
///     parent_item: Some("HashMap::new".to_string()),
///     language: "rust".to_string(),
///     tags: vec!["example".to_string(), "collections".to_string()],
///     line_count: 15,
/// };
///
/// assert_eq!(metadata.language, "rust");
/// assert_eq!(metadata.line_count, 15);
/// assert_eq!(metadata.tags.len(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSampleMetadata {
    /// Source file path where the code sample was extracted
    pub source_file: String,

    /// Documentation context describing the sample's purpose
    pub doc_context: Option<String>,

    /// Parent item (function, struct, module) this sample relates to
    pub parent_item: Option<String>,

    /// Programming language of the sample
    pub language: String,

    /// Tags for categorization and search
    pub tags: Vec<String>,

    /// Number of lines in the code sample
    pub line_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_sample_metadata_creation() {
        let metadata = CodeSampleMetadata {
            source_file: "src/lib.rs".to_string(),
            doc_context: Some("Example of async function usage".to_string()),
            parent_item: Some("tokio::spawn".to_string()),
            language: "rust".to_string(),
            tags: vec!["async".to_string(), "tokio".to_string()],
            line_count: 20,
        };

        assert_eq!(metadata.source_file, "src/lib.rs");
        assert_eq!(
            metadata.doc_context,
            Some("Example of async function usage".to_string())
        );
        assert_eq!(metadata.parent_item, Some("tokio::spawn".to_string()));
        assert_eq!(metadata.language, "rust");
        assert_eq!(metadata.tags.len(), 2);
        assert_eq!(metadata.line_count, 20);
    }

    #[test]
    fn test_code_sample_metadata_optional_fields() {
        let metadata = CodeSampleMetadata {
            source_file: "examples/simple.rs".to_string(),
            doc_context: None,
            parent_item: None,
            language: "rust".to_string(),
            tags: vec![],
            line_count: 5,
        };

        assert!(metadata.doc_context.is_none());
        assert!(metadata.parent_item.is_none());
        assert!(metadata.tags.is_empty());
    }

    #[test]
    fn test_code_sample_metadata_with_tags() {
        let metadata = CodeSampleMetadata {
            source_file: "tests/integration.rs".to_string(),
            doc_context: Some("Integration test example".to_string()),
            parent_item: Some("Database::connect".to_string()),
            language: "rust".to_string(),
            tags: vec![
                "test".to_string(),
                "integration".to_string(),
                "database".to_string(),
            ],
            line_count: 30,
        };

        assert_eq!(metadata.tags.len(), 3);
        assert!(metadata.tags.contains(&"test".to_string()));
        assert!(metadata.tags.contains(&"integration".to_string()));
        assert!(metadata.tags.contains(&"database".to_string()));
    }

    #[test]
    fn test_code_sample_metadata_serde_roundtrip() {
        let original = CodeSampleMetadata {
            source_file: "examples/advanced.rs".to_string(),
            doc_context: Some("Advanced usage pattern".to_string()),
            parent_item: Some("Runtime::block_on".to_string()),
            language: "rust".to_string(),
            tags: vec!["advanced".to_string(), "runtime".to_string()],
            line_count: 25,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CodeSampleMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(original.source_file, deserialized.source_file);
        assert_eq!(original.doc_context, deserialized.doc_context);
        assert_eq!(original.parent_item, deserialized.parent_item);
        assert_eq!(original.language, deserialized.language);
        assert_eq!(original.tags, deserialized.tags);
        assert_eq!(original.line_count, deserialized.line_count);
    }

    #[test]
    fn test_code_sample_metadata_clone() {
        let metadata = CodeSampleMetadata {
            source_file: "src/main.rs".to_string(),
            doc_context: Some("Main function example".to_string()),
            parent_item: Some("main".to_string()),
            language: "rust".to_string(),
            tags: vec!["example".to_string()],
            line_count: 10,
        };

        let cloned = metadata.clone();
        assert_eq!(metadata.source_file, cloned.source_file);
        assert_eq!(metadata.language, cloned.language);
        assert_eq!(metadata.line_count, cloned.line_count);
    }

    #[test]
    fn test_code_sample_metadata_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CodeSampleMetadata>();
        assert_sync::<CodeSampleMetadata>();
    }
}
