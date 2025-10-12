//! Actor responsible for chunking documentation text into embeddable segments
//!
//! The `ProcessorActor` is a stateless worker that subscribes to `DocumentationExtracted` events
//! and processes the extracted documentation into semantically meaningful chunks for vectorization.
//! It implements intelligent text segmentation that preserves context while staying within token limits.

use acton_reactive::prelude::*;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::actors::config::ProcessConfig;
use crate::crate_specifier::CrateSpecifier;
use crate::messages::{DocumentationChunked, DocumentationExtracted, PersistDocChunk};
use crate::types::ChunkMetadata;

/// Stateless actor for chunking documentation text
///
/// This actor subscribes to `DocumentationExtracted` events and performs the following:
/// 1. Re-reads documentation files from the extracted crate directory
/// 2. Concatenates documentation content with logical separators
/// 3. Splits text into semantic chunks (paragraphs, doc comment blocks)
/// 4. Groups chunks to target configured chunk size with overlap
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
/// - Broadcasts: `DocumentationChunked`
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
    /// * `config` - Processing configuration including chunk size and overlap
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
    /// documentation text, broadcasting success events.
    ///
    /// This follows the simple actor pattern where only the handle is returned,
    /// as the ProcessorActor has no startup data to provide to the application.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    /// * `config` - Processing configuration including chunk size and overlap
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
                    Ok((chunks, total_tokens_estimated)) => {
                        let chunk_count = chunks.len() as u32;

                        // Persist all chunks to database before broadcasting event
                        for (index, (content, source_file)) in chunks.into_iter().enumerate() {
                            let chunk_id = format!(
                                "{}_{}_{:03}",
                                msg.specifier.name().replace('-', "_"),
                                msg.specifier.version().to_string().replace('.', "_"),
                                index
                            );

                            let metadata = ChunkMetadata {
                                content_type: "markdown".to_string(),
                                start_line: None,
                                end_line: None,
                                token_count: estimate_tokens(content.len()) as usize,
                                char_count: content.len(),
                                parent_module: None,
                                item_type: None,
                                item_name: None,
                            };

                            broker
                                .broadcast(PersistDocChunk {
                                    specifier: msg.specifier.clone(),
                                    chunk_index: index,
                                    chunk_id,
                                    content,
                                    source_file,
                                    metadata,
                                })
                                .await;
                        }

                        // Broadcast DocumentationChunked event with statistics
                        broker
                            .broadcast(DocumentationChunked {
                                specifier: msg.specifier,
                                features: msg.features,
                                chunk_count,
                                total_tokens_estimated,
                            })
                            .await;
                    }
                    Err(e) => {
                        // Log error - in a real implementation, you might want to broadcast
                        // a DocumentationChunkingFailed message similar to FileReaderActor
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

/// Chunks documentation from a crate directory
///
/// This function re-reads documentation files, concatenates them, and chunks
/// the text into semantically meaningful segments for vectorization.
///
/// # Arguments
///
/// * `config` - Processing configuration with chunk size and overlap settings
/// * `specifier` - Crate name and version for context
/// * `extracted_path` - Path to the extracted crate directory
///
/// # Returns
///
/// Returns a tuple of (chunks, total_tokens_estimated) on success where chunks
/// is a vector of (content, source_file) tuples
///
/// # Errors
///
/// Returns an error if:
/// - Files cannot be read
/// - Documentation is empty
/// - Chunking logic fails
async fn chunk_documentation(
    config: &ProcessConfig,
    specifier: &CrateSpecifier,
    extracted_path: &Path,
) -> Result<(Vec<(String, String)>, u32)> {
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

    // Read and concatenate all documentation files
    let mut full_documentation = String::new();
    let mut primary_source_file = String::from("combined");

    // Read README.md if it exists
    let readme_path = crate_dir.join("README.md");
    if readme_path.exists() {
        let readme_content = fs::read_to_string(&readme_path)
            .with_context(|| format!("Failed to read README.md from {}", readme_path.display()))?;
        full_documentation.push_str("=== README.md ===\n\n");
        full_documentation.push_str(&readme_content);
        full_documentation.push_str("\n\n");
        primary_source_file = String::from("README.md");
    }

    // Read Cargo.toml
    let cargo_toml_path = crate_dir.join("Cargo.toml");
    if cargo_toml_path.exists() {
        let cargo_content = fs::read_to_string(&cargo_toml_path).with_context(|| {
            format!("Failed to read Cargo.toml from {}", cargo_toml_path.display())
        })?;
        full_documentation.push_str("=== Cargo.toml ===\n\n");
        full_documentation.push_str(&cargo_content);
        full_documentation.push_str("\n\n");
    }

    // Read lib.rs or src/lib.rs or src/main.rs
    let lib_paths = vec![
        crate_dir.join("lib.rs"),
        crate_dir.join("src").join("lib.rs"),
        crate_dir.join("src").join("main.rs"),
    ];

    for lib_path in lib_paths {
        if lib_path.exists() {
            let lib_content = fs::read_to_string(&lib_path)
                .with_context(|| format!("Failed to read source from {}", lib_path.display()))?;
            full_documentation.push_str(&format!("=== {} ===\n\n", lib_path.file_name().unwrap().to_string_lossy()));
            full_documentation.push_str(&lib_content);
            full_documentation.push_str("\n\n");
            break;
        }
    }

    // Check if we have any documentation
    if full_documentation.is_empty() {
        anyhow::bail!("No documentation content found for {}", specifier);
    }

    // Chunk the documentation
    let text_chunks = create_semantic_chunks(&full_documentation, config.chunk_size, config.chunk_overlap);

    // Pair each chunk with its source file
    let chunks: Vec<(String, String)> = text_chunks
        .into_iter()
        .map(|content| (content, primary_source_file.clone()))
        .collect();

    // Estimate total tokens
    let total_chars: usize = chunks.iter().map(|(content, _)| content.len()).sum();
    let total_tokens_estimated = estimate_tokens(total_chars);

    Ok((chunks, total_tokens_estimated))
}

/// Creates semantic chunks from documentation text
///
/// This function splits documentation on natural boundaries (paragraphs, blank lines)
/// and groups them into chunks targeting the specified size with overlap.
///
/// # Arguments
///
/// * `text` - Full documentation text to chunk
/// * `target_chunk_size` - Target chunk size in characters
/// * `overlap_size` - Overlap between chunks in characters
///
/// # Returns
///
/// Returns a vector of text chunks
fn create_semantic_chunks(text: &str, target_chunk_size: usize, overlap_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();

    // Split on double newlines (paragraph boundaries)
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .collect();

    if paragraphs.is_empty() {
        return chunks;
    }

    let mut current_chunk = String::new();
    let mut overlap_buffer = String::new();

    for paragraph in paragraphs {
        let paragraph_with_newline = format!("{}\n\n", paragraph.trim());

        // If adding this paragraph would exceed target size and we have content
        if !current_chunk.is_empty() && current_chunk.len() + paragraph_with_newline.len() > target_chunk_size {
            // Save the current chunk
            chunks.push(current_chunk.clone());

            // Start new chunk with overlap from previous chunk
            if !overlap_buffer.is_empty() {
                current_chunk = overlap_buffer.clone();
            } else {
                current_chunk.clear();
            }
        }

        // Add the paragraph to current chunk
        current_chunk.push_str(&paragraph_with_newline);

        // Update overlap buffer (last N characters of current chunk)
        if current_chunk.len() > overlap_size {
            let start_idx = current_chunk.len() - overlap_size;
            overlap_buffer = current_chunk[start_idx..].to_string();
        } else {
            overlap_buffer = current_chunk.clone();
        }
    }

    // Add the final chunk if it has content
    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    // If no chunks were created (text too small or no paragraphs), create a single chunk
    if chunks.is_empty() && !text.trim().is_empty() {
        chunks.push(text.to_string());
    }

    chunks
}

/// Estimates token count from character count
///
/// Uses a rough approximation of 4 characters per token (typical for English text).
///
/// # Arguments
///
/// * `char_count` - Number of characters
///
/// # Returns
///
/// Returns estimated number of tokens
#[inline]
fn estimate_tokens(char_count: usize) -> u32 {
    // Rough approximation: 4 characters per token on average for English text
    ((char_count as f64) / 4.0).ceil() as u32
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
"#;
        fs::write(crate_dir.join("src").join("lib.rs"), lib_rs_content)
            .expect("Failed to write lib.rs");

        // Create README.md
        let readme_content = "# Test Crate\n\nThis is a test crate.\n\nIt has multiple paragraphs.\n";
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
        assert!(total_tokens > 0, "Should have estimated some tokens");
        // Verify chunks have both content and source file
        for (content, source_file) in &chunks {
            assert!(!content.is_empty(), "Chunk content should not be empty");
            assert!(!source_file.is_empty(), "Source file should not be empty");
        }

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
    fn test_create_semantic_chunks_basic() {
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = create_semantic_chunks(text, 100, 20);

        assert!(!chunks.is_empty(), "Should create at least one chunk");
    }

    #[test]
    fn test_create_semantic_chunks_with_overlap() {
        let text = "A".repeat(500) + "\n\n" + &"B".repeat(500);
        let chunks = create_semantic_chunks(&text, 600, 100);

        assert!(chunks.len() >= 2, "Should create multiple chunks");
        // Check that chunks have some overlap (second chunk should start with end of first)
        if chunks.len() >= 2 {
            let first_end = &chunks[0][chunks[0].len() - 100..];
            assert!(chunks[1].starts_with(first_end), "Chunks should have overlap");
        }
    }

    #[test]
    fn test_create_semantic_chunks_empty_text() {
        let text = "";
        let chunks = create_semantic_chunks(text, 100, 20);

        assert!(chunks.is_empty(), "Should create no chunks for empty text");
    }

    #[test]
    fn test_create_semantic_chunks_single_paragraph() {
        let text = "Single paragraph with some content.";
        let chunks = create_semantic_chunks(text, 100, 20);

        assert_eq!(chunks.len(), 1, "Should create one chunk for single paragraph");
    }

    #[test]
    fn test_create_semantic_chunks_respects_target_size() {
        let paragraphs: Vec<String> = (0..10).map(|i| format!("Paragraph {}.", i)).collect();
        let text = paragraphs.join("\n\n");
        let chunks = create_semantic_chunks(&text, 50, 10);

        // All chunks except possibly the last should be around target size
        for (i, chunk) in chunks.iter().enumerate() {
            if i < chunks.len() - 1 {
                // Not the last chunk - should be close to or over target size
                assert!(chunk.len() >= 30, "Chunk {} should have reasonable size", i);
            }
        }
    }

    #[test]
    fn test_estimate_tokens_zero() {
        assert_eq!(estimate_tokens(0), 0);
    }

    #[test]
    fn test_estimate_tokens_small() {
        let tokens = estimate_tokens(100);
        assert_eq!(tokens, 25); // 100 / 4 = 25
    }

    #[test]
    fn test_estimate_tokens_large() {
        let tokens = estimate_tokens(10000);
        assert_eq!(tokens, 2500); // 10000 / 4 = 2500
    }

    #[test]
    fn test_estimate_tokens_rounding() {
        let tokens = estimate_tokens(10); // 10 / 4 = 2.5, should ceil to 3
        assert_eq!(tokens, 3);
    }
}
