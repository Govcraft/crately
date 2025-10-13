//! Shared types used across the Crately actor system
//!
//! This module contains common data structures and metadata types that are used
//! by multiple actors and message types throughout the system.
//!
//! # Organization
//!
//! Types are organized by their primary purpose:
//! - **ID Types**: Unique identifiers for builds and other entities
//! - **Hash Types**: Content-addressable hashing for deduplication
//! - **Metadata Types**: Structured metadata for documentation and code samples
//! - **Search Types**: Results and data structures for vector similarity search
//!
//! # Usage
//!
//! ```rust
//! use crately::types::{BuildId, ContentHash, ChunkMetadata, SearchResult};
//! ```

mod chunk_metadata;
mod code_sample_metadata;
mod search_result;
mod ids;
mod hashes;

pub use chunk_metadata::ChunkMetadata;
pub use code_sample_metadata::CodeSampleMetadata;
pub use search_result::SearchResult;
pub use ids::{BuildId, BuildIdError};
pub use hashes::{ContentHash, ContentHashError};
