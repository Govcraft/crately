//! Message types for the Crately actor system.
//!
//! # Organization Pattern
//!
//! This module follows a **one-file-per-message-type** pattern where each message struct
//! is defined in its own file and re-exported from this module root. This organizational
//! strategy provides several critical benefits:
//!
//! ## Benefits of File-Per-Message Organization
//!
//! ### 1. Discoverability
//! - Each message type has a clear, dedicated location in the codebase
//! - New developers can quickly find message definitions without navigating large files
//! - IDE navigation and search tools work more effectively with focused files
//! - File structure mirrors the logical structure of the message system
//!
//! ### 2. Maintainability
//! - Changes to one message type don't affect others
//! - Easier to review changes in pull requests (isolated diffs)
//! - Message-specific documentation stays with its definition
//! - Reduces cognitive load when working on individual message types
//!
//! ### 3. Merge Conflict Reduction
//! - Multiple developers can work on different messages simultaneously
//! - Git merges are cleaner when changes are in separate files
//! - Less risk of accidental modifications to unrelated messages
//! - Parallel development is safer and more efficient
//!
//! ### 4. Testing and Documentation
//! - Tests can be colocated with message definitions using `#[cfg(test)]` modules
//! - Documentation examples are easier to maintain when focused on single types
//! - Message-specific edge cases and invariants are clearly documented
//!
//! ## Message Passing Architecture
//!
//! Crately uses the acton-reactive framework for actor-based concurrency. Messages are
//! the fundamental unit of communication between actors. Each message type:
//!
//! - Is decorated with the `#[acton_message]` macro
//! - Automatically implements `Clone`, `Debug`, and `ActonMessage` traits
//! - Should contain only the data necessary for the operation (keep messages small)
//! - Should use clear, descriptive names that express intent
//! - Should prefer immutable fields for safety in concurrent scenarios
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use crately::messages::Init;
//! use acton_reactive::prelude::*;
//!
//! // Messages are sent to actors via the actor runtime
//! async fn send_init_message(actor_handle: ActorHandle<MyActor>) {
//!     let init_msg = Init;
//!     // Send message to actor (actual sending mechanism depends on actor implementation)
//!     // actor_handle.send(init_msg).await;
//! }
//! ```
//!
//! ## Adding New Messages
//!
//! To add a new message type:
//!
//! 1. Create a new file in `src/messages/` named after the message (e.g., `download.rs`)
//! 2. Define the message struct with `#[acton_message]` macro
//! 3. Add comprehensive documentation explaining the message's purpose and usage
//! 4. Re-export the message from this module using `pub use`
//! 5. Ensure the message type is small and contains only essential data
//!
//! Example:
//! ```rust,ignore
//! // In src/messages/download.rs
//! use acton_reactive::prelude::*;
//!
//! /// Message requesting download of a specific crate version.
//! #[acton_message]
//! pub struct Download {
//!     /// The crate name to download
//!     pub name: String,
//!     /// The version to download
//!     pub version: String,
//! }
//! ```
//!
//! Then in this file:
//! ```rust,ignore
//! mod download;
//! pub use download::Download;
//! ```

mod build_created;
mod build_query_response;
mod check_processing_timeouts;
mod chunk_deduplicated;
mod chunk_hash_computed;
mod chunk_persistence_progress;
mod chunks_persistence_complete;
mod config_reload_failed;
mod crate_downloaded;
mod pipeline_config_changed;
mod crate_download_failed;
mod crate_list_response;
mod crate_persisted;
mod crate_processing_complete;
mod crate_processing_failed;
mod crate_query_response;
mod crate_received;
mod database_error;
mod database_ready;
mod database_warning;
mod doc_chunk_persisted;
mod doc_chunks_query_response;
mod documentation_chunked;
mod documentation_extracted;
mod documentation_extraction_failed;
mod documentation_vectorized;
mod embedding_failed;
mod embedding_generated;
mod embedding_persisted;
mod embedding_persistence_progress;
mod embeddings_persistence_complete;
mod init;
mod key_pressed;
mod keyboard_handler_started;
mod list_crates;
mod persist_code_sample;
mod persist_crate;
mod persist_doc_chunk;
mod persist_embedding;
mod print_error;
mod print_progress;
mod print_separator;
mod print_spinner;
mod print_success;
mod print_warning;
mod stop_spinner;
mod update_spinner;
mod query_build;
mod query_crate;
mod query_doc_chunks;
mod query_similar_docs;
mod retry_crate_processing;
mod server_reloaded;
mod server_started;
mod set_raw_mode;
mod similar_docs_response;
mod start_server;
mod stop_keyboard_handler;
mod stop_server;

