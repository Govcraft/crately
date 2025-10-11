//! SearchResult - Vector search result structure
//!
//! Represents a single result from semantic similarity search, containing
//! the crate identifier, chunk information, content, and similarity score.

use crate::crate_specifier::CrateSpecifier;
use serde::{Deserialize, Serialize};

/// A single result from vector similarity search
///
/// Contains the matched documentation chunk with its crate context,
/// content, and similarity score for ranking and display.
///
/// # Examples
///
/// ```
/// use crately::types::SearchResult;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let result = SearchResult {
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     chunk_id: "chunk_123".to_string(),
///     content: "Serde is a framework for serializing...".to_string(),
///     similarity_score: 0.95,
/// };
///
/// assert_eq!(result.similarity_score, 0.95);
/// assert!(result.similarity_score > 0.7); // High relevance
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The crate this documentation chunk belongs to
    pub specifier: CrateSpecifier,

    /// Unique identifier for the documentation chunk
    pub chunk_id: String,

    /// The actual documentation content text
    pub content: String,

    /// Cosine similarity score (0.0 to 1.0, higher is more similar)
    pub similarity_score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_search_result_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let result = SearchResult {
            specifier: specifier.clone(),
            chunk_id: "chunk_abc123".to_string(),
            content: "Tokio is an asynchronous runtime...".to_string(),
            similarity_score: 0.88,
        };

        assert_eq!(result.specifier, specifier);
        assert_eq!(result.chunk_id, "chunk_abc123");
        assert_eq!(result.content, "Tokio is an asynchronous runtime...");
        assert_eq!(result.similarity_score, 0.88);
    }

    #[test]
    fn test_search_result_high_similarity() {
        let result = SearchResult {
            specifier: CrateSpecifier::from_str("axum@0.7.0").unwrap(),
            chunk_id: "chunk_xyz789".to_string(),
            content: "Web framework built on top of tower...".to_string(),
            similarity_score: 0.99,
        };

        assert!(result.similarity_score > 0.9);
        assert!(result.similarity_score <= 1.0);
    }

    #[test]
    fn test_search_result_low_similarity() {
        let result = SearchResult {
            specifier: CrateSpecifier::from_str("rand@0.8.0").unwrap(),
            chunk_id: "chunk_def456".to_string(),
            content: "Random number generation...".to_string(),
            similarity_score: 0.65,
        };

        assert!(result.similarity_score < 0.7);
        assert!(result.similarity_score >= 0.0);
    }

    #[test]
    fn test_search_result_serde_roundtrip() {
        let original = SearchResult {
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            chunk_id: "chunk_001".to_string(),
            content: "Serialization framework...".to_string(),
            similarity_score: 0.92,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&json).unwrap();

        assert_eq!(original.specifier, deserialized.specifier);
        assert_eq!(original.chunk_id, deserialized.chunk_id);
        assert_eq!(original.content, deserialized.content);
        assert_eq!(original.similarity_score, deserialized.similarity_score);
    }

    #[test]
    fn test_search_result_clone() {
        let result = SearchResult {
            specifier: CrateSpecifier::from_str("tracing@0.1.0").unwrap(),
            chunk_id: "chunk_999".to_string(),
            content: "Application-level tracing...".to_string(),
            similarity_score: 0.85,
        };

        let cloned = result.clone();
        assert_eq!(result.specifier, cloned.specifier);
        assert_eq!(result.chunk_id, cloned.chunk_id);
        assert_eq!(result.similarity_score, cloned.similarity_score);
    }

    #[test]
    fn test_search_result_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<SearchResult>();
        assert_sync::<SearchResult>();
    }
}
