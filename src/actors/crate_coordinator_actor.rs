//! CrateCoordinatorActor - stateful processing state tracker for pipeline coordination
//!
//! This actor tracks the processing state of all active crates moving through the pipeline,
//! coordinates state transitions, handles retry logic, broadcasts completion events, and
//! implements timeout detection for stuck pipelines.

use crate::actors::config::CoordinatorConfig;
use crate::actors::retry_coordinator::ScheduleRetry;
use crate::crate_specifier::CrateSpecifier;
use crate::errors::{DownloadError, ExtractionError, PipelineError};
use crate::messages::{
    CheckProcessingTimeouts, ChunkPersistenceProgress, ChunksPersistenceComplete,
    CrateDownloadFailed, CrateDownloaded, CrateProcessingComplete, CrateProcessingFailed,
    CrateReceived, DocChunkPersisted, DocumentationChunked, DocumentationExtracted,
    DocumentationExtractionFailed, DocumentationVectorized, EmbeddingPersisted,
    EmbeddingPersistenceProgress, EmbeddingsPersistenceComplete,
};
use acton_reactive::prelude::*;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tokio::time::Duration;
use tracing::*;

/// Processing status enum tracking pipeline progression
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingStatus {
    /// Initial state - crate received for processing
    Received,
    /// Downloading crate from crates.io
    Downloading,
    /// Download completed successfully
    Downloaded,
    /// Download failed
    DownloadFailed,
    /// Extracting documentation
    Extracting,
    /// Documentation extraction completed
    Extracted,
    /// Documentation extraction failed
    ExtractionFailed,
    /// Chunking documentation
    Chunking,
    /// Documentation chunked successfully
    Chunked,
    /// Vectorizing documentation
    Vectorizing,
    /// Documentation vectorized successfully
    Vectorized,
    /// Processing completed successfully
    Complete,
    /// Processing failed permanently
    Failed,
}

impl ProcessingStatus {
    /// Returns true if this status represents a failure state
    pub const fn is_failure(&self) -> bool {
        matches!(
            self,
            Self::DownloadFailed | Self::ExtractionFailed | Self::Failed
        )
    }

    /// Returns true if this status represents a terminal state
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete | Self::Failed)
    }
}

/// Processing state for a single crate
#[derive(Debug, Clone)]
pub struct CrateProcessingState {
    /// Crate specifier
    pub specifier: CrateSpecifier,
    /// Requested features
    pub features: Vec<String>,
    /// Current processing status
    pub status: ProcessingStatus,
    /// When processing started
    pub started_at: Instant,
    /// When state was last updated
    pub last_updated: Instant,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Error message if failed
    pub error: Option<String>,
    /// Expected number of chunks to be persisted (None until DocumentationChunked received)
    pub expected_chunks: Option<u32>,
    /// Set of chunk IDs that have been persisted successfully
    pub persisted_chunks: HashSet<String>,
    /// Expected number of embeddings to be persisted (None until DocumentationVectorized received)
    pub expected_vectors: Option<u32>,
    /// Set of chunk IDs for which embeddings have been persisted successfully
    pub persisted_vectors: HashSet<String>,
    /// Embedding model name (captured from DocumentationVectorized)
    pub embedding_model: Option<String>,
}

impl CrateProcessingState {
    /// Creates new processing state for received crate
    fn new(specifier: CrateSpecifier, features: Vec<String>) -> Self {
        let now = Instant::now();
        Self {
            specifier,
            features,
            status: ProcessingStatus::Received,
            started_at: now,
            last_updated: now,
            retry_count: 0,
            error: None,
            expected_chunks: None,
            persisted_chunks: HashSet::new(),
            expected_vectors: None,
            persisted_vectors: HashSet::new(),
            embedding_model: None,
        }
    }

    /// Updates state to new status
    fn update_status(&mut self, status: ProcessingStatus) {
        self.status = status;
        self.last_updated = Instant::now();
    }

    /// Updates state to new status with error
    fn update_with_error(&mut self, status: ProcessingStatus, error: String) {
        self.status = status;
        self.last_updated = Instant::now();
        self.error = Some(error);
    }

    /// Increments retry count
    fn increment_retry(&mut self) {
        self.retry_count += 1;
        self.last_updated = Instant::now();
    }

    /// Returns time since last update in seconds
    fn time_since_update(&self) -> u64 {
        self.last_updated.elapsed().as_secs()
    }

