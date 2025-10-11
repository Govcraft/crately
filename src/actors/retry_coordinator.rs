//! RetryCoordinator actor for managing retry attempts across pipeline stages
//!
//! This actor centralizes retry logic for all pipeline errors, tracking active retry attempts,
//! calculating backoff delays, and broadcasting retry events when appropriate.

use crate::crate_specifier::CrateSpecifier;
use crate::errors::PipelineError;
use crate::messages::RetryCrateProcessing;
use crate::retry_policy::RetryPolicy;
use acton_reactive::prelude::*;
use std::collections::HashMap;
use tokio::time::sleep;

/// RetryCoordinator actor model
///
/// Manages retry attempts for pipeline errors with exponential backoff and jitter.
/// Tracks retry history to prevent infinite loops and enforces maximum attempt limits.
#[acton_actor]
pub struct RetryCoordinator {
    /// Retry policy configuration
    policy: RetryPolicy,

    /// Active retry attempts per crate (specifier -> current attempt number)
    active_retries: HashMap<CrateSpecifier, u32>,
}

/// Schedule a retry attempt for a failed pipeline operation
#[acton_message(raw)]
pub struct ScheduleRetry {
    /// The crate that failed processing
    pub specifier: CrateSpecifier,

    /// The features requested for this crate
    pub features: Vec<String>,

    /// The error that occurred
    pub error: PipelineError,
}

impl RetryCoordinator {
    /// Spawns a new RetryCoordinator actor
    ///
    /// # Arguments
    ///
    /// * `runtime` - The agent runtime for actor management
    /// * `policy` - Retry policy configuration
    ///
    /// # Returns
    ///
    /// A handle to the spawned actor
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crately::actors::retry_coordinator::RetryCoordinator;
    /// use crately::retry_policy::RetryPolicy;
    /// use acton_reactive::prelude::*;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut runtime = ActonApp::launch();
    /// let policy = RetryPolicy::default();
    /// let handle = RetryCoordinator::spawn(&mut runtime, policy).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn spawn(
        runtime: &mut AgentRuntime,
        policy: RetryPolicy,
    ) -> anyhow::Result<AgentHandle> {
        let mut builder = runtime.new_agent::<RetryCoordinator>().await;

        // Initialize with custom policy
        builder.model = Self {
            policy,
            active_retries: HashMap::new(),
        };

        // Handle ScheduleRetry messages
        builder.mutate_on::<ScheduleRetry>(|agent, envelope| {
            let msg = envelope.message().clone();
            let specifier = msg.specifier.clone();

            // Check if error is retryable
            if !msg.error.is_retryable() {
                tracing::debug!(
                    specifier = %specifier,
                    error = %msg.error,
                    "Error is not retryable, skipping retry"
                );
                return AgentReply::immediate();
            }

            // Get current attempt count
            let current_attempt = agent
                .model
                .active_retries
                .get(&specifier)
                .copied()
                .unwrap_or(0)
                + 1;

            // Check if we should retry
            if !agent.model.policy.should_retry(current_attempt) {
                tracing::warn!(
                    specifier = %specifier,
                    current_attempt = current_attempt,
                    max_attempts = agent.model.policy.max_attempts,
                    "Max retry attempts reached, giving up"
                );
                // Clean up tracking
                agent.model.active_retries.remove(&specifier);
                return AgentReply::immediate();
            }

            // Update retry count
            agent
                .model
                .active_retries
                .insert(specifier.clone(), current_attempt);

            // Calculate delay with exponential backoff and jitter
            let delay = agent.model.policy.calculate_delay(current_attempt);
            let max_attempts = agent.model.policy.max_attempts;

            tracing::info!(
                specifier = %specifier,
                current_attempt = current_attempt,
                max_attempts = max_attempts,
                delay_ms = delay.as_millis(),
                "Scheduling retry attempt"
            );

            // Clone what we need for async block
            let broker = agent.broker().clone();

            // Return async reply that waits and broadcasts
            AgentReply::from_async(async move {
                sleep(delay).await;

                tracing::debug!(
                    specifier = %specifier,
                    "Retry delay elapsed, broadcasting retry event"
                );

                // Broadcast retry event
                broker
                    .broadcast(RetryCrateProcessing {
                        specifier,
                        features: msg.features,
                        retry_attempt: current_attempt,
                    })
                    .await;
            })
        });

        Ok(builder.start().await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::DownloadError;
    use std::str::FromStr;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_spawn_retry_coordinator() {
        let mut runtime = ActonApp::launch();
        let policy = RetryPolicy::default();

        let _handle = RetryCoordinator::spawn(&mut runtime, policy)
            .await
            .expect("Failed to spawn RetryCoordinator");

        // Clean shutdown
        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_schedule_retry_non_retryable_error() {
        let mut runtime = ActonApp::launch();
        let policy = RetryPolicy::default();

        let handle = RetryCoordinator::spawn(&mut runtime, policy)
            .await
            .expect("Failed to spawn RetryCoordinator");

        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();

        // CrateNotFound is not retryable
        let error = PipelineError::Download(DownloadError::CrateNotFound {
            specifier: specifier.clone(),
        });

        handle
            .send(ScheduleRetry {
                specifier,
                features: vec![],
                error,
            })
            .await;

        // Give time for processing
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Clean shutdown
        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_schedule_retry_retryable_error() {
        let mut runtime = ActonApp::launch();
        let policy = RetryPolicy {
            max_attempts: 2,
            initial_delay_ms: 10, // Short delay for testing
            max_delay_ms: 100,
            backoff_multiplier: 2.0,
            jitter_factor: 0.0,
        };

        let handle = RetryCoordinator::spawn(&mut runtime, policy)
            .await
            .expect("Failed to spawn RetryCoordinator");

        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();

        // NetworkFailure with retryable=true
        let error = PipelineError::Download(DownloadError::NetworkFailure {
            specifier: specifier.clone(),
            details: "connection timeout".to_string(),
            retryable: true,
        });

        handle
            .send(ScheduleRetry {
                specifier,
                features: vec!["macros".to_string()],
                error,
            })
            .await;

        // Give time for processing
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Clean shutdown
        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_max_retries_reached() {
        let mut runtime = ActonApp::launch();
        let policy = RetryPolicy {
            max_attempts: 1,
            initial_delay_ms: 10,
            max_delay_ms: 100,
            backoff_multiplier: 2.0,
            jitter_factor: 0.0,
        };

        let handle = RetryCoordinator::spawn(&mut runtime, policy)
            .await
            .expect("Failed to spawn RetryCoordinator");

        let specifier = CrateSpecifier::from_str("hyper@1.0.0").unwrap();

        // First retry
        let error1 = PipelineError::Download(DownloadError::Timeout {
            specifier: specifier.clone(),
            timeout_secs: 30,
        });

        handle
            .send(ScheduleRetry {
                specifier: specifier.clone(),
                features: vec![],
                error: error1,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(20)).await;

        // Second retry - should be rejected
        let error2 = PipelineError::Download(DownloadError::Timeout {
            specifier: specifier.clone(),
            timeout_secs: 30,
        });

        handle
            .send(ScheduleRetry {
                specifier,
                features: vec![],
                error: error2,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(20)).await;

        // Clean shutdown
        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[test]
    fn test_retry_coordinator_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<RetryCoordinator>();
        assert_sync::<RetryCoordinator>();
    }
}
