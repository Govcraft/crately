//! Download error types for crate retrieval from crates.io

use thiserror::Error;
use crate::crate_specifier::CrateSpecifier;

/// Errors that occur during crate download from crates.io
#[derive(Error, Debug, Clone)]
pub enum DownloadError {
    /// Network request failed
    #[error("Network request failed for {specifier}: {details}")]
    NetworkFailure {
        /// The crate that failed to download
        specifier: CrateSpecifier,
        /// Error details
        details: String,
        /// Whether this error can be retried
        retryable: bool,
    },

    /// Crate not found on crates.io
    #[error("Crate {specifier} not found on crates.io")]
    CrateNotFound {
        /// The crate that was not found
        specifier: CrateSpecifier,
    },

    /// Rate limit exceeded
    #[error("Rate limit exceeded for {specifier}. Retry after {retry_after_secs}s")]
    RateLimitExceeded {
        /// The crate being downloaded
        specifier: CrateSpecifier,
        /// Seconds to wait before retry
        retry_after_secs: u64,
    },

    /// Invalid response from server
    #[error("Invalid response for {specifier}: {reason}")]
    InvalidResponse {
        /// The crate being downloaded
        specifier: CrateSpecifier,
        /// Reason for invalid response
        reason: String,
    },

    /// Download timeout
    #[error("Download timeout for {specifier} after {timeout_secs}s")]
    Timeout {
        /// The crate being downloaded
        specifier: CrateSpecifier,
        /// Timeout duration in seconds
        timeout_secs: u64,
    },
}

impl DownloadError {
    /// Returns the crate specifier from any download error variant
    pub fn specifier(&self) -> &CrateSpecifier {
        match self {
            DownloadError::NetworkFailure { specifier, .. } => specifier,
            DownloadError::CrateNotFound { specifier } => specifier,
            DownloadError::RateLimitExceeded { specifier, .. } => specifier,
            DownloadError::InvalidResponse { specifier, .. } => specifier,
            DownloadError::Timeout { specifier, .. } => specifier,
        }
    }

    /// Returns whether this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            DownloadError::NetworkFailure { retryable, .. } => *retryable,
            DownloadError::CrateNotFound { .. } => false, // Not retryable
            DownloadError::RateLimitExceeded { .. } => true, // Retryable after backoff
            DownloadError::InvalidResponse { .. } => false, // Not retryable
            DownloadError::Timeout { .. } => true, // Retryable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_network_failure_display() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let error = DownloadError::NetworkFailure {
            specifier,
            details: "connection refused".to_string(),
            retryable: true,
        };
        assert!(error.to_string().contains("Network request failed"));
        assert!(error.to_string().contains("serde@1.0.0"));
    }

    #[test]
    fn test_crate_not_found_not_retryable() {
        let specifier = CrateSpecifier::from_str("nonexistent@1.0.0").unwrap();
        let error = DownloadError::CrateNotFound { specifier };
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_rate_limit_retryable() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let error = DownloadError::RateLimitExceeded {
            specifier,
            retry_after_secs: 60,
        };
        assert!(error.is_retryable());
        assert!(error.to_string().contains("60"));
    }

    #[test]
    fn test_specifier_extraction() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let error = DownloadError::Timeout {
            specifier: specifier.clone(),
            timeout_secs: 30,
        };
        assert_eq!(error.specifier(), &specifier);
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<DownloadError>();
        assert_sync::<DownloadError>();
    }
}
