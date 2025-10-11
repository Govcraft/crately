//! Extraction error types for crate archive extraction

use thiserror::Error;
use crate::crate_specifier::CrateSpecifier;
use std::path::PathBuf;

/// Errors that occur during crate archive extraction
#[derive(Error, Debug, Clone)]
pub enum ExtractionError {
    /// Archive is corrupted or invalid
    #[error("Failed to extract {specifier} archive: {details}")]
    ArchiveCorrupted {
        /// The crate being extracted
        specifier: CrateSpecifier,
        /// Error details
        details: String,
    },

    /// Insufficient disk space
    #[error("Insufficient disk space for {specifier}. Required: {required_bytes} bytes")]
    InsufficientDiskSpace {
        /// The crate being extracted
        specifier: CrateSpecifier,
        /// Required bytes
        required_bytes: u64,
    },

    /// Permission denied
    #[error("Permission denied writing to {path}")]
    PermissionDenied {
        /// The crate being extracted
        specifier: CrateSpecifier,
        /// Path where permission was denied
        path: PathBuf,
    },

    /// Invalid archive format
    #[error("Invalid archive format for {specifier}: {reason}")]
    InvalidArchiveFormat {
        /// The crate being extracted
        specifier: CrateSpecifier,
        /// Reason for invalid format
        reason: String,
    },
}

impl ExtractionError {
    /// Returns the crate specifier from any extraction error variant
    pub fn specifier(&self) -> &CrateSpecifier {
        match self {
            ExtractionError::ArchiveCorrupted { specifier, .. } => specifier,
            ExtractionError::InsufficientDiskSpace { specifier, .. } => specifier,
            ExtractionError::PermissionDenied { specifier, .. } => specifier,
            ExtractionError::InvalidArchiveFormat { specifier, .. } => specifier,
        }
    }

    /// Returns whether this error is retryable
    /// Most extraction errors require user intervention
    pub fn is_retryable(&self) -> bool {
        match self {
            ExtractionError::ArchiveCorrupted { .. } => true, // May work if re-downloaded
            ExtractionError::InsufficientDiskSpace { .. } => false, // User must free space
            ExtractionError::PermissionDenied { .. } => false, // User must fix permissions
            ExtractionError::InvalidArchiveFormat { .. } => false, // Fundamental incompatibility
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_archive_corrupted_display() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let error = ExtractionError::ArchiveCorrupted {
            specifier,
            details: "unexpected EOF".to_string(),
        };
        assert!(error.to_string().contains("Failed to extract"));
    }

    #[test]
    fn test_insufficient_disk_space_not_retryable() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let error = ExtractionError::InsufficientDiskSpace {
            specifier,
            required_bytes: 1024000,
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_permission_denied() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let error = ExtractionError::PermissionDenied {
            specifier,
            path: PathBuf::from("/tmp/crates"),
        };
        assert!(!error.is_retryable());
        assert!(error.to_string().contains("Permission denied"));
    }

    #[test]
    fn test_specifier_extraction() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();
        let error = ExtractionError::InvalidArchiveFormat {
            specifier: specifier.clone(),
            reason: "not a tar.gz file".to_string(),
        };
        assert_eq!(error.specifier(), &specifier);
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ExtractionError>();
        assert_sync::<ExtractionError>();
    }
}
