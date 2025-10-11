//! CrateProcessingFailed message - processing failure event
//!
//! This message is broadcast when crate processing fails at any stage of the pipeline
//! (download, extraction, compilation, chunking, vectorization). It contains the crate
//! specifier, the stage where failure occurred, and error details.

use crate::crate_specifier::CrateSpecifier;
use acton_reactive::prelude::*;

/// Event broadcast when crate processing fails at any pipeline stage
///
/// This message is sent when a crate fails to process successfully, indicating
/// the stage of failure and error details. Subscribers can use this to update
/// UI, trigger error handling, or initiate recovery procedures.
///
/// # Actor Communication Pattern
///
/// Publisher: Any pipeline actor (CrateDownloader, DocumentationExtractor, etc.)
/// Subscribers: DatabaseActor (for persistence), Console (for user feedback)
///
/// # Examples
///
/// ```
/// use crately::messages::CrateProcessingFailed;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let msg = CrateProcessingFailed {
///     specifier: CrateSpecifier::from_str("serde@1.0.0").unwrap(),
///     stage: "chunking".to_string(),
///     error: "Failed to parse documentation: invalid markdown syntax".to_string(),
/// };
///
/// assert_eq!(msg.stage, "chunking");
/// ```
#[acton_message]
pub struct CrateProcessingFailed {
    /// The crate that failed processing
    pub specifier: CrateSpecifier,

    /// The pipeline stage where failure occurred
    /// (e.g., "downloading", "extracting", "compiling_docs", "chunking", "vectorizing")
    pub stage: String,

    /// Error message describing the failure
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_crate_processing_failed_creation() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let msg = CrateProcessingFailed {
            specifier: specifier.clone(),
            stage: "compiling_docs".to_string(),
            error: "rustdoc compilation failed".to_string(),
        };

        assert_eq!(msg.specifier, specifier);
        assert_eq!(msg.stage, "compiling_docs");
        assert_eq!(msg.error, "rustdoc compilation failed");
    }

    #[test]
    fn test_crate_processing_failed_download_stage() {
        let msg = CrateProcessingFailed {
            specifier: CrateSpecifier::from_str("axum@0.7.0").unwrap(),
            stage: "downloading".to_string(),
            error: "Network connection timeout".to_string(),
        };

        assert_eq!(msg.stage, "downloading");
    }

    #[test]
    fn test_crate_processing_failed_extraction_stage() {
        let msg = CrateProcessingFailed {
            specifier: CrateSpecifier::from_str("serde_json@1.0.0").unwrap(),
            stage: "extracting".to_string(),
            error: "Corrupted archive file".to_string(),
        };

        assert_eq!(msg.stage, "extracting");
    }

    #[test]
    fn test_crate_processing_failed_chunking_stage() {
        let msg = CrateProcessingFailed {
            specifier: CrateSpecifier::from_str("tracing@0.1.0").unwrap(),
            stage: "chunking".to_string(),
            error: "Invalid documentation format".to_string(),
        };

        assert_eq!(msg.stage, "chunking");
    }

    #[test]
    fn test_crate_processing_failed_vectorization_stage() {
        let msg = CrateProcessingFailed {
            specifier: CrateSpecifier::from_str("hyper@1.0.0").unwrap(),
            stage: "vectorizing".to_string(),
            error: "Embedding API rate limit exceeded".to_string(),
        };

        assert_eq!(msg.stage, "vectorizing");
    }

    #[test]
    fn test_crate_processing_failed_clone() {
        let msg = CrateProcessingFailed {
            specifier: CrateSpecifier::from_str("rand@0.8.0").unwrap(),
            stage: "extracting".to_string(),
            error: "File system permission denied".to_string(),
        };

        let cloned = msg.clone();
        assert_eq!(msg.specifier, cloned.specifier);
        assert_eq!(msg.stage, cloned.stage);
        assert_eq!(msg.error, cloned.error);
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CrateProcessingFailed>();
        assert_sync::<CrateProcessingFailed>();
    }
}
