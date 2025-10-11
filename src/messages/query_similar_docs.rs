//! QuerySimilarDocs message - request semantic similarity search
//!
//! This message is sent to the DatabaseActor to perform vector similarity search
//! for documentation chunks based on a query embedding vector.

use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::*;

/// Request to perform vector similarity search for documentation
///
/// This message is sent to the DatabaseActor to search for documentation
/// chunks that are semantically similar to the provided query vector.
/// Results are ranked by cosine similarity and returned via SimilarDocsResponse.
///
/// # Actor Communication Pattern
///
/// Publisher: QueryHandler or API endpoint
/// Subscriber: DatabaseActor (performs search)
///
/// # Examples
///
/// ```
/// use crately::messages::QuerySimilarDocs;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// // Example query vector (typically 1536 dimensions for text-embedding-3-small)
/// let query_vector = vec![0.123, -0.456, 0.789, /* ... more values ... */];
///
/// let msg = QuerySimilarDocs {
///     query_vector,
///     limit: Some(10),
///     crate_filter: Some(CrateSpecifier::from_str("serde@1.0.0").unwrap()),
/// };
///
/// assert_eq!(msg.limit, Some(10));
/// assert!(msg.crate_filter.is_some());
/// ```
#[acton_message]
pub struct QuerySimilarDocs {
    /// The embedding vector for the search query
    /// (typically 1536 dimensions for text-embedding-3-small)
    pub query_vector: Vec<f32>,

    /// Maximum number of results to return (defaults to 10 if None)
    pub limit: Option<usize>,

    /// Optional filter to restrict search to a specific crate
    pub crate_filter: Option<CrateSpecifier>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_vector(dimension: usize) -> Vec<f32> {
        (0..dimension).map(|i| (i as f32) * 0.001).collect()
    }

    #[test]
    fn test_query_similar_docs_creation() {
        let query_vector = create_test_vector(1536);
        let msg = QuerySimilarDocs {
            query_vector: query_vector.clone(),
            limit: Some(10),
            crate_filter: None,
        };

        assert_eq!(msg.query_vector.len(), 1536);
        assert_eq!(msg.limit, Some(10));
        assert!(msg.crate_filter.is_none());
    }

    #[test]
    fn test_query_similar_docs_with_crate_filter() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let msg = QuerySimilarDocs {
            query_vector: create_test_vector(1536),
            limit: Some(5),
            crate_filter: Some(specifier.clone()),
        };

        assert_eq!(msg.limit, Some(5));
        assert_eq!(msg.crate_filter, Some(specifier));
    }

    #[test]
    fn test_query_similar_docs_no_limit() {
        let msg = QuerySimilarDocs {
            query_vector: create_test_vector(1536),
            limit: None,
            crate_filter: None,
        };

        assert!(msg.limit.is_none());
    }

    #[test]
    fn test_query_similar_docs_large_limit() {
        let msg = QuerySimilarDocs {
            query_vector: create_test_vector(1536),
            limit: Some(100),
            crate_filter: None,
        };

        assert_eq!(msg.limit, Some(100));
    }

    #[test]
    fn test_query_similar_docs_different_vector_dimension() {
        let msg = QuerySimilarDocs {
            query_vector: create_test_vector(3072),
            limit: Some(20),
            crate_filter: None,
        };

        assert_eq!(msg.query_vector.len(), 3072);
    }

    #[test]
    fn test_query_similar_docs_with_specific_crate() {
        let crate_filter = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let msg = QuerySimilarDocs {
            query_vector: create_test_vector(512),
            limit: Some(15),
            crate_filter: Some(crate_filter.clone()),
        };

        assert_eq!(msg.limit, Some(15));
        assert_eq!(msg.crate_filter, Some(crate_filter));
    }

    #[test]
    fn test_query_similar_docs_clone() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let msg = QuerySimilarDocs {
            query_vector: create_test_vector(1536),
            limit: Some(10),
            crate_filter: Some(specifier.clone()),
        };

        let cloned = msg.clone();
        assert_eq!(msg.query_vector.len(), cloned.query_vector.len());
        assert_eq!(msg.limit, cloned.limit);
        assert_eq!(msg.crate_filter, cloned.crate_filter);
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<QuerySimilarDocs>();
        assert_sync::<QuerySimilarDocs>();
    }
}
