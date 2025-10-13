//! BuildCreated - Event broadcast when a build entity is successfully created
//!
//! This message is broadcast by DatabaseActor after successfully creating a build
//! entity and establishing its relationships to crate and features in the graph database.

use acton_reactive::prelude::*;

/// Event broadcast when a build entity is created in the database
///
/// This message is broadcast via the broker by DatabaseActor after successfully
/// creating a build entity and establishing its relationships to crate and features.
/// The build entity represents a specific compilation configuration (crate + version + features).
///
/// # Fields
///
/// * `build_id` - The content-addressable BuildId (deterministic based on crate+version+features)
/// * `crate_name` - The name of the crate this build is for
/// * `crate_version` - The version of the crate this build is for
/// * `features` - The feature flags enabled in this build
/// * `record_id` - The SurrealDB record ID for the build entity
///
/// # Example
///
/// ```no_run
/// use crately::messages::BuildCreated;
///
/// let event = BuildCreated {
///     build_id: "build_af1349b9f5f9a1a6a0404dea36dcc949".to_string(),
///     crate_name: "serde".to_string(),
///     crate_version: "1.0.0".to_string(),
///     features: vec!["derive".to_string()],
///     record_id: "build:af1349b9".to_string(),
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. DatabaseActor receives PersistCrate message
/// 2. DatabaseActor creates/gets crate entity
/// 3. DatabaseActor creates/gets feature entities
/// 4. DatabaseActor creates build entity with content-addressable BuildId
/// 5. DatabaseActor establishes relationships (build->of->crate, build->enables->feature)
/// 6. DatabaseActor broadcasts BuildCreated event
/// 7. Subscribers react (Console for display, pipeline actors for processing)
///
/// # Subscribers
///
/// - **Console**: Displays user-facing success message
/// - **CrateDownloader**: May initiate download and processing pipeline
/// - **MetricsActor**: Tracks build creation rate and deduplication statistics
#[acton_message]
pub struct BuildCreated {
    /// Content-addressable BuildId (deterministic based on crate+version+features)
    pub build_id: String,

    /// Name of the crate this build is for
    pub crate_name: String,

    /// Version of the crate this build is for
    pub crate_version: String,

    /// Feature flags enabled in this build
    pub features: Vec<String>,

    /// SurrealDB record ID for the build entity
    pub record_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_created_creation() {
        let event = BuildCreated {
            build_id: "build_af1349b9f5f9a1a6a0404dea36dcc949".to_string(),
            crate_name: "serde".to_string(),
            crate_version: "1.0.0".to_string(),
            features: vec!["derive".to_string()],
            record_id: "build:af1349b9".to_string(),
        };

        let cloned = event.clone();
        assert_eq!(event.build_id, cloned.build_id);
        assert_eq!(event.crate_name, cloned.crate_name);
        assert_eq!(event.crate_version, cloned.crate_version);
        assert_eq!(event.features, cloned.features);
        assert_eq!(event.record_id, cloned.record_id);
    }

    #[test]
    fn test_build_created_debug() {
        let event = BuildCreated {
            build_id: "build_xyz789".to_string(),
            crate_name: "tokio".to_string(),
            crate_version: "1.35.0".to_string(),
            features: vec!["full".to_string()],
            record_id: "build:xyz789".to_string(),
        };

        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("BuildCreated"));
    }

    #[test]
    fn test_build_created_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<BuildCreated>();
        assert_sync::<BuildCreated>();
    }

    #[test]
    fn test_build_created_empty_features() {
        let event = BuildCreated {
            build_id: "build_nofeatures".to_string(),
            crate_name: "anyhow".to_string(),
            crate_version: "1.0.75".to_string(),
            features: vec![],
            record_id: "build:nofeatures".to_string(),
        };

        assert!(event.features.is_empty());
    }

    #[test]
    fn test_build_created_multiple_features() {
        let event = BuildCreated {
            build_id: "build_multifeatures".to_string(),
            crate_name: "tokio".to_string(),
            crate_version: "1.35.0".to_string(),
            features: vec![
                "full".to_string(),
                "rt-multi-thread".to_string(),
                "macros".to_string(),
            ],
            record_id: "build:multifeatures".to_string(),
        };

        assert_eq!(event.features.len(), 3);
        assert!(event.features.contains(&"full".to_string()));
        assert!(event.features.contains(&"rt-multi-thread".to_string()));
        assert!(event.features.contains(&"macros".to_string()));
    }

    #[test]
    fn test_build_created_clone_independence() {
        let original = BuildCreated {
            build_id: "build_original".to_string(),
            crate_name: "test".to_string(),
            crate_version: "1.0.0".to_string(),
            features: vec!["feature1".to_string()],
            record_id: "build:original".to_string(),
        };

        let mut cloned = original.clone();
        cloned.build_id = "build_modified".to_string();

        assert_ne!(original.build_id, cloned.build_id);
        assert_eq!(original.crate_name, cloned.crate_name);
    }
}
