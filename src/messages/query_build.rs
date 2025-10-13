//! QueryBuild - Request to find a build by crate identifier and feature set
//!
//! This message is sent to DatabaseActor to query for a specific build that matches
//! both the crate (name + version) and the exact feature configuration. Uses graph
//! traversal to efficiently locate the build.

use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::*;

/// Query a specific build by crate identifier and feature set
///
/// This message queries for a build record that matches both the crate
/// (name + version) and the exact feature configuration. Unlike QueryCrate
/// which returns crate metadata with all possible features, this returns
/// a specific build with its realized feature set.
///
/// # Graph Traversal
///
/// Uses single-query graph traversal:
/// ```surql
/// SELECT
///     build.id as build_id,
///     build.features as features,
///     build.feature_hash as feature_hash,
///     build.status as status,
///     build.created_at as created_at,
///     build.updated_at as updated_at,
///     count(<-contains<-content_block) as chunk_count
/// FROM crate WHERE name = $name AND version = $version
/// ->has_build->build[WHERE features = $features]
/// ```
///
/// # Use Cases
///
/// - Get BuildId for subsequent chunk queries
/// - Verify a feature configuration exists
/// - Check build processing status
/// - Retrieve build-specific metadata
///
/// # Response
///
/// DatabaseActor broadcasts `BuildQueryResponse` with build information
/// or None if no matching build exists.
///
/// # Example
///
/// ```no_run
/// use crately::messages::QueryBuild;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let message = QueryBuild {
///     specifier: CrateSpecifier::from_str("serde@1.0.200").unwrap(),
///     features: vec!["derive".to_string()],
/// };
/// // db_handle.send(message).await;
/// ```
#[acton_message]
pub struct QueryBuild {
    /// The crate identifier (name + version)
    pub specifier: CrateSpecifier,

    /// The exact feature set for this build
    /// Empty vec means default features only
    pub features: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_query_build_basic() {
        let specifier = CrateSpecifier::from_str("serde@1.0.200").unwrap();
        let features = vec!["derive".to_string()];

        let query = QueryBuild {
            specifier: specifier.clone(),
            features: features.clone(),
        };

        assert_eq!(query.specifier, specifier);
        assert_eq!(query.features, features);
    }

    #[test]
    fn test_query_build_no_features() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.0").unwrap();
        let query = QueryBuild {
            specifier: specifier.clone(),
            features: vec![],
        };

        assert_eq!(query.specifier, specifier);
        assert!(query.features.is_empty());
    }

    #[test]
    fn test_query_build_multiple_features() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let features = vec![
            "full".to_string(),
            "rt-multi-thread".to_string(),
            "macros".to_string(),
        ];

        let query = QueryBuild {
            specifier: specifier.clone(),
            features: features.clone(),
        };

        assert_eq!(query.features.len(), 3);
        assert!(query.features.contains(&"full".to_string()));
    }

    #[test]
    fn test_query_build_clone() {
        let original = QueryBuild {
            specifier: CrateSpecifier::from_str("axum@0.7.0").unwrap(),
            features: vec!["json".to_string()],
        };

        let cloned = original.clone();
        assert_eq!(original.specifier, cloned.specifier);
        assert_eq!(original.features, cloned.features);
    }

    #[test]
    fn test_query_build_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<QueryBuild>();
        assert_sync::<QueryBuild>();
    }

    #[test]
    fn test_query_build_debug() {
        let query = QueryBuild {
            specifier: CrateSpecifier::from_str("test@1.0.0").unwrap(),
            features: vec!["feature1".to_string()],
        };

        let debug_str = format!("{:?}", query);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("QueryBuild"));
    }
}
