//! Event broadcast when all crate processing pipeline stages complete.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Event broadcast when all crate processing pipeline stages complete.
///
/// This message is broadcast by CrateCoordinatorActor after all pipeline
/// stages complete successfully. It signals the end of the processing pipeline
/// and that the crate is fully processed and available.
///
/// # Fields
///
/// * `specifier` - The crate that was processed
/// * `features` - Feature flags for this crate
/// * `total_duration_ms` - Total pipeline time from CrateReceived to completion
/// * `stages_completed` - Number of pipeline stages completed
///
/// # Message Flow
///
/// 1. CrateCoordinatorActor receives DocumentationVectorized event
/// 2. CrateCoordinatorActor updates state to Complete
/// 3. CrateCoordinatorActor calculates total duration
/// 4. CrateCoordinatorActor broadcasts CrateProcessingComplete event
/// 5. Multiple subscribers react:
///    - Console: Displays final success message
///    - DatabaseActor: May update final statistics
///    - MetricsActor (future): Records end-to-end timing
///
/// # Subscribers
///
/// - **Console**: User feedback - "✓ Processing complete: serde@1.0.0 (15.2s total)"
/// - **DatabaseActor**: Optionally updates final statistics
/// - **MetricsActor** (future): Records end-to-end success metrics
///
/// # Example
///
/// ```no_run
/// use crately::messages::CrateProcessingComplete;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
/// let event = CrateProcessingComplete {
///     specifier,
///     features: vec!["derive".to_string()],
///     total_duration_ms: 15234,
///     stages_completed: 4,
/// };
/// // broker.broadcast(event).await;
/// ```
#[acton_message]
pub struct CrateProcessingComplete {
    /// The crate that was processed.
    ///
    /// NOTE: Currently unused by handlers but broadcast for observability.
    /// Reserved for completion event subscribers (Console, MetricsActor).
    #[allow(dead_code)]
    pub specifier: CrateSpecifier,
    /// Feature flags for this crate.
    ///
    /// NOTE: Currently unused but reserved for completion event logging.
    /// When Console subscribes to this event, features will be displayed in
    /// the final success message to confirm which configuration was processed.
    ///
    /// See issue #55 for planned completion reporting.
    #[allow(dead_code)]
    pub features: Vec<String>,
    /// Total pipeline duration in milliseconds.
    ///
    /// NOTE: Currently unused by handlers but broadcast for observability.
    /// Reserved for timing display and performance metrics tracking.
    #[allow(dead_code)]
    pub total_duration_ms: u64,
    /// Number of pipeline stages completed.
    ///
    /// NOTE: Currently unused but reserved for progress tracking and metrics.
    /// Planned uses:
    /// - Console: Display "Completed 4/4 stages" in success message
    /// - MetricsActor: Track which stages completed for pipeline analytics
    /// - Alerting: Detect partial completion scenarios
    ///
    /// See issue #55 for planned progress reporting.
    #[allow(dead_code)]
    pub stages_completed: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_message_creation() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let message = CrateProcessingComplete {
            specifier: specifier.clone(),
            features: vec!["derive".to_string()],
            total_duration_ms: 15234,
            stages_completed: 4,
        };
        assert_eq!(message.specifier, specifier);
        assert_eq!(message.total_duration_ms, 15234);
        assert_eq!(message.stages_completed, 4);
    }

    #[test]
    fn test_message_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let message = CrateProcessingComplete {
            specifier,
            features: vec![],
            total_duration_ms: 20000,
            stages_completed: 4,
        };
        let cloned = message.clone();
        assert_eq!(message.specifier, cloned.specifier);
        assert_eq!(message.total_duration_ms, cloned.total_duration_ms);
        assert_eq!(message.stages_completed, cloned.stages_completed);
    }

    #[test]
    fn test_message_debug() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let message = CrateProcessingComplete {
            specifier,
            features: vec![],
            total_duration_ms: 18000,
            stages_completed: 4,
        };
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("CrateProcessingComplete"));
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CrateProcessingComplete>();
        assert_sync::<CrateProcessingComplete>();
    }

    #[test]
    fn test_timing_accuracy() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();
        let message = CrateProcessingComplete {
            specifier,
            features: vec![],
            total_duration_ms: 0, // Edge case: instant processing
            stages_completed: 4,
        };
        assert_eq!(message.total_duration_ms, 0);
    }

    #[test]
    fn test_stages_completed_validation() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.0").unwrap();
        let message = CrateProcessingComplete {
            specifier,
            features: vec![],
            total_duration_ms: 12000,
            stages_completed: 4,
        };
        assert_eq!(message.stages_completed, 4);
    }

    #[test]
    fn test_long_processing_time() {
        let specifier = CrateSpecifier::from_str("huge-crate@1.0.0").unwrap();
        let message = CrateProcessingComplete {
            specifier,
            features: vec![],
            total_duration_ms: u64::MAX, // Edge case: maximum duration
            stages_completed: 4,
        };
        assert_eq!(message.total_duration_ms, u64::MAX);
    }

    #[test]
    fn test_zero_stages() {
        let specifier = CrateSpecifier::from_str("test@1.0.0").unwrap();
        let message = CrateProcessingComplete {
            specifier,
            features: vec![],
            total_duration_ms: 5000,
            stages_completed: 0, // Edge case: zero stages (unusual but valid)
        };
        assert_eq!(message.stages_completed, 0);
    }

    #[test]
    fn test_features_passthrough() {
        let specifier = CrateSpecifier::from_str("test@1.0.0").unwrap();
        let features = vec!["feat1".to_string(), "feat2".to_string(), "feat3".to_string()];
        let message = CrateProcessingComplete {
            specifier,
            features: features.clone(),
            total_duration_ms: 10000,
            stages_completed: 4,
        };
        assert_eq!(message.features, features);
    }
}
