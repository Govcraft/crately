//! Crately library - Rust crate documentation and metadata service
//!
//! This library provides the core functionality for downloading, processing, and
//! vectorizing Rust crate documentation. It is exposed as a library to enable
//! integration testing while maintaining the binary entry point in main.rs.

#![forbid(unsafe_code)]

pub mod actors;
pub mod cli;
pub mod colors;
pub mod crate_specifier;
pub mod errors;
pub mod logging;
pub mod messages;
pub mod request;
pub mod response;
pub mod retry_policy;
pub mod runtime;
pub mod types;

// Re-export commonly used types for convenience
pub use crate_specifier::CrateSpecifier;
