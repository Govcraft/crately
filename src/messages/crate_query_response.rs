//! Event broadcast with crate query results from the database.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Event broadcast containing the results of a crate query.
///
/// This message is broadcast via the broker by DatabaseActor in response to a
/// `QueryCrate` request. It contains the crate metadata if found, or `None` in
/// the specifier field if the crate was not found in the database.
///
/// # Fields
///
/// * `specifier` - The crate name and version if found, `None` if not found
/// * `features` - List of feature flags enabled for this crate
/// * `status` - Current processing status ("pending", "downloading", "complete", etc.)
/// * `created_at` - ISO 8601 timestamp when the crate was first persisted
///
/// # Example
///
/// ```no_run
/// use crately::messages::CrateQueryResponse;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// // Successful query result
/// let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
/// let response = CrateQueryResponse {
///     specifier: Some(specifier),
///     features: vec!["derive".to_string()],
///     status: "complete".to_string(),
///     created_at: "2024-01-15T10:30:00Z".to_string(),
/// };
/// // broker.broadcast(response).await;
///
/// // Not found result
/// let not_found = CrateQueryResponse {
///     specifier: None,
///     features: vec![],
///     status: "not_found".to_string(),
///     created_at: String::new(),
/// };
/// // broker.broadcast(not_found).await;
/// ```
///
/// # Message Flow
///
/// 1. Client sends QueryCrate to DatabaseActor
/// 2. DatabaseActor searches database for matching crate
/// 3. DatabaseActor broadcasts CrateQueryResponse with results
/// 4. Subscribers (Console, ServerActor) receive the results
/// 5. If specifier is None, crate was not found
///
/// # Subscribers
///
/// - **Console**: Displays query results to user
/// - **ServerActor**: Constructs HTTP response with crate data
#[acton_message]
pub struct CrateQueryResponse {
    /// The crate specifier if found, None if not found in database
    pub specifier: Option<CrateSpecifier>,
    /// Feature flags enabled for this crate
    pub features: Vec<String>,
    /// Current processing status of the crate
    pub status: String,
    /// ISO 8601 timestamp when crate was first persisted
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Verify that CrateQueryResponse can be created with a found crate.
    #[test]
    fn test_crate_query_response_found() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let response = CrateQueryResponse {
            specifier: Some(specifier.clone()),
            features: vec!["derive".to_string()],
            status: "complete".to_string(),
            created_at: "2024-01-15T10:30:00Z".to_string(),
        };
        assert!(response.specifier.is_some());
        assert_eq!(response.features.len(), 1);
        assert_eq!(response.status, "complete");
    }

    /// Verify that CrateQueryResponse can represent a not-found result.
    #[test]
    fn test_crate_query_response_not_found() {
        let response = CrateQueryResponse {
            specifier: None,
            features: vec![],
            status: "not_found".to_string(),
            created_at: String::new(),
        };
        assert!(response.specifier.is_none());
        assert!(response.features.is_empty());
        assert_eq!(response.status, "not_found");
    }

    /// Verify that CrateQueryResponse can be cloned.
    #[test]
    fn test_crate_query_response_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let response = CrateQueryResponse {
            specifier: Some(specifier),
            features: vec!["full".to_string()],
            status: "pending".to_string(),
            created_at: "2024-01-15T10:30:00Z".to_string(),
        };
        let cloned = response.clone();
        assert_eq!(response.status, cloned.status);
        assert_eq!(response.features, cloned.features);
    }

    /// Verify that CrateQueryResponse implements Debug.
    #[test]
    fn test_crate_query_response_debug() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.75").unwrap();
        let response = CrateQueryResponse {
            specifier: Some(specifier),
            features: vec![],
            status: "downloading".to_string(),
            created_at: "2024-01-15T10:30:00Z".to_string(),
        };
        let debug_str = format!("{:?}", response);
        assert!(!debug_str.is_empty());
    }

    /// Verify that CrateQueryResponse is Send + Sync (required for actor message passing).
    #[test]
    fn test_crate_query_response_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<CrateQueryResponse>();
        assert_sync::<CrateQueryResponse>();
    }

    /// Verify that CrateQueryResponse works with multiple features.
    #[test]
    fn test_crate_query_response_multiple_features() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let response = CrateQueryResponse {
            specifier: Some(specifier),
            features: vec![
                "full".to_string(),
                "macros".to_string(),
                "rt-multi-thread".to_string(),
            ],
            status: "complete".to_string(),
            created_at: "2024-01-15T10:30:00Z".to_string(),
        };
        assert_eq!(response.features.len(), 3);
        assert!(response.features.contains(&"full".to_string()));
        assert!(response.features.contains(&"macros".to_string()));
    }
}
