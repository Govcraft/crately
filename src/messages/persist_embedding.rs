//! PersistEmbedding message - request to persist vector embedding
//!
//! This message is sent to the DatabaseActor to persist a vector embedding
//! for a documentation chunk along with model metadata.

use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::*;

/// Request to persist a vector embedding to the database
///
/// This message is sent by the vectorization actor to the DatabaseActor
/// to store a vector embedding for a documentation chunk, along with
/// metadata about the embedding model used.
///
/// # Actor Communication Pattern
///
/// Publisher: VectorizationActor
/// Subscriber: DatabaseActor (for persistence)
///
/// # Examples
///
/// ```
/// use crately::messages::PersistEmbedding;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// // Example 1536-dimensional embedding vector (truncated for brevity)
/// let vector = vec![0.123, -0.456, 0.789, /* ... 1533 more values ... */];
///
/// let msg = PersistEmbedding {
///     chunk_id: "serde_1.0.0_chunk_000".to_string(),
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     vector,
///     model_name: "text-embedding-3-small".to_string(),
///     model_version: "3".to_string(),
/// };
///
/// assert_eq!(msg.model_name, "text-embedding-3-small");
/// ```
#[acton_message]
pub struct PersistEmbedding {
    /// Unique identifier of the documentation chunk being vectorized
    pub chunk_id: String,

    /// The crate this embedding belongs to
    pub specifier: CrateSpecifier,

    /// The embedding vector (typically 1536 dimensions for text-embedding-3-small)
    pub vector: Vec<f32>,

    /// Name of the embedding model used (e.g., "text-embedding-3-small")
    pub model_name: String,

    /// Version of the embedding model
    pub model_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_vector(dimension: usize) -> Vec<f32> {
        (0..dimension).map(|i| (i as f32) * 0.001).collect()
    }

    #[test]
    fn test_persist_embedding_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let vector = create_test_vector(1536);

        let msg = PersistEmbedding {
            chunk_id: "tokio_1.35.0_chunk_005".to_string(),
            specifier: specifier.clone(),
            vector: vector.clone(),
            model_name: "text-embedding-3-small".to_string(),
            model_version: "3".to_string(),
        };

        assert_eq!(msg.chunk_id, "tokio_1.35.0_chunk_005");
        assert_eq!(msg.specifier, specifier);
        assert_eq!(msg.vector.len(), 1536);
        assert_eq!(msg.model_name, "text-embedding-3-small");
        assert_eq!(msg.model_version, "3");
    }

    #[test]
    fn test_persist_embedding_different_model() {
        let msg = PersistEmbedding {
            chunk_id: "axum_0.7.0_chunk_000".to_string(),
            specifier: CrateSpecifier::from_str("axum@0.7.0").unwrap(),
            vector: create_test_vector(3072),
            model_name: "text-embedding-3-large".to_string(),
            model_version: "3".to_string(),
        };

        assert_eq!(msg.vector.len(), 3072);
        assert_eq!(msg.model_name, "text-embedding-3-large");
    }

    #[test]
    fn test_persist_embedding_vector_values() {
        let vector = vec![0.123, -0.456, 0.789];
        let msg = PersistEmbedding {
            chunk_id: "serde_1.0.0_chunk_003".to_string(),
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            vector: vector.clone(),
            model_name: "test-model".to_string(),
            model_version: "1".to_string(),
        };

        assert_eq!(msg.vector[0], 0.123);
        assert_eq!(msg.vector[1], -0.456);
        assert_eq!(msg.vector[2], 0.789);
    }

    #[test]
    fn test_persist_embedding_clone() {
        let msg = PersistEmbedding {
            chunk_id: "tracing_0.1.0_chunk_010".to_string(),
            specifier: CrateSpecifier::from_str("tracing@0.1.0").unwrap(),
            vector: create_test_vector(512),
            model_name: "text-embedding-3-small".to_string(),
            model_version: "3".to_string(),
        };

        let cloned = msg.clone();
        assert_eq!(msg.chunk_id, cloned.chunk_id);
        assert_eq!(msg.specifier, cloned.specifier);
        assert_eq!(msg.vector.len(), cloned.vector.len());
        assert_eq!(msg.model_name, cloned.model_name);
        assert_eq!(msg.model_version, cloned.model_version);
    }

    #[test]
    fn test_persist_embedding_empty_vector() {
        let msg = PersistEmbedding {
            chunk_id: "test_chunk".to_string(),
            specifier: CrateSpecifier::from_str("test@1.0.0").unwrap(),
            vector: vec![],
            model_name: "test-model".to_string(),
            model_version: "1".to_string(),
        };

        assert_eq!(msg.vector.len(), 0);
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<PersistEmbedding>();
        assert_sync::<PersistEmbedding>();
    }
}
