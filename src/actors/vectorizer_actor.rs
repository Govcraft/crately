//! Actor responsible for generating vector embeddings from documentation chunks
//!
//! The `VectorizerActor` is a stateless worker that subscribes to `DocumentationChunked` events
//! and generates vector embeddings for each chunk. It reads chunks from the database, calls the
//! embedding API, and persists the vectors.

use acton_reactive::prelude::*;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use thiserror::Error;

use crate::actors::config::VectorizeConfig;
use crate::crate_specifier::CrateSpecifier;
use crate::messages::{DocumentationChunked, DocumentationVectorized, PersistEmbedding};

/// Errors that can occur during embedding generation
#[derive(Debug, Error)]
pub enum EmbeddingError {
    /// API key not found in environment
    #[error("API key not found. Set OPENAI_API_KEY or EMBEDDING_API_KEY environment variable")]
    MissingApiKey,

    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    /// API returned an error response
    #[error("API error: {0}")]
    ApiError(String),

    /// Response parsing failed
    #[error("Failed to parse API response: {0}")]
    ParseError(String),

    /// Invalid response format
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),

    /// Dimension mismatch
    #[error("Vector dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

/// OpenAI API request format for embedding generation
#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    /// Text to embed
    input: String,
    /// Model name (e.g., "text-embedding-3-small")
    model: String,
    /// Encoding format (always "float" for f32 vectors)
    encoding_format: String,
}

/// OpenAI API response format for embedding generation
#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    /// Response object type (should be "list")
    #[serde(rename = "object")]
    _object: String,
    /// Array of embedding data
    data: Vec<EmbeddingData>,
    /// Model used for generation
    #[serde(rename = "model")]
    _model: String,
    /// Token usage statistics
    #[serde(rename = "usage")]
    _usage: Option<Usage>,
}

/// Embedding data within the API response
#[derive(Debug, Deserialize)]
struct EmbeddingData {
    /// Object type (should be "embedding")
    #[serde(rename = "object")]
    _object: String,
    /// Index in the response array
    #[serde(rename = "index")]
    _index: usize,
    /// The actual embedding vector
    embedding: Vec<f32>,
}

