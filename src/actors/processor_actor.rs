//! Actor responsible for chunking documentation text into embeddable segments
//!
//! The `ProcessorActor` is a stateless worker that subscribes to `DocumentationExtracted` events
//! and processes the extracted documentation into semantically meaningful chunks for vectorization.
//! It uses the text-splitter crate with tiktoken-rs for accurate token-based chunking.
//!
//! # Key Features
//!
//! - **Accurate Token Counting**: Uses tiktoken tokenization matching OpenAI models
//! - **Semantic-Aware Splitting**: Leverages markdown and code-aware splitters
//! - **Comprehensive Source Code Processing**: Processes ALL public-facing API code
//! - **Multi-Content Type Support**: Handles README.md (markdown), Cargo.toml (markdown), and Rust source (code splitter)

use acton_reactive::prelude::*;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use text_splitter::{ChunkConfig, MarkdownSplitter, TextSplitter};
use tiktoken_rs::CoreBPE;
use walkdir::WalkDir;

use crate::actors::config::{ProcessConfig, TokenEncoding};
use crate::crate_specifier::CrateSpecifier;
use crate::messages::{DocumentationChunked, DocumentationExtracted, PersistDocChunk};
use crate::types::ChunkMetadata;

/// Stateless actor for chunking documentation text
///
/// This actor subscribes to `DocumentationExtracted` events and performs the following:
/// 1. Discovers all Rust source files in the crate
/// 2. Reads documentation files (README.md, Cargo.toml) and ALL Rust source files
/// 3. Splits text using appropriate splitters:
///    - Markdown splitter for README.md and Cargo.toml (respects headers, lists, code blocks)
///    - Code splitter for Rust files (respects function boundaries, structs, impl blocks, modules)
/// 4. Uses tiktoken for accurate token counting
/// 5. Broadcasts `DocumentationChunked` on success
///
/// # State
///
/// The actor is stateless and relies on configuration provided through the
/// `ProcessConfig` which is immutable after initialization.
///
/// # Message Flow
///
/// - Subscribes to: `DocumentationExtracted`
/// - Broadcasts: `DocumentationChunked`, `PersistDocChunk`
#[acton_actor]
pub struct ProcessorActor {
    /// Processing configuration (immutable)
    config: ProcessConfig,
}

impl ProcessorActor {
    /// Creates a new ProcessorActor with the given configuration
    ///
    /// This constructor encapsulates the initialization logic, storing the
    /// immutable processing configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Processing configuration including chunk size, overlap, and token encoding
    ///
    /// # Returns
    ///
    /// Returns a new `ProcessorActor` instance ready to be spawned.
    pub fn new(config: ProcessConfig) -> Self {
        Self { config }
    }

    /// Spawns, configures, and starts a new ProcessorActor
    ///
    /// This is the standard factory method for creating ProcessorActor actors.
    /// The ProcessorActor subscribes to `DocumentationExtracted` events and chunks
    /// documentation text using text-splitter with tiktoken-rs for accurate token-based chunking.
    ///
    /// This follows the simple actor pattern where only the handle is returned,
    /// as the ProcessorActor has no startup data to provide to the application.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    /// * `config` - Processing configuration including chunk size, overlap, and token encoding
    ///
    /// # Returns
    ///
    /// Returns `AgentHandle` to the started ProcessorActor for message passing.
    ///
    /// # When to Use
    ///
    /// Call this during application startup to initialize the documentation chunking
    /// subsystem. The actor will be ready to process `DocumentationExtracted` events
    /// and coordinate with other actors in the crate processing pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if actor creation or initialization fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use acton_reactive::prelude::*;
    /// use crately::actors::processor_actor::ProcessorActor;
    /// use crately::actors::config::ProcessConfig;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut runtime = ActonApp::launch();
    ///
    ///     // Spawn the ProcessorActor for handling documentation chunking
    ///     let config = ProcessConfig::default();
    ///     let processor = ProcessorActor::spawn(&mut runtime, config).await?;
    ///
    ///     runtime.shutdown_all().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn spawn(runtime: &mut AgentRuntime, config: ProcessConfig) -> Result<AgentHandle> {
        let mut builder = runtime
            .new_agent_with_name::<ProcessorActor>("processor".to_string())
            .await;

        // Initialize actor with configuration
        builder.model = ProcessorActor::new(config);

