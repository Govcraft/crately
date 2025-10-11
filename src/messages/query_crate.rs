//! Message requesting retrieval of crate metadata from the database.

use acton_reactive::prelude::*;

/// Request to query a crate record by name and optionally by version.
///
/// This message is sent to the DatabaseActor to query for a specific crate's
/// metadata. The DatabaseActor will search the database and broadcast a
/// `CrateQueryResponse` message with the requested data or None if not found.
///
/// # Fields
///
/// * `name` - The crate name to query
/// * `version` - Optional version string. If `None`, returns the latest version
///
/// # Example
///
/// ```no_run
/// use crately::messages::QueryCrate;
///
/// // Query specific version
/// let query_msg = QueryCrate {
///     name: "serde".to_string(),
///     version: Some("1.0.0".to_string()),
/// };
/// // database_handle.send(query_msg).await;
///
/// // Query latest version
/// let query_latest = QueryCrate {
///     name: "tokio".to_string(),
///     version: None,
/// };
/// // database_handle.send(query_latest).await;
/// ```
///
/// # Message Flow
///
/// 1. Client sends QueryCrate to DatabaseActor
/// 2. DatabaseActor searches crate table by name and version
/// 3. If version is None, DatabaseActor returns latest version
/// 4. DatabaseActor broadcasts CrateQueryResponse with results
/// 5. Subscribers receive the query results
#[acton_message]
pub struct QueryCrate {
    /// The name of the crate to query
    pub name: String,
    /// The version of the crate to query (None = latest version)
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that QueryCrate can be created with a specific version.
    #[test]
    fn test_query_crate_with_version() {
        let query = QueryCrate {
            name: "serde".to_string(),
            version: Some("1.0.0".to_string()),
        };
        assert_eq!(query.name, "serde");
        assert_eq!(query.version, Some("1.0.0".to_string()));
    }

    /// Verify that QueryCrate can be created without a version (latest).
    #[test]
    fn test_query_crate_latest_version() {
        let query = QueryCrate {
            name: "tokio".to_string(),
            version: None,
        };
        assert_eq!(query.name, "tokio");
        assert!(query.version.is_none());
    }

    /// Verify that QueryCrate can be cloned.
    #[test]
    fn test_query_crate_clone() {
        let query = QueryCrate {
            name: "anyhow".to_string(),
            version: Some("1.0.75".to_string()),
        };
        let cloned = query.clone();
        assert_eq!(query.name, cloned.name);
        assert_eq!(query.version, cloned.version);
    }

    /// Verify that QueryCrate implements Debug.
    #[test]
    fn test_query_crate_debug() {
        let query = QueryCrate {
            name: "axum".to_string(),
            version: Some("0.7.0".to_string()),
        };
        let debug_str = format!("{:?}", query);
        assert!(!debug_str.is_empty());
    }

    /// Verify that QueryCrate is Send + Sync (required for actor message passing).
    #[test]
    fn test_query_crate_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<QueryCrate>();
        assert_sync::<QueryCrate>();
    }
}
