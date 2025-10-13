//! Actor responsible for generating vector embeddings from documentation chunks
//!
//! The `VectorizerActor` is a stateless worker that subscribes to `DocumentationChunked` events
//! and generates vector embeddings for documentation chunks using batch processing.
//!
//! # Architecture Pattern
//!
//! This actor follows the **stateless event broadcaster** pattern:
//! - Subscribes to `DocumentationChunked` events (trigger)
//! - Queries chunks from DatabaseActor via `QueryDocChunks` message
//! - Receives `DocChunksQueryResponse` with actual chunk content
//! - Batches chunks for efficient API calls (up to 2048 per batch)
//! - Calls embedding API with batched inputs
//! - Broadcasts `EmbeddingGenerated` for each successful embedding
//! - Broadcasts `EmbeddingFailed` for errors (with retryability classification)
//! - NO retry logic - RetryCoordinator handles all retries
//!
//! # Event Flow
//!
//! ```text
//! DocumentationChunked → VectorizerActor
//!                             ↓
//!                      QueryDocChunks → DatabaseActor
//!                             ↓
//!                      DocChunksQueryResponse
//!                             ↓
//!                      Batch chunks (max 2048)
//!                             ↓
//!                      Call embedding API
//!                             ↓
//!         ┌───────────────────┴───────────────────┐
//!         ↓                                       ↓
//! EmbeddingGenerated (success)         EmbeddingFailed (error)
//!         ↓                                       ↓
//! DatabaseActor (persist)              RetryCoordinator (retry if retryable)
//! ```

use acton_reactive::prelude::*;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

use crate::actors::config::VectorizeConfig;
use crate::messages::{
    DocChunkData, DocChunksQueryResponse, DocumentationChunked, EmbeddingFailed, EmbeddingGenerated,
    QueryDocChunks,
};

/// Errors that can occur during embedding generation
#[derive(Debug, Error, Clone)]
pub enum EmbeddingError {
    /// API key not found in environment
    #[error("API key not found. Set OPENAI_API_KEY or EMBEDDING_API_KEY environment variable")]
    MissingApiKey,

    /// HTTP request failed (transient - network issues)
    #[error("HTTP request failed: {0}")]
    RequestFailed(String),

    /// API returned an error response
    #[error("API error (status {status}): {message}")]
    ApiError {
        /// HTTP status code
        status: u16,
        /// Error message from API
        message: String,
    },

    /// Response parsing failed
    #[error("Failed to parse API response: {0}")]
    ParseError(String),

    /// Invalid response format
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),

    /// Dimension mismatch (permanent error)
    #[error("Vector dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Batch size exceeded
    #[error("Batch size {actual} exceeds maximum {max}")]
    BatchSizeExceeded { actual: usize, max: usize },
}

impl EmbeddingError {
    /// Classify error as retryable or permanent
    ///
    /// Retryable errors:
    /// - Network failures (timeouts, connection errors)
    /// - Rate limits (429)
    /// - Server errors (5xx)
    ///
    /// Permanent errors:
    /// - Invalid API key (401, 403)
    /// - Bad requests (4xx except 429)
    /// - Dimension mismatches
    /// - Batch size violations
    pub fn is_retryable(&self) -> bool {
        match self {
            // Transient network errors
            EmbeddingError::RequestFailed(_) => true,

            // API errors classified by status code
            EmbeddingError::ApiError { status, .. } => {
                // Rate limits and server errors are retryable
                *status == 429 || (500..600).contains(status)
            }

            // Permanent errors
            EmbeddingError::MissingApiKey => false,
            EmbeddingError::ParseError(_) => false,
            EmbeddingError::InvalidResponse(_) => false,
            EmbeddingError::DimensionMismatch { .. } => false,
            EmbeddingError::BatchSizeExceeded { .. } => false,
        }
    }
}

/// OpenAI API request format for batch embedding generation
#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    /// Array of texts to embed (up to 2048)
    input: Vec<String>,
    /// Model name (e.g., "text-embedding-3-small")
    model: String,
    /// Encoding format (always "float" for f32 vectors)
    encoding_format: String,
}