        // Subscribe to DocumentationExtracted events
        builder.mutate_on::<DocumentationExtracted>(|agent, envelope| {
            let msg = envelope.message().clone();
            let broker = agent.broker().clone();
            let config = agent.model.config.clone();

            AgentReply::from_async(async move {
                match chunk_documentation(&config, &msg.specifier, &msg.extracted_path).await {
                    Ok((chunks, total_tokens)) => {
                        let chunk_count = chunks.len() as u32;

                        // Persist all chunks to database before broadcasting event
                        for (index, (content, source_file, content_type, token_count)) in chunks.into_iter().enumerate() {
                            let chunk_id = format!(
                                "{}_{}_{:03}",
                                msg.specifier.name().replace('-', "_"),
                                msg.specifier.version().to_string().replace('.', "_"),
                                index
                            );

                            let metadata = ChunkMetadata {
                                content_type,
                                start_line: None,
                                end_line: None,
                                token_count,
                                char_count: content.len(),
                                parent_module: None,
                                item_type: None,
                                item_name: None,
                            };

                            broker
                                .broadcast(PersistDocChunk {
                                    specifier: msg.specifier.clone(),
                                    chunk_index: index,
                                    _chunk_id: chunk_id,
                                    content,
                                    source_file,
                                    metadata,
                                    features: Some(msg.features.clone()),
                                })
                                .await;
                        }

                        // Broadcast DocumentationChunked event with statistics
                        broker
                            .broadcast(DocumentationChunked {
                                specifier: msg.specifier,
                                features: msg.features,
                                chunk_count,
                                total_tokens_estimated: total_tokens,
                            })
                            .await;
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to chunk documentation for {}: {:#}",
                            msg.specifier,
                            e
                        );
                    }
                }
            })
        });

        let handle = builder.start().await;

        // Subscribe to DocumentationExtracted events through the broker
        handle.subscribe::<DocumentationExtracted>().await;

        Ok(handle)
    }
}

/// Content type for a chunk of documentation
#[derive(Debug, Clone, PartialEq, Eq)]
enum ContentType {
    Markdown,
    Rust,
    Toml,
}

impl ContentType {
    fn as_str(&self) -> &str {
        match self {
            Self::Markdown => "markdown",
            Self::Rust => "rust",
            Self::Toml => "toml",
        }
    }
}

/// Recursively finds all Rust source files in the src/ directory
///
/// This discovers ALL .rs files including:
/// - src/lib.rs or src/main.rs (entry points)
/// - src/module.rs files
/// - src/module/mod.rs files
/// - src/module/submodule.rs files
/// - Nested module hierarchies
///
/// # Arguments
///
/// * `crate_dir` - Path to the extracted crate directory
///
/// # Returns
///
/// Returns a vector of paths to all discovered Rust source files
///
/// # Errors
///
/// Returns an error if directory traversal fails
fn find_rust_source_files(crate_dir: &Path) -> Result<Vec<PathBuf>> {
    let src_dir = crate_dir.join("src");

    if !src_dir.exists() {
        tracing::warn!("src directory does not exist: {}", src_dir.display());
        return Ok(Vec::new());
    }

    let mut rust_files = Vec::new();

    for entry in WalkDir::new(&src_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only include .rs files
        if path.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            // Skip test files and benchmark files
            let path_str = path.to_string_lossy();
            if !path_str.contains("/tests/") && !path_str.contains("/benches/") {
                rust_files.push(path.to_path_buf());
            }
        }
    }

    // Sort for deterministic ordering
    rust_files.sort();

    tracing::debug!(
        "Discovered {} Rust source files in {}",
        rust_files.len(),
        src_dir.display()
    );

    Ok(rust_files)
}

/// Creates a tokenizer from the configured encoding
///
/// # Arguments
///
/// * `encoding` - Token encoding type to use
///
/// # Returns
///
/// Returns a CoreBPE tokenizer for the specified encoding
///
/// # Errors
///
/// Returns an error if the tokenizer cannot be created
fn create_tokenizer(encoding: TokenEncoding) -> Result<CoreBPE> {
    let bpe = match encoding {
        TokenEncoding::Cl100k => tiktoken_rs::cl100k_base()?,
        TokenEncoding::O200k => tiktoken_rs::o200k_base()?,
        TokenEncoding::P50k => tiktoken_rs::p50k_base()?,
    };
    Ok(bpe)
}

