use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Broadcast when content hash is computed for a chunk
///
/// This event provides progress visibility into graph-based deduplication.
/// Multiple subscribers can track progress, display stats, or persist metadata.
#[acton_message(raw)]
#[derive(Clone)]
pub struct ChunkHashComputed {
    /// The crate being processed
    pub specifier: CrateSpecifier,
    /// Build identifier
    pub build_id: String,
    /// Chunk identifier
    pub _chunk_id: String,
    /// Content hash (SHA-256 hex)
    pub _content_hash: String,
    /// Progress: how many chunks have been hashed so far
    pub hashed_count: u32,
    /// Total chunks in this build
    pub total_chunks: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_chunk_hash_computed_construction() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let msg = ChunkHashComputed {
            specifier: specifier.clone(),
            build_id: "build_123".to_string(),
            _chunk_id: "chunk_001".to_string(),
            _content_hash: "abc123def456".to_string(),
            hashed_count: 5,
            total_chunks: 20,
        };

        assert_eq!(msg.specifier, specifier);
        assert_eq!(msg.build_id, "build_123");
        assert_eq!(msg._chunk_id, "chunk_001");
        assert_eq!(msg._content_hash, "abc123def456");
        assert_eq!(msg.hashed_count, 5);
        assert_eq!(msg.total_chunks, 20);
    }

    #[test]
    fn test_chunk_hash_computed_first_chunk() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let msg = ChunkHashComputed {
            specifier,
            build_id: "build_first".to_string(),
            _chunk_id: "chunk_000".to_string(),
            _content_hash: "hash_first".to_string(),
            hashed_count: 1,
            total_chunks: 50,
        };

        assert_eq!(msg.hashed_count, 1);
        assert_eq!(msg.total_chunks, 50);
    }

    #[test]
    fn test_chunk_hash_computed_last_chunk() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let total = 100;
        let msg = ChunkHashComputed {
            specifier,
            build_id: "build_last".to_string(),
            _chunk_id: "chunk_099".to_string(),
            _content_hash: "hash_last".to_string(),
            hashed_count: total,
            total_chunks: total,
        };

        assert_eq!(msg.hashed_count, msg.total_chunks);
    }

    #[test]
    fn test_chunk_hash_computed_progress_tracking() {
        let specifier = CrateSpecifier::from_str("test@1.0.0").unwrap();
        let msg = ChunkHashComputed {
            specifier,
            build_id: "build_progress".to_string(),
            _chunk_id: "chunk_010".to_string(),
            _content_hash: "hash_progress".to_string(),
            hashed_count: 10,
            total_chunks: 25,
        };

        assert!(msg.hashed_count < msg.total_chunks);
        let progress_percent = (msg.hashed_count as f64 / msg.total_chunks as f64) * 100.0;
        assert_eq!(progress_percent, 40.0);
    }

    #[test]
    fn test_chunk_hash_computed_clone() {
        let specifier = CrateSpecifier::from_str("cloneable@1.0.0").unwrap();
        let msg = ChunkHashComputed {
            specifier: specifier.clone(),
            build_id: "build_clone".to_string(),
            _chunk_id: "chunk_005".to_string(),
            _content_hash: "hash_clone".to_string(),
            hashed_count: 7,
            total_chunks: 15,
        };

        let cloned = msg.clone();
        assert_eq!(msg.specifier, cloned.specifier);
        assert_eq!(msg.build_id, cloned.build_id);
        assert_eq!(msg._chunk_id, cloned._chunk_id);
        assert_eq!(msg._content_hash, cloned._content_hash);
        assert_eq!(msg.hashed_count, cloned.hashed_count);
        assert_eq!(msg.total_chunks, cloned.total_chunks);
    }

    #[test]
    fn test_chunk_hash_computed_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ChunkHashComputed>();
        assert_sync::<ChunkHashComputed>();
    }
}
