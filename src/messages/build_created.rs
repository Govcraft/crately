//! BuildCreated - Event broadcast when a new crate build starts
//!
//! This message is broadcast when the system begins processing a crate build,
//! establishing the build's identity and configuration in the graph database.

use crate::crate_specifier::CrateSpecifier;
use crate::types::BuildId;
use acton_reactive::prelude::*;

/// Event broadcast when a new crate documentation build begins
///
/// This message establishes a build's identity in the system and initiates the
/// build graph node creation. Multiple actors may subscribe to track build lifecycle,
/// update UI, or coordinate downstream processing.
///
/// # Build Uniqueness
///
/// Each build is uniquely identified by its BuildId (ULID). Even if the same
/// crate version is built multiple times (e.g., with different feature flags),
/// each build receives a unique ID for provenance tracking.
///
/// # Fields
///
/// * `build_id` - Unique ULID-based identifier for this build
/// * `specifier` - Crate name and version being built
/// * `features` - Optional list of enabled cargo features
/// * `timestamp` - Unix timestamp (seconds) when build started
///
/// # Example
///
/// ```no_run
/// use crately::messages::BuildCreated;
/// use crately::types::BuildId;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let event = BuildCreated {
///     build_id: BuildId::new(),
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     features: Some(vec!["derive".to_string()]),
///     timestamp: 1704067200, // 2024-01-01 00:00:00 UTC
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. CrateDownloader receives CrateReceived message
/// 2. CrateDownloader generates new BuildId
/// 3. CrateDownloader broadcasts BuildCreated with build details
/// 4. DatabaseActor creates build node in graph database
/// 5. Console actor displays build start notification
/// 6. Coordinator actors track build lifecycle state
///
/// # Subscribers
///
/// - **DatabaseActor**: Creates build node in graph database
/// - **Console**: Displays build start notification to user
/// - **CoordinatorActor**: Tracks build state machine
/// - **MetricsActor**: Records build start time for analytics
#[acton_message]
pub struct BuildCreated {
    /// Unique identifier for this build (ULID-based)
    pub build_id: BuildId,

    /// Crate name and version being built
    pub specifier: CrateSpecifier,

    /// Optional list of enabled cargo features for this build
    pub features: Option<Vec<String>>,

    /// Unix timestamp (seconds since epoch) when build started
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_build_created_new() {
        let build_id = BuildId::new();
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();

        let event = BuildCreated {
            build_id: build_id.clone(),
            specifier: specifier.clone(),
            features: Some(vec!["derive".to_string()]),
            timestamp: 1704067200,
        };

        assert_eq!(event.build_id, build_id);
        assert_eq!(event.specifier, specifier);
        assert_eq!(event.features, Some(vec!["derive".to_string()]));
        assert_eq!(event.timestamp, 1704067200);
    }

    #[test]
    fn test_build_created_no_features() {
        let event = BuildCreated {
            build_id: BuildId::new(),
            specifier: CrateSpecifier::from_str("tokio@1.35.0").unwrap(),
            features: None,
            timestamp: 1704067200,
        };

        assert!(event.features.is_none());
    }

    #[test]
    fn test_build_created_multiple_features() {
        let event = BuildCreated {
            build_id: BuildId::new(),
            specifier: CrateSpecifier::from_str("axum@0.7.0").unwrap(),
            features: Some(vec![
                "tokio".to_string(),
                "json".to_string(),
                "macros".to_string(),
            ]),
            timestamp: 1704067200,
        };

        assert_eq!(event.features.as_ref().unwrap().len(), 3);
        assert!(event.features.as_ref().unwrap().contains(&"tokio".to_string()));
    }

    #[test]
    fn test_build_created_clone() {
        let original = BuildCreated {
            build_id: BuildId::new(),
            specifier: CrateSpecifier::from_str("tracing@0.1.0").unwrap(),
            features: Some(vec!["log".to_string()]),
            timestamp: 1704067200,
        };

        let cloned = original.clone();
        assert_eq!(original.build_id, cloned.build_id);
        assert_eq!(original.specifier, cloned.specifier);
        assert_eq!(original.features, cloned.features);
        assert_eq!(original.timestamp, cloned.timestamp);
    }

    #[test]
    fn test_build_created_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<BuildCreated>();
        assert_sync::<BuildCreated>();
    }

    #[test]
    fn test_build_created_debug() {
        let event = BuildCreated {
            build_id: BuildId::new(),
            specifier: CrateSpecifier::from_str("anyhow@1.0.0").unwrap(),
            features: None,
            timestamp: 1704067200,
        };

        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("BuildCreated"));
    }

    #[test]
    fn test_build_created_timestamp_range() {
        let timestamps = vec![
            0,           // Unix epoch
            1704067200,  // 2024-01-01
            2147483647,  // Max 32-bit signed int
        ];

        for ts in timestamps {
            let event = BuildCreated {
                build_id: BuildId::new(),
                specifier: CrateSpecifier::from_str("test@1.0.0").unwrap(),
                features: None,
                timestamp: ts,
            };
            assert_eq!(event.timestamp, ts);
        }
    }

    #[test]
    fn test_build_created_unique_build_ids() {
        let spec = CrateSpecifier::from_str("serde@1.0.0").unwrap();

        let event1 = BuildCreated {
            build_id: BuildId::new(),
            specifier: spec.clone(),
            features: None,
            timestamp: 1704067200,
        };

        let event2 = BuildCreated {
            build_id: BuildId::new(),
            specifier: spec.clone(),
            features: None,
            timestamp: 1704067200,
        };

        // Even with identical crate and features, BuildIds should differ
        assert_ne!(event1.build_id, event2.build_id);
    }

    #[test]
    fn test_build_created_empty_features() {
        let event = BuildCreated {
            build_id: BuildId::new(),
            specifier: CrateSpecifier::from_str("test@1.0.0").unwrap(),
            features: Some(vec![]),
            timestamp: 1704067200,
        };

        assert_eq!(event.features, Some(vec![]));
    }
}