pub use build_created::BuildCreated;
pub use build_query_response::{BuildInfo, BuildQueryResponse};
pub use check_processing_timeouts::CheckProcessingTimeouts;
pub use chunk_deduplicated::ChunkDeduplicated;
pub use chunk_hash_computed::ChunkHashComputed;
pub use chunk_persistence_progress::ChunkPersistenceProgress;
pub use chunks_persistence_complete::ChunksPersistenceComplete;
pub use config_reload_failed::ConfigReloadFailed;
pub use crate_downloaded::CrateDownloaded;
pub use pipeline_config_changed::PipelineConfigChanged;
pub use crate_download_failed::{CrateDownloadFailed, DownloadErrorKind};
pub use crate_list_response::{CrateListResponse, CrateSummary};
pub use crate_persisted::CratePersisted;
pub use crate_processing_complete::CrateProcessingComplete;
pub use crate_processing_failed::CrateProcessingFailed;
pub use crate_query_response::CrateQueryResponse;
pub use crate_received::CrateReceived;
pub use database_error::DatabaseError;
pub use database_ready::DatabaseReady;
pub use database_warning::DatabaseWarning;
pub use doc_chunk_persisted::DocChunkPersisted;
pub use doc_chunks_query_response::{DocChunkData, DocChunksQueryResponse};
pub use documentation_chunked::DocumentationChunked;
pub use documentation_extracted::DocumentationExtracted;
pub use documentation_extraction_failed::{DocumentationExtractionFailed, ExtractionErrorKind};
pub use documentation_vectorized::DocumentationVectorized;
pub use embedding_failed::EmbeddingFailed;
pub use embedding_generated::EmbeddingGenerated;
pub use embedding_persisted::EmbeddingPersisted;
pub use embedding_persistence_progress::EmbeddingPersistenceProgress;
pub use embeddings_persistence_complete::EmbeddingsPersistenceComplete;
pub use init::Init;
pub use key_pressed::KeyPressed;
pub(crate) use keyboard_handler_started::KeyboardHandlerStarted;
pub use list_crates::ListCrates;
pub use persist_code_sample::PersistCodeSample;
pub use persist_crate::PersistCrate;
pub use persist_doc_chunk::PersistDocChunk;
pub use persist_embedding::PersistEmbedding;
pub use print_error::PrintError;
pub use print_progress::PrintProgress;
pub use print_separator::PrintSeparator;
pub use print_spinner::PrintSpinner;
pub use print_success::PrintSuccess;
pub use print_warning::PrintWarning;
pub use stop_spinner::StopSpinner;
pub use update_spinner::UpdateSpinner;
pub use query_build::QueryBuild;
pub use query_crate::QueryCrate;
pub use query_doc_chunks::QueryDocChunks;
pub use query_similar_docs::QuerySimilarDocs;
pub use retry_crate_processing::RetryCrateProcessing;
pub use server_reloaded::ServerReloaded;
pub use server_started::ServerStarted;
pub use set_raw_mode::SetRawMode;
pub use similar_docs_response::SimilarDocsResponse;
pub use start_server::StartServer;
pub use stop_keyboard_handler::StopKeyboardHandler;
pub use stop_server::StopServer;