/// Token usage statistics
#[derive(Debug, Deserialize)]
struct Usage {
    /// Number of prompt tokens
    #[serde(rename = "prompt_tokens")]
    _prompt_tokens: usize,
    /// Total tokens used
    #[serde(rename = "total_tokens")]
    _total_tokens: usize,
}

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
        builder.act_on::<DocumentationChunked>(|agent, envelope| {
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
/// This function generates real embeddings by calling an OpenAI-compatible API for each chunk
/// and broadcasts `PersistEmbedding` messages to the DatabaseActor for persistence.
///
/// # Arguments
///
/// * `config` - Vectorization configuration with API settings and model
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
    // TODO(#58): In the future, we should:
    // See: https://github.com/Govcraft/crately/issues/58
    // 1. Query actual chunk text from database using chunk IDs
    // 2. Batch chunks for API calls (OpenAI supports up to 2048 inputs per request)
    // 3. Implement batching for efficiency
    //
    // For now, we'll generate mock text and call the API one chunk at a time

    // Simulate processing each chunk
    for index in 0..chunk_count {
        let chunk_id = format!(
            "{}_{}_{:03}",
            specifier.name().replace('-', "_"),
            specifier.version().to_string().replace('.', "_"),
            index
        );

        // TODO(#57, #58): Query actual chunk text from database
        // See: https://github.com/Govcraft/crately/issues/57
        // See: https://github.com/Govcraft/crately/issues/58
        // For now, use a placeholder text
        let chunk_text = format!(
            "Documentation chunk {} for {} version {}",
            index,
            specifier.name(),
            specifier.version()
        );

        // Generate real embedding vector by calling the API
        let embedding_vector = generate_embedding(config, &chunk_text).await?;

        // Broadcast PersistEmbedding message to DatabaseActor
        broker
            .broadcast(PersistEmbedding {
                chunk_id: chunk_id.clone(),
                specifier: specifier.clone(),
                vector: embedding_vector,
                model_name: config.model_name.clone(),
                model_version: config.model_version.clone(),
            })
            .await;
    }

    Ok(chunk_count)
}

/// Calls OpenAI-compatible embedding API to generate real vectors
///
/// Implements retry logic with exponential backoff and proper error handling.
/// Supports any OpenAI-compatible embedding API endpoint.
///
/// # Arguments
///
/// * `config` - Vectorization configuration with API settings
/// * `text` - Text content to embed
///
/// # Returns
///
/// Returns a vector of f32 values representing the semantic embedding
///
/// # Errors
///
/// Returns an error if:
/// - API request fails after retries
/// - Response parsing fails
/// - API returns error response
/// - API key is missing
/// - Vector dimension doesn't match configuration
async fn generate_embedding(
    config: &VectorizeConfig,
    text: &str,
) -> Result<Vec<f32>, EmbeddingError> {
    // Get API key from environment variable
    let api_key = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("EMBEDDING_API_KEY"))
        .map_err(|_| EmbeddingError::MissingApiKey)?;

    // Build HTTP client with timeout
    let timeout = Duration::from_secs(config.request_timeout_secs);
    let client = Client::builder().timeout(timeout).build()?;

    // Create API request body
    let request_body = EmbeddingRequest {
        input: text.to_string(),
        model: config.model_name.clone(),
        encoding_format: "float".to_string(),
    };

    // Implement retry logic with exponential backoff
    let mut attempts = 0;
    let mut retry_delay = Duration::from_secs(config.retry_delay_secs);

    loop {
        attempts += 1;

        match client
            .post(&config.api_endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
        {
            Ok(response) => {
                // Check if response is successful
                if !response.status().is_success() {
                    let status = response.status();
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());

                    // Retry on rate limit (429) or server errors (5xx)
                    if (status.as_u16() == 429 || status.is_server_error())
                        && attempts <= config.max_retries
                    {
                        tracing::warn!(
                            "API request failed with status {}, retrying in {:?} (attempt {}/{})",
                            status,
                            retry_delay,
                            attempts,
                            config.max_retries
                        );
                        tokio::time::sleep(retry_delay).await;
                        retry_delay *= 2; // Exponential backoff
                        continue;
                    }

                    return Err(EmbeddingError::ApiError(format!(
                        "Status {}: {}",
                        status, error_text
                    )));
                }

                // Parse JSON response
                let embedding_response: EmbeddingResponse = response
                    .json()
                    .await
                    .map_err(|e| EmbeddingError::ParseError(e.to_string()))?;

                // Extract embedding vector from first data element
                if embedding_response.data.is_empty() {
                    return Err(EmbeddingError::InvalidResponse(
                        "No embedding data in response".to_string(),
                    ));
                }

                let embedding = embedding_response.data[0].embedding.clone();

                // Verify dimension matches configuration
                if embedding.len() != config.vector_dimension {
                    return Err(EmbeddingError::DimensionMismatch {
                        expected: config.vector_dimension,
                        actual: embedding.len(),
                    });
                }

                return Ok(embedding);
            }
            Err(e) => {
                // Retry on network errors
                if attempts <= config.max_retries {
                    tracing::warn!(
                        "Network error: {}, retrying in {:?} (attempt {}/{})",
                        e,
                        retry_delay,
                        attempts,
                        config.max_retries
                    );
                    tokio::time::sleep(retry_delay).await;
                    retry_delay *= 2; // Exponential backoff
                    continue;
                }

                return Err(EmbeddingError::RequestFailed(e));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a mock embedding for testing purposes
    ///
    /// This function provides deterministic mock embeddings for testing when
    /// an API key is not available. It should only be used in tests.
    fn generate_mock_embedding(dimension: usize) -> Vec<f32> {
        // Create a deterministic vector based on dimension
        // Values in range [-0.5, 0.5] for reasonable mock data
        (0..dimension)
            .map(|i| ((i % 100) as f32 / 100.0) - 0.5)
            .collect()
    }

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
        assert_eq!(
            embedding1, embedding2,
            "Mock embeddings should be deterministic"
        );
    }

    #[test]
    fn test_generate_mock_embedding_zero_dimension() {
        let embedding = generate_mock_embedding(0);
        assert!(embedding.is_empty(), "Zero dimension should produce empty vector");
    }

    #[test]
    fn test_embedding_error_display() {
        let error = EmbeddingError::MissingApiKey;
        assert!(error.to_string().contains("API key not found"));

        let error = EmbeddingError::DimensionMismatch {
            expected: 1536,
            actual: 384,
        };
        assert!(error.to_string().contains("expected 1536"));
        assert!(error.to_string().contains("got 384"));
    }

    #[test]
    fn test_embedding_request_serialization() {
        let request = EmbeddingRequest {
            input: "test text".to_string(),
            model: "text-embedding-3-small".to_string(),
            encoding_format: "float".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("test text"));
        assert!(json.contains("text-embedding-3-small"));
        assert!(json.contains("float"));
    }

    #[test]
    fn test_embedding_response_deserialization() {
        let json = r#"{
            "object": "list",
            "data": [
                {
                    "object": "embedding",
                    "index": 0,
                    "embedding": [0.1, 0.2, 0.3]
                }
            ],
            "model": "text-embedding-3-small",
            "usage": {
                "prompt_tokens": 5,
                "total_tokens": 5
            }
        }"#;

        let response: EmbeddingResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response._object, "list");
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].embedding.len(), 3);
        assert_eq!(response._model, "text-embedding-3-small");
    }
}
