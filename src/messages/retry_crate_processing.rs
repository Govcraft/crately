//! RetryCrateProcessing message - retry event for failed pipeline operations
//!
//! This message is broadcast by the RetryCoordinator when it's time to retry a failed crate processing
//! operation after the calculated backoff delay has elapsed.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Event broadcast when RetryCoordinator schedules a retry attempt
///
/// This message is sent after the exponential backoff delay has elapsed, indicating that
/// the pipeline should retry processing the crate. Actors that handle crate processing
/// should subscribe to this message and re-initiate the appropriate pipeline stage.
///
/// # Actor Communication Pattern
///
/// Publisher: RetryCoordinator (after delay)
/// Subscribers: CrateDownloader, other pipeline actors
///
/// # Timing
///
/// This message is broadcast after the calculated backoff delay, which uses exponential
/// backoff with jitter to prevent thundering herd problems when multiple failures occur.
///
/// # Examples
///
/// ```
/// use crately::messages::RetryCrateProcessing;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let msg = RetryCrateProcessing {
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     features: vec!["derive".to_string()],
///     retry_attempt: 1,
/// };
///
/// assert_eq!(msg.retry_attempt, 1);
/// ```
#[acton_message]
pub struct RetryCrateProcessing {
    /// The crate to retry processing
    pub specifier: CrateSpecifier,

    /// The features requested for this crate
    pub features: Vec<String>,

    /// The retry attempt number (1-indexed, first retry is 1)
    pub retry_attempt: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_retry_crate_processing_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let msg = RetryCrateProcessing {
            specifier: specifier.clone(),
            features: vec!["full".to_string()],
            retry_attempt: 2,
        };

        assert_eq!(msg.specifier, specifier);
        assert_eq!(msg.features, vec!["full".to_string()]);
        assert_eq!(msg.retry_attempt, 2);
    }

    #[test]
    fn test_retry_crate_processing_no_features() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let msg = RetryCrateProcessing {
            specifier: specifier.clone(),
            features: vec![],
            retry_attempt: 1,
        };

        assert_eq!(msg.specifier, specifier);
        assert!(msg.features.is_empty());
        assert_eq!(msg.retry_attempt, 1);
    }

    #[test]
    fn test_retry_crate_processing_multiple_features() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let msg = RetryCrateProcessing {
            specifier,
            features: vec![
                "derive".to_string(),
                "rc".to_string(),
                "alloc".to_string(),
            ],
            retry_attempt: 3,
        };

        assert_eq!(msg.features.len(), 3);
        assert_eq!(msg.retry_attempt, 3);
    }

    #[test]
    fn test_retry_crate_processing_clone() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();
        let msg1 = RetryCrateProcessing {
            specifier,
            features: vec!["log".to_string()],
            retry_attempt: 1,
        };

        let msg2 = msg1.clone();
        assert_eq!(msg1.specifier, msg2.specifier);
        assert_eq!(msg1.features, msg2.features);
        assert_eq!(msg1.retry_attempt, msg2.retry_attempt);
    }

    #[test]
    fn test_retry_attempt_sequence() {
        let specifier = CrateSpecifier::from_str("hyper@1.0.0").unwrap();

        let first_retry = RetryCrateProcessing {
            specifier: specifier.clone(),
            features: vec![],
            retry_attempt: 1,
        };
        assert_eq!(first_retry.retry_attempt, 1);

        let second_retry = RetryCrateProcessing {
            specifier: specifier.clone(),
            features: vec![],
            retry_attempt: 2,
        };
        assert_eq!(second_retry.retry_attempt, 2);

        let third_retry = RetryCrateProcessing {
            specifier,
            features: vec![],
            retry_attempt: 3,
        };
        assert_eq!(third_retry.retry_attempt, 3);
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<RetryCrateProcessing>();
        assert_sync::<RetryCrateProcessing>();
    }
}
