//! BuildQueryResponse - Response broadcast with build query results
//!
//! This message is broadcast by DatabaseActor in response to QueryBuild requests,
//! containing complete build information based on the query parameters.

use crate::crate_specifier::CrateSpecifier;
use crate::types::BuildId;
use acton_reactive::prelude::*;
use serde::{Deserialize, Serialize};

/// Response broadcast containing build query results
///
/// This message is broadcast by DatabaseActor after processing a QueryBuild request.
/// Multiple actors can subscribe to handle the response for different purposes
/// (display, caching, analytics, etc.).
///
/// # Response Pattern
///
/// Following the pub/sub pattern:
/// 1. DatabaseActor receives QueryBuild
/// 2. DatabaseActor performs query
/// 3. DatabaseActor broadcasts BuildQueryResponse
/// 4. Multiple subscribers can react to the response
///
/// # Fields
///
/// * `response_id` - Correlation ID matching the query (if provided)
/// * `build_id` - ID of the queried build
/// * `found` - Whether the build was found in the database
/// * `build_info` - Optional complete build information
///
/// # Example
///
/// ```no_run
/// use crately::messages::{BuildQueryResponse, BuildInfo};
/// use crately::types::BuildId;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let response = BuildQueryResponse {
///     response_id: Some("req_12345".to_string()),
///     build_id: BuildId::new(),
///     found: true,
///     build_info: Some(BuildInfo {
///         specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///         features: Some(vec!["derive".to_string()]),
///         timestamp: 1704067200,
///         chunk_count: 42,
///         unique_content_count: 38,
///         deduplication_rate: 0.095,
///     }),
/// };
/// // broker.broadcast(response).await;
/// ```
///
/// # Message Flow
///
/// 1. DatabaseActor receives QueryBuild
/// 2. DatabaseActor queries build by ID
/// 3. If found and `include_chunks`, fetch chunk associations
/// 4. If found and `include_metrics`, compute statistics
/// 5. DatabaseActor broadcasts BuildQueryResponse
/// 6. Subscribers handle response (UI update, cache, etc.)
///
/// # Subscribers
///
/// - **QueryRequester**: Original requester receives correlated response
/// - **CacheActor**: Caches build info for future queries
/// - **Console**: Displays build information to user
#[acton_message]
pub struct BuildQueryResponse {
    /// Correlation ID matching the QueryBuild request (if provided)
    pub response_id: Option<String>,

    /// ID of the queried build
    pub build_id: BuildId,

    /// Whether the build was found in the database
    pub found: bool,

    /// Complete build information (if found)
    pub build_info: Option<BuildInfo>,
}

