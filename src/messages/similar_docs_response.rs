//! SimilarDocsResponse message - semantic search results
//!
//! This message is broadcast by the DatabaseActor after performing vector similarity
//! search, containing the ranked results of semantically similar documentation chunks.

use crate::types::SearchResult;
use acton_reactive::prelude::*;

/// Response containing results from vector similarity search
///
/// This message is broadcast by the DatabaseActor after performing semantic
/// search over documentation embeddings. Results are ranked by cosine similarity
/// and include the crate context, chunk content, and similarity scores.
///
/// # Actor Communication Pattern
///
/// Publisher: DatabaseActor (after QuerySimilarDocs)
/// Subscribers: API handlers, UI components, query processors
///
/// # Examples
///
/// ```
/// use crately::messages::SimilarDocsResponse;
/// use crately::types::SearchResult;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let results = vec![
///     SearchResult {
///         specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///         chunk_id: "serde_1.0.0_chunk_005".to_string(),
///         content: "Serde is a framework for serializing...".to_string(),
///         similarity_score: 0.95,
///     },
///     SearchResult {
///         specifier: CrateSpecifier::from_str("serde_json@1.0.0").unwrap(),
///         chunk_id: "serde_json_1.0.0_chunk_003".to_string(),
///         content: "JSON serialization utilities...".to_string(),
///         similarity_score: 0.88,
///     },
/// ];
///
/// let msg = SimilarDocsResponse { results };
///
/// assert_eq!(msg.results.len(), 2);
/// assert!(msg.results[0].similarity_score > msg.results[1].similarity_score);
/// ```
#[acton_message]
pub struct SimilarDocsResponse {
    /// Ranked search results, sorted by similarity score (highest first)
    pub results: Vec<SearchResult>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crate_specifier::CrateSpecifier;
    use crate::types::SearchResult;
    use std::str::FromStr;

    fn create_test_result(crate_name: &str, score: f32) -> SearchResult {
        let specifier = CrateSpecifier::from_str(crate_name).unwrap();
        SearchResult {
            specifier,
            chunk_id: format!("{}_chunk_001", crate_name),
            content: format!("Documentation for {}", crate_name),
            similarity_score: score,
        }
    }

    #[test]
    fn test_similar_docs_response_creation() {
        let results = vec![
            create_test_result("tokio@1.35.0", 0.95),
            create_test_result("axum@0.7.0", 0.88),
            create_test_result("serde@1.0.0", 0.82),
        ];

        let msg = SimilarDocsResponse {
            results: results.clone(),
        };

        assert_eq!(msg.results.len(), 3);
        assert_eq!(msg.results[0].similarity_score, 0.95);
        assert_eq!(msg.results[1].similarity_score, 0.88);
        assert_eq!(msg.results[2].similarity_score, 0.82);
    }

    #[test]
    fn test_similar_docs_response_empty() {
        let msg = SimilarDocsResponse { results: vec![] };

        assert!(msg.results.is_empty());
    }

    #[test]
    fn test_similar_docs_response_single_result() {
        let results = vec![create_test_result("tracing@0.1.0", 0.99)];

        let msg = SimilarDocsResponse {
            results: results.clone(),
        };

        assert_eq!(msg.results.len(), 1);
        assert_eq!(msg.results[0].similarity_score, 0.99);
    }

    #[test]
    fn test_similar_docs_response_sorted_by_score() {
        let results = vec![
            create_test_result("crate1@1.0.0", 0.95),
            create_test_result("crate2@1.0.0", 0.88),
            create_test_result("crate3@1.0.0", 0.82),
            create_test_result("crate4@1.0.0", 0.75),
        ];

        let msg = SimilarDocsResponse {
            results: results.clone(),
        };

        // Verify results are sorted descending by similarity score
        for i in 0..msg.results.len() - 1 {
            assert!(msg.results[i].similarity_score >= msg.results[i + 1].similarity_score);
        }
    }

    #[test]
    fn test_similar_docs_response_many_results() {
        let results: Vec<SearchResult> = (0..100)
            .map(|i| {
                let score = 1.0 - (i as f32 * 0.01);
                create_test_result(&format!("crate{}@1.0.0", i), score)
            })
            .collect();

        let msg = SimilarDocsResponse {
            results: results.clone(),
        };

        assert_eq!(msg.results.len(), 100);
        assert_eq!(msg.results[0].similarity_score, 1.0);
        assert!(msg.results[99].similarity_score < 0.1);
    }

    #[test]
    fn test_similar_docs_response_clone() {
        let results = vec![
            create_test_result("serde@1.0.0", 0.90),
            create_test_result("tokio@1.35.0", 0.85),
        ];

        let msg = SimilarDocsResponse {
            results: results.clone(),
        };

        let cloned = msg.clone();
        assert_eq!(msg.results.len(), cloned.results.len());
        assert_eq!(
            msg.results[0].similarity_score,
            cloned.results[0].similarity_score
        );
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<SimilarDocsResponse>();
        assert_sync::<SimilarDocsResponse>();
    }
}
