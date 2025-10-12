//! CrateCoordinatorActor - stateful processing state tracker for pipeline coordination
//!
//! This actor tracks the processing state of all active crates moving through the pipeline,
//! coordinates state transitions, handles retry logic, broadcasts completion events, and
//! implements timeout detection for stuck pipelines.

use crate::actors::config::CoordinatorConfig;
use crate::crate_specifier::CrateSpecifier;
use crate::messages::{
    CheckProcessingTimeouts, CrateDownloadFailed, CrateDownloaded, CrateProcessingComplete,
    CrateProcessingFailed, CrateReceived, DocumentationChunked, DocumentationExtracted,
    DocumentationExtractionFailed, DocumentationVectorized, RetryCrateProcessing,
};
use acton_reactive::prelude::*;
use std::collections::HashMap;
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
}

impl CrateCoordinatorActor {
    /// Spawns a new CrateCoordinatorActor
    ///
    /// This actor subscribes to all pipeline events to track processing state,
    /// coordinates state transitions, handles retries with exponential backoff,
    /// and broadcasts CrateProcessingComplete when all stages finish successfully.
    ///
    /// # Arguments
    ///
    /// * `runtime` - The agent runtime for actor management
    /// * `config` - Coordinator configuration
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
    /// let handle = CrateCoordinatorActor::spawn(&mut runtime, config).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(runtime))]
    pub async fn spawn(
        runtime: &mut AgentRuntime,
        config: CoordinatorConfig,
    ) -> anyhow::Result<AgentHandle> {
        let mut builder = runtime.new_agent::<CrateCoordinatorActor>().await;

        // Initialize with custom configuration
        builder.model = Self {
            config,
            processing_states: HashMap::new(),
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

        // Handle CrateDownloadFailed - handle download failure
        builder.mutate_on::<CrateDownloadFailed>(|agent, envelope| {
            let msg = envelope.message().clone();
            let specifier = msg.specifier.clone();
            let error = msg.error_message.clone();

            if let Some(state) = agent.model.processing_states.get_mut(&specifier) {
                state.update_with_error(ProcessingStatus::DownloadFailed, error.clone());

                // Verify we're in a failure state using is_failure() utility
                debug_assert!(state.status.is_failure(), "DownloadFailed should be a failure state");

                state.increment_retry();

                let retry_count = state.retry_count;
                let max_retries = agent.model.config.max_retries;
                let features = state.features.clone();

                if retry_count < max_retries {
                    // Schedule retry with exponential backoff
                    let delay_secs = agent.model.config.retry_backoff_secs * (2_u64.pow(retry_count));
                    let delay = Duration::from_secs(delay_secs);

                    warn!(
                        specifier = %specifier,
                        retry_count = retry_count,
                        max_retries = max_retries,
                        delay_secs = delay_secs,
                        "Scheduling download retry"
                    );

                    let broker = agent.broker().clone();

                    return AgentReply::from_async(async move {
                        tokio::time::sleep(delay).await;

                        broker
                            .broadcast(RetryCrateProcessing {
                                specifier,
                                features,
                                retry_attempt: retry_count,
                            })
                            .await;
                    });
                } else {
                    // Max retries exceeded
                    state.update_status(ProcessingStatus::Failed);
                    error!(
                        specifier = %specifier,
                        retry_count = retry_count,
                        max_retries = max_retries,
                        "Max download retries exceeded, marking as failed"
                    );

                    let broker = agent.broker().clone();

                    return AgentReply::from_async(async move {
                        broker
                            .broadcast(CrateProcessingFailed {
                                specifier,
                                stage: "downloading".to_string(),
                                error,
                            })
                            .await;
                    });
                }
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

        // Handle DocumentationExtractionFailed - handle extraction failure
        builder.mutate_on::<DocumentationExtractionFailed>(|agent, envelope| {
            let msg = envelope.message().clone();
            let specifier = msg.specifier.clone();
            let error = msg.error_message.clone();

            if let Some(state) = agent.model.processing_states.get_mut(&specifier) {
                state.update_with_error(ProcessingStatus::ExtractionFailed, error.clone());

                // Verify we're in a failure state using is_failure() utility
                debug_assert!(state.status.is_failure(), "ExtractionFailed should be a failure state");

                state.increment_retry();

                let retry_count = state.retry_count;
                let max_retries = agent.model.config.max_retries;
                let features = state.features.clone();

                if retry_count < max_retries {
                    // Schedule retry with exponential backoff
                    let delay_secs = agent.model.config.retry_backoff_secs * (2_u64.pow(retry_count));
                    let delay = Duration::from_secs(delay_secs);

                    warn!(
                        specifier = %specifier,
                        retry_count = retry_count,
                        max_retries = max_retries,
                        delay_secs = delay_secs,
                        "Scheduling extraction retry"
                    );

                    let broker = agent.broker().clone();

                    return AgentReply::from_async(async move {
                        tokio::time::sleep(delay).await;

                        broker
                            .broadcast(RetryCrateProcessing {
                                specifier,
                                features,
                                retry_attempt: retry_count,
                            })
                            .await;
                    });
                } else {
                    // Max retries exceeded
                    state.update_status(ProcessingStatus::Failed);
                    error!(
                        specifier = %specifier,
                        retry_count = retry_count,
                        max_retries = max_retries,
                        "Max extraction retries exceeded, marking as failed"
                    );

                    let broker = agent.broker().clone();

                    return AgentReply::from_async(async move {
                        broker
                            .broadcast(CrateProcessingFailed {
                                specifier,
                                stage: "extracting".to_string(),
                                error,
                            })
                            .await;
                    });
                }
            }

            AgentReply::immediate()
        });

        // Handle DocumentationChunked - update to Chunked and transition to Vectorizing
        builder.mutate_on::<DocumentationChunked>(|agent, envelope| {
            let msg = envelope.message();
            let specifier = &msg.specifier;

            if let Some(state) = agent.model.processing_states.get_mut(specifier) {
                state.update_status(ProcessingStatus::Chunked);
                debug!(
                    specifier = %specifier,
                    status = ?state.status,
                    "Chunking completed, preparing for vectorization"
                );
                // Transition to Vectorizing status - next stage
                state.update_status(ProcessingStatus::Vectorizing);
            }

            AgentReply::immediate()
        });

        // Handle DocumentationVectorized - update to Vectorized and broadcast Complete
        builder.mutate_on::<DocumentationVectorized>(|agent, envelope| {
            let msg = envelope.message();
            let specifier = &msg.specifier;

            if let Some(state) = agent.model.processing_states.get_mut(specifier) {
                // First transition to Vectorized status
                state.update_status(ProcessingStatus::Vectorized);
                debug!(
                    specifier = %specifier,
                    status = ?state.status,
                    "Vectorization completed"
                );

                // Then transition to Complete
                state.update_status(ProcessingStatus::Complete);

                let total_duration_ms = state.total_duration_ms();
                let features = state.features.clone();
                let specifier_clone = specifier.clone();

                info!(
                    specifier = %specifier,
                    total_duration_ms = total_duration_ms,
                    "Crate processing completed successfully"
                );

                let broker = agent.broker().clone();

                // Broadcast completion event
                return AgentReply::from_async(async move {
                    broker
                        .broadcast(CrateProcessingComplete {
                            specifier: specifier_clone,
                            features,
                            total_duration_ms,
                            stages_completed: 4, // download → extract → chunk → vectorize
                        })
                        .await;
                });
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

        Ok(builder.start().await)
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
        let mut runtime = ActonApp::launch();
        let config = CoordinatorConfig::default();

        let _handle = CrateCoordinatorActor::spawn(&mut runtime, config)
            .await
            .expect("Failed to spawn CrateCoordinatorActor");

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_coordinator_tracks_crate_received() {
        let mut runtime = ActonApp::launch();
        let config = CoordinatorConfig::default();

        let handle = CrateCoordinatorActor::spawn(&mut runtime, config)
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
        tokio::time::sleep(Duration::from_millis(10)).await;

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
}
