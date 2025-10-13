//! Event broadcast when embedding generation fails for a documentation chunk.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Event broadcast when embedding generation fails for a documentation chunk.
///
/// This message is broadcast by the VectorizerActor when the embedding API call
/// fails or returns an invalid response. The event triggers error handling and
/// potential retry coordination.
///
/// # Fields
///
/// * `specifier` - The crate this failed embedding belongs to
/// * `chunk_id` - Unique identifier for the failed documentation chunk
/// * `error` - Error message describing the failure
/// * `retryable` - Whether this error can be retried (transient vs. permanent)
///
/// # Message Flow
///
/// 1. VectorizerActor calls embedding API and encounters error
/// 2. VectorizerActor classifies error as retryable or permanent
/// 3. VectorizerActor broadcasts EmbeddingFailed with classification
/// 4. Multiple subscribers react:
///    - Console: Displays error message to user
///    - RetryCoordinator: Schedules retry if retryable=true (via ScheduleRetry)
///    - CrateCoordinatorActor: Updates error state tracking (optional)
///
/// # Error Classification
///
/// - **Retryable**: Network failures, rate limits (429), server errors (5xx)
/// - **Permanent**: Invalid API key, malformed requests (4xx except 429), dimension mismatches
///
/// # Example
///
/// ```no_run
/// use crately::messages::EmbeddingFailed;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
///
/// // Retryable error (rate limit)
/// let retryable_event = EmbeddingFailed {
///     specifier: specifier.clone(),
///     chunk_id: "serde_1_0_0_042".to_string(),
///     error: "API rate limit exceeded (429)".to_string(),
///     retryable: true,
/// };
/// // broker.broadcast(retryable_event).await;
///
/// // Permanent error (invalid API key)
/// let permanent_event = EmbeddingFailed {
///     specifier,
///     chunk_id: "serde_1_0_0_043".to_string(),
///     error: "Invalid API key".to_string(),
///     retryable: false,
/// };
/// // broker.broadcast(permanent_event).await;
/// ```
#[acton_message]
pub struct EmbeddingFailed {
    /// The crate this failed embedding belongs to
    #[allow(dead_code)]
    pub specifier: CrateSpecifier,
    /// Unique chunk identifier (e.g., "serde_1_0_0_042")
    #[allow(dead_code)]
    pub chunk_id: String,
    /// Error message describing the failure
    #[allow(dead_code)]
    pub error: String,
    /// Whether this error can be retried
    #[allow(dead_code)]
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_embedding_failed_creation_retryable() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let event = EmbeddingFailed {
            specifier: specifier.clone(),
            chunk_id: "tokio_1_35_0_010".to_string(),
            error: "Network timeout".to_string(),
            retryable: true,
        };

        assert_eq!(event.specifier, specifier);
        assert_eq!(event.chunk_id, "tokio_1_35_0_010");
        assert_eq!(event.error, "Network timeout");
        assert!(event.retryable);
    }

    #[test]
    fn test_embedding_failed_creation_permanent() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let event = EmbeddingFailed {
            specifier: specifier.clone(),
            chunk_id: "axum_0_7_0_005".to_string(),
            error: "Invalid API key".to_string(),
            retryable: false,
        };

        assert_eq!(event.specifier, specifier);
        assert_eq!(event.chunk_id, "axum_0_7_0_005");
        assert_eq!(event.error, "Invalid API key");
        assert!(!event.retryable);
    }

    #[test]
    fn test_embedding_failed_clone() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let event = EmbeddingFailed {
            specifier,
            chunk_id: "serde_1_0_0_000".to_string(),
            error: "API rate limit exceeded".to_string(),
            retryable: true,
        };

        let cloned = event.clone();
        assert_eq!(event.chunk_id, cloned.chunk_id);
        assert_eq!(event.error, cloned.error);
        assert_eq!(event.retryable, cloned.retryable);
    }

    #[test]
    fn test_embedding_failed_debug() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.75").unwrap();
        let event = EmbeddingFailed {
            specifier,
            chunk_id: "anyhow_1_0_75_001".to_string(),
            error: "Server error".to_string(),
            retryable: true,
        };

        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("EmbeddingFailed"));
    }

    #[test]
    fn test_embedding_failed_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<EmbeddingFailed>();
        assert_sync::<EmbeddingFailed>();
    }

    #[test]
    fn test_embedding_failed_error_classification() {
        let specifier = CrateSpecifier::from_str("test@1.0.0").unwrap();

        // Retryable errors
        let retryable_errors = vec![
            ("Network timeout", true),
            ("Connection refused", true),
            ("Rate limit exceeded (429)", true),
            ("Internal server error (500)", true),
            ("Service unavailable (503)", true),
        ];

        for (error_msg, expected_retryable) in retryable_errors {
            let event = EmbeddingFailed {
                specifier: specifier.clone(),
                chunk_id: "test_1_0_0_000".to_string(),
                error: error_msg.to_string(),
                retryable: expected_retryable,
            };
            assert_eq!(
                event.retryable, expected_retryable,
                "Error '{}' should be retryable={}",
                error_msg, expected_retryable
            );
        }

        // Permanent errors
        let permanent_errors = vec![
            ("Invalid API key", false),
            ("Bad request (400)", false),
            ("Unauthorized (401)", false),
            ("Forbidden (403)", false),
            ("Not found (404)", false),
            ("Dimension mismatch", false),
        ];

        for (error_msg, expected_retryable) in permanent_errors {
            let event = EmbeddingFailed {
                specifier: specifier.clone(),
                chunk_id: "test_1_0_0_000".to_string(),
                error: error_msg.to_string(),
                retryable: expected_retryable,
            };
            assert_eq!(
                event.retryable, expected_retryable,
                "Error '{}' should be retryable={}",
                error_msg, expected_retryable
            );
        }
    }
}
