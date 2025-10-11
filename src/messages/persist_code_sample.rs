//! PersistCodeSample message - request to persist code sample
//!
//! This message is sent to the DatabaseActor to persist a code sample
//! extracted from documentation with its associated metadata.

use crate::crate_specifier::CrateSpecifier;
use crate::types::CodeSampleMetadata;
use acton_reactive::prelude::*;

/// Request to persist a code sample to the database
///
/// This message is sent by the code extraction actor to the DatabaseActor
/// to store a code sample extracted from documentation along with its
/// metadata for later retrieval and analysis.
///
/// # Actor Communication Pattern
///
/// Publisher: CodeExtractionActor
/// Subscriber: DatabaseActor (for persistence)
///
/// # Examples
///
/// ```
/// use crately::messages::PersistCodeSample;
/// use crately::crate_specifier::CrateSpecifier;
/// use crately::types::CodeSampleMetadata;
/// use std::str::FromStr;
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
/// let code = r#"
/// use std::collections::HashMap;
///
/// fn main() {
///     let mut map = HashMap::new();
///     map.insert("key", "value");
/// }
/// "#;
///
/// let msg = PersistCodeSample {
///     specifier: CrateSpecifier::from_str("std@1.0.0").unwrap(),
///     sample_index: 0,
///     code: code.to_string(),
///     sample_type: "example".to_string(),
///     metadata,
/// };
///
/// assert_eq!(msg.sample_index, 0);
/// assert_eq!(msg.sample_type, "example");
/// ```
#[acton_message]
pub struct PersistCodeSample {
    /// The crate this code sample belongs to
    pub specifier: CrateSpecifier,

    /// Zero-based index of this code sample within the crate
    pub sample_index: usize,

    /// The code sample text
    pub code: String,

    /// Type of code sample (example, test, usage, snippet)
    pub sample_type: String,

    /// Additional metadata about the code sample
    pub metadata: CodeSampleMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_metadata() -> CodeSampleMetadata {
        CodeSampleMetadata {
            source_file: "examples/basic.rs".to_string(),
            doc_context: Some("Basic usage example".to_string()),
            parent_item: Some("tokio::spawn".to_string()),
            language: "rust".to_string(),
            tags: vec!["async".to_string(), "runtime".to_string()],
            line_count: 10,
        }
    }

    #[test]
    fn test_persist_code_sample_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let code = "async fn example() { tokio::spawn(async { }).await; }".to_string();
        let metadata = create_test_metadata();

        let msg = PersistCodeSample {
            specifier: specifier.clone(),
            sample_index: 5,
            code: code.clone(),
            sample_type: "example".to_string(),
            metadata: metadata.clone(),
        };

        assert_eq!(msg.specifier, specifier);
        assert_eq!(msg.sample_index, 5);
        assert_eq!(msg.code, code);
        assert_eq!(msg.sample_type, "example");
        assert_eq!(msg.metadata.language, "rust");
    }

    #[test]
    fn test_persist_code_sample_test_type() {
        let msg = PersistCodeSample {
            specifier: CrateSpecifier::from_str("axum@0.7.0").unwrap(),
            sample_index: 0,
            code: "#[test] fn test_router() { assert!(true); }".to_string(),
            sample_type: "test".to_string(),
            metadata: CodeSampleMetadata {
                source_file: "tests/integration.rs".to_string(),
                doc_context: None,
                parent_item: Some("Router".to_string()),
                language: "rust".to_string(),
                tags: vec!["test".to_string()],
                line_count: 3,
            },
        };

        assert_eq!(msg.sample_type, "test");
        assert_eq!(msg.metadata.tags, vec!["test".to_string()]);
    }

    #[test]
    fn test_persist_code_sample_usage_type() {
        let code = r#"
let client = reqwest::Client::new();
let response = client.get("https://api.example.com").send().await?;
"#
        .to_string();

        let msg = PersistCodeSample {
            specifier: CrateSpecifier::from_str("reqwest@0.11.0").unwrap(),
            sample_index: 3,
            code,
            sample_type: "usage".to_string(),
            metadata: CodeSampleMetadata {
                source_file: "src/client.rs".to_string(),
                doc_context: Some("Client usage example".to_string()),
                parent_item: Some("Client::get".to_string()),
                language: "rust".to_string(),
                tags: vec!["http".to_string(), "client".to_string()],
                line_count: 2,
            },
        };

        assert_eq!(msg.sample_type, "usage");
    }

    #[test]
    fn test_persist_code_sample_snippet_type() {
        let msg = PersistCodeSample {
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            sample_index: 10,
            code: "#[derive(Serialize, Deserialize)]".to_string(),
            sample_type: "snippet".to_string(),
            metadata: CodeSampleMetadata {
                source_file: "src/ser.rs".to_string(),
                doc_context: Some("Derive macro example".to_string()),
                parent_item: None,
                language: "rust".to_string(),
                tags: vec!["macro".to_string(), "derive".to_string()],
                line_count: 1,
            },
        };

        assert_eq!(msg.sample_type, "snippet");
        assert_eq!(msg.metadata.line_count, 1);
    }

    #[test]
    fn test_persist_code_sample_clone() {
        let msg = PersistCodeSample {
            specifier: CrateSpecifier::from_str("tracing@0.1.0").unwrap(),
            sample_index: 2,
            code: "tracing::info!(\"Hello, world!\");".to_string(),
            sample_type: "snippet".to_string(),
            metadata: create_test_metadata(),
        };

        let cloned = msg.clone();
        assert_eq!(msg.specifier, cloned.specifier);
        assert_eq!(msg.sample_index, cloned.sample_index);
        assert_eq!(msg.code, cloned.code);
        assert_eq!(msg.sample_type, cloned.sample_type);
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<PersistCodeSample>();
        assert_sync::<PersistCodeSample>();
    }
}
