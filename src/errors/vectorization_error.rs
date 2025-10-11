//! Vectorization error types for embedding generation

use thiserror::Error;
use crate::crate_specifier::CrateSpecifier;

/// Errors that occur during vectorization and embedding generation
#[derive(Error, Debug, Clone)]
pub enum VectorizationError {
    /// Embedding model unavailable
    #[error("Embedding model unavailable for {specifier}: {reason}")]
    ModelUnavailable {
        /// The crate being vectorized
        specifier: CrateSpecifier,
        /// Reason for unavailability
        reason: String,
        /// Whether this error can be retried
        retryable: bool,
    },

    /// Embedding generation failed
    #[error("Embedding generation failed for {specifier}: {details}")]
    EmbeddingGenerationFailed {
        /// The crate being vectorized
        specifier: CrateSpecifier,
        /// Error details
        details: String,
    },

    /// Text too large for model
    #[error("Text too large for {specifier}: {size_bytes} bytes (max: {max_bytes})")]
    TextTooLarge {
        /// The crate being vectorized
        specifier: CrateSpecifier,
        /// Actual size in bytes
        size_bytes: usize,
        /// Maximum allowed size
        max_bytes: usize,
    },

    /// Invalid text encoding
    #[error("Invalid encoding for {specifier}")]
    InvalidEncoding {
        /// The crate being vectorized
        specifier: CrateSpecifier,
    },
}

impl VectorizationError {
    /// Returns the crate specifier from any vectorization error variant
    pub fn specifier(&self) -> &CrateSpecifier {
        match self {
            VectorizationError::ModelUnavailable { specifier, .. } => specifier,
            VectorizationError::EmbeddingGenerationFailed { specifier, .. } => specifier,
            VectorizationError::TextTooLarge { specifier, .. } => specifier,
            VectorizationError::InvalidEncoding { specifier } => specifier,
        }
    }

    /// Returns whether this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            VectorizationError::ModelUnavailable { retryable, .. } => *retryable,
            VectorizationError::EmbeddingGenerationFailed { .. } => true, // May be transient
            VectorizationError::TextTooLarge { .. } => false, // Terminal - need re-chunking
            VectorizationError::InvalidEncoding { .. } => false, // Terminal - bad input
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_model_unavailable_display() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let error = VectorizationError::ModelUnavailable {
            specifier,
            reason: "API quota exceeded".to_string(),
            retryable: true,
        };
        assert!(error.to_string().contains("Embedding model unavailable"));
        assert!(error.is_retryable());
    }

    #[test]
    fn test_text_too_large_not_retryable() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let error = VectorizationError::TextTooLarge {
            specifier,
            size_bytes: 100000,
            max_bytes: 50000,
        };
        assert!(!error.is_retryable());
        assert!(error.to_string().contains("100000"));
    }

    #[test]
    fn test_embedding_generation_failed() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let error = VectorizationError::EmbeddingGenerationFailed {
            specifier,
            details: "timeout".to_string(),
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn test_specifier_extraction() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();
        let error = VectorizationError::InvalidEncoding {
            specifier: specifier.clone(),
        };
        assert_eq!(error.specifier(), &specifier);
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<VectorizationError>();
        assert_sync::<VectorizationError>();
    }
}
