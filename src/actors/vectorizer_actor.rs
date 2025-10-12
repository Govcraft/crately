//! Actor responsible for generating vector embeddings from documentation chunks
//!
//! The `VectorizerActor` is a stateless worker that subscribes to `DocumentationChunked` events
//! and generates vector embeddings for each chunk. It reads chunks from the database, calls the
//! embedding API (or mock for initial implementation), and persists the vectors.

use acton_reactive::prelude::*;
use anyhow::Result;
use std::time::Instant;

use crate::actors::config::VectorizeConfig;
use crate::crate_specifier::CrateSpecifier;
use crate::messages::{DocumentationChunked, DocumentationVectorized, PersistEmbedding};

/// Stateless actor for generating vector embeddings
///
/// This actor subscribes to `DocumentationChunked` events and performs the following:
/// 1. Receives notification that chunks have been created and persisted
/// 2. Queries chunks from database using chunk IDs
/// 3. Generates embeddings using configured embedding model (mock for now)
/// 4. Persists embeddings to database via `PersistEmbedding` messages
/// 5. Broadcasts `DocumentationVectorized` on success
///
/// # State
///
/// The actor is stateless and relies on configuration provided through the
/// `VectorizeConfig` which is immutable after initialization.
///
/// # Message Flow
///
/// - Subscribes to: `DocumentationChunked`
/// - Broadcasts: `PersistEmbedding` (for each chunk), `DocumentationVectorized`
#[acton_actor]
pub struct VectorizerActor {
    /// Vectorization configuration (immutable)
    config: VectorizeConfig,
}

impl VectorizerActor {
    /// Creates a new VectorizerActor with the given configuration
    ///
    /// This constructor encapsulates the initialization logic, storing the
    /// immutable vectorization configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Vectorization configuration including model name and batch size
    ///
    /// # Returns
    ///
    /// Returns a new `VectorizerActor` instance ready to be spawned.
    pub fn new(config: VectorizeConfig) -> Self {
        Self { config }
    }

    /// Spawns, configures, and starts a new VectorizerActor
    ///
    /// This is the standard factory method for creating VectorizerActor actors.
    /// The VectorizerActor subscribes to `DocumentationChunked` events and generates
    /// embeddings for documentation chunks, broadcasting success events.
    ///
    /// This follows the simple actor pattern where only the handle is returned,
    /// as the VectorizerActor has no startup data to provide to the application.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    /// * `config` - Vectorization configuration including model and dimensions
    ///
    /// # Returns
    ///
    /// Returns `AgentHandle` to the started VectorizerActor for message passing.
    ///
    /// # When to Use
    ///
    /// Call this during application startup to initialize the vectorization
    /// subsystem. The actor will be ready to process `DocumentationChunked` events
    /// and coordinate with other actors in the crate processing pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if actor creation or initialization fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use acton_reactive::prelude::*;
    /// use crately::actors::vectorizer_actor::VectorizerActor;
    /// use crately::actors::config::VectorizeConfig;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut runtime = ActonApp::launch();
    ///
    ///     // Spawn the VectorizerActor for handling embedding generation
    ///     let config = VectorizeConfig::default();
    ///     let vectorizer = VectorizerActor::spawn(&mut runtime, config).await?;
    ///
    ///     runtime.shutdown_all().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn spawn(runtime: &mut AgentRuntime, config: VectorizeConfig) -> Result<AgentHandle> {
        let mut builder = runtime
            .new_agent_with_name::<VectorizerActor>("vectorizer".to_string())
            .await;

        // Initialize actor with configuration
        builder.model = VectorizerActor::new(config);

        // Subscribe to DocumentationChunked events
        builder.mutate_on::<DocumentationChunked>(|agent, envelope| {
            let msg = envelope.message().clone();
            let broker = agent.broker().clone();
            let config = agent.model.config.clone();

            AgentReply::from_async(async move {
                let start_time = Instant::now();

                match vectorize_chunks(&config, &broker, &msg.specifier, msg.chunk_count).await {
                    Ok(vector_count) => {
                        let duration_ms = start_time.elapsed().as_millis() as u64;

                        // Broadcast DocumentationVectorized event with statistics
                        broker
                            .broadcast(DocumentationVectorized {
                                specifier: msg.specifier,
                                features: msg.features,
                                vector_count,
                                embedding_model: config.model_name.clone(),
                                vectorization_duration_ms: duration_ms,
                            })
                            .await;
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to vectorize documentation for {}: {:#}",
                            msg.specifier,
                            e
                        );
                    }
                }
            })
        });

        let handle = builder.start().await;

        // Subscribe to DocumentationChunked events through the broker
        handle.subscribe::<DocumentationChunked>().await;

        Ok(handle)
    }
}