/// OpenAI API response format for batch embedding generation
#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    /// Response object type (should be "list")
    #[serde(rename = "object")]
    _object: String,
    /// Array of embedding data
    data: Vec<EmbeddingData>,
    /// Model used for generation
    model: String,
    /// Token usage statistics
    usage: Usage,
}

/// Embedding data within the API response
#[derive(Debug, Deserialize)]
struct EmbeddingData {
    /// Object type (should be "embedding")
    #[serde(rename = "object")]
    _object: String,
    /// Index in the request array
    #[allow(dead_code)]
    index: usize,
    /// The actual embedding vector
    embedding: Vec<f32>,
}

/// Token usage statistics
#[derive(Debug, Deserialize)]
struct Usage {
    /// Number of prompt tokens
    prompt_tokens: usize,
    /// Total tokens used
    total_tokens: usize,
}

/// Stateless actor for generating vector embeddings with batch processing
///
/// This actor subscribes to `DocumentationChunked` events and performs the following:
/// 1. Receives notification that chunks have been created and persisted
/// 2. Sends QueryDocChunks to DatabaseActor to retrieve chunk content
/// 3. Subscribes to DocChunksQueryResponse to receive chunk data
/// 4. Batches chunks for API efficiency (up to 2048 per batch)
/// 5. Generates embeddings using configured embedding model
/// 6. Broadcasts EmbeddingGenerated for each successful embedding
/// 7. Broadcasts EmbeddingFailed for errors (NO retry logic in this actor)
///
/// # State
///
/// The actor is stateless and relies on configuration provided through the
/// `VectorizeConfig` which is immutable after initialization.
///
/// # Message Flow
///
/// - Subscribes to: `DocumentationChunked`, `DocChunksQueryResponse`
/// - Sends to: DatabaseActor via `QueryDocChunks`
/// - Broadcasts: `EmbeddingGenerated` (per chunk), `EmbeddingFailed` (on error)
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
    /// The VectorizerActor subscribes to `DocumentationChunked` and `DocChunksQueryResponse`
    /// events to generate embeddings for documentation chunks using batch processing.
    ///
    /// This follows the simple actor pattern where only the handle is returned,
    /// as the VectorizerActor has no startup data to provide to the application.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    /// * `config` - Vectorization configuration including model, dimensions, and batch size
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

        // Handler 1: DocumentationChunked - triggers chunk query
        // Uses act_on because it's stateless (just broadcasts QueryDocChunks)
        builder.act_on::<DocumentationChunked>(|agent, envelope| {
            let msg = envelope.message().clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                tracing::debug!(
                    specifier = %msg.specifier,
                    chunk_count = msg.chunk_count,
                    "Received DocumentationChunked event, querying chunks from database"
                );

                // Send QueryDocChunks to DatabaseActor
                broker
                    .broadcast(QueryDocChunks {
                        specifier: msg.specifier,
                    })
                    .await;
            })
        });

        // Handler 2: DocChunksQueryResponse - processes chunks and generates embeddings
        // Uses act_on because it's stateless (broadcasts events, no actor state mutation)
        builder.act_on::<DocChunksQueryResponse>(|agent, envelope| {
            let msg = envelope.message().clone();
            let broker = agent.broker().clone();
            let config = agent.model.config.clone();

            AgentReply::from_async(async move {
                if msg.chunks.is_empty() {
                    tracing::warn!(
                        specifier = %msg.specifier,
                        "Received empty chunk list, skipping vectorization"
                    );
                    return;
                }

                tracing::info!(
                    specifier = %msg.specifier,
                    chunk_count = msg.chunks.len(),
                    batch_size = config.batch_size,
                    "Processing chunks for vectorization"
                );

                // Process chunks in batches
                let batches = create_batches(&msg.chunks, config.batch_size);

                for (batch_idx, batch) in batches.iter().enumerate() {
                    tracing::debug!(
                        specifier = %msg.specifier,
                        batch_idx = batch_idx,
                        batch_size = batch.len(),
                        "Processing batch"
                    );

                    // Generate embeddings for this batch
                    match generate_batch_embeddings(&config, batch).await {
                        Ok(embeddings) => {
                            // Broadcast EmbeddingGenerated for each successful embedding
                            for (chunk, vector) in batch.iter().zip(embeddings.iter()) {
                                broker
                                    .broadcast(EmbeddingGenerated {
                                        specifier: msg.specifier.clone(),
                                        chunk_id: chunk.chunk_id.clone(),
                                        vector: vector.clone(),
                                        model_name: config.model_name.clone(),
                                        model_version: config.model_version.clone(),
                                    })
                                    .await;
                            }
                        }
                        Err(e) => {
                            // Broadcast EmbeddingFailed for each chunk in the failed batch
                            let retryable = e.is_retryable();
                            for chunk in batch.iter() {
                                tracing::error!(
                                    specifier = %msg.specifier,
                                    chunk_id = %chunk.chunk_id,
                                    error = %e,
                                    retryable = retryable,
                                    "Failed to generate embedding"
                                );

                                broker
                                    .broadcast(EmbeddingFailed {
                                        specifier: msg.specifier.clone(),
                                        chunk_id: chunk.chunk_id.clone(),
                                        error: e.to_string(),
                                        retryable,
                                    })
                                    .await;
                            }
                        }
                    }
                }
            })
        });

        let handle = builder.start().await;

        // Subscribe to both events through the broker
        handle.subscribe::<DocumentationChunked>().await;
        handle.subscribe::<DocChunksQueryResponse>().await;

        Ok(handle)
    }
}

