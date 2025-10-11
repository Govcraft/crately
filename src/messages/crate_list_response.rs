//! Event broadcast with paginated list of crates from the database.

use acton_reactive::prelude::*;

/// Summarized information about a single crate for list responses.
///
/// This helper type provides a lightweight representation of a crate for list operations.
/// It contains only essential identifying information, avoiding the overhead of full
/// crate metadata when displaying lists.
///
/// # Fields
///
/// * `name` - The crate name
/// * `version` - The crate version
/// * `status` - Current processing status ("pending", "downloading", "complete", etc.)
///
/// # Example
///
/// ```no_run
/// use crately::messages::CrateSummary;
///
/// let summary = CrateSummary {
///     name: "serde".to_string(),
///     version: "1.0.0".to_string(),
///     status: "complete".to_string(),
/// };
/// ```
#[acton_message]
pub struct CrateSummary {
    /// The crate name
    pub name: String,
    /// The crate version
    pub version: String,
    /// Current processing status
    pub status: String,
}

/// Event broadcast containing a paginated list of crate summaries.
///
/// This message is broadcast via the broker by DatabaseActor in response to a
/// `ListCrates` request. It contains an array of crate summaries along with
/// pagination metadata indicating the total number of crates available.
///
/// # Fields
///
/// * `crates` - Vector of crate summaries matching the query
/// * `total_count` - Total number of crates in the database (for pagination UI)
///
/// # Example
///
/// ```no_run
/// use crately::messages::{CrateListResponse, CrateSummary};
///
/// let response = CrateListResponse {
///     crates: vec![
///         CrateSummary {
///             name: "serde".to_string(),
///             version: "1.0.0".to_string(),
///             status: "complete".to_string(),
///         },
///         CrateSummary {
///             name: "tokio".to_string(),
///             version: "1.35.0".to_string(),
///             status: "pending".to_string(),
///         },
///     ],
///     total_count: 150, // Total crates in database
/// };
/// // broker.broadcast(response).await;
/// ```
///
/// # Message Flow
///
/// 1. Client sends ListCrates to DatabaseActor with pagination params
/// 2. DatabaseActor queries crate table with LIMIT and OFFSET
/// 3. DatabaseActor counts total crates for pagination metadata
/// 4. DatabaseActor broadcasts CrateListResponse with results
/// 5. Subscribers (Console, ServerActor) receive the list
///
/// # Pagination Calculation
///
/// Use `total_count` to calculate pagination:
/// ```ignore
/// let total_pages = (response.total_count + limit - 1) / limit;
/// let has_next_page = offset + response.crates.len() < response.total_count;
/// ```
///
/// # Subscribers
///
/// - **Console**: Displays crate list to user with pagination info
/// - **ServerActor**: Constructs JSON response with pagination metadata
#[acton_message]
pub struct CrateListResponse {
    /// Vector of crate summaries for this page
    pub crates: Vec<CrateSummary>,
    /// Total number of crates in the database
    pub total_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that CrateSummary can be created and cloned.
    #[test]
    fn test_crate_summary_creation() {
        let summary = CrateSummary {
            name: "serde".to_string(),
            version: "1.0.0".to_string(),
            status: "complete".to_string(),
        };
        let cloned = summary.clone();
        assert_eq!(summary.name, cloned.name);
        assert_eq!(summary.version, cloned.version);
        assert_eq!(summary.status, cloned.status);
    }

    /// Verify that CrateSummary implements Debug.
    #[test]
    fn test_crate_summary_debug() {
        let summary = CrateSummary {
            name: "tokio".to_string(),
            version: "1.35.0".to_string(),
            status: "pending".to_string(),
        };
        let debug_str = format!("{:?}", summary);
        assert!(!debug_str.is_empty());
    }

    /// Verify that CrateListResponse can be created with multiple summaries.
    #[test]
    fn test_crate_list_response_creation() {
        let response = CrateListResponse {
            crates: vec![
                CrateSummary {
                    name: "serde".to_string(),
                    version: "1.0.0".to_string(),
                    status: "complete".to_string(),
                },
                CrateSummary {
                    name: "tokio".to_string(),
                    version: "1.35.0".to_string(),
                    status: "downloading".to_string(),
                },
            ],
            total_count: 150,
        };
        assert_eq!(response.crates.len(), 2);
        assert_eq!(response.total_count, 150);
    }

    /// Verify that CrateListResponse can handle empty results.
    #[test]
    fn test_crate_list_response_empty() {
        let response = CrateListResponse {
            crates: vec![],
            total_count: 0,
        };
        assert!(response.crates.is_empty());
        assert_eq!(response.total_count, 0);
    }

    /// Verify that CrateListResponse can be cloned.
    #[test]
    fn test_crate_list_response_clone() {
        let response = CrateListResponse {
            crates: vec![CrateSummary {
                name: "anyhow".to_string(),
                version: "1.0.75".to_string(),
                status: "complete".to_string(),
            }],
            total_count: 42,
        };
        let cloned = response.clone();
        assert_eq!(response.crates.len(), cloned.crates.len());
        assert_eq!(response.total_count, cloned.total_count);
    }

    /// Verify that CrateListResponse implements Debug.
    #[test]
    fn test_crate_list_response_debug() {
        let response = CrateListResponse {
            crates: vec![CrateSummary {
                name: "axum".to_string(),
                version: "0.7.0".to_string(),
                status: "pending".to_string(),
            }],
            total_count: 100,
        };
        let debug_str = format!("{:?}", response);
        assert!(!debug_str.is_empty());
    }

    /// Verify that types are Send + Sync (required for actor message passing).
    #[test]
    fn test_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<CrateSummary>();
        assert_sync::<CrateSummary>();
        assert_send::<CrateListResponse>();
        assert_sync::<CrateListResponse>();
    }

    /// Verify that CrateListResponse handles large total_count.
    #[test]
    fn test_crate_list_response_large_total() {
        let response = CrateListResponse {
            crates: vec![CrateSummary {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
                status: "complete".to_string(),
            }],
            total_count: 1_000_000,
        };
        assert_eq!(response.total_count, 1_000_000);
    }

    /// Verify that CrateSummary fields are accessible.
    #[test]
    fn test_crate_summary_fields() {
        let summary = CrateSummary {
            name: "regex".to_string(),
            version: "1.10.0".to_string(),
            status: "complete".to_string(),
        };
        assert_eq!(summary.name, "regex");
        assert_eq!(summary.version, "1.10.0");
        assert_eq!(summary.status, "complete");
    }
}
