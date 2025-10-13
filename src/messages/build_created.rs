use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Broadcast when a new content graph build is created
///
/// This event signals that graph-based content analysis has started for a crate.
/// The build will track all chunks, compute content hashes, and perform deduplication.
#[acton_message(raw)]
#[derive(Clone)]
pub struct BuildCreated {
    /// The crate being analyzed
    pub specifier: CrateSpecifier,
    /// Unique build identifier for this analysis session
    pub build_id: String,
    /// Number of chunks to be analyzed
    pub total_chunks: u32,
    /// Timestamp when build was created
    pub created_at: std::time::SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use std::time::SystemTime;

    #[test]
    fn test_build_created_construction() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let build_id = "build_abc123".to_string();
        let total_chunks = 42;
        let created_at = SystemTime::now();

        let msg = BuildCreated {
            specifier: specifier.clone(),
            build_id: build_id.clone(),
            total_chunks,
            created_at,
        };

        assert_eq!(msg.specifier, specifier);
        assert_eq!(msg.build_id, build_id);
        assert_eq!(msg.total_chunks, total_chunks);
        assert_eq!(msg.created_at, created_at);
    }

    #[test]
    fn test_build_created_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let msg = BuildCreated {
            specifier: specifier.clone(),
            build_id: "build_xyz789".to_string(),
            total_chunks: 100,
            created_at: SystemTime::now(),
        };

        let cloned = msg.clone();
        assert_eq!(msg.specifier, cloned.specifier);
        assert_eq!(msg.build_id, cloned.build_id);
        assert_eq!(msg.total_chunks, cloned.total_chunks);
        assert_eq!(msg.created_at, cloned.created_at);
    }

    #[test]
    fn test_build_created_zero_chunks() {
        let specifier = CrateSpecifier::from_str("empty@0.1.0").unwrap();
        let msg = BuildCreated {
            specifier,
            build_id: "build_empty".to_string(),
            total_chunks: 0,
            created_at: SystemTime::now(),
        };

        assert_eq!(msg.total_chunks, 0);
    }

    #[test]
    fn test_build_created_large_chunk_count() {
        let specifier = CrateSpecifier::from_str("large@2.0.0").unwrap();
        let msg = BuildCreated {
            specifier,
            build_id: "build_large".to_string(),
            total_chunks: 10_000,
            created_at: SystemTime::now(),
        };

        assert_eq!(msg.total_chunks, 10_000);
    }

    #[test]
    fn test_build_created_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<BuildCreated>();
        assert_sync::<BuildCreated>();
    }
}
