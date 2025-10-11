//! Processing error types for documentation parsing and chunking

use thiserror::Error;
use crate::crate_specifier::CrateSpecifier;
use std::path::PathBuf;

/// Errors that occur during documentation processing
#[derive(Error, Debug, Clone)]
pub enum ProcessingError {
    /// Documentation parsing failed
    #[error("Failed to parse documentation for {specifier}: {details}")]
    DocumentationParseFailure {
        /// The crate being processed
        specifier: CrateSpecifier,
        /// File that failed to parse
        file_path: PathBuf,
        /// Error details
        details: String,
    },

    /// Text chunking failed
    #[error("Text chunking failed for {specifier}: {reason}")]
    ChunkingFailure {
        /// The crate being processed
        specifier: CrateSpecifier,
        /// Reason for failure
        reason: String,
    },

    /// Required files missing
    #[error("Missing required files in {specifier}: {missing_files:?}")]
    MissingRequiredFiles {
        /// The crate being processed
        specifier: CrateSpecifier,
        /// List of missing files
        missing_files: Vec<String>,
    },

    /// Resource limit exceeded
    #[error("Resource limit exceeded for {specifier}: {limit_type}")]
    ResourceLimitExceeded {
        /// The crate being processed
        specifier: CrateSpecifier,
        /// Type of limit exceeded (memory, time, etc.)
        limit_type: String,
    },
}

impl ProcessingError {
    /// Returns the crate specifier from any processing error variant
    pub fn specifier(&self) -> &CrateSpecifier {
        match self {
            ProcessingError::DocumentationParseFailure { specifier, .. } => specifier,
            ProcessingError::ChunkingFailure { specifier, .. } => specifier,
            ProcessingError::MissingRequiredFiles { specifier, .. } => specifier,
            ProcessingError::ResourceLimitExceeded { specifier, .. } => specifier,
        }
    }

    /// Returns whether this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            ProcessingError::DocumentationParseFailure { .. } => false, // Structural issue
            ProcessingError::ChunkingFailure { .. } => false, // Algorithm issue
            ProcessingError::MissingRequiredFiles { .. } => false, // Incomplete crate
            ProcessingError::ResourceLimitExceeded { .. } => false, // Terminal failure
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_documentation_parse_failure_display() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let error = ProcessingError::DocumentationParseFailure {
            specifier,
            file_path: PathBuf::from("/tmp/lib.rs"),
            details: "invalid syntax".to_string(),
        };
        assert!(error.to_string().contains("Failed to parse"));
    }

    #[test]
    fn test_missing_required_files() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let error = ProcessingError::MissingRequiredFiles {
            specifier,
            missing_files: vec!["Cargo.toml".to_string(), "README.md".to_string()],
        };
        assert!(!error.is_retryable());
        assert!(error.to_string().contains("Cargo.toml"));
    }

    #[test]
    fn test_resource_limit_exceeded() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let error = ProcessingError::ResourceLimitExceeded {
            specifier,
            limit_type: "memory".to_string(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_specifier_extraction() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();
        let error = ProcessingError::ChunkingFailure {
            specifier: specifier.clone(),
            reason: "text too large".to_string(),
        };
        assert_eq!(error.specifier(), &specifier);
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ProcessingError>();
        assert_sync::<ProcessingError>();
    }
}
