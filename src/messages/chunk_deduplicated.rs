use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Broadcast when duplicate chunk is detected during graph analysis
///
/// This event signals that deduplication logic has identified a duplicate chunk
/// and resolved it to reuse an existing node in the content graph.
#[acton_message(raw)]
#[derive(Clone)]
pub struct ChunkDeduplicated {
    /// The crate being processed
    pub specifier: CrateSpecifier,
    /// Build identifier
    pub build_id: String,
    /// Duplicate chunk identifier
    pub duplicate_chunk_id: String,
    /// Content hash of the duplicate
    pub content_hash: String,
    /// Original chunk identifier that will be reused
    pub original_chunk_id: String,
    /// Total duplicates found so far in this build
    pub total_duplicates: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_chunk_deduplicated_construction() {
        let specifier = CrateSpecifier::from_str("duplicate@1.0.0").unwrap();
        let msg = ChunkDeduplicated {
            specifier: specifier.clone(),
            build_id: "build_dedup".to_string(),
            duplicate_chunk_id: "chunk_005".to_string(),
            content_hash: "abc123def456".to_string(),
            original_chunk_id: "chunk_002".to_string(),
            total_duplicates: 3,
        };

        assert_eq!(msg.specifier, specifier);
        assert_eq!(msg.build_id, "build_dedup");
        assert_eq!(msg.duplicate_chunk_id, "chunk_005");
        assert_eq!(msg.content_hash, "abc123def456");
        assert_eq!(msg.original_chunk_id, "chunk_002");
        assert_eq!(msg.total_duplicates, 3);
    }

    #[test]
    fn test_chunk_deduplicated_first_duplicate() {
        let specifier = CrateSpecifier::from_str("first@1.0.0").unwrap();
        let msg = ChunkDeduplicated {
            specifier,
            build_id: "build_first_dup".to_string(),
            duplicate_chunk_id: "chunk_010".to_string(),
            content_hash: "hash_first_dup".to_string(),
            original_chunk_id: "chunk_005".to_string(),
            total_duplicates: 1,
        };

        assert_eq!(msg.total_duplicates, 1);
    }

    #[test]
    fn test_chunk_deduplicated_many_duplicates() {
        let specifier = CrateSpecifier::from_str("many@1.0.0").unwrap();
        let msg = ChunkDeduplicated {
            specifier,
            build_id: "build_many".to_string(),
            duplicate_chunk_id: "chunk_100".to_string(),
            content_hash: "hash_common".to_string(),
            original_chunk_id: "chunk_001".to_string(),
            total_duplicates: 50,
        };

        assert_eq!(msg.total_duplicates, 50);
    }

    #[test]
    fn test_chunk_deduplicated_clone() {
        let specifier = CrateSpecifier::from_str("cloneable@1.0.0").unwrap();
        let msg = ChunkDeduplicated {
            specifier: specifier.clone(),
            build_id: "build_clone".to_string(),
            duplicate_chunk_id: "chunk_020".to_string(),
            content_hash: "hash_clone".to_string(),
            original_chunk_id: "chunk_010".to_string(),
            total_duplicates: 5,
        };

        let cloned = msg.clone();
        assert_eq!(msg.specifier, cloned.specifier);
        assert_eq!(msg.build_id, cloned.build_id);
        assert_eq!(msg.duplicate_chunk_id, cloned.duplicate_chunk_id);
        assert_eq!(msg.content_hash, cloned.content_hash);
        assert_eq!(msg.original_chunk_id, cloned.original_chunk_id);
        assert_eq!(msg.total_duplicates, cloned.total_duplicates);
    }

    #[test]
    fn test_chunk_deduplicated_same_hash_different_chunks() {
        let specifier = CrateSpecifier::from_str("same_hash@1.0.0").unwrap();
        let msg = ChunkDeduplicated {
            specifier,
            build_id: "build_same_hash".to_string(),
            duplicate_chunk_id: "chunk_015".to_string(),
            content_hash: "shared_hash_value".to_string(),
            original_chunk_id: "chunk_003".to_string(),
            total_duplicates: 2,
        };

        assert_ne!(msg.duplicate_chunk_id, msg.original_chunk_id);
        assert_eq!(msg.content_hash, "shared_hash_value");
    }

    #[test]
    fn test_chunk_deduplicated_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ChunkDeduplicated>();
        assert_sync::<ChunkDeduplicated>();
    }
}
