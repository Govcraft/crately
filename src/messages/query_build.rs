//! QueryBuild - Request to query build information by BuildId
//!
//! This message is sent to DatabaseActor to retrieve complete information about
//! a specific build, including its metadata, associated chunks, and processing status.

use crate::types::BuildId;
use acton_reactive::prelude::*;

/// Request to query build information from the database
///
/// This message requests complete information about a specific build from the
/// DatabaseActor. The response is broadcast via BuildQueryResponse, following
/// the pub/sub pattern to decouple query from response handling.
///
/// # Query Pattern
///
/// This follows the query-response pattern in the actor system:
/// 1. Requester sends QueryBuild to DatabaseActor
/// 2. DatabaseActor performs database query
/// 3. DatabaseActor broadcasts BuildQueryResponse
/// 4. Requester (and any other subscribers) receives response
///
/// # Fields
///
/// * `build_id` - Unique identifier of the build to query
/// * `response_id` - Optional correlation ID for request/response matching
/// * `include_chunks` - Whether to include associated chunk information
/// * `include_metrics` - Whether to include processing metrics
///
/// # Example
///
/// ```no_run
/// use crately::messages::QueryBuild;
/// use crately::types::BuildId;
///
/// let query = QueryBuild {
///     build_id: BuildId::try_from("build_01HZQKR9VF8P6QXWM7YJDG2K4N".to_string()).unwrap(),
///     response_id: Some("req_12345".to_string()),
///     include_chunks: true,
///     include_metrics: false,
/// };
/// // database_handle.send(query).await;
/// ```
///
/// # Message Flow
///
/// 1. Actor sends QueryBuild to DatabaseActor
/// 2. DatabaseActor queries: `SELECT * FROM build WHERE id = $build_id`
/// 3. If `include_chunks`, join with chunk references
/// 4. If `include_metrics`, compute processing statistics
/// 5. DatabaseActor broadcasts BuildQueryResponse
/// 6. Requester receives response via subscription
///
/// # Subscribers
///
/// - **DatabaseActor**: Primary handler, performs query and broadcasts response
#[acton_message]
pub struct QueryBuild {
    /// Unique identifier of the build to query
    pub build_id: BuildId,

    /// Optional correlation ID for matching request to response
    /// Useful when multiple queries are in flight simultaneously
    pub response_id: Option<String>,

    /// Whether to include associated chunk information in response
    pub include_chunks: bool,

    /// Whether to include processing metrics (deduplication rate, timing, etc.)
    pub include_metrics: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_build_minimal() {
        let build_id = BuildId::new();

        let query = QueryBuild {
            build_id: build_id.clone(),
            response_id: None,
            include_chunks: false,
            include_metrics: false,
        };

        assert_eq!(query.build_id, build_id);
        assert!(query.response_id.is_none());
        assert!(!query.include_chunks);
        assert!(!query.include_metrics);
    }

    #[test]
    fn test_query_build_with_response_id() {
        let query = QueryBuild {
            build_id: BuildId::new(),
            response_id: Some("req_12345".to_string()),
            include_chunks: false,
            include_metrics: false,
        };

        assert_eq!(query.response_id, Some("req_12345".to_string()));
    }

    #[test]
    fn test_query_build_include_all() {
        let query = QueryBuild {
            build_id: BuildId::new(),
            response_id: Some("req_full".to_string()),
            include_chunks: true,
            include_metrics: true,
        };

        assert!(query.include_chunks);
        assert!(query.include_metrics);
    }

    #[test]
    fn test_query_build_include_chunks_only() {
        let query = QueryBuild {
            build_id: BuildId::new(),
            response_id: None,
            include_chunks: true,
            include_metrics: false,
        };

        assert!(query.include_chunks);
        assert!(!query.include_metrics);
    }

    #[test]
    fn test_query_build_include_metrics_only() {
        let query = QueryBuild {
            build_id: BuildId::new(),
            response_id: None,
            include_chunks: false,
            include_metrics: true,
        };

        assert!(!query.include_chunks);
        assert!(query.include_metrics);
    }

    #[test]
    fn test_query_build_parsed_build_id() {
        let build_id = BuildId::try_from("build_01HZQKR9VF8P6QXWM7YJDG2K4N".to_string())
            .expect("Should parse valid BuildId");

        let query = QueryBuild {
            build_id: build_id.clone(),
            response_id: None,
            include_chunks: false,
            include_metrics: false,
        };

        assert_eq!(query.build_id.as_str(), "build_01HZQKR9VF8P6QXWM7YJDG2K4N");
    }

    #[test]
    fn test_query_build_clone() {
        let original = QueryBuild {
            build_id: BuildId::new(),
            response_id: Some("req_clone".to_string()),
            include_chunks: true,
            include_metrics: false,
        };

        let cloned = original.clone();
        assert_eq!(original.build_id, cloned.build_id);
        assert_eq!(original.response_id, cloned.response_id);
        assert_eq!(original.include_chunks, cloned.include_chunks);
        assert_eq!(original.include_metrics, cloned.include_metrics);
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
            build_id: BuildId::new(),
            response_id: Some("req_debug".to_string()),
            include_chunks: true,
            include_metrics: true,
        };

        let debug_str = format!("{:?}", query);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("QueryBuild"));
    }

    #[test]
    fn test_query_build_various_response_ids() {
        let response_ids = vec![
            "req_001",
            "query_abc123",
            "correlation_uuid_xyz",
            "build_query_42",
        ];

        for response_id in response_ids {
            let query = QueryBuild {
                build_id: BuildId::new(),
                response_id: Some(response_id.to_string()),
                include_chunks: false,
                include_metrics: false,
            };
            assert_eq!(query.response_id, Some(response_id.to_string()));
        }
    }

    #[test]
    fn test_query_build_empty_response_id() {
        let query = QueryBuild {
            build_id: BuildId::new(),
            response_id: Some(String::new()),
            include_chunks: false,
            include_metrics: false,
        };

        assert_eq!(query.response_id, Some(String::new()));
    }

    #[test]
    fn test_query_build_flag_combinations() {
        let combinations = vec![
            (false, false),
            (true, false),
            (false, true),
            (true, true),
        ];

        for (chunks, metrics) in combinations {
            let query = QueryBuild {
                build_id: BuildId::new(),
                response_id: None,
                include_chunks: chunks,
                include_metrics: metrics,
            };
            assert_eq!(query.include_chunks, chunks);
            assert_eq!(query.include_metrics, metrics);
        }
    }
}
