//! Event broadcast when an embedding is successfully generated for a documentation chunk.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Event broadcast when an embedding is successfully generated for a documentation chunk.
///
/// This message is broadcast by the VectorizerActor after successfully calling the
/// embedding API and receiving a valid vector for a single chunk. The event triggers
/// persistence of the embedding to the database.
///
/// # Fields
///
/// * `specifier` - The crate this embedding belongs to
/// * `chunk_id` - Unique identifier for the documentation chunk
/// * `vector` - The embedding vector (f32 values)
/// * `model_name` - Embedding model used (e.g., "text-embedding-3-small")
/// * `model_version` - Model version for tracking
///
/// # Message Flow
///
/// 1. VectorizerActor receives DocChunksQueryResponse
/// 2. VectorizerActor batches chunks and calls embedding API
/// 3. VectorizerActor broadcasts EmbeddingGenerated for each successful embedding
/// 4. Multiple subscribers react:
///    - DatabaseActor: Persists embedding to database (via PersistEmbedding subscription)
///    - Console: Displays progress (optional granular feedback)
///    - CrateCoordinatorActor: Updates processing state (optional)
///
/// # Example
///
/// ```no_run
/// use crately::messages::EmbeddingGenerated;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
/// let event = EmbeddingGenerated {
///     specifier,
///     chunk_id: "serde_1_0_0_042".to_string(),
///     vector: vec![0.1; 384],  // 384-dimensional vector
///     model_name: "text-embedding-3-small".to_string(),
///     model_version: "v1".to_string(),
/// };
/// // broker.broadcast(event).await;
/// ```
#[acton_message]
pub struct EmbeddingGenerated {
    /// The crate this embedding belongs to
    #[allow(dead_code)]
    pub specifier: CrateSpecifier,
    /// Unique chunk identifier (e.g., "serde_1_0_0_042")
    #[allow(dead_code)]
    pub chunk_id: String,
    /// The embedding vector
    #[allow(dead_code)]
    pub vector: Vec<f32>,
    /// Embedding model name
    #[allow(dead_code)]
    pub model_name: String,
    /// Model version
    #[allow(dead_code)]
    pub model_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_embedding_generated_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let vector = vec![0.1, 0.2, 0.3];
        let event = EmbeddingGenerated {
            specifier: specifier.clone(),
            chunk_id: "tokio_1_35_0_010".to_string(),
            vector: vector.clone(),
            model_name: "text-embedding-3-small".to_string(),
            model_version: "v1".to_string(),
        };

        assert_eq!(event.specifier, specifier);
        assert_eq!(event.chunk_id, "tokio_1_35_0_010");
        assert_eq!(event.vector.len(), 3);
        assert_eq!(event.model_name, "text-embedding-3-small");
    }

    #[test]
    fn test_embedding_generated_clone() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let event = EmbeddingGenerated {
            specifier,
            chunk_id: "axum_0_7_0_005".to_string(),
            vector: vec![0.5; 384],
            model_name: "text-embedding-3-large".to_string(),
            model_version: "v2".to_string(),
        };

        let cloned = event.clone();
        assert_eq!(event.chunk_id, cloned.chunk_id);
        assert_eq!(event.vector.len(), cloned.vector.len());
        assert_eq!(event.model_name, cloned.model_name);
    }

    #[test]
    fn test_embedding_generated_debug() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let event = EmbeddingGenerated {
            specifier,
            chunk_id: "serde_1_0_0_000".to_string(),
            vector: vec![0.1, 0.2],
            model_name: "test-model".to_string(),
            model_version: "v1".to_string(),
        };

        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("EmbeddingGenerated"));
    }

    #[test]
    fn test_embedding_generated_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<EmbeddingGenerated>();
        assert_sync::<EmbeddingGenerated>();
    }

    #[test]
    fn test_embedding_generated_vector_dimensions() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.75").unwrap();

        // Test 384-dimensional vector (text-embedding-3-small)
        let event_384 = EmbeddingGenerated {
            specifier: specifier.clone(),
            chunk_id: "anyhow_1_0_75_000".to_string(),
            vector: vec![0.0; 384],
            model_name: "text-embedding-3-small".to_string(),
            model_version: "v1".to_string(),
        };
        assert_eq!(event_384.vector.len(), 384);

        // Test 1536-dimensional vector (text-embedding-3-large)
        let event_1536 = EmbeddingGenerated {
            specifier,
            chunk_id: "anyhow_1_0_75_001".to_string(),
            vector: vec![0.0; 1536],
            model_name: "text-embedding-3-large".to_string(),
            model_version: "v1".to_string(),
        };
        assert_eq!(event_1536.vector.len(), 1536);
    }

    #[test]
    fn test_embedding_generated_empty_vector() {
        let specifier = CrateSpecifier::from_str("test@1.0.0").unwrap();
        let event = EmbeddingGenerated {
            specifier,
            chunk_id: "test_1_0_0_000".to_string(),
            vector: vec![],
            model_name: "test-model".to_string(),
            model_version: "v1".to_string(),
        };

        assert_eq!(event.vector.len(), 0);
    }
}
