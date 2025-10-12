//! Event broadcast when documentation is chunked into embeddable segments.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Event broadcast when documentation is chunked into embeddable segments.
///
/// This message is broadcast by ProcessorActor after successfully parsing
/// the documentation text and splitting it into appropriately-sized chunks
/// for vectorization. Each chunk represents a semantic unit that will be
/// embedded independently.
///
/// # Fields
///
/// * `specifier` - The crate that was processed
/// * `features` - Feature flags for this crate
/// * `chunk_count` - Number of text chunks created
/// * `total_tokens_estimated` - Estimated total tokens across all chunks
///
/// # Message Flow
///
/// 1. ProcessorActor receives DocumentationExtracted event
/// 2. ProcessorActor parses and chunks documentation text
/// 3. ProcessorActor creates semantically meaningful chunks
/// 4. ProcessorActor broadcasts DocumentationChunked event
/// 5. Multiple subscribers react:
///    - Console: Displays progress message
///    - VectorizerActor: Begins vectorization (next stage)
///    - CrateCoordinatorActor: Updates state to "Chunked"
///
/// # Subscribers
///
/// - **Console**: User feedback - "Chunked documentation: 42 chunks (~8500 tokens)"
/// - **VectorizerActor**: Triggers embedding API calls (primary pipeline progression)
/// - **CrateCoordinatorActor** (future): State tracking - "Chunked"
///
/// # Example
///
/// ```no_run
/// use crately::messages::DocumentationChunked;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let specifier = CrateSpecifier::from_str("tracing@0.1.40").unwrap();
/// let event = DocumentationChunked {
///     specifier,
///     features: vec![],
///     chunk_count: 42,
///     total_tokens_estimated: 8500,
/// };
/// // broker.broadcast(event).await;
/// ```
#[acton_message]
pub struct DocumentationChunked {
    /// The crate that was processed
    pub specifier: CrateSpecifier,
    /// Feature flags for this crate
    pub features: Vec<String>,
    /// Number of text chunks created
    pub chunk_count: u32,
    /// Estimated total tokens across all chunks.
    ///
    /// NOTE: Currently unused but reserved for token tracking and cost estimation.
    /// Planned uses:
    /// - Console: Display token estimate in progress message ("42 chunks, ~8500 tokens")
    /// - MetricsActor: Track token usage for embedding API cost estimation
    /// - CostTracker: Calculate estimated API costs before vectorization
    ///
    /// See issue #55 for planned token tracking implementation.
    #[allow(dead_code)]
    pub total_tokens_estimated: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_message_creation() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let message = DocumentationChunked {
            specifier: specifier.clone(),
            features: vec!["derive".to_string()],
            chunk_count: 30,
            total_tokens_estimated: 6000,
        };
        assert_eq!(message.specifier, specifier);
        assert_eq!(message.chunk_count, 30);
        assert_eq!(message.total_tokens_estimated, 6000);
    }

    #[test]
    fn test_message_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let message = DocumentationChunked {
            specifier,
            features: vec![],
            chunk_count: 50,
            total_tokens_estimated: 10000,
        };
        let cloned = message.clone();
        assert_eq!(message.specifier, cloned.specifier);
        assert_eq!(message.chunk_count, cloned.chunk_count);
        assert_eq!(message.total_tokens_estimated, cloned.total_tokens_estimated);
    }

    #[test]
    fn test_message_debug() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let message = DocumentationChunked {
            specifier,
            features: vec![],
            chunk_count: 42,
            total_tokens_estimated: 8500,
        };
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("DocumentationChunked"));
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<DocumentationChunked>();
        assert_sync::<DocumentationChunked>();
    }

    #[test]
    fn test_chunk_count_validation() {
        let specifier = CrateSpecifier::from_str("tracing@0.1.0").unwrap();
        let message = DocumentationChunked {
            specifier,
            features: vec![],
            chunk_count: 1,
            total_tokens_estimated: 200,
        };
        assert_eq!(message.chunk_count, 1);
        assert!(message.chunk_count > 0);
    }

    #[test]
    fn test_token_estimation_accuracy() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.0").unwrap();
        let chunk_count = 25;
        let avg_tokens_per_chunk = 300;
        let message = DocumentationChunked {
            specifier,
            features: vec![],
            chunk_count,
            total_tokens_estimated: chunk_count * avg_tokens_per_chunk,
        };
        assert_eq!(message.total_tokens_estimated, 7500);
        assert_eq!(message.total_tokens_estimated / message.chunk_count, avg_tokens_per_chunk);
    }

    #[test]
    fn test_zero_chunks_edge_case() {
        let specifier = CrateSpecifier::from_str("empty-crate@1.0.0").unwrap();
        let message = DocumentationChunked {
            specifier,
            features: vec![],
            chunk_count: 0, // Edge case: no chunks created
            total_tokens_estimated: 0,
        };
        assert_eq!(message.chunk_count, 0);
        assert_eq!(message.total_tokens_estimated, 0);
    }

    #[test]
    fn test_large_chunk_count() {
        let specifier = CrateSpecifier::from_str("huge-docs@1.0.0").unwrap();
        let message = DocumentationChunked {
            specifier,
            features: vec![],
            chunk_count: 1000,
            total_tokens_estimated: 250000,
        };
        assert_eq!(message.chunk_count, 1000);
        assert!(message.total_tokens_estimated > 0);
    }
}