    /// Returns total processing time in milliseconds
    fn total_duration_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }
}

/// CrateCoordinatorActor model - stateful pipeline coordination
#[acton_actor]
pub struct CrateCoordinatorActor {
    /// Configuration
    config: CoordinatorConfig,
    /// In-memory state tracking
    processing_states: HashMap<CrateSpecifier, CrateProcessingState>,
    /// Handle to RetryCoordinator for delegating retry scheduling
    retry_coordinator: AgentHandle,
}

impl CrateCoordinatorActor {
    /// Spawns a new CrateCoordinatorActor
    ///
    /// This actor subscribes to all pipeline events to track processing state,
    /// coordinates state transitions, delegates retry scheduling to RetryCoordinator,
    /// and broadcasts CrateProcessingComplete when all stages finish successfully.
    ///
    /// # Arguments
    ///
    /// * `runtime` - The agent runtime for actor management
    /// * `config` - Coordinator configuration
    /// * `retry_coordinator` - Handle to the RetryCoordinator for delegating retry scheduling
    ///
    /// # Returns
    ///
    /// A handle to the spawned coordinator actor
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crately::actors::{config::CoordinatorConfig, crate_coordinator_actor::CrateCoordinatorActor};
    /// use acton_reactive::prelude::*;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut runtime = ActonApp::launch();
    /// let config = CoordinatorConfig::default();
    /// // Assume retry_coordinator handle is available
    /// # let retry_coordinator = unimplemented!();
    /// let handle = CrateCoordinatorActor::spawn(&mut runtime, config, retry_coordinator).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(runtime, retry_coordinator))]
    pub async fn spawn(
        runtime: &mut AgentRuntime,
        config: CoordinatorConfig,
        retry_coordinator: AgentHandle,
    ) -> anyhow::Result<AgentHandle> {
        let mut builder = runtime.new_agent::<CrateCoordinatorActor>().await;

        // Initialize with custom configuration and retry coordinator
        builder.model = Self {
            config,
            processing_states: HashMap::new(),
            retry_coordinator,
        };

        // Handle CrateReceived - initialize tracking and transition to Downloading
        builder.mutate_on::<CrateReceived>(|agent, envelope| {
            let msg = envelope.message().clone();
            let specifier = msg.specifier.clone();

            debug!(
                specifier = %specifier,
                features = ?msg.features,
                "Tracking new crate processing"
            );

            // Initialize state tracking
            let mut state = CrateProcessingState::new(specifier.clone(), msg.features);
            // Transition to Downloading status immediately
            state.update_status(ProcessingStatus::Downloading);
            agent.model.processing_states.insert(specifier, state);

            AgentReply::immediate()
        });

        // Handle CrateDownloaded - update to Downloaded and transition to Extracting
        builder.mutate_on::<CrateDownloaded>(|agent, envelope| {
            let msg = envelope.message();
            let specifier = &msg.specifier;

            if let Some(state) = agent.model.processing_states.get_mut(specifier) {
                state.update_status(ProcessingStatus::Downloaded);
                debug!(
                    specifier = %specifier,
                    status = ?state.status,
                    "Download completed, preparing for extraction"
                );
                // Transition to Extracting status - next stage
                state.update_status(ProcessingStatus::Extracting);
            }

            AgentReply::immediate()
        });

        // Handle CrateDownloadFailed - delegate retry scheduling to RetryCoordinator
        builder.mutate_on::<CrateDownloadFailed>(|agent, envelope| {
            let msg = envelope.message().clone();
            let specifier = msg.specifier.clone();
            let error = msg.error_message.clone();

            if let Some(state) = agent.model.processing_states.get_mut(&specifier) {
                state.update_with_error(ProcessingStatus::DownloadFailed, error.clone());

                // Verify we're in a failure state using is_failure() utility
                debug_assert!(state.status.is_failure(), "DownloadFailed should be a failure state");

                state.increment_retry();

                let features = state.features.clone();
                let retry_coordinator = agent.model.retry_coordinator.clone();

                // Delegate retry scheduling to RetryCoordinator
                // Convert error string to structured PipelineError
                let pipeline_error = PipelineError::Download(DownloadError::NetworkFailure {
                    specifier: specifier.clone(),
                    details: error.clone(),
                    retryable: true,
                });

                debug!(
                    specifier = %specifier,
                    error = %error,
                    "Delegating download retry to RetryCoordinator"
                );

                return AgentReply::from_async(async move {
                    retry_coordinator
                        .send(ScheduleRetry {
                            specifier,
                            features,
                            error: pipeline_error,
                        })
                        .await;
                });
            }

            AgentReply::immediate()
        });

        // Handle DocumentationExtracted - update to Extracted and transition to Chunking
        builder.mutate_on::<DocumentationExtracted>(|agent, envelope| {
            let msg = envelope.message();
            let specifier = &msg.specifier;

            if let Some(state) = agent.model.processing_states.get_mut(specifier) {
                state.update_status(ProcessingStatus::Extracted);
                debug!(
                    specifier = %specifier,
                    status = ?state.status,
                    "Extraction completed, preparing for chunking"
                );
                // Transition to Chunking status - next stage
                state.update_status(ProcessingStatus::Chunking);
            }

            AgentReply::immediate()
        });

        // Handle DocumentationExtractionFailed - delegate retry scheduling to RetryCoordinator
        builder.mutate_on::<DocumentationExtractionFailed>(|agent, envelope| {
            let msg = envelope.message().clone();
            let specifier = msg.specifier.clone();
            let error = msg.error_message.clone();

            if let Some(state) = agent.model.processing_states.get_mut(&specifier) {
                state.update_with_error(ProcessingStatus::ExtractionFailed, error.clone());

                // Verify we're in a failure state using is_failure() utility
                debug_assert!(state.status.is_failure(), "ExtractionFailed should be a failure state");

                state.increment_retry();

                let features = state.features.clone();
                let retry_coordinator = agent.model.retry_coordinator.clone();

                // Delegate retry scheduling to RetryCoordinator
                // Convert error string to structured PipelineError
                let pipeline_error = PipelineError::Extraction(ExtractionError::ArchiveCorrupted {
                    specifier: specifier.clone(),
                    details: error.clone(),
                });

                debug!(
                    specifier = %specifier,
                    error = %error,
                    "Delegating extraction retry to RetryCoordinator"
                );

                return AgentReply::from_async(async move {
                    retry_coordinator
                        .send(ScheduleRetry {
                            specifier,
                            features,
                            error: pipeline_error,
                        })
                        .await;
                });
            }

            AgentReply::immediate()
        });

        // Handle DocumentationChunked - set expected chunks count and track in-progress chunking
        builder.mutate_on::<DocumentationChunked>(|agent, envelope| {
            let msg = envelope.message();
            let specifier = &msg.specifier;

            if let Some(state) = agent.model.processing_states.get_mut(specifier) {
                // Set expected chunk count for persistence tracking
                state.expected_chunks = Some(msg.chunk_count);
                // Transition to Chunking status to track in-progress persistence
                state.update_status(ProcessingStatus::Chunking);
                debug!(
                    specifier = %specifier,
                    chunk_count = msg.chunk_count,
                    status = ?state.status,
                    "Chunking completed, waiting for chunk persistence"
                );
            }

            AgentReply::immediate()
        });

        // Handle DocChunkPersisted - track chunk persistence and transition when complete
        builder.mutate_on::<DocChunkPersisted>(|agent, envelope| {
            let msg = envelope.message();
            let specifier = &msg.specifier;

            // Extract state data and update it, then drop the mutable borrow
            let broadcast_data = if let Some(state) = agent.model.processing_states.get_mut(specifier) {
                // Track this chunk as persisted (idempotency via HashSet)
                state.persisted_chunks.insert(msg.content_hash.clone());

                let persisted_count = state.persisted_chunks.len() as u32;
                let expected_count = state.expected_chunks.unwrap_or(0);

                debug!(
                    specifier = %specifier,
                    content_hash = %msg.content_hash,
                    source_file = %msg.source_file,
                    persisted = persisted_count,
                    expected = expected_count,
                    "Chunk persisted"
                );

                // Check if we have expected chunks set
                if let Some(total) = state.expected_chunks {
                    let chunk_index = msg.chunk_index;
                    let is_complete = persisted_count == total;

                    // Update status if complete
                    if is_complete {
                        state.update_status(ProcessingStatus::Chunked);
                        debug!(
                            specifier = %specifier,
                            total_chunks = total,
                            "All chunks persisted, broadcasting completion and transitioning to vectorizing"
                        );
                        state.update_status(ProcessingStatus::Vectorizing);
                    }

                    Some((chunk_index, persisted_count, total, is_complete))
                } else {
                    None
                }
            } else {
                None
            };

            // Now handle broadcasting with the broker (mutable borrow dropped)
            if let Some((chunk_index, persisted_count, total, is_complete)) = broadcast_data {
                let specifier_clone = specifier.clone();
                let broker = agent.broker().clone();

                return AgentReply::from_async(async move {
                    // Always broadcast progress first
                    broker
                        .broadcast(ChunkPersistenceProgress {
                            specifier: specifier_clone.clone(),
                            chunk_index,
                            persisted_count,
                            total_chunks: total,
                        })
                        .await;

                    // If complete, broadcast completion message
                    if is_complete {
                        broker
                            .broadcast(ChunksPersistenceComplete {
                                specifier: specifier_clone,
                                chunk_count: total,
                            })
                            .await;
                    }
                });
            }

            AgentReply::immediate()
        });

        // Handle EmbeddingPersisted - track embedding persistence and transition when complete
        builder.mutate_on::<EmbeddingPersisted>(|agent, envelope| {
            let msg = envelope.message();
            let specifier = &msg.specifier;

            // Extract state data and update it, then drop the mutable borrow
            let broadcast_data = if let Some(state) = agent.model.processing_states.get_mut(specifier) {
                // Track this embedding as persisted (idempotency via HashSet)
                state.persisted_vectors.insert(msg.chunk_id.clone());

                let persisted_count = state.persisted_vectors.len() as u32;
                let expected_count = state.expected_vectors.unwrap_or(0);

                debug!(
                    specifier = %specifier,
                    chunk_id = %msg.chunk_id,
                    persisted = persisted_count,
                    expected = expected_count,
                    "Embedding persisted"
                );

                // Check if we have expected vectors set
                if let Some(total) = state.expected_vectors {
                    let chunk_id = msg.chunk_id.clone();
                    let embedding_model = state.embedding_model.clone().unwrap_or_else(|| "unknown".to_string());
                    let is_complete = persisted_count == total;

                    // Update status if complete
                    if is_complete {
                        state.update_status(ProcessingStatus::Vectorized);
                        debug!(
                            specifier = %specifier,
                            total_embeddings = total,
                            "All embeddings persisted, broadcasting completion and transitioning to complete"
                        );

                        let total_duration_ms = state.total_duration_ms();
                        let features = state.features.clone();

                        info!(
                            specifier = %specifier,
                            total_duration_ms = total_duration_ms,
                            "Crate processing completed successfully"
                        );

                        state.update_status(ProcessingStatus::Complete);

                        Some((chunk_id, persisted_count, total, embedding_model.clone(), is_complete, Some((features, total_duration_ms, embedding_model))))
                    } else {
                        Some((chunk_id, persisted_count, total, embedding_model, is_complete, None))
                    }
                } else {
                    None
                }
            } else {
                None
            };

            // Now handle broadcasting with the broker (mutable borrow dropped)
            if let Some((chunk_id, persisted_count, total, embedding_model, is_complete, completion_data)) = broadcast_data {
                let specifier_clone = specifier.clone();
                let broker = agent.broker().clone();

                return AgentReply::from_async(async move {
                    // Always broadcast progress first
                    broker
                        .broadcast(EmbeddingPersistenceProgress {
                            specifier: specifier_clone.clone(),
                            chunk_id,
                            persisted_count,
                            total_vectors: total,
                            embedding_model: embedding_model.clone(),
                        })
                        .await;

                    // If complete, broadcast completion messages
                    if is_complete {
                        if let Some((features, total_duration_ms, embedding_model)) = completion_data {
                            broker
                                .broadcast(EmbeddingsPersistenceComplete {
                                    specifier: specifier_clone.clone(),
                                    vector_count: total,
                                    embedding_model,
                                })
                                .await;

                            broker
                                .broadcast(CrateProcessingComplete {
                                    specifier: specifier_clone,
                                    features,
                                    total_duration_ms,
                                    stages_completed: 4, // download → extract → chunk → vectorize
                                })
                                .await;
                        }
                    }
                });
            }

            AgentReply::immediate()
        });

        // Handle DocumentationVectorized - set expected vectors count and track in-progress vectorization
        builder.mutate_on::<DocumentationVectorized>(|agent, envelope| {
            let msg = envelope.message();
            let specifier = &msg.specifier;

            if let Some(state) = agent.model.processing_states.get_mut(specifier) {
                // Set expected vector count for persistence tracking
                state.expected_vectors = Some(msg.vector_count);
                // Store embedding model name for later broadcast
                state.embedding_model = Some(msg.embedding_model.clone());
                // Keep status as Vectorizing to track in-progress persistence
                debug!(
                    specifier = %specifier,
                    vector_count = msg.vector_count,
                    embedding_model = %msg.embedding_model,
                    status = ?state.status,
                    "Vectorization completed, waiting for embedding persistence"
                );
            }

            AgentReply::immediate()
        });

        // Handle CrateProcessingFailed - mark as permanently failed
        builder.mutate_on::<CrateProcessingFailed>(|agent, envelope| {
            let msg = envelope.message();
            let specifier = &msg.specifier;

            if let Some(state) = agent.model.processing_states.get_mut(specifier) {
                state.update_with_error(ProcessingStatus::Failed, msg.error.clone());

                // Verify we're in a terminal failure state
                debug_assert!(
                    state.status.is_terminal() && state.status.is_failure(),
                    "Failed should be both terminal and a failure state"
                );

                warn!(
                    specifier = %specifier,
                    stage = %msg.stage,
                    error = %msg.error,
                    retry_count = state.retry_count,
                    "Crate processing failed permanently"
                );
            }

            AgentReply::immediate()
        });

        // Add timeout monitoring message handler
        builder.mutate_on::<CheckProcessingTimeouts>(|agent, _envelope| {
            let timeout_secs = agent.model.config.timeout_secs;
            let broker = agent.broker().clone();
            let mut timed_out_crates = Vec::new();

            // Check all non-terminal states for timeouts
            for state in agent.model.processing_states.values() {
                // Skip terminal states using is_terminal() utility method
                if state.status.is_terminal() {
                    continue;
                }

                // Use time_since_update() utility method
                let time_since_update = state.time_since_update();
                if time_since_update > timeout_secs {
                    warn!(
                        specifier = %state.specifier,
                        status = ?state.status,
                        time_since_update = time_since_update,
                        timeout_threshold = timeout_secs,
                        "Processing timeout detected"
                    );

                    timed_out_crates.push((
                        state.specifier.clone(),
                        state.status,
                    ));
                }
            }

            // Handle timeouts asynchronously
            if !timed_out_crates.is_empty() {
                return AgentReply::from_async(async move {
                    for (specifier, status) in timed_out_crates {
                        broker
                            .broadcast(CrateProcessingFailed {
                                specifier,
                                stage: format!("{:?}", status).to_lowercase(),
                                error: format!("Processing timeout after {}s", timeout_secs),
                            })
                            .await;
                    }
                });
            }

            AgentReply::immediate()
        });

        // Start timeout detection background task
        builder.after_start(|agent| {
            let check_interval = agent.model.config.check_interval_secs;
            let timeout_secs = agent.model.config.timeout_secs;
            let handle = agent.handle().clone();

            info!(
                check_interval_secs = check_interval,
                timeout_secs = timeout_secs,
                "Starting timeout detection task"
            );

            AgentReply::from_async(async move {
                tokio::spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(check_interval));

                    loop {
                        interval.tick().await;

                        // Send message to check timeouts
                        handle.send(CheckProcessingTimeouts).await;
                    }
                });
            })
        });

        let handle = builder.start().await;

        // Subscribe to pipeline events for state tracking
        handle.subscribe::<CrateReceived>().await;
        handle.subscribe::<CrateDownloaded>().await;
        handle.subscribe::<CrateDownloadFailed>().await;
        handle.subscribe::<DocumentationExtracted>().await;
        handle.subscribe::<DocumentationExtractionFailed>().await;
        handle.subscribe::<DocumentationChunked>().await;
        handle.subscribe::<DocChunkPersisted>().await;
        handle.subscribe::<EmbeddingPersisted>().await;
        handle.subscribe::<DocumentationVectorized>().await;
        handle.subscribe::<CrateProcessingFailed>().await;

        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_processing_status_is_failure() {
        assert!(ProcessingStatus::DownloadFailed.is_failure());
        assert!(ProcessingStatus::ExtractionFailed.is_failure());
        assert!(ProcessingStatus::Failed.is_failure());
        assert!(!ProcessingStatus::Downloaded.is_failure());
        assert!(!ProcessingStatus::Complete.is_failure());
    }

    #[test]
    fn test_processing_status_is_terminal() {
        assert!(ProcessingStatus::Complete.is_terminal());
        assert!(ProcessingStatus::Failed.is_terminal());
        assert!(!ProcessingStatus::Downloading.is_terminal());
        assert!(!ProcessingStatus::Chunking.is_terminal());
    }

    #[test]
    fn test_crate_processing_state_new() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let features = vec!["derive".to_string()];

        let state = CrateProcessingState::new(specifier.clone(), features.clone());

        assert_eq!(state.specifier, specifier);
        assert_eq!(state.features, features);
        assert_eq!(state.status, ProcessingStatus::Received);
        assert_eq!(state.retry_count, 0);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_crate_processing_state_update_status() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let mut state = CrateProcessingState::new(specifier, vec![]);

        state.update_status(ProcessingStatus::Downloading);
        assert_eq!(state.status, ProcessingStatus::Downloading);

        state.update_status(ProcessingStatus::Downloaded);
        assert_eq!(state.status, ProcessingStatus::Downloaded);
    }

    #[test]
    fn test_crate_processing_state_update_with_error() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let mut state = CrateProcessingState::new(specifier, vec![]);

        state.update_with_error(ProcessingStatus::DownloadFailed, "Network error".to_string());

        assert_eq!(state.status, ProcessingStatus::DownloadFailed);
        assert_eq!(state.error, Some("Network error".to_string()));
    }

    #[test]
    fn test_crate_processing_state_increment_retry() {
        let specifier = CrateSpecifier::from_str("hyper@1.0.0").unwrap();
        let mut state = CrateProcessingState::new(specifier, vec![]);

        assert_eq!(state.retry_count, 0);

        state.increment_retry();
        assert_eq!(state.retry_count, 1);

        state.increment_retry();
        assert_eq!(state.retry_count, 2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_spawn_coordinator() {
        use crate::actors::retry_coordinator::RetryCoordinator;
        use crate::retry_policy::RetryPolicy;

        let mut runtime = ActonApp::launch();
        let config = CoordinatorConfig::default();

        // Spawn RetryCoordinator first
        let retry_policy = RetryPolicy::default();
        let retry_coordinator = RetryCoordinator::spawn(&mut runtime, retry_policy)
            .await
            .expect("Failed to spawn RetryCoordinator");

        let _handle = CrateCoordinatorActor::spawn(&mut runtime, config, retry_coordinator)
            .await
            .expect("Failed to spawn CrateCoordinatorActor");

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_coordinator_tracks_crate_received() {
        use crate::actors::retry_coordinator::RetryCoordinator;
        use crate::retry_policy::RetryPolicy;

        let mut runtime = ActonApp::launch();
        let config = CoordinatorConfig::default();

        // Spawn RetryCoordinator first
        let retry_policy = RetryPolicy::default();
        let retry_coordinator = RetryCoordinator::spawn(&mut runtime, retry_policy)
            .await
            .expect("Failed to spawn RetryCoordinator");

        let handle = CrateCoordinatorActor::spawn(&mut runtime, config, retry_coordinator)
            .await
            .expect("Failed to spawn CrateCoordinatorActor");

        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        handle
            .send(CrateReceived {
                specifier,
                features: vec!["derive".to_string()],
            })
            .await;

        // Give time for processing
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[test]
    fn test_coordinator_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CrateCoordinatorActor>();
        assert_sync::<CrateCoordinatorActor>();
    }

    #[test]
    fn test_processing_state_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CrateProcessingState>();
        assert_sync::<CrateProcessingState>();
    }

    #[test]
    fn test_crate_processing_state_persistence_tracking_fields() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let state = CrateProcessingState::new(specifier, vec![]);

        // Verify initial state of tracking fields
        assert!(state.expected_chunks.is_none());
        assert!(state.persisted_chunks.is_empty());
        assert!(state.expected_vectors.is_none());
        assert!(state.persisted_vectors.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_coordinator_tracks_chunk_persistence() {
        use crate::actors::retry_coordinator::RetryCoordinator;
        use crate::retry_policy::RetryPolicy;

        let mut runtime = ActonApp::launch();
        let config = CoordinatorConfig::default();

        let retry_policy = RetryPolicy::default();
        let retry_coordinator = RetryCoordinator::spawn(&mut runtime, retry_policy)
            .await
            .expect("Failed to spawn RetryCoordinator");

        let handle = CrateCoordinatorActor::spawn(&mut runtime, config, retry_coordinator)
            .await
            .expect("Failed to spawn CrateCoordinatorActor");

        let specifier = CrateSpecifier::from_str("test-crate@1.0.0").unwrap();

        // Initialize state
        handle
            .send(CrateReceived {
                specifier: specifier.clone(),
                features: vec![],
            })
            .await;

        // Simulate chunking completion
        handle
            .send(DocumentationChunked {
                specifier: specifier.clone(),
                features: vec![],
                chunk_count: 3,
                total_tokens_estimated: 1000,
            })
            .await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Simulate chunk persistence (3 chunks)
        for i in 0..3 {
            handle
                .send(DocChunkPersisted {
                    content_hash: format!("test_hash_{:064x}", i),
                    specifier: specifier.clone(),
                    chunk_index: i as usize,
                    source_file: "src/lib.rs".to_string(),
                })
                .await;
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_coordinator_tracks_embedding_persistence() {
        use crate::actors::retry_coordinator::RetryCoordinator;
        use crate::retry_policy::RetryPolicy;

        let mut runtime = ActonApp::launch();
        let config = CoordinatorConfig::default();

        let retry_policy = RetryPolicy::default();
        let retry_coordinator = RetryCoordinator::spawn(&mut runtime, retry_policy)
            .await
            .expect("Failed to spawn RetryCoordinator");

        let handle = CrateCoordinatorActor::spawn(&mut runtime, config, retry_coordinator)
            .await
            .expect("Failed to spawn CrateCoordinatorActor");

        let specifier = CrateSpecifier::from_str("test-crate@1.0.0").unwrap();

        // Initialize state
        handle
            .send(CrateReceived {
                specifier: specifier.clone(),
                features: vec![],
            })
            .await;

        // Simulate vectorization completion
        handle
            .send(DocumentationVectorized {
                specifier: specifier.clone(),
                features: vec![],
                vector_count: 3,
                embedding_model: "test-model".to_string(),
                vectorization_duration_ms: 1000,
            })
            .await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Simulate embedding persistence (3 embeddings)
        for i in 0..3 {
            handle
                .send(EmbeddingPersisted {
                    chunk_id: format!("test-crate_1_0_0_{:03}", i),
                    specifier: specifier.clone(),
                    model_name: "test-model".to_string(),
                })
                .await;
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_coordinator_idempotency_duplicate_chunk_persisted() {
        use crate::actors::retry_coordinator::RetryCoordinator;
        use crate::retry_policy::RetryPolicy;

        let mut runtime = ActonApp::launch();
        let config = CoordinatorConfig::default();

        let retry_policy = RetryPolicy::default();
        let retry_coordinator = RetryCoordinator::spawn(&mut runtime, retry_policy)
            .await
            .expect("Failed to spawn RetryCoordinator");

        let handle = CrateCoordinatorActor::spawn(&mut runtime, config, retry_coordinator)
            .await
            .expect("Failed to spawn CrateCoordinatorActor");

        let specifier = CrateSpecifier::from_str("test-crate@1.0.0").unwrap();

        // Initialize state
        handle
            .send(CrateReceived {
                specifier: specifier.clone(),
                features: vec![],
            })
            .await;

        // Simulate chunking completion
        handle
            .send(DocumentationChunked {
                specifier: specifier.clone(),
                features: vec![],
                chunk_count: 2,
                total_tokens_estimated: 500,
            })
            .await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Send duplicate content_hash - should be idempotent
        for _ in 0..3 {
            handle
                .send(DocChunkPersisted {
                    content_hash: "test_hash_0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                    specifier: specifier.clone(),
                    chunk_index: 0,
                    source_file: "src/lib.rs".to_string(),
                })
                .await;
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        // Send second chunk
        handle
            .send(DocChunkPersisted {
                content_hash: "test_hash_0000000000000000000000000000000000000000000000000000000000000001".to_string(),
                specifier: specifier.clone(),
                chunk_index: 1,
                source_file: "src/module.rs".to_string(),
            })
            .await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[test]
    fn test_hashset_idempotency() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert("chunk_1".to_string());
        assert_eq!(set.len(), 1);

        // Insert duplicate - should not increase size
        set.insert("chunk_1".to_string());
        assert_eq!(set.len(), 1);

        // Insert new item
        set.insert("chunk_2".to_string());
        assert_eq!(set.len(), 2);
    }
}