/// Complete information about a build
///
/// Contains all metadata, statistics, and associations for a documentation build.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildInfo {
    /// Crate name and version for this build
    pub specifier: CrateSpecifier,

    /// Optional list of enabled cargo features
    pub features: Option<Vec<String>>,

    /// Unix timestamp when build started
    pub timestamp: i64,

    /// Total number of chunks in this build
    pub chunk_count: usize,

    /// Number of unique content nodes (after deduplication)
    pub unique_content_count: usize,

    /// Deduplication rate (0.0 = no deduplication, 1.0 = all duplicates)
    /// Calculated as: (chunk_count - unique_content_count) / chunk_count
    pub deduplication_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_build_info() -> BuildInfo {
        BuildInfo {
            specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
            features: Some(vec!["derive".to_string()]),
            timestamp: 1704067200,
            chunk_count: 100,
            unique_content_count: 85,
            deduplication_rate: 0.15,
        }
    }

    #[test]
    fn test_build_query_response_found() {
        let build_id = BuildId::new();
        let build_info = create_test_build_info();

        let response = BuildQueryResponse {
            response_id: Some("req_001".to_string()),
            build_id: build_id.clone(),
            found: true,
            build_info: Some(build_info.clone()),
        };

        assert_eq!(response.response_id, Some("req_001".to_string()));
        assert_eq!(response.build_id, build_id);
        assert!(response.found);
        assert!(response.build_info.is_some());
    }

    #[test]
    fn test_build_query_response_not_found() {
        let build_id = BuildId::new();

        let response = BuildQueryResponse {
            response_id: Some("req_002".to_string()),
            build_id: build_id.clone(),
            found: false,
            build_info: None,
        };

        assert!(!response.found);
        assert!(response.build_info.is_none());
    }

    #[test]
    fn test_build_query_response_no_correlation_id() {
        let response = BuildQueryResponse {
            response_id: None,
            build_id: BuildId::new(),
            found: true,
            build_info: Some(create_test_build_info()),
        };

        assert!(response.response_id.is_none());
    }

    #[test]
    fn test_build_info_deduplication_rate() {
        let info = BuildInfo {
            specifier: CrateSpecifier::from_str("tokio@1.35.0").unwrap(),
            features: None,
            timestamp: 1704067200,
            chunk_count: 50,
            unique_content_count: 40,
            deduplication_rate: 0.20, // (50 - 40) / 50 = 0.20
        };

        assert_eq!(info.chunk_count, 50);
        assert_eq!(info.unique_content_count, 40);
        assert!((info.deduplication_rate - 0.20).abs() < 0.001);
    }

    #[test]
    fn test_build_info_no_deduplication() {
        let info = BuildInfo {
            specifier: CrateSpecifier::from_str("axum@0.7.0").unwrap(),
            features: Some(vec!["json".to_string()]),
            timestamp: 1704067200,
            chunk_count: 30,
            unique_content_count: 30,
            deduplication_rate: 0.0,
        };

        assert_eq!(info.chunk_count, info.unique_content_count);
        assert_eq!(info.deduplication_rate, 0.0);
    }

    #[test]
    fn test_build_info_high_deduplication() {
        let info = BuildInfo {
            specifier: CrateSpecifier::from_str("test@1.0.0").unwrap(),
            features: None,
            timestamp: 1704067200,
            chunk_count: 100,
            unique_content_count: 10,
            deduplication_rate: 0.90,
        };

        assert_eq!(info.chunk_count, 100);
        assert_eq!(info.unique_content_count, 10);
        assert!((info.deduplication_rate - 0.90).abs() < 0.001);
    }

    #[test]
    fn test_build_info_with_features() {
        let features = vec!["tokio".to_string(), "json".to_string(), "macros".to_string()];
        let info = BuildInfo {
            specifier: CrateSpecifier::from_str("axum@0.7.0").unwrap(),
            features: Some(features.clone()),
            timestamp: 1704067200,
            chunk_count: 75,
            unique_content_count: 70,
            deduplication_rate: 0.067,
        };

        assert_eq!(info.features, Some(features));
    }

    #[test]
    fn test_build_info_no_features() {
        let info = BuildInfo {
            specifier: CrateSpecifier::from_str("anyhow@1.0.0").unwrap(),
            features: None,
            timestamp: 1704067200,
            chunk_count: 20,
            unique_content_count: 18,
            deduplication_rate: 0.10,
        };

        assert!(info.features.is_none());
    }

    #[test]
    fn test_build_query_response_clone() {
        let original = BuildQueryResponse {
            response_id: Some("req_clone".to_string()),
            build_id: BuildId::new(),
            found: true,
            build_info: Some(create_test_build_info()),
        };

        let cloned = original.clone();
        assert_eq!(original.response_id, cloned.response_id);
        assert_eq!(original.build_id, cloned.build_id);
        assert_eq!(original.found, cloned.found);
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
            response_id: Some("req_debug".to_string()),
            build_id: BuildId::new(),
            found: true,
            build_info: Some(create_test_build_info()),
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

        assert_eq!(original.specifier, deserialized.specifier);
        assert_eq!(original.chunk_count, deserialized.chunk_count);
        assert_eq!(original.unique_content_count, deserialized.unique_content_count);
    }

    #[test]
    fn test_build_info_clone() {
        let original = create_test_build_info();
        let cloned = original.clone();

        assert_eq!(original.specifier, cloned.specifier);
        assert_eq!(original.chunk_count, cloned.chunk_count);
        assert_eq!(original.deduplication_rate, cloned.deduplication_rate);
    }
}
