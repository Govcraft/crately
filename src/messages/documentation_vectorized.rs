//! Event broadcast when documentation chunks are vectorized via embedding API.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Event broadcast when documentation chunks are vectorized via embedding API.
///
/// This message is broadcast by VectorizerActor after successfully calling
/// the embedding API for all documentation chunks and receiving vector
/// embeddings. The vectors are ready to be stored in the database.
///
/// # Fields
///
/// * `specifier` - The crate that was processed
/// * `features` - Feature flags for this crate
/// * `vector_count` - Number of vectors created (should match chunk count)
/// * `embedding_model` - Name of embedding model used (e.g., "text-embedding-3-small")
/// * `vectorization_duration_ms` - Time taken for API calls in milliseconds
///
/// # Message Flow
///
/// 1. VectorizerActor receives DocumentationChunked event
/// 2. VectorizerActor calls embedding API for each chunk
/// 3. VectorizerActor receives all vector embeddings
/// 4. VectorizerActor broadcasts DocumentationVectorized event
/// 5. Multiple subscribers react:
///    - Console: Displays progress message with timing
///    - DatabaseActor: Stores vectors in database (next stage)
///    - CrateCoordinatorActor: Updates state to "Vectorized"
///    - MetricsActor (future): Records API usage and costs
///
/// # Subscribers
///
/// - **Console**: User feedback - "Vectorized 42 chunks (2.3s)"
/// - **DatabaseActor**: Stores vectors in database (primary pipeline progression)
/// - **CrateCoordinatorActor** (future): State tracking - "Vectorized"
/// - **MetricsActor** (future): Tracks embedding API usage and costs
///
/// # Example
///
/// ```no_run
/// use crately::messages::DocumentationVectorized;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
/// let event = DocumentationVectorized {
///     specifier,
///     features: vec!["full".to_string()],
///     vector_count: 42,
///     embedding_model: "text-embedding-3-small".to_string(),
///     vectorization_duration_ms: 2340,
/// };
/// // broker.broadcast(event).await;
/// ```
#[acton_message]
pub struct DocumentationVectorized {
    /// The crate that was processed
    pub specifier: CrateSpecifier,
    /// Feature flags for this crate.
    ///
    /// NOTE: Currently unused but reserved for vectorization event logging.
    /// When Console subscribes to this event, features will be displayed
    /// to provide context about which crate configuration was vectorized.
    ///
    /// See issue #55 for planned event reporting.
    #[allow(dead_code)]
    pub features: Vec<String>,
    /// Number of vectors created
    pub vector_count: u32,
    /// Embedding model used
    pub embedding_model: String,
    /// Time taken for vectorization in milliseconds.
    ///
    /// NOTE: Currently unused but reserved for performance metrics and API usage tracking.
    /// Planned uses:
    /// - Console: Display timing in progress message ("Vectorized 42 chunks in 2.3s")
    /// - MetricsActor: Track embedding API performance and latency
    /// - CostTracker: Calculate actual API time and usage statistics
    ///
    /// See issue #55 for planned metrics implementation.
    #[allow(dead_code)]
    pub vectorization_duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_message_creation() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let message = DocumentationVectorized {
            specifier: specifier.clone(),
            features: vec!["derive".to_string()],
            vector_count: 30,
            embedding_model: "text-embedding-3-small".to_string(),
            vectorization_duration_ms: 1500,
        };
        assert_eq!(message.specifier, specifier);
        assert_eq!(message.vector_count, 30);
        assert_eq!(message.embedding_model, "text-embedding-3-small");
    }

    #[test]
    fn test_message_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let message = DocumentationVectorized {
            specifier,
            features: vec![],
            vector_count: 50,
            embedding_model: "text-embedding-ada-002".to_string(),
            vectorization_duration_ms: 3000,
        };
        let cloned = message.clone();
        assert_eq!(message.specifier, cloned.specifier);
        assert_eq!(message.vector_count, cloned.vector_count);
        assert_eq!(message.embedding_model, cloned.embedding_model);
        assert_eq!(message.vectorization_duration_ms, cloned.vectorization_duration_ms);
    }

    #[test]
    fn test_message_debug() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let message = DocumentationVectorized {
            specifier,
            features: vec![],
            vector_count: 42,
            embedding_model: "text-embedding-3-large".to_string(),
            vectorization_duration_ms: 2340,
        };
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("DocumentationVectorized"));
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<DocumentationVectorized>();
        assert_sync::<DocumentationVectorized>();
    }

    #[test]
    fn test_model_name_validation() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();
        let models = vec![
            "text-embedding-3-small",
            "text-embedding-3-large",
            "text-embedding-ada-002",
        ];

        for model in models {
            let message = DocumentationVectorized {
                specifier: specifier.clone(),
                features: vec![],
                vector_count: 10,
                embedding_model: model.to_string(),
                vectorization_duration_ms: 1000,
            };
            assert_eq!(message.embedding_model, model);
        }
    }

    #[test]
    fn test_timing_metrics() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.0").unwrap();
        let message = DocumentationVectorized {
            specifier,
            features: vec![],
            vector_count: 25,
            embedding_model: "text-embedding-3-small".to_string(),
            vectorization_duration_ms: 0, // Edge case: instant vectorization
        };
        assert_eq!(message.vectorization_duration_ms, 0);
    }

    #[test]
    fn test_vector_count() {
        let specifier = CrateSpecifier::from_str("huge-docs@1.0.0").unwrap();
        let message = DocumentationVectorized {
            specifier,
            features: vec![],
            vector_count: 1000,
            embedding_model: "text-embedding-3-small".to_string(),
            vectorization_duration_ms: 10000,
        };
        assert_eq!(message.vector_count, 1000);
        assert!(message.vector_count > 0);
    }

    #[test]
    fn test_empty_model_name() {
        let specifier = CrateSpecifier::from_str("test@1.0.0").unwrap();
        let message = DocumentationVectorized {
            specifier,
            features: vec![],
            vector_count: 5,
            embedding_model: String::new(), // Edge case: empty model name
            vectorization_duration_ms: 500,
        };
        assert!(message.embedding_model.is_empty());
    }
}
