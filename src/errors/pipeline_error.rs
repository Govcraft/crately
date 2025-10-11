//! Unified pipeline error wrapper

use thiserror::Error;
use crate::crate_specifier::CrateSpecifier;
use super::{DownloadError, ExtractionError, ProcessingError, VectorizationError, StorageError};

/// Unified error type for the entire crate processing pipeline
#[derive(Error, Debug, Clone)]
pub enum PipelineError {
    /// Download stage failed
    #[error("Download stage failed: {0}")]
    Download(#[from] DownloadError),

    /// Extraction stage failed
    #[error("Extraction stage failed: {0}")]
    Extraction(#[from] ExtractionError),

    /// Processing stage failed
    #[error("Processing stage failed: {0}")]
    Processing(#[from] ProcessingError),

    /// Vectorization stage failed
    #[error("Vectorization stage failed: {0}")]
    Vectorization(#[from] VectorizationError),

    /// Storage stage failed
    #[error("Storage stage failed: {0}")]
    Storage(#[from] StorageError),
}

impl PipelineError {
    /// Returns the crate specifier from any pipeline error variant
    /// Returns None for errors that don't have a specific crate (e.g., connection errors)
    pub fn specifier(&self) -> Option<&CrateSpecifier> {
        match self {
            PipelineError::Download(e) => Some(e.specifier()),
            PipelineError::Extraction(e) => Some(e.specifier()),
            PipelineError::Processing(e) => Some(e.specifier()),
            PipelineError::Vectorization(e) => Some(e.specifier()),
            PipelineError::Storage(e) => e.specifier(),
        }
    }

    /// Returns whether this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            PipelineError::Download(e) => e.is_retryable(),
            PipelineError::Extraction(e) => e.is_retryable(),
            PipelineError::Processing(e) => e.is_retryable(),
            PipelineError::Vectorization(e) => e.is_retryable(),
            PipelineError::Storage(e) => e.is_retryable(),
        }
    }

    /// Returns the processing stage that failed
    pub fn stage(&self) -> &'static str {
        match self {
            PipelineError::Download(_) => "download",
            PipelineError::Extraction(_) => "extraction",
            PipelineError::Processing(_) => "processing",
            PipelineError::Vectorization(_) => "vectorization",
            PipelineError::Storage(_) => "storage",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_from_download_error() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let download_error = DownloadError::CrateNotFound {
            specifier: specifier.clone(),
        };
        let pipeline_error: PipelineError = download_error.into();
        assert_eq!(pipeline_error.stage(), "download");
        assert_eq!(pipeline_error.specifier(), Some(&specifier));
    }

    #[test]
    fn test_from_storage_error() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let storage_error = StorageError::DuplicateEntry {
            specifier: specifier.clone(),
        };
        let pipeline_error: PipelineError = storage_error.into();
        assert_eq!(pipeline_error.stage(), "storage");
        assert!(!pipeline_error.is_retryable());
    }

    #[test]
    fn test_stage_names() {
        let specifier = CrateSpecifier::from_str("test@1.0.0").unwrap();

        let stages = vec![
            (PipelineError::Download(DownloadError::CrateNotFound { specifier: specifier.clone() }), "download"),
            (PipelineError::Extraction(ExtractionError::InvalidArchiveFormat {
                specifier: specifier.clone(),
                reason: "test".to_string()
            }), "extraction"),
            (PipelineError::Processing(ProcessingError::ChunkingFailure {
                specifier: specifier.clone(),
                reason: "test".to_string()
            }), "processing"),
            (PipelineError::Vectorization(VectorizationError::InvalidEncoding {
                specifier: specifier.clone()
            }), "vectorization"),
            (PipelineError::Storage(StorageError::DuplicateEntry {
                specifier: specifier.clone()
            }), "storage"),
        ];

        for (error, expected_stage) in stages {
            assert_eq!(error.stage(), expected_stage);
        }
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<PipelineError>();
        assert_sync::<PipelineError>();
    }
}
