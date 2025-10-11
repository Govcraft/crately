//! Event broadcast when all crate processing pipeline stages complete.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Event broadcast when all crate processing pipeline stages complete.
///
/// This message is broadcast by DatabaseActor after successfully storing
/// all vectorized documentation chunks in the database. It signals the end
/// of the processing pipeline and that the crate is fully searchable.
///
/// # Fields
///
/// * `specifier` - The crate that was processed
/// * `features` - Feature flags for this crate
/// * `total_duration_ms` - Total pipeline time from CrateReceived to completion
/// * `record_id` - SurrealDB record ID for the persisted crate
///
/// # Message Flow
///
/// 1. DatabaseActor receives DocumentationVectorized event
/// 2. DatabaseActor stores all vectors in database
/// 3. DatabaseActor updates crate status to "complete"
/// 4. DatabaseActor broadcasts CrateProcessingComplete event
/// 5. Multiple subscribers react:
///    - Console: Displays final success message
///    - ServerActor: May send HTTP response (if request still pending)
///    - CrateCoordinatorActor: Updates state to "Complete"
///    - MetricsActor (future): Records end-to-end timing
///
/// # Subscribers
///
/// - **Console**: User feedback - "✓ Processing complete: serde@1.0.0 (15.2s total)"
/// - **ServerActor** (future): Constructs HTTP response with record_id
/// - **CrateCoordinatorActor** (future): State tracking - "Complete"
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
///     record_id: "crate:serde_1_0_0".to_string(),
/// };
/// // broker.broadcast(event).await;
/// ```
#[acton_message]
pub struct CrateProcessingComplete {
    /// The crate that was processed
    pub specifier: CrateSpecifier,
    /// Feature flags for this crate
    pub features: Vec<String>,
    /// Total pipeline duration in milliseconds
    pub total_duration_ms: u64,
    /// SurrealDB record ID
    pub record_id: String,
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
            record_id: "crate:serde_1_0_0".to_string(),
        };
        assert_eq!(message.specifier, specifier);
        assert_eq!(message.total_duration_ms, 15234);
        assert_eq!(message.record_id, "crate:serde_1_0_0");
    }

    #[test]
    fn test_message_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let message = CrateProcessingComplete {
            specifier,
            features: vec![],
            total_duration_ms: 20000,
            record_id: "crate:tokio_1_0_0".to_string(),
        };
        let cloned = message.clone();
        assert_eq!(message.specifier, cloned.specifier);
        assert_eq!(message.total_duration_ms, cloned.total_duration_ms);
        assert_eq!(message.record_id, cloned.record_id);
    }

    #[test]
    fn test_message_debug() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let message = CrateProcessingComplete {
            specifier,
            features: vec![],
            total_duration_ms: 18000,
            record_id: "crate:axum_0_7_0".to_string(),
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
            record_id: "crate:tracing_0_1_0".to_string(),
        };
        assert_eq!(message.total_duration_ms, 0);
    }

    #[test]
    fn test_record_id_format_validation() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.0").unwrap();
        let record_id = "crate:anyhow_1_0_0";
        let message = CrateProcessingComplete {
            specifier,
            features: vec![],
            total_duration_ms: 12000,
            record_id: record_id.to_string(),
        };
        assert_eq!(message.record_id, record_id);
        assert!(message.record_id.starts_with("crate:"));
    }

    #[test]
    fn test_long_processing_time() {
        let specifier = CrateSpecifier::from_str("huge-crate@1.0.0").unwrap();
        let message = CrateProcessingComplete {
            specifier,
            features: vec![],
            total_duration_ms: u64::MAX, // Edge case: maximum duration
            record_id: "crate:huge_crate_1_0_0".to_string(),
        };
        assert_eq!(message.total_duration_ms, u64::MAX);
    }

    #[test]
    fn test_empty_record_id() {
        let specifier = CrateSpecifier::from_str("test@1.0.0").unwrap();
        let message = CrateProcessingComplete {
            specifier,
            features: vec![],
            total_duration_ms: 5000,
            record_id: String::new(), // Edge case: empty record ID
        };
        assert!(message.record_id.is_empty());
    }

    #[test]
    fn test_features_passthrough() {
        let specifier = CrateSpecifier::from_str("test@1.0.0").unwrap();
        let features = vec!["feat1".to_string(), "feat2".to_string(), "feat3".to_string()];
        let message = CrateProcessingComplete {
            specifier,
            features: features.clone(),
            total_duration_ms: 10000,
            record_id: "crate:test_1_0_0".to_string(),
        };
        assert_eq!(message.features, features);
    }
}
