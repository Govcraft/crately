//! Event broadcast when a crate download fails.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Event broadcast when a crate download fails.
///
/// This message is broadcast by CrateDownloader actor when it cannot
/// successfully download or extract a crate. This allows error handling,
/// user notification, and state cleanup to happen via subscriptions.
///
/// # Fields
///
/// * `specifier` - The crate that failed to download
/// * `features` - Feature flags that were requested
/// * `error_message` - Human-readable error description
/// * `error_kind` - Categorized error type for programmatic handling
///
/// # Message Flow
///
/// 1. CrateDownloader receives CrateReceived event
/// 2. CrateDownloader attempts download from crates.io
/// 3. Download or extraction fails
/// 4. CrateDownloader broadcasts CrateDownloadFailed event
/// 5. Multiple subscribers react:
///    - Console: Displays error message to user
///    - CrateCoordinatorActor: Updates state to "Failed"
///    - DatabaseActor: May persist failure details
///    - AlertingActor (future): May send alert for repeated failures
///
/// # Subscribers
///
/// - **Console**: User feedback - "Failed to download serde@1.0.0: network error"
/// - **CrateCoordinatorActor** (future): State tracking - "Failed"
/// - **DatabaseActor** (future): Persists failure details for audit/retry
/// - **AlertingActor** (future): Monitors failure rates and alerts
///
/// # Example
///
/// ```no_run
/// use crately::messages::{CrateDownloadFailed, DownloadErrorKind};
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let specifier = CrateSpecifier::from_str("nonexistent@1.0.0").unwrap();
/// let event = CrateDownloadFailed {
///     specifier,
///     features: vec![],
///     error_message: "Crate not found on crates.io".to_string(),
///     error_kind: DownloadErrorKind::NotFound,
/// };
/// // broker.broadcast(event).await;
/// ```
#[acton_message]
pub struct CrateDownloadFailed {
    /// The crate that failed to download
    pub specifier: CrateSpecifier,
    /// Feature flags that were requested.
    ///
    /// NOTE: Currently unused but reserved for better error context in logs and diagnostics.
    /// When Console subscribes to this event, features will be included in error messages
    /// to help users understand which feature configuration failed.
    ///
    /// See issue #55 for planned error reporting improvements.
    #[allow(dead_code)]
    pub features: Vec<String>,
    /// Human-readable error description
    pub error_message: String,
    /// Categorized error type
    pub error_kind: DownloadErrorKind,
}

/// Categories of download errors for programmatic handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadErrorKind {
    /// Crate not found on crates.io
    NotFound,
    /// Network connectivity issue
    NetworkError,
    /// Invalid tarball or extraction failure
    ExtractionError,
    /// Timeout during download
    Timeout,
    /// Other unclassified error
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_message_creation() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let message = CrateDownloadFailed {
            specifier: specifier.clone(),
            features: vec![],
            error_message: "Network timeout".to_string(),
            error_kind: DownloadErrorKind::Timeout,
        };
        assert_eq!(message.specifier, specifier);
        assert_eq!(message.error_kind, DownloadErrorKind::Timeout);
    }

    #[test]
    fn test_message_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let message = CrateDownloadFailed {
            specifier,
            features: vec![],
            error_message: "Not found".to_string(),
            error_kind: DownloadErrorKind::NotFound,
        };
        let cloned = message.clone();
        assert_eq!(message.specifier, cloned.specifier);
        assert_eq!(message.error_kind, cloned.error_kind);
        assert_eq!(message.error_message, cloned.error_message);
    }

    #[test]
    fn test_message_debug() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let message = CrateDownloadFailed {
            specifier,
            features: vec![],
            error_message: "Extraction failed".to_string(),
            error_kind: DownloadErrorKind::ExtractionError,
        };
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("CrateDownloadFailed"));
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CrateDownloadFailed>();
        assert_sync::<CrateDownloadFailed>();
    }

    #[test]
    fn test_error_kind_matching() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();
        let message = CrateDownloadFailed {
            specifier,
            features: vec![],
            error_message: "Network error".to_string(),
            error_kind: DownloadErrorKind::NetworkError,
        };
        match message.error_kind {
            DownloadErrorKind::NetworkError => {} // Correct match
            _ => panic!("Expected NetworkError"),
        }
    }

    #[test]
    fn test_error_message_content() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.0").unwrap();
        let error_msg = "Detailed error description";
        let message = CrateDownloadFailed {
            specifier,
            features: vec![],
            error_message: error_msg.to_string(),
            error_kind: DownloadErrorKind::Other,
        };
        assert_eq!(message.error_message, error_msg);
    }

    #[test]
    fn test_all_error_kinds() {
        let specifier = CrateSpecifier::from_str("test@1.0.0").unwrap();

        let kinds = vec![
            DownloadErrorKind::NotFound,
            DownloadErrorKind::NetworkError,
            DownloadErrorKind::ExtractionError,
            DownloadErrorKind::Timeout,
            DownloadErrorKind::Other,
        ];

        for kind in kinds {
            let message = CrateDownloadFailed {
                specifier: specifier.clone(),
                features: vec![],
                error_message: "Test".to_string(),
                error_kind: kind.clone(),
            };
            assert_eq!(message.error_kind, kind);
        }
    }
}
