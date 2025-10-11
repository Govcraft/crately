//! Message requesting a paginated list of all crates from the database.

use acton_reactive::prelude::*;

/// Request to retrieve a paginated list of all crates in the database.
///
/// This message is sent to the DatabaseActor to request a list of crate summaries.
/// The response includes pagination support via limit and offset parameters.
/// The DatabaseActor will broadcast a `CrateListResponse` message with the results.
///
/// # Fields
///
/// * `limit` - Maximum number of results to return (default: 100 if None)
/// * `offset` - Number of records to skip for pagination (default: 0 if None)
///
/// # Example
///
/// ```no_run
/// use crately::messages::ListCrates;
///
/// // Request first page with default limit (100)
/// let list_msg = ListCrates {
///     limit: None,
///     offset: None,
/// };
/// // database_handle.send(list_msg).await;
///
/// // Request specific page with custom limit
/// let paginated = ListCrates {
///     limit: Some(50),
///     offset: Some(100), // Skip first 100 records
/// };
/// // database_handle.send(paginated).await;
///
/// // Request first 10 records
/// let small_page = ListCrates {
///     limit: Some(10),
///     offset: Some(0),
/// };
/// // database_handle.send(small_page).await;
/// ```
///
/// # Message Flow
///
/// 1. Client sends ListCrates to DatabaseActor
/// 2. DatabaseActor queries crate table with limit and offset
/// 3. DatabaseActor counts total crates for pagination metadata
/// 4. DatabaseActor broadcasts CrateListResponse with results
/// 5. Subscribers receive the list and pagination details
///
/// # Pagination
///
/// The offset and limit fields work together to enable pagination:
/// - **offset**: Skip this many records from the start
/// - **limit**: Return at most this many records
///
/// Example pagination pattern:
/// - Page 1: `offset=0, limit=50`
/// - Page 2: `offset=50, limit=50`
/// - Page 3: `offset=100, limit=50`
#[acton_message]
pub struct ListCrates {
    /// Maximum number of results to return (default 100)
    pub limit: Option<u32>,
    /// Pagination offset - number of records to skip (default 0)
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that ListCrates can be created with default pagination.
    #[test]
    fn test_list_crates_default_pagination() {
        let list = ListCrates {
            limit: None,
            offset: None,
        };
        assert!(list.limit.is_none());
        assert!(list.offset.is_none());
    }

    /// Verify that ListCrates can be created with custom limit.
    #[test]
    fn test_list_crates_with_limit() {
        let list = ListCrates {
            limit: Some(50),
            offset: None,
        };
        assert_eq!(list.limit, Some(50));
        assert!(list.offset.is_none());
    }

    /// Verify that ListCrates can be created with offset for pagination.
    #[test]
    fn test_list_crates_with_offset() {
        let list = ListCrates {
            limit: Some(100),
            offset: Some(200),
        };
        assert_eq!(list.limit, Some(100));
        assert_eq!(list.offset, Some(200));
    }

    /// Verify that ListCrates can be cloned.
    #[test]
    fn test_list_crates_clone() {
        let list = ListCrates {
            limit: Some(25),
            offset: Some(50),
        };
        let cloned = list.clone();
        assert_eq!(list.limit, cloned.limit);
        assert_eq!(list.offset, cloned.offset);
    }

    /// Verify that ListCrates implements Debug.
    #[test]
    fn test_list_crates_debug() {
        let list = ListCrates {
            limit: Some(10),
            offset: Some(0),
        };
        let debug_str = format!("{:?}", list);
        assert!(!debug_str.is_empty());
    }

    /// Verify that ListCrates is Send + Sync (required for actor message passing).
    #[test]
    fn test_list_crates_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<ListCrates>();
        assert_sync::<ListCrates>();
    }

    /// Verify edge case: zero offset is valid.
    #[test]
    fn test_list_crates_zero_offset() {
        let list = ListCrates {
            limit: Some(100),
            offset: Some(0),
        };
        assert_eq!(list.offset, Some(0));
    }

    /// Verify edge case: zero limit is valid (though may return empty list).
    #[test]
    fn test_list_crates_zero_limit() {
        let list = ListCrates {
            limit: Some(0),
            offset: None,
        };
        assert_eq!(list.limit, Some(0));
    }
}