/// Creates batches of chunks for efficient API processing
///
/// Splits chunks into batches of the specified size, respecting the maximum
/// batch size limit (2048 for OpenAI API).
///
/// # Arguments
///
/// * `chunks` - All documentation chunks to batch
/// * `batch_size` - Desired batch size (must be <= 2048)
///
/// # Returns
///
/// Returns a vector of batches, where each batch is a slice of chunks
fn create_batches(chunks: &[DocChunkData], batch_size: usize) -> Vec<Vec<DocChunkData>> {
    let effective_batch_size = batch_size.min(2048);

    chunks
        .chunks(effective_batch_size)
        .map(|chunk_slice| chunk_slice.to_vec())
        .collect()
}

/// Generates embeddings for a batch of chunks using the embedding API
///
/// Calls the OpenAI-compatible embedding API with multiple inputs for efficient
/// batch processing. Returns vectors in the same order as input chunks.
///
/// # Arguments
///
/// * `config` - Vectorization configuration with API settings
/// * `chunks` - Batch of chunks to embed (up to 2048)
///
/// # Returns
///
/// Returns a vector of embedding vectors, one per input chunk, in order
///
/// # Errors
///
/// Returns an error if:
/// - Batch size exceeds 2048
/// - API request fails
/// - Response parsing fails
/// - Vector dimensions don't match configuration
async fn generate_batch_embeddings(
    config: &VectorizeConfig,
    chunks: &[DocChunkData],
) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    // Validate batch size
    if chunks.len() > 2048 {
        return Err(EmbeddingError::BatchSizeExceeded {
            actual: chunks.len(),
            max: 2048,
        });
    }

    if chunks.is_empty() {
        return Ok(vec![]);
    }

    // Get API key from environment
    let api_key = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("EMBEDDING_API_KEY"))
        .map_err(|_| EmbeddingError::MissingApiKey)?;

    // Build HTTP client with timeout
    let timeout = Duration::from_secs(config.request_timeout_secs);
    let client = Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| EmbeddingError::RequestFailed(e.to_string()))?;

    // Extract text from chunks for batch request
    let texts: Vec<String> = chunks.iter().map(|chunk| chunk.content.clone()).collect();

    // Create batch API request
    let request_body = EmbeddingRequest {
        input: texts,
        model: config.model_name.clone(),
        encoding_format: "float".to_string(),
    };

    // Make API request (NO retry logic - RetryCoordinator handles retries)
    let response = client
        .post(&config.api_endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| EmbeddingError::RequestFailed(e.to_string()))?;

    // Check response status
    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        return Err(EmbeddingError::ApiError {
            status: status.as_u16(),
            message: error_text,
        });
    }

    // Parse JSON response
    let embedding_response: EmbeddingResponse = response
        .json()
        .await
        .map_err(|e| EmbeddingError::ParseError(e.to_string()))?;

    // Validate response data
    if embedding_response.data.is_empty() {
        return Err(EmbeddingError::InvalidResponse(
            "No embedding data in response".to_string(),
        ));
    }

    if embedding_response.data.len() != chunks.len() {
        return Err(EmbeddingError::InvalidResponse(format!(
            "Expected {} embeddings, got {}",
            chunks.len(),
            embedding_response.data.len()
        )));
    }

    // Extract and validate embeddings
    let mut embeddings = Vec::with_capacity(embedding_response.data.len());

    for embedding_data in &embedding_response.data {
        // Verify dimension matches configuration
        if embedding_data.embedding.len() != config.vector_dimension {
            return Err(EmbeddingError::DimensionMismatch {
                expected: config.vector_dimension,
                actual: embedding_data.embedding.len(),
            });
        }

        embeddings.push(embedding_data.embedding.clone());
    }

    // Log token usage
    tracing::debug!(
        batch_size = chunks.len(),
        prompt_tokens = embedding_response.usage.prompt_tokens,
        total_tokens = embedding_response.usage.total_tokens,
        model = embedding_response.model,
        "Successfully generated batch embeddings"
    );

    Ok(embeddings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_batches_single_batch() {
        let chunks: Vec<DocChunkData> = (0..50)
            .map(|i| DocChunkData {
                chunk_id: format!("chunk_{}", i),
                chunk_index: i,
                content: format!("Content {}", i),
                content_type: "text".to_string(),
                source_file: "test.md".to_string(),
                token_count: 100,
                char_count: 500,
            })
            .collect();

        let batches = create_batches(&chunks, 100);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 50);
    }

    #[test]
    fn test_create_batches_multiple_batches() {
        let chunks: Vec<DocChunkData> = (0..250)
            .map(|i| DocChunkData {
                chunk_id: format!("chunk_{}", i),
                chunk_index: i,
                content: format!("Content {}", i),
                content_type: "text".to_string(),
                source_file: "test.md".to_string(),
                token_count: 100,
                char_count: 500,
            })
            .collect();

        let batches = create_batches(&chunks, 100);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 100);
        assert_eq!(batches[1].len(), 100);
        assert_eq!(batches[2].len(), 50);
    }

    #[test]
    fn test_create_batches_respects_max_size() {
        let chunks: Vec<DocChunkData> = (0..3000)
            .map(|i| DocChunkData {
                chunk_id: format!("chunk_{}", i),
                chunk_index: i,
                content: format!("Content {}", i),
                content_type: "text".to_string(),
                source_file: "test.md".to_string(),
                token_count: 100,
                char_count: 500,
            })
            .collect();

        // Request size larger than max - should be clamped to 2048
        let batches = create_batches(&chunks, 3000);
        assert_eq!(batches[0].len(), 2048);
    }

    #[test]
    fn test_create_batches_empty() {
        let chunks: Vec<DocChunkData> = vec![];
        let batches = create_batches(&chunks, 100);
        assert_eq!(batches.len(), 0);
    }

    #[test]
    fn test_embedding_error_is_retryable_network() {
        let error = EmbeddingError::RequestFailed("Connection timeout".to_string());
        assert!(error.is_retryable());
    }

    #[test]
    fn test_embedding_error_is_retryable_rate_limit() {
        let error = EmbeddingError::ApiError {
            status: 429,
            message: "Rate limit exceeded".to_string(),
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn test_embedding_error_is_retryable_server_error() {
        let error = EmbeddingError::ApiError {
            status: 500,
            message: "Internal server error".to_string(),
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn test_embedding_error_not_retryable_auth() {
        let error = EmbeddingError::ApiError {
            status: 401,
            message: "Unauthorized".to_string(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_embedding_error_not_retryable_bad_request() {
        let error = EmbeddingError::ApiError {
            status: 400,
            message: "Bad request".to_string(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_embedding_error_not_retryable_missing_key() {
        let error = EmbeddingError::MissingApiKey;
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_embedding_error_not_retryable_dimension_mismatch() {
        let error = EmbeddingError::DimensionMismatch {
            expected: 1536,
            actual: 384,
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_embedding_error_not_retryable_batch_size() {
        let error = EmbeddingError::BatchSizeExceeded {
            actual: 3000,
            max: 2048,
        };
        assert!(!error.is_retryable());
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
        assert_eq!(actor.config.batch_size, config.batch_size);
    }
}
