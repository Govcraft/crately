//! Error types for the Crately crate processing pipeline.
//!
//! This module provides a comprehensive error handling strategy using domain-driven
//! error types with `thiserror` for compile-time safety and exhaustive matching.
//!
//! # Error Type Hierarchy
//!
//! - **DownloadError**: Network I/O failures during crate download
//! - **ExtractionError**: File system errors during archive extraction
//! - **ProcessingError**: CPU-bound processing failures (parsing, chunking)
//! - **VectorizationError**: Embedding generation failures
//! - **StorageError**: Database persistence failures
//! - **PipelineError**: Unified error wrapper for the entire pipeline
//!
//! # Design Principles
//!
//! - **Domain-Driven**: Error types map to processing stages
//! - **Strongly-Typed**: Compile-time safety with exhaustive matching
//! - **Context-Rich**: Include operation details and recovery hints
//! - **Pub/Sub Broadcast**: All errors flow through the broker as events

mod download_error;
mod extraction_error;
mod pipeline_error;
mod processing_error;
mod storage_error;
mod vectorization_error;

pub use download_error::DownloadError;
pub use extraction_error::ExtractionError;
pub use pipeline_error::PipelineError;
pub use processing_error::ProcessingError;
pub use storage_error::StorageError;
pub use vectorization_error::VectorizationError;
