//! Message requesting retrieval of documentation chunks by crate specifier.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Request to query all documentation chunks for a specific crate.
///
/// This message is sent to the DatabaseActor to retrieve all chunk records
/// for a given crate. The DatabaseActor will query the doc_chunk table and
/// broadcast a `DocChunksQueryResponse` message with the chunk data.
///
/// # Fields
///
/// * `specifier` - The crate name and version to query chunks for
///
/// # Message Flow
///
/// 1. VectorizerActor sends QueryDocChunks to DatabaseActor
/// 2. DatabaseActor queries doc_chunk table by crate_id
/// 3. DatabaseActor orders chunks by chunk_index for consistent processing
/// 4. DatabaseActor broadcasts DocChunksQueryResponse with chunk data
/// 5. VectorizerActor receives chunks and generates embeddings
///
/// # Example
///
/// ```no_run
/// use crately::messages::QueryDocChunks;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
/// let query_msg = QueryDocChunks { specifier };
/// // database_handle.send(query_msg).await;
/// ```
#[acton_message]
pub struct QueryDocChunks {
    /// The crate to query chunks for
    pub specifier: CrateSpecifier,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Verify that QueryDocChunks can be created with a valid specifier.
    #[test]
    fn test_query_doc_chunks_creation() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let query = QueryDocChunks { specifier: specifier.clone() };
        assert_eq!(query.specifier.name(), "serde");
        assert_eq!(query.specifier.version().to_string(), "1.0.0");
    }

    /// Verify that QueryDocChunks can be cloned.
    #[test]
    fn test_query_doc_chunks_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let query = QueryDocChunks { specifier: specifier.clone() };
        let cloned = query.clone();
        assert_eq!(query.specifier.name(), cloned.specifier.name());
        assert_eq!(query.specifier.version(), cloned.specifier.version());
    }

    /// Verify that QueryDocChunks implements Debug.
    #[test]
    fn test_query_doc_chunks_debug() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.75").unwrap();
        let query = QueryDocChunks { specifier };
        let debug_str = format!("{:?}", query);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("QueryDocChunks"));
    }

    /// Verify that QueryDocChunks is Send + Sync (required for actor message passing).
    #[test]
    fn test_query_doc_chunks_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<QueryDocChunks>();
        assert_sync::<QueryDocChunks>();
    }
}