/// Generates embeddings for documentation chunks
///
/// This function generates mock embeddings for each chunk and broadcasts `PersistEmbedding`
/// messages to the DatabaseActor for persistence. In a real implementation, this would call
/// an embedding API (e.g., OpenAI's embeddings endpoint) to generate actual vector embeddings.
///
/// # Arguments
///
/// * `config` - Vectorization configuration with model settings
/// * `broker` - Message broker handle for broadcasting persistence messages
/// * `specifier` - Crate name and version for context
/// * `chunk_count` - Number of chunks to vectorize
///
/// # Returns
///
/// Returns the number of vectors created on success
///
/// # Errors
///
/// Returns an error if:
/// - Database queries fail
/// - Embedding generation fails
/// - Vector persistence fails
async fn vectorize_chunks(
    config: &VectorizeConfig,
    broker: &AgentHandle,
    specifier: &CrateSpecifier,
    chunk_count: u32,
) -> Result<u32> {
    // For now, we'll generate mock embeddings
    // In a real implementation, this would:
    // 1. Query chunks from database using chunk IDs
    // 2. Batch chunks for API calls
    // 3. Call embedding API for each batch
    // 4. Handle rate limiting and retries
    // 5. Persist embeddings to database via PersistEmbedding messages

    // Simulate processing each chunk
    for index in 0..chunk_count {
        let chunk_id = format!(
            "{}_{}_{:03}",
            specifier.name().replace('-', "_"),
            specifier.version().to_string().replace('.', "_"),
            index
        );

        // Generate mock embedding vector with configured dimensions
        let mock_vector = generate_mock_embedding(config.vector_dimension);

        // Broadcast PersistEmbedding message to DatabaseActor
        broker
            .broadcast(PersistEmbedding {
                chunk_id: chunk_id.clone(),
                specifier: specifier.clone(),
                vector: mock_vector,
                model_name: config.model_name.clone(),
                model_version: config.model_version.clone(),
            })
            .await;
    }

    Ok(chunk_count)
}

/// Generates a mock embedding vector
///
/// Creates a vector of the specified dimension filled with deterministic
/// pseudo-random values for testing purposes.
///
/// # Arguments
///
/// * `dimension` - The dimension of the embedding vector to generate
///
/// # Returns
///
/// Returns a vector of f32 values representing the mock embedding
fn generate_mock_embedding(dimension: usize) -> Vec<f32> {
    // Generate deterministic mock embeddings for testing
    // In production, this would be replaced by actual API calls
    (0..dimension)
        .map(|i| ((i as f32 * 0.1) % 1.0) - 0.5)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_vectorizer_actor_spawn_creates_actor() {
        let mut runtime = ActonApp::launch();
        let config = VectorizeConfig::default();
        let result = VectorizerActor::spawn(&mut runtime, config).await;
        assert!(result.is_ok(), "VectorizerActor spawn should succeed");

        let handle = result.unwrap();
        let stop_result = handle.stop().await;
        assert!(
            stop_result.is_ok(),
            "VectorizerActor should stop gracefully"
        );
    }

    #[test]
    fn test_vectorizer_actor_new() {
        let config = VectorizeConfig::default();
        let actor = VectorizerActor::new(config.clone());
        assert_eq!(actor.config.model_name, config.model_name);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_vectorize_chunks_success() {
        let mut runtime = ActonApp::launch();
        let specifier = CrateSpecifier::from_str("test_crate@1.0.0").unwrap();
        let config = VectorizeConfig::default();

        // Create a simple agent to get access to the broker
        let mut builder = runtime.new_agent::<VectorizerActor>().await;
        builder.model = VectorizerActor::new(config.clone());
        let handle = builder.start().await;

        // Get broker from the agent handle
        let broker = handle.get_broker().expect("Broker should be available");

        let result = vectorize_chunks(&config, &broker, &specifier, 5).await;
        assert!(result.is_ok(), "Vectorization should succeed");

        let vector_count = result.unwrap();
        assert_eq!(vector_count, 5, "Should have created 5 vectors");

        handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_vectorize_chunks_zero_chunks() {
        let mut runtime = ActonApp::launch();
        let specifier = CrateSpecifier::from_str("empty@1.0.0").unwrap();
        let config = VectorizeConfig::default();

        // Create a simple agent to get access to the broker
        let mut builder = runtime.new_agent::<VectorizerActor>().await;
        builder.model = VectorizerActor::new(config.clone());
        let handle = builder.start().await;

        // Get broker from the agent handle
        let broker = handle.get_broker().expect("Broker should be available");

        let result = vectorize_chunks(&config, &broker, &specifier, 0).await;
        assert!(result.is_ok(), "Vectorization of zero chunks should succeed");

        let vector_count = result.unwrap();
        assert_eq!(vector_count, 0, "Should have created 0 vectors");

        handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();
    }

    #[test]
    fn test_generate_mock_embedding_dimension_384() {
        let embedding = generate_mock_embedding(384);
        assert_eq!(embedding.len(), 384, "Should generate 384-dimensional vector");
    }

    #[test]
    fn test_generate_mock_embedding_dimension_1536() {
        let embedding = generate_mock_embedding(1536);
        assert_eq!(
            embedding.len(),
            1536,
            "Should generate 1536-dimensional vector"
        );
    }

    #[test]
    fn test_generate_mock_embedding_values_in_range() {
        let embedding = generate_mock_embedding(100);
        for value in embedding {
            assert!(
                (-0.5..=0.5).contains(&value),
                "Embedding values should be in range [-0.5, 0.5]"
            );
        }
    }

    #[test]
    fn test_generate_mock_embedding_deterministic() {
        let embedding1 = generate_mock_embedding(50);
        let embedding2 = generate_mock_embedding(50);
        assert_eq!(embedding1, embedding2, "Mock embeddings should be deterministic");
    }

    #[test]
    fn test_generate_mock_embedding_zero_dimension() {
        let embedding = generate_mock_embedding(0);
        assert!(embedding.is_empty(), "Zero dimension should produce empty vector");
    }
}
