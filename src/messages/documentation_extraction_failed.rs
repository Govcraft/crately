//! Event broadcast when documentation extraction fails.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Event broadcast when documentation extraction fails.
///
/// This message is broadcast by FileReaderActor when it cannot successfully
/// extract documentation from a downloaded crate directory. This allows error
/// handling, user notification, and state cleanup to happen via subscriptions.
///
/// # Fields
///
/// * `specifier` - The crate that failed documentation extraction
/// * `features` - Feature flags that were requested
/// * `error_message` - Human-readable error description
/// * `error_kind` - Categorized error type for programmatic handling
/// * `elapsed_ms` - Time taken before failure (for metrics)
///
/// # Message Flow
///
/// 1. FileReaderActor receives CrateDownloaded event
/// 2. FileReaderActor attempts to read documentation files
/// 3. Extraction fails due to missing files, permission issues, etc.
/// 4. FileReaderActor broadcasts DocumentationExtractionFailed event
/// 5. Multiple subscribers react:
///    - Console: Displays error message to user
///    - CrateCoordinatorActor: Updates state to "ExtractionFailed"
///    - DatabaseActor: May persist failure details
///    - RetryCoordinator (future): May schedule retry for transient errors
///
/// # Subscribers
///
/// - **Console**: User feedback - "Failed to extract docs from serde@1.0.0: file not found"
/// - **CrateCoordinatorActor** (future): State tracking - "ExtractionFailed"
/// - **DatabaseActor** (future): Persists failure details for audit/retry
/// - **RetryCoordinator** (future): Schedules retries for transient failures
///
/// # Example
///
/// ```no_run
/// use crately::messages::{DocumentationExtractionFailed, ExtractionErrorKind};
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let specifier = CrateSpecifier::from_str("broken-crate@1.0.0").unwrap();
/// let event = DocumentationExtractionFailed {
///     specifier,
///     features: vec![],
///     error_message: "Cargo.toml not found".to_string(),
///     error_kind: ExtractionErrorKind::FileNotFound,
///     elapsed_ms: 50,
/// };
/// // broker.broadcast(event).await;
/// ```
#[acton_message]
pub struct DocumentationExtractionFailed {
    /// The crate that failed documentation extraction
    pub specifier: CrateSpecifier,
    /// Feature flags that were requested
    pub features: Vec<String>,
    /// Human-readable error description
    pub error_message: String,
    /// Categorized error type
    pub error_kind: ExtractionErrorKind,
    /// Time elapsed before failure in milliseconds
    pub elapsed_ms: u64,
}

/// Categories of documentation extraction errors for programmatic handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionErrorKind {
    /// Required file not found (Cargo.toml, lib.rs, etc.)
    FileNotFound,
    /// File exceeds configured size limit
    FileTooLarge,
    /// Permission denied reading file
    PermissionDenied,
    /// Invalid file format or encoding
    InvalidFormat,
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
        let message = DocumentationExtractionFailed {
            specifier: specifier.clone(),
            features: vec![],
            error_message: "File not found".to_string(),
            error_kind: ExtractionErrorKind::FileNotFound,
            elapsed_ms: 100,
        };
        assert_eq!(message.specifier, specifier);
        assert_eq!(message.error_kind, ExtractionErrorKind::FileNotFound);
        assert_eq!(message.elapsed_ms, 100);
    }

    #[test]
    fn test_message_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let message = DocumentationExtractionFailed {
            specifier,
            features: vec![],
            error_message: "File too large".to_string(),
            error_kind: ExtractionErrorKind::FileTooLarge,
            elapsed_ms: 200,
        };
        let cloned = message.clone();
        assert_eq!(message.specifier, cloned.specifier);
        assert_eq!(message.error_kind, cloned.error_kind);
        assert_eq!(message.error_message, cloned.error_message);
        assert_eq!(message.elapsed_ms, cloned.elapsed_ms);
    }

    #[test]
    fn test_message_debug() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let message = DocumentationExtractionFailed {
            specifier,
            features: vec![],
            error_message: "Permission denied".to_string(),
            error_kind: ExtractionErrorKind::PermissionDenied,
            elapsed_ms: 50,
        };
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("DocumentationExtractionFailed"));
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<DocumentationExtractionFailed>();
        assert_sync::<DocumentationExtractionFailed>();
    }

    #[test]
    fn test_error_kind_matching() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();
        let message = DocumentationExtractionFailed {
            specifier,
            features: vec![],
            error_message: "Invalid format".to_string(),
            error_kind: ExtractionErrorKind::InvalidFormat,
            elapsed_ms: 75,
        };
        match message.error_kind {
            ExtractionErrorKind::InvalidFormat => {} // Correct match
            _ => panic!("Expected InvalidFormat"),
        }
    }

    #[test]
    fn test_error_message_content() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.0").unwrap();
        let error_msg = "Detailed error description";
        let message = DocumentationExtractionFailed {
            specifier,
            features: vec![],
            error_message: error_msg.to_string(),
            error_kind: ExtractionErrorKind::Other,
            elapsed_ms: 125,
        };
        assert_eq!(message.error_message, error_msg);
    }

    #[test]
    fn test_all_error_kinds() {
        let specifier = CrateSpecifier::from_str("test@1.0.0").unwrap();

        let kinds = vec![
            ExtractionErrorKind::FileNotFound,
            ExtractionErrorKind::FileTooLarge,
            ExtractionErrorKind::PermissionDenied,
            ExtractionErrorKind::InvalidFormat,
            ExtractionErrorKind::Other,
        ];

        for kind in kinds {
            let message = DocumentationExtractionFailed {
                specifier: specifier.clone(),
                features: vec![],
                error_message: "Test".to_string(),
                error_kind: kind.clone(),
                elapsed_ms: 150,
            };
            assert_eq!(message.error_kind, kind);
        }
    }

    #[test]
    fn test_elapsed_time_tracking() {
        let specifier = CrateSpecifier::from_str("timing@1.0.0").unwrap();
        let message = DocumentationExtractionFailed {
            specifier,
            features: vec![],
            error_message: "Test".to_string(),
            error_kind: ExtractionErrorKind::Other,
            elapsed_ms: 0, // Edge case: instant failure
        };
        assert_eq!(message.elapsed_ms, 0);
    }
}
