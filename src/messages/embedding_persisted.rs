//! Event broadcast when a vector embedding is successfully persisted to the database.

use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::*;

/// Event broadcast when a vector embedding is successfully persisted.
///
/// This message is broadcast via the broker by DatabaseActor after successfully
/// storing a vector embedding in the database. This enables the CrateCoordinatorActor
/// to track completion of embedding persistence and coordinate proper state transitions.
///
/// # Actor Communication Pattern
///
/// Publisher: DatabaseActor (after successful PersistEmbedding operation)
/// Subscribers: CrateCoordinatorActor (for completion tracking)
///
/// # Fields
///
/// * `chunk_id` - Unique identifier for the chunk that was vectorized
/// * `specifier` - The crate this embedding belongs to
/// * `model_name` - Name of the embedding model used
///
/// # Example
///
/// ```no_run
/// use crately::messages::EmbeddingPersisted;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let event = EmbeddingPersisted {
///     chunk_id: "serde_1.0.0_chunk_005".to_string(),
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     model_name: "text-embedding-3-small".to_string(),
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. DatabaseActor receives PersistEmbedding message
/// 2. DatabaseActor stores embedding vector in SurrealDB
/// 3. DatabaseActor broadcasts EmbeddingPersisted with embedding details
/// 4. CrateCoordinatorActor updates embedding persistence state
///
/// # Subscribers
///
/// - **CrateCoordinatorActor**: Tracks embedding persistence completion
#[acton_message]
pub struct EmbeddingPersisted {
    /// Unique identifier for the chunk that was vectorized
    pub chunk_id: String,

    /// The crate this embedding belongs to
    pub specifier: CrateSpecifier,

    /// Name of the embedding model used (e.g., "text-embedding-3-small")
    pub model_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Verify that EmbeddingPersisted can be created and cloned.
    #[test]
    fn test_embedding_persisted_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let event = EmbeddingPersisted {
            chunk_id: "tokio_1.35.0_chunk_003".to_string(),
            specifier: specifier.clone(),
            model_name: "text-embedding-3-small".to_string(),
        };
        let cloned = event.clone();
        assert_eq!(event.chunk_id, cloned.chunk_id);
        assert_eq!(event.specifier, cloned.specifier);
        assert_eq!(event.model_name, cloned.model_name);
    }

    /// Verify that EmbeddingPersisted implements Debug.
    #[test]
    fn test_embedding_persisted_debug() {
        let event = EmbeddingPersisted {
            chunk_id: "serde_1.0.0_chunk_000".to_string(),
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            model_name: "text-embedding-3-small".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
    }

    /// Verify that EmbeddingPersisted is Send + Sync (required for actor message passing).
    #[test]
    fn test_embedding_persisted_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<EmbeddingPersisted>();
        assert_sync::<EmbeddingPersisted>();
    }

    /// Verify that EmbeddingPersisted fields contain expected data.
    #[test]
    fn test_embedding_persisted_fields() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let event = EmbeddingPersisted {
            chunk_id: "axum_0.7.0_chunk_010".to_string(),
            specifier: specifier.clone(),
            model_name: "text-embedding-3-large".to_string(),
        };
        assert_eq!(event.chunk_id, "axum_0.7.0_chunk_010");
        assert_eq!(event.specifier, specifier);
        assert_eq!(event.model_name, "text-embedding-3-large");
    }

    /// Verify different model names are supported.
    #[test]
    fn test_embedding_persisted_different_models() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();

        let event_small = EmbeddingPersisted {
            chunk_id: "tracing_0.1.0_chunk_000".to_string(),
            specifier: specifier.clone(),
            model_name: "text-embedding-3-small".to_string(),
        };
        assert_eq!(event_small.model_name, "text-embedding-3-small");

        let event_large = EmbeddingPersisted {
            chunk_id: "tracing_0.1.0_chunk_001".to_string(),
            specifier: specifier.clone(),
            model_name: "text-embedding-3-large".to_string(),
        };
        assert_eq!(event_large.model_name, "text-embedding-3-large");
    }
}