/// Creates a markdown splitter with the configured chunk size and overlap
///
/// # Arguments
///
/// * `config` - Processing configuration with chunk size and overlap
/// * `tokenizer` - Tokenizer for accurate token counting
///
/// # Returns
///
/// Returns a configured MarkdownSplitter
fn create_markdown_splitter(config: &ProcessConfig, tokenizer: CoreBPE) -> MarkdownSplitter<CoreBPE> {
    let chunk_config = ChunkConfig::new(config.chunk_size)
        .with_sizer(tokenizer)
        .with_overlap(config.chunk_overlap)
        .expect("chunk_size > chunk_overlap guaranteed by config validation");

    MarkdownSplitter::new(chunk_config)
}

/// Creates a code splitter for Rust source with the configured chunk size and overlap
///
/// # Arguments
///
/// * `config` - Processing configuration with chunk size and overlap
/// * `tokenizer` - Tokenizer for accurate token counting
///
/// # Returns
///
/// Returns a configured TextSplitter with Rust tree-sitter support
fn create_code_splitter(config: &ProcessConfig, tokenizer: CoreBPE) -> TextSplitter<CoreBPE> {
    let chunk_config = ChunkConfig::new(config.chunk_size)
        .with_sizer(tokenizer)
        .with_overlap(config.chunk_overlap)
        .expect("chunk_size > chunk_overlap guaranteed by config validation");

    TextSplitter::new(chunk_config)
}

/// Counts tokens in text using the tokenizer
///
/// # Arguments
///
/// * `tokenizer` - Tokenizer to use for counting
/// * `text` - Text to count tokens in
///
/// # Returns
///
/// Returns the number of tokens in the text
fn count_tokens(tokenizer: &CoreBPE, text: &str) -> usize {
    tokenizer.encode_with_special_tokens(text).len()
}

/// Processes content with the appropriate splitter and returns chunks
///
/// # Arguments
///
/// * `splitter` - Splitter to use (can be markdown or code splitter via trait object)
/// * `tokenizer` - Tokenizer for accurate token counting
/// * `content` - Content to chunk
/// * `source_file` - Source file name for tracking
/// * `content_type` - Type of content being processed
/// * `total_tokens` - Mutable reference to accumulate total token count
///
/// # Returns
///
/// Returns a vector of tuples: (content, source_file, content_type, token_count)
///
/// # Errors
///
/// Returns an error if chunking fails
fn process_content(
    chunks: Vec<&str>,
    tokenizer: &CoreBPE,
    source_file: &str,
    content_type: ContentType,
    total_tokens: &mut u32,
) -> Vec<(String, String, String, usize)> {
    let mut result = Vec::new();

    for chunk in chunks {
        let chunk_str = chunk.to_string();
        let token_count = count_tokens(tokenizer, &chunk_str);

        *total_tokens += token_count as u32;

        tracing::trace!(
            source_file = %source_file,
            content_type = %content_type.as_str(),
            token_count = token_count,
            "Processing chunk"
        );

        result.push((
            chunk_str,
            source_file.to_string(),
            content_type.as_str().to_string(),
            token_count,
        ));
    }

    result
}

