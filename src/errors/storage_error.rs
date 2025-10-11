//! Storage error types for database persistence

use thiserror::Error;
use crate::crate_specifier::CrateSpecifier;

/// Errors that occur during database storage operations
#[derive(Error, Debug, Clone)]
pub enum StorageError {
    /// Database write failed
    #[error("Database write failed for {specifier}: {details}")]
    WriteFailed {
        /// The crate being stored
        specifier: CrateSpecifier,
        /// Operation that failed
        operation: String,
        /// Error details
        details: String,
        /// Whether this error can be retried
        retryable: bool,
    },

    /// Duplicate entry
    #[error("Duplicate entry for {specifier}")]
    DuplicateEntry {
        /// The crate that already exists
        specifier: CrateSpecifier,
    },

    /// Database connection lost
    #[error("Database connection lost: {reason}")]
    ConnectionLost {
        /// Reason for connection loss
        reason: String,
        /// Whether this error can be retried
        retryable: bool,
    },

    /// Schema validation failed
    #[error("Schema validation failed for {specifier}: {reason}")]
    SchemaValidationFailed {
        /// The crate being stored
        specifier: CrateSpecifier,
        /// Reason for validation failure
        reason: String,
    },

    /// Transaction conflict
    #[error("Transaction conflict for {specifier}")]
    TransactionConflict {
        /// The crate involved in conflict
        specifier: CrateSpecifier,
        /// Whether this error can be retried
        retryable: bool,
    },
}

impl StorageError {
    /// Returns the crate specifier from any storage error variant
    /// Returns None for connection-level errors that don't have a specific crate
    pub fn specifier(&self) -> Option<&CrateSpecifier> {
        match self {
            StorageError::WriteFailed { specifier, .. } => Some(specifier),
            StorageError::DuplicateEntry { specifier } => Some(specifier),
            StorageError::ConnectionLost { .. } => None,
            StorageError::SchemaValidationFailed { specifier, .. } => Some(specifier),
            StorageError::TransactionConflict { specifier, .. } => Some(specifier),
        }
    }

    /// Returns whether this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            StorageError::WriteFailed { retryable, .. } => *retryable,
            StorageError::DuplicateEntry { .. } => false, // Already exists
            StorageError::ConnectionLost { retryable, .. } => *retryable,
            StorageError::SchemaValidationFailed { .. } => false, // Data structure issue
            StorageError::TransactionConflict { retryable, .. } => *retryable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_write_failed_display() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let error = StorageError::WriteFailed {
            specifier,
            operation: "insert".to_string(),
            details: "timeout".to_string(),
            retryable: true,
        };
        assert!(error.to_string().contains("Database write failed"));
        assert!(error.is_retryable());
    }

    #[test]
    fn test_duplicate_entry_not_retryable() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let error = StorageError::DuplicateEntry {
            specifier,
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_connection_lost() {
        let error = StorageError::ConnectionLost {
            reason: "network error".to_string(),
            retryable: true,
        };
        assert!(error.is_retryable());
        assert!(error.specifier().is_none()); // Connection errors have no specific crate
    }

    #[test]
    fn test_transaction_conflict() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let error = StorageError::TransactionConflict {
            specifier: specifier.clone(),
            retryable: true,
        };
        assert!(error.is_retryable());
        assert_eq!(error.specifier(), Some(&specifier));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<StorageError>();
        assert_sync::<StorageError>();
    }
}
