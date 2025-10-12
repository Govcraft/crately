//! Event broadcast when documentation is extracted from a downloaded crate.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;
use std::path::PathBuf;

/// Event broadcast when documentation is extracted from a downloaded crate.
///
/// This message is broadcast by FileReaderActor after successfully reading
/// relevant documentation files (Cargo.toml, README.md, lib.rs doc comments,
/// etc.) from the extracted crate directory.
///
/// # Fields
///
/// * `specifier` - The crate that was processed
/// * `features` - Feature flags for this crate
/// * `documentation_bytes` - Total bytes of documentation extracted
/// * `file_count` - Number of documentation files processed
/// * `extracted_path` - Path to the extracted crate directory for re-reading files
///
/// # Message Flow
///
/// 1. FileReaderActor receives CrateDownloaded event
/// 2. FileReaderActor reads documentation from extracted path
/// 3. FileReaderActor parses Cargo.toml, README, doc comments
/// 4. FileReaderActor broadcasts DocumentationExtracted event
/// 5. Multiple subscribers react:
///    - Console: Displays progress message
///    - ProcessorActor: Begins chunking documentation (next stage)
///    - CrateCoordinatorActor: Updates state to "Extracted"
///
/// # Subscribers
///
/// - **Console**: User feedback - "Extracted documentation: 15 files, 45KB"
/// - **ProcessorActor**: Triggers text chunking (primary pipeline progression)
/// - **CrateCoordinatorActor** (future): State tracking - "Extracted"
///
/// # Example
///
/// ```no_run
/// use crately::messages::DocumentationExtracted;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
/// use std::path::PathBuf;
///
/// let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
/// let event = DocumentationExtracted {
///     specifier,
///     features: vec!["macros".to_string()],
///     documentation_bytes: 46080,  // 45KB
///     file_count: 15,
///     extracted_path: PathBuf::from("/tmp/crately/axum-0.7.0"),
/// };
/// // broker.broadcast(event).await;
/// ```
#[acton_message]
pub struct DocumentationExtracted {
    /// The crate that was processed
    pub specifier: CrateSpecifier,
    /// Feature flags for this crate
    pub features: Vec<String>,
    /// Total bytes of documentation extracted
    pub documentation_bytes: u64,
    /// Number of files processed
    pub file_count: u32,
    /// Path to the extracted crate directory for re-reading files
    pub extracted_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_message_creation() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let extracted_path = PathBuf::from("/tmp/serde-1.0.0");
        let message = DocumentationExtracted {
            specifier: specifier.clone(),
            features: vec!["derive".to_string()],
            documentation_bytes: 50000,
            file_count: 10,
            extracted_path: extracted_path.clone(),
        };
        assert_eq!(message.specifier, specifier);
        assert_eq!(message.documentation_bytes, 50000);
        assert_eq!(message.file_count, 10);
        assert_eq!(message.extracted_path, extracted_path);
    }

    #[test]
    fn test_message_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let extracted_path = PathBuf::from("/tmp/tokio-1.0.0");
        let message = DocumentationExtracted {
            specifier,
            features: vec![],
            documentation_bytes: 100000,
            file_count: 20,
            extracted_path,
        };
        let cloned = message.clone();
        assert_eq!(message.specifier, cloned.specifier);
        assert_eq!(message.documentation_bytes, cloned.documentation_bytes);
        assert_eq!(message.file_count, cloned.file_count);
        assert_eq!(message.extracted_path, cloned.extracted_path);
    }

    #[test]
    fn test_message_debug() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let message = DocumentationExtracted {
            specifier,
            features: vec![],
            documentation_bytes: 46080,
            file_count: 15,
            extracted_path: PathBuf::from("/tmp/axum-0.7.0"),
        };
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("DocumentationExtracted"));
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<DocumentationExtracted>();
        assert_sync::<DocumentationExtracted>();
    }

    #[test]
    fn test_byte_count_accuracy() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();
        let message = DocumentationExtracted {
            specifier,
            features: vec![],
            documentation_bytes: 123456,
            file_count: 25,
            extracted_path: PathBuf::from("/tmp/tracing-0.1.0"),
        };
        assert_eq!(message.documentation_bytes, 123456);
        assert!(message.documentation_bytes > 0);
    }

    #[test]
    fn test_zero_file_count_edge_case() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.0").unwrap();
        let message = DocumentationExtracted {
            specifier,
            features: vec![],
            documentation_bytes: 0,
            file_count: 0, // Edge case: no files found
            extracted_path: PathBuf::from("/tmp/anyhow-1.0.0"),
        };
        assert_eq!(message.file_count, 0);
        assert_eq!(message.documentation_bytes, 0);
    }

    #[test]
    fn test_large_documentation() {
        let specifier = CrateSpecifier::from_str("huge-crate@1.0.0").unwrap();
        let message = DocumentationExtracted {
            specifier,
            features: vec![],
            documentation_bytes: u64::MAX, // Very large documentation
            file_count: 1000,
            extracted_path: PathBuf::from("/tmp/huge-crate-1.0.0"),
        };
        assert_eq!(message.documentation_bytes, u64::MAX);
        assert_eq!(message.file_count, 1000);
    }
}