/// Chunks documentation from a crate directory
///
/// This function discovers all documentation files and Rust source files,
/// processes them with the appropriate splitters (markdown or code), and
/// returns chunks ready for vectorization.
///
/// # Arguments
///
/// * `config` - Processing configuration with chunk size, overlap, and token encoding
/// * `specifier` - Crate name and version for context
/// * `extracted_path` - Path to the extracted crate directory
///
/// # Returns
///
/// Returns a tuple of (chunks, total_tokens) on success where chunks
/// is a vector of (content, source_file, content_type, token_count) tuples
///
/// # Errors
///
/// Returns an error if:
/// - Files cannot be read
/// - Crate directory doesn't exist
/// - Tokenizer creation fails
/// - Chunking logic fails
async fn chunk_documentation(
    config: &ProcessConfig,
    specifier: &CrateSpecifier,
    extracted_path: &Path,
) -> Result<(Vec<(String, String, String, usize)>, u32)> {
    // The extracted path is: download_dir/crate-version/
    // The tarball extracts to: download_dir/crate-version/crate-version/
    let crate_dir = extracted_path.join(format!(
        "{}-{}",
        specifier.name(),
        specifier.version()
    ));

    // Verify the crate directory exists
    if !crate_dir.exists() {
        anyhow::bail!(
            "Crate directory does not exist: {}",
            crate_dir.display()
        );
    }

    // Create tokenizer and splitters
    let tokenizer = create_tokenizer(config.token_encoding)
        .with_context(|| format!("Failed to create tokenizer for {:?}", config.token_encoding))?;

    let markdown_splitter = create_markdown_splitter(config, tokenizer.clone());
    let code_splitter = create_code_splitter(config, tokenizer.clone());

    let mut all_chunks = Vec::new();
    let mut total_tokens = 0u32;

    // Process README.md with markdown splitter
    let readme_path = crate_dir.join("README.md");
    if readme_path.exists() {
        let content = fs::read_to_string(&readme_path)
            .with_context(|| format!("Failed to read README.md from {}", readme_path.display()))?;

        if !content.trim().is_empty() {
            let chunks = markdown_splitter.chunks(&content).collect::<Vec<_>>();
            let processed = process_content(
                chunks,
                &tokenizer,
                "README.md",
                ContentType::Markdown,
                &mut total_tokens,
            );
            all_chunks.extend(processed);
        }
    }

    // Process Cargo.toml with markdown splitter (TOML with comments is markdown-ish)
    let cargo_toml_path = crate_dir.join("Cargo.toml");
    if cargo_toml_path.exists() {
        let content = fs::read_to_string(&cargo_toml_path).with_context(|| {
            format!("Failed to read Cargo.toml from {}", cargo_toml_path.display())
        })?;

        if !content.trim().is_empty() {
            let chunks = markdown_splitter.chunks(&content).collect::<Vec<_>>();
            let processed = process_content(
                chunks,
                &tokenizer,
                "Cargo.toml",
                ContentType::Toml,
                &mut total_tokens,
            );
            all_chunks.extend(processed);
        }
    }

    // Discover and process ALL Rust source files with code splitter
    let rust_files = find_rust_source_files(&crate_dir)?;

    for rust_file in rust_files {
        let content = fs::read_to_string(&rust_file)
            .with_context(|| format!("Failed to read Rust source from {}", rust_file.display()))?;

        if !content.trim().is_empty() {
            // Get relative path from crate_dir for better tracking
            let source_file = rust_file
                .strip_prefix(&crate_dir)
                .unwrap_or(&rust_file)
                .to_string_lossy()
                .to_string();

            let chunks = code_splitter.chunks(&content).collect::<Vec<_>>();
            let processed = process_content(
                chunks,
                &tokenizer,
                &source_file,
                ContentType::Rust,
                &mut total_tokens,
            );
            all_chunks.extend(processed);
        }
    }

    // Check if we have any documentation
    if all_chunks.is_empty() {
        anyhow::bail!("No documentation content found for {}", specifier);
    }

    tracing::info!(
        "Chunked {} total chunks ({} tokens) for {}",
        all_chunks.len(),
        total_tokens,
        specifier
    );

    Ok((all_chunks, total_tokens))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::str::FromStr;

    fn create_test_crate_structure(base_dir: &Path, crate_name: &str, version: &str) -> std::path::PathBuf {
        // Create structure: base_dir/crate-version/crate-version/
        let outer_dir = base_dir.join(format!("{}-{}", crate_name, version));
        let crate_dir = outer_dir.join(format!("{}-{}", crate_name, version));
        fs::create_dir_all(&crate_dir).expect("Failed to create test crate directory");

        // Create Cargo.toml
        let cargo_toml_content = format!(
            r#"
[package]
name = "{}"
version = "{}"
description = "Test crate"
"#,
            crate_name, version
        );
        fs::write(crate_dir.join("Cargo.toml"), cargo_toml_content)
            .expect("Failed to write Cargo.toml");

        // Create src/lib.rs with documentation
        fs::create_dir_all(crate_dir.join("src")).expect("Failed to create src directory");
        let lib_rs_content = r#"
//! Test crate documentation
//!
//! This is a test crate for documentation extraction.
//!
//! It has multiple paragraphs of documentation.

/// Example function with documentation
pub fn example() {
    println!("example");
}

/// Another function with more documentation
///
/// This has multiple lines of doc comments
/// to test chunking behavior.
pub fn another() {
    println!("another");
}

/// A struct to test code chunking
pub struct TestStruct {
    /// Field documentation
    pub field: String,
}

impl TestStruct {
    /// Constructor documentation
    pub fn new(field: String) -> Self {
        Self { field }
    }
}
"#;
        fs::write(crate_dir.join("src").join("lib.rs"), lib_rs_content)
            .expect("Failed to write lib.rs");

        // Create README.md
        let readme_content = "# Test Crate\n\nThis is a test crate.\n\nIt has multiple paragraphs.\n\n## Features\n\n- Feature 1\n- Feature 2\n";
        fs::write(crate_dir.join("README.md"), readme_content).expect("Failed to write README.md");

        outer_dir
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_processor_actor_spawn_creates_actor() {
        let mut runtime = ActonApp::launch();
        let config = ProcessConfig::default();
        let result = ProcessorActor::spawn(&mut runtime, config).await;
        assert!(result.is_ok(), "ProcessorActor spawn should succeed");

        let handle = result.unwrap();
        let stop_result = handle.stop().await;
        assert!(
            stop_result.is_ok(),
            "ProcessorActor should stop gracefully"
        );
    }

    #[test]
    fn test_processor_actor_new() {
        let config = ProcessConfig::default();
        let actor = ProcessorActor::new(config.clone());
        assert_eq!(actor.config.chunk_size, config.chunk_size);
        assert_eq!(actor.config.chunk_overlap, config.chunk_overlap);
        assert_eq!(actor.config.token_encoding, config.token_encoding);
    }

    #[tokio::test]
    async fn test_chunk_documentation_success() {
        let temp_dir = env::temp_dir().join(format!("crately_test_chunk_{}", rand::random::<u32>()));
        let _ = fs::create_dir_all(&temp_dir);

        let extracted_path = create_test_crate_structure(&temp_dir, "test_crate", "1.0.0");
        let specifier = CrateSpecifier::from_str("test_crate@1.0.0").unwrap();
        let config = ProcessConfig::default();

        let result = chunk_documentation(&config, &specifier, &extracted_path).await;
        assert!(result.is_ok(), "Documentation chunking should succeed");

        let (chunks, total_tokens) = result.unwrap();
        assert!(!chunks.is_empty(), "Should have created at least one chunk");
        assert!(total_tokens > 0, "Should have counted some tokens");

        // Verify chunks have all required data
        for (content, source_file, content_type, token_count) in &chunks {
            assert!(!content.is_empty(), "Chunk content should not be empty");
            assert!(!source_file.is_empty(), "Source file should not be empty");
            assert!(!content_type.is_empty(), "Content type should not be empty");
            assert!(*token_count > 0, "Token count should be > 0");
        }

        // Verify we processed multiple file types
        let content_types: std::collections::HashSet<_> = chunks.iter().map(|(_, _, ct, _)| ct.clone()).collect();
        assert!(content_types.len() > 1, "Should have processed multiple content types");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_chunk_documentation_missing_directory() {
        let temp_dir = env::temp_dir().join(format!("crately_test_chunk_missing_{}", rand::random::<u32>()));
        let specifier = CrateSpecifier::from_str("nonexistent@1.0.0").unwrap();
        let config = ProcessConfig::default();

        let result = chunk_documentation(&config, &specifier, &temp_dir).await;
        assert!(result.is_err(), "Should fail with missing directory");

        let error_str = format!("{}", result.unwrap_err());
        assert!(error_str.contains("does not exist"));
    }

    #[test]
    fn test_find_rust_source_files() {
        let temp_dir = env::temp_dir().join(format!("crately_test_find_{}", rand::random::<u32>()));
        let _ = fs::create_dir_all(&temp_dir);

        let crate_dir = create_test_crate_structure(&temp_dir, "test_find", "1.0.0")
            .join("test_find-1.0.0");

        let files = find_rust_source_files(&crate_dir).unwrap();
        assert!(!files.is_empty(), "Should find at least one Rust file");
        assert!(files.iter().any(|p| p.ends_with("lib.rs")), "Should find lib.rs");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_create_tokenizer_cl100k_base() {
        let result = create_tokenizer(TokenEncoding::Cl100k);
        assert!(result.is_ok(), "Should create cl100k_base tokenizer");
    }

    #[test]
    fn test_count_tokens() {
        let tokenizer = create_tokenizer(TokenEncoding::Cl100k).unwrap();
        let text = "Hello world, this is a test.";
        let count = count_tokens(&tokenizer, text);
        assert!(count > 0, "Should count tokens");
        assert!(count < 20, "Token count should be reasonable for short text");
    }

    #[test]
    fn test_content_type_as_str() {
        assert_eq!(ContentType::Markdown.as_str(), "markdown");
        assert_eq!(ContentType::Rust.as_str(), "rust");
        assert_eq!(ContentType::Toml.as_str(), "toml");
    }
}
