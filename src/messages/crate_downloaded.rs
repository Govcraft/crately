//! Event broadcast when a crate is successfully downloaded and extracted.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;
use std::path::PathBuf;

/// Event broadcast when a crate is successfully downloaded and extracted.
///
/// This message is broadcast by CrateDownloader actor after successfully
/// downloading the crate tarball from crates.io and extracting it to the
/// filesystem. It signals that files are ready for the next pipeline stage.
///
/// # Fields
///
/// * `specifier` - The crate name and version that was downloaded
/// * `features` - Feature flags for this crate (passed through pipeline)
/// * `extracted_path` - Filesystem path where crate was extracted
/// * `download_duration_ms` - Time taken to download and extract (for metrics)
///
/// # Message Flow
///
/// 1. CrateDownloader receives CrateReceived event
/// 2. CrateDownloader downloads tarball from crates.io
/// 3. CrateDownloader extracts tarball to filesystem
/// 4. CrateDownloader broadcasts CrateDownloaded event
/// 5. Multiple subscribers react:
///    - Console: Displays success message with timing
///    - FileReaderActor: Begins reading extracted files (next stage)
///    - CrateCoordinatorActor: Updates state to "Downloaded"
///    - MetricsActor (future): Records download timing
///
/// # Subscribers
///
/// - **Console**: User feedback - "Downloaded serde@1.0.0 (1.2s)"
/// - **FileReaderActor**: Triggers documentation extraction (primary pipeline progression)
/// - **CrateCoordinatorActor** (future): State tracking - "Downloaded"
/// - **MetricsActor** (future): Records download timing and success rate
///
/// # Example
///
/// ```no_run
/// use crately::messages::CrateDownloaded;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::path::PathBuf;
/// use std::str::FromStr;
///
/// let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
/// let event = CrateDownloaded {
///     specifier,
///     features: vec!["full".to_string()],
///     extracted_path: PathBuf::from("/tmp/crates/tokio-1.35.0"),
///     download_duration_ms: 1234,
/// };
/// // broker.broadcast(event).await;
/// ```
#[acton_message]
pub struct CrateDownloaded {
    /// The crate that was downloaded
    pub specifier: CrateSpecifier,
    /// Feature flags for this crate
    pub features: Vec<String>,
    /// Path where crate was extracted
    pub extracted_path: PathBuf,
    /// Download and extraction time in milliseconds.
    ///
    /// NOTE: Currently unused but reserved for future performance metrics collection.
    /// Planned use cases:
    /// - Console: Display timing in success messages ("Downloaded in 1.2s")
    /// - MetricsActor: Track download performance over time
    /// - DatabaseActor: Store timing data for analytics
    ///
    /// See issue #55 for planned metrics implementation.
    #[allow(dead_code)]
    pub download_duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_message_creation() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let message = CrateDownloaded {
            specifier: specifier.clone(),
            features: vec!["derive".to_string()],
            extracted_path: PathBuf::from("/tmp/serde-1.0.0"),
            download_duration_ms: 500,
        };
        assert_eq!(message.specifier, specifier);
        assert_eq!(message.download_duration_ms, 500);
    }

    #[test]
    fn test_message_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let message = CrateDownloaded {
            specifier,
            features: vec![],
            extracted_path: PathBuf::from("/tmp/tokio"),
            download_duration_ms: 1000,
        };
        let cloned = message.clone();
        assert_eq!(message.specifier, cloned.specifier);
        assert_eq!(message.extracted_path, cloned.extracted_path);
        assert_eq!(message.download_duration_ms, cloned.download_duration_ms);
    }

    #[test]
    fn test_message_debug() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let message = CrateDownloaded {
            specifier,
            features: vec![],
            extracted_path: PathBuf::from("/tmp/axum"),
            download_duration_ms: 750,
        };
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("CrateDownloaded"));
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CrateDownloaded>();
        assert_sync::<CrateDownloaded>();
    }

    #[test]
    fn test_pathbuf_handling() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();
        let path = PathBuf::from("/var/tmp/crates/tracing-0.1.0");
        let message = CrateDownloaded {
            specifier,
            features: vec![],
            extracted_path: path.clone(),
            download_duration_ms: 200,
        };
        assert_eq!(message.extracted_path, path);
        assert!(message.extracted_path.is_absolute());
    }

    #[test]
    fn test_timing_metrics() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.0").unwrap();
        let message = CrateDownloaded {
            specifier,
            features: vec![],
            extracted_path: PathBuf::from("/tmp/anyhow"),
            download_duration_ms: 0, // Edge case: instant download
        };
        assert_eq!(message.download_duration_ms, 0);
    }
}
