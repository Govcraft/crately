//! BuildQueryResponse - Response containing build information for a specific feature configuration
//!
//! This message is broadcast by DatabaseActor in response to QueryBuild requests,
//! providing complete information about a build including its unique identifier,
//! feature set, processing status, and timestamps.

use crate::crate_specifier::CrateSpecifier;
use crate::types::BuildId;
use acton_reactive::prelude::*;
use serde::{Deserialize, Serialize};

/// Response containing build information for a specific feature configuration
///
/// This response provides complete information about a build, including its
/// unique identifier, feature set, processing status, and timestamps. If the
/// requested build does not exist, `build_info` will be None.
///
/// # None Response
///
/// If `build_info` is None, no build exists for the requested crate +
/// feature combination. The crate may exist but not have been built with
/// those features.
///
/// # Example
///
/// ```no_run
/// use crately::messages::{BuildQueryResponse, BuildInfo};
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// # /*
/// builder.act_on::<BuildQueryResponse>(|agent, envelope| {
///     let response = envelope.message();
///     if let Some(build) = &response.build_info {
///         println!(
///             "Build {} with features {:?} is {}",
///             build.build_id,
///             build.features,
///             build.status
///         );
///     }
///     AgentReply::immediate()
/// });
/// # */
/// ```
#[acton_message]
pub struct BuildQueryResponse {
    /// The crate identifier that was queried
    pub _specifier: CrateSpecifier,

    /// The feature set that was queried
    pub _features: Vec<String>,

    /// Build information if found, None if no matching build exists
    pub _build_info: Option<BuildInfo>,
}

/// Complete information about a build
///
/// Provides all metadata about a specific build including its processing
/// state and feature configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildInfo {
    /// Unique build identifier (e.g., "build:serde_1_0_200_derive")
    pub build_id: BuildId,

    /// Features enabled for this build
    pub features: Vec<String>,

    /// Content hash of feature set (for deduplication)
    pub feature_hash: String,

    /// Processing status (pending, chunked, vectorized, complete, failed)
    pub status: String,

    /// When this build was created
    pub created_at: String,

    /// Last status update timestamp
    pub updated_at: String,

    /// Number of content blocks in this build
    pub chunk_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_build_info() -> BuildInfo {
        BuildInfo {
            build_id: BuildId::from_str("build_abc123def456abc123def456abc12345").unwrap(),
            features: vec!["derive".to_string()],
            feature_hash: "feature_hash_abc123".to_string(),
            status: "complete".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T01:00:00Z".to_string(),
            chunk_count: 42,
        }
    }

    #[test]
    fn test_build_query_response_found() {
        let specifier = CrateSpecifier::from_str("serde@1.0.200").unwrap();
        let features = vec!["derive".to_string()];
        let build_info = create_test_build_info();

        let response = BuildQueryResponse {
            _specifier: specifier.clone(),
            _features: features.clone(),
            _build_info: Some(build_info.clone()),
        };

        assert_eq!(response._specifier, specifier);
        assert_eq!(response._features, features);
        assert!(response._build_info.is_some());
    }

    #[test]
    fn test_build_query_response_not_found() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let features = vec!["full".to_string()];

        let response = BuildQueryResponse {
            _specifier: specifier.clone(),
            _features: features.clone(),
            _build_info: None,
        };

        assert!(response._build_info.is_none());
    }

    #[test]
    fn test_build_info_all_fields() {
        let info = create_test_build_info();

        assert!(!info.build_id.as_str().is_empty());
        assert_eq!(info.features, vec!["derive".to_string()]);
        assert_eq!(info.feature_hash, "feature_hash_abc123");
        assert_eq!(info.status, "complete");
        assert_eq!(info.chunk_count, 42);
    }

    #[test]
    fn test_build_info_no_features() {
        let info = BuildInfo {
            build_id: BuildId::from_str("build_def456abc123def456abc123def45678").unwrap(),
            features: vec![],
            feature_hash: "empty_hash".to_string(),
            status: "pending".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            chunk_count: 0,
        };

        assert!(info.features.is_empty());
        assert_eq!(info.chunk_count, 0);
    }

    #[test]
    fn test_build_info_multiple_features() {
        let features = vec![
            "tokio".to_string(),
            "json".to_string(),
            "macros".to_string(),
        ];
        let info = BuildInfo {
            build_id: BuildId::from_str("build_a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4").unwrap(),
            features: features.clone(),
            feature_hash: "multi_feature_hash".to_string(),
            status: "complete".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T01:30:00Z".to_string(),
            chunk_count: 150,
        };

        assert_eq!(info.features, features);
        assert_eq!(info.chunk_count, 150);
    }

    #[test]
    fn test_build_query_response_clone() {
        let original = BuildQueryResponse {
            _specifier: CrateSpecifier::from_str("axum@0.7.0").unwrap(),
            _features: vec!["json".to_string()],
            _build_info: Some(create_test_build_info()),
        };

        let cloned = original.clone();
        assert_eq!(original._specifier, cloned._specifier);
        assert_eq!(original._features, cloned._features);
    }

    #[test]
    fn test_build_query_response_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<BuildQueryResponse>();
        assert_sync::<BuildQueryResponse>();
    }

    #[test]
    fn test_build_query_response_debug() {
        let response = BuildQueryResponse {
            _specifier: CrateSpecifier::from_str("test@1.0.0").unwrap(),
            _features: vec![],
            _build_info: Some(create_test_build_info()),
        };

        let debug_str = format!("{:?}", response);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("BuildQueryResponse"));
    }

    #[test]
    fn test_build_info_serde_roundtrip() {
        let original = create_test_build_info();
        let json = serde_json::to_string(&original).expect("Should serialize");
        let deserialized: BuildInfo = serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(original.build_id, deserialized.build_id);
        assert_eq!(original.features, deserialized.features);
        assert_eq!(original.chunk_count, deserialized.chunk_count);
    }

    #[test]
    fn test_build_info_clone() {
        let original = create_test_build_info();
        let cloned = original.clone();

        assert_eq!(original.build_id, cloned.build_id);
        assert_eq!(original.status, cloned.status);
        assert_eq!(original.chunk_count, cloned.chunk_count);
    }
}
