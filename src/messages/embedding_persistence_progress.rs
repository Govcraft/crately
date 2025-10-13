use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::acton_message;

/// Progress notification for embedding vectorization and persistence.
///
/// This message is broadcast by the CrateCoordinatorActor after each individual
/// embedding is persisted to the database. It allows subscribers (like the Console
/// actor) to display real-time progress of the vectorization process.
///
/// # Publisher
/// - `CrateCoordinatorActor`: Broadcasts after receiving each `EmbeddingPersisted` message
///
/// # Subscribers
/// - `Console`: Displays vectorization progress in the terminal
///
/// # Example Output
/// ```text
/// ⋯ Vectorizing embedding 15/58 for serde@1.0.0 with text-embedding-3-small
/// ```
#[acton_message]
pub struct EmbeddingPersistenceProgress {
    /// The crate specifier for which embeddings are being vectorized
    pub specifier: CrateSpecifier,

    /// The ID of the chunk that was vectorized
    pub chunk_id: String,

    /// Number of embeddings persisted so far
    pub persisted_count: u32,

    /// Total number of embeddings expected for this crate
    pub total_vectors: u32,

    /// The embedding model being used (e.g., "text-embedding-3-small")
    pub embedding_model: String,
}
