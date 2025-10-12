//! Event broadcast when all embeddings for a crate have been successfully persisted.

use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::*;

/// Event broadcast when all embeddings for a crate have been persisted to the database.
///
/// This message is broadcast via the broker by CrateCoordinatorActor after tracking that all
/// expected embeddings have been persisted (via EmbeddingPersisted events). This enables the
/// DatabaseActor to safely update the crate status to "vectorized" knowing that all embedding
/// data is already in the database.
///
/// # Actor Communication Pattern
///
/// Publisher: CrateCoordinatorActor (after tracking all EmbeddingPersisted events)
/// Subscribers: DatabaseActor (for status update after confirmed persistence)
///
/// # Purpose
///
/// This message solves a race condition where DatabaseActor previously attempted to validate
/// embedding existence immediately upon receiving DocumentationVectorized, before persistence
/// was complete. By waiting for this confirmation message, we ensure status transitions happen
/// only after data is safely stored.
///
/// # Fields
///
/// * `specifier` - The crate for which all embeddings have been persisted
/// * `vector_count` - Total number of embeddings that were persisted
/// * `embedding_model` - Name of the embedding model used
///
/// # Example
///
/// ```no_run
/// use crately::messages::EmbeddingsPersistenceComplete;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let event = EmbeddingsPersistenceComplete {
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     vector_count: 18,
///     embedding_model: "text-embedding-3-small".to_string(),
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. VectorizerActor broadcasts DocumentationVectorized + multiple PersistEmbedding messages
/// 2. DatabaseActor processes each PersistEmbedding → broadcasts EmbeddingPersisted
/// 3. CrateCoordinatorActor tracks EmbeddingPersisted events
/// 4. When all expected embeddings are persisted, CrateCoordinatorActor broadcasts EmbeddingsPersistenceComplete
/// 5. DatabaseActor receives this message and updates status to "vectorized"
///
/// # Subscribers
///
/// - **DatabaseActor**: Updates crate status to "vectorized" after receiving this confirmation
#[acton_message]
pub struct EmbeddingsPersistenceComplete {
    /// The crate for which all embeddings have been persisted
    pub specifier: CrateSpecifier,

    /// Total number of embeddings that were persisted
    pub vector_count: u32,

    /// Name of the embedding model used (e.g., "text-embedding-3-small")
    pub embedding_model: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Verify that EmbeddingsPersistenceComplete can be created and cloned.
    #[test]
    fn test_embeddings_persistence_complete_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let event = EmbeddingsPersistenceComplete {
            specifier: specifier.clone(),
            vector_count: 25,
            embedding_model: "text-embedding-3-small".to_string(),
        };
        let cloned = event.clone();
        assert_eq!(event.specifier, cloned.specifier);
        assert_eq!(event.vector_count, cloned.vector_count);
        assert_eq!(event.embedding_model, cloned.embedding_model);
    }

    /// Verify that EmbeddingsPersistenceComplete implements Debug.
    #[test]
    fn test_embeddings_persistence_complete_debug() {
        let event = EmbeddingsPersistenceComplete {
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            vector_count: 18,
            embedding_model: "text-embedding-3-large".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
    }

    /// Verify that EmbeddingsPersistenceComplete is Send + Sync (required for actor message passing).
    #[test]
    fn test_embeddings_persistence_complete_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<EmbeddingsPersistenceComplete>();
        assert_sync::<EmbeddingsPersistenceComplete>();
    }

    /// Verify that EmbeddingsPersistenceComplete fields contain expected data.
    #[test]
    fn test_embeddings_persistence_complete_fields() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let event = EmbeddingsPersistenceComplete {
            specifier: specifier.clone(),
            vector_count: 42,
            embedding_model: "text-embedding-ada-002".to_string(),
        };
        assert_eq!(event.specifier, specifier);
        assert_eq!(event.vector_count, 42);
        assert_eq!(event.embedding_model, "text-embedding-ada-002");
    }

    /// Verify different embedding models are supported.
    #[test]
    fn test_embeddings_persistence_complete_different_models() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();

        let event_small = EmbeddingsPersistenceComplete {
            specifier: specifier.clone(),
            vector_count: 10,
            embedding_model: "text-embedding-3-small".to_string(),
        };
        assert_eq!(event_small.embedding_model, "text-embedding-3-small");

        let event_large = EmbeddingsPersistenceComplete {
            specifier: specifier.clone(),
            vector_count: 10,
            embedding_model: "text-embedding-3-large".to_string(),
        };
        assert_eq!(event_large.embedding_model, "text-embedding-3-large");
    }

    /// Verify zero vectors is valid (edge case for crates with no documentation).
    #[test]
    fn test_embeddings_persistence_complete_zero_vectors() {
        let event = EmbeddingsPersistenceComplete {
            specifier: CrateSpecifier::from_str("empty-crate@0.1.0").unwrap(),
            vector_count: 0,
            embedding_model: "text-embedding-3-small".to_string(),
        };
        assert_eq!(event.vector_count, 0);
    }

    /// Verify large vector counts are supported.
    #[test]
    fn test_embeddings_persistence_complete_large_count() {
        let event = EmbeddingsPersistenceComplete {
            specifier: CrateSpecifier::from_str("massive-crate@1.0.0").unwrap(),
            vector_count: 10_000,
            embedding_model: "text-embedding-3-small".to_string(),
        };
        assert_eq!(event.vector_count, 10_000);
    }
}
