//! Actor responsible for extracting documentation from downloaded crate files
//!
//! The `FileReaderActor` is a stateless worker that subscribes to `CrateDownloaded` events
//! and extracts documentation from the downloaded crate directory. It reads README files,
//! Cargo.toml metadata, and source file documentation to produce a structured documentation
//! payload for the next pipeline stage.

use acton_reactive::prelude::*;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::actors::config::ReadConfig;
use crate::crate_specifier::CrateSpecifier;
use crate::messages::{
    CrateDownloaded, DocumentationExtracted, DocumentationExtractionFailed, ExtractionErrorKind,
};

/// Stateless actor for extracting documentation from downloaded crates
///
/// This actor subscribes to `CrateDownloaded` events and performs the following:
/// 1. Reads documentation files from the extracted crate directory
/// 2. Extracts content from README.md, lib.rs, and Cargo.toml
/// 3. Aggregates documentation into a structured format
/// 4. Broadcasts `DocumentationExtracted` on success or `DocumentationExtractionFailed` on error
///
/// # State
///
/// The actor is stateless and relies on configuration provided through the
/// `ReadConfig` which is immutable after initialization.
///
/// # Message Flow
///
/// - Subscribes to: `CrateDownloaded`
/// - Broadcasts: `DocumentationExtracted`, `DocumentationExtractionFailed`
#[acton_actor]
pub struct FileReaderActor {
    /// Read configuration (immutable)
    config: ReadConfig,
}

impl FileReaderActor {
    /// Creates a new FileReaderActor with the given configuration
    ///
    /// This constructor encapsulates the initialization logic, storing the
    /// immutable read configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Read configuration including file patterns and size limits
    ///
    /// # Returns
    ///
    /// Returns a new `FileReaderActor` instance ready to be spawned.
    pub fn new(config: ReadConfig) -> Self {
        Self { config }
    }

    /// Spawns, configures, and starts a new FileReaderActor
    ///
    /// This is the standard factory method for creating FileReaderActor actors.
    /// The FileReaderActor subscribes to `CrateDownloaded` events and extracts
    /// documentation from crate directories, broadcasting success or failure events.
    ///
    /// This follows the simple actor pattern where only the handle is returned,
    /// as the FileReaderActor has no startup data to provide to the application.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    /// * `config` - Read configuration including file patterns and size limits
    ///
    /// # Returns
    ///
    /// Returns `AgentHandle` to the started FileReaderActor for message passing.
    ///
    /// # When to Use
    ///
    /// Call this during application startup to initialize the documentation extraction
    /// subsystem. The actor will be ready to process `CrateDownloaded` events and
    /// coordinate with other actors in the crate processing pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if actor creation or initialization fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use acton_reactive::prelude::*;
    /// use crately::actors::file_reader_actor::FileReaderActor;
    /// use crately::actors::config::ReadConfig;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut runtime = ActonApp::launch();
    ///
    ///     // Spawn the FileReaderActor for handling documentation extraction
    ///     let config = ReadConfig::default();
    ///     let file_reader = FileReaderActor::spawn(&mut runtime, config).await?;
    ///
    ///     runtime.shutdown_all().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn spawn(runtime: &mut AgentRuntime, config: ReadConfig) -> Result<AgentHandle> {
        let mut builder = runtime
            .new_agent_with_name::<FileReaderActor>("file_reader".to_string())
            .await;

        // Initialize actor with configuration
        builder.model = FileReaderActor::new(config);

        // Subscribe to CrateDownloaded events
        builder.mutate_on::<CrateDownloaded>(|agent, envelope| {
            let msg = envelope.message().clone();
            let broker = agent.broker().clone();
            let config = agent.model.config.clone();

            AgentReply::from_async(async move {
                let start_time = Instant::now();

                match extract_documentation(&config, &msg.specifier, &msg.extracted_path).await {
                    Ok((documentation_bytes, file_count)) => {
                        broker
                            .broadcast(DocumentationExtracted {
                                specifier: msg.specifier,
                                features: msg.features,
                                documentation_bytes,
                                file_count,
                                extracted_path: msg.extracted_path,
                            })
                            .await;
                    }
                    Err(e) => {
                        let error_kind = classify_extraction_error(&e);
                        let error_message = format!("{:#}", e);

                        broker
                            .broadcast(DocumentationExtractionFailed {
                                specifier: msg.specifier,
                                features: msg.features,
                                error_message,
                                error_kind,
                                elapsed_ms: start_time.elapsed().as_millis() as u64,
                            })
                            .await;
                    }
                }
            })
        });

        let handle = builder.start().await;

        // Subscribe to CrateDownloaded events through the broker
        handle.subscribe::<CrateDownloaded>().await;

        Ok(handle)
    }
}

/// Extracts documentation from a crate directory
///
/// This function reads documentation from multiple sources within the crate:
/// 1. README.md (if present)
/// 2. Cargo.toml metadata (description, documentation link)
/// 3. lib.rs or src/lib.rs module-level documentation
///
/// # Arguments
///
/// * `config` - Read configuration with file patterns and size limits
/// * `specifier` - Crate name and version for context
/// * `extracted_path` - Path to the extracted crate directory
///
/// # Returns
///
/// Returns a tuple of (total_bytes_extracted, file_count) on success
///
/// # Errors
///
/// Returns an error if:
/// - Required files cannot be read
/// - File size exceeds configured limits
/// - Directory structure is invalid
async fn extract_documentation(
    config: &ReadConfig,
    specifier: &CrateSpecifier,
    extracted_path: &Path,
) -> Result<(u64, u32)> {
    let mut total_bytes = 0u64;
    let mut file_count = 0u32;

    // The extracted path is: download_dir/crate-version/
    // The tarball extracts to: download_dir/crate-version/crate-version/
    // So we need to look inside the nested directory
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

    // Extract README.md (optional)
    if let Some(readme_bytes) = read_optional_file(&crate_dir.join("README.md"), config)? {
        total_bytes += readme_bytes;
        file_count += 1;
    }

    // Extract Cargo.toml (required for metadata)
    let cargo_toml_path = crate_dir.join("Cargo.toml");
    if cargo_toml_path.exists() {
        let cargo_toml_bytes = read_file_with_size_check(&cargo_toml_path, config)?;
        total_bytes += cargo_toml_bytes;
        file_count += 1;
    } else {
        anyhow::bail!(
            "Cargo.toml not found in crate directory: {}",
            crate_dir.display()
        );
    }

    // Extract lib.rs or src/lib.rs (main library source)
    let lib_paths = vec![
        crate_dir.join("lib.rs"),
        crate_dir.join("src").join("lib.rs"),
        crate_dir.join("src").join("main.rs"), // Fallback for binary crates
    ];

    let mut found_source = false;
    for lib_path in lib_paths {
        if lib_path.exists() {
            let lib_bytes = read_file_with_size_check(&lib_path, config)?;
            total_bytes += lib_bytes;
            file_count += 1;
            found_source = true;
            break;
        }
    }

    if !found_source {
        anyhow::bail!(
            "No lib.rs or main.rs found in crate directory: {}",
            crate_dir.display()
        );
    }

    // Return total bytes and file count
    Ok((total_bytes, file_count))
}

/// Reads an optional file, returning None if it doesn't exist
///
/// # Arguments
///
/// * `path` - Path to the file
/// * `config` - Read configuration with size limits
///
/// # Returns
///
/// Returns Some(bytes_read) if file exists and was read successfully, None if file doesn't exist
///
/// # Errors
///
/// Returns an error if file exists but cannot be read or exceeds size limits
fn read_optional_file(path: &Path, config: &ReadConfig) -> Result<Option<u64>> {
    if !path.exists() {
        return Ok(None);
    }

    let bytes = read_file_with_size_check(path, config)?;
    Ok(Some(bytes))
}

/// Reads a file with size checking
///
/// # Arguments
///
/// * `path` - Path to the file
/// * `config` - Read configuration with size limits
///
/// # Returns
///
/// Returns the number of bytes read on success
///
/// # Errors
///
/// Returns an error if:
/// - File cannot be read
/// - File size exceeds configured maximum
fn read_file_with_size_check(path: &Path, config: &ReadConfig) -> Result<u64> {
    // Check file size before reading
    let metadata = fs::metadata(path)
        .with_context(|| format!("Failed to read file metadata: {}", path.display()))?;

    let file_size = metadata.len();

    if file_size > config.max_file_size_bytes {
        anyhow::bail!(
            "File size ({} bytes) exceeds maximum allowed ({} bytes): {}",
            file_size,
            config.max_file_size_bytes,
            path.display()
        );
    }

    // Read the file (we don't need to store the content, just verify it's readable)
    let _content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    Ok(file_size)
}

/// Classifies an extraction error into a specific `ExtractionErrorKind` for programmatic handling
///
/// # Arguments
///
/// * `error` - The error to classify
///
/// # Returns
///
/// Returns the appropriate `ExtractionErrorKind` based on error characteristics
fn classify_extraction_error(error: &anyhow::Error) -> ExtractionErrorKind {
    let error_str = format!("{:#}", error);

    if error_str.contains("does not exist") || error_str.contains("not found") {
        ExtractionErrorKind::FileNotFound
    } else if error_str.contains("exceeds maximum") || error_str.contains("too large") {
        ExtractionErrorKind::FileTooLarge
    } else if error_str.contains("Permission denied")
        || error_str.contains("access denied")
        || error_str.contains("Forbidden")
    {
        ExtractionErrorKind::PermissionDenied
    } else if error_str.contains("Invalid") || error_str.contains("malformed") {
        ExtractionErrorKind::InvalidFormat
    } else {
        ExtractionErrorKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;
    use std::str::FromStr;

    fn create_test_crate_structure(base_dir: &Path, crate_name: &str, version: &str) -> PathBuf {
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

        // Create src/lib.rs
        fs::create_dir_all(crate_dir.join("src")).expect("Failed to create src directory");
        let lib_rs_content = r#"
//! Test crate documentation
//!
//! This is a test crate for documentation extraction.

pub fn example() {}
"#;
        fs::write(crate_dir.join("src").join("lib.rs"), lib_rs_content)
            .expect("Failed to write lib.rs");

        // Create README.md
        let readme_content = "# Test Crate\n\nThis is a test crate.\n";
        fs::write(crate_dir.join("README.md"), readme_content).expect("Failed to write README.md");

        outer_dir
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_file_reader_actor_spawn_creates_actor() {
        let mut runtime = ActonApp::launch();
        let config = ReadConfig::default();
        let result = FileReaderActor::spawn(&mut runtime, config).await;
        assert!(result.is_ok(), "FileReaderActor spawn should succeed");

        let handle = result.unwrap();
        let stop_result = handle.stop().await;
        assert!(
            stop_result.is_ok(),
            "FileReaderActor should stop gracefully"
        );
    }

    #[test]
    fn test_file_reader_actor_new() {
        let config = ReadConfig::default();
        let actor = FileReaderActor::new(config.clone());
        assert_eq!(actor.config.timeout_secs, config.timeout_secs);
    }

    #[tokio::test]
    async fn test_extract_documentation_success() {
        let temp_dir = env::temp_dir().join(format!("crately_test_extract_{}", rand::random::<u32>()));
        let _ = fs::create_dir_all(&temp_dir);

        let extracted_path = create_test_crate_structure(&temp_dir, "test_crate", "1.0.0");
        let specifier = CrateSpecifier::from_str("test_crate@1.0.0").unwrap();
        let config = ReadConfig::default();

        let result = extract_documentation(&config, &specifier, &extracted_path).await;
        assert!(result.is_ok(), "Documentation extraction should succeed");

        let (bytes, file_count) = result.unwrap();
        assert!(bytes > 0, "Should have extracted some bytes");
        assert_eq!(file_count, 3, "Should have extracted 3 files (README, Cargo.toml, lib.rs)");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_extract_documentation_missing_directory() {
        let temp_dir = env::temp_dir().join(format!("crately_test_missing_{}", rand::random::<u32>()));
        let specifier = CrateSpecifier::from_str("nonexistent@1.0.0").unwrap();
        let config = ReadConfig::default();

        let result = extract_documentation(&config, &specifier, &temp_dir).await;
        assert!(result.is_err(), "Should fail with missing directory");

        let error_str = format!("{}", result.unwrap_err());
        assert!(error_str.contains("does not exist"));
    }

    #[tokio::test]
    async fn test_extract_documentation_missing_cargo_toml() {
        let temp_dir = env::temp_dir().join(format!("crately_test_no_cargo_{}", rand::random::<u32>()));
        let extracted_path = temp_dir.join("test_crate-1.0.0");
        let crate_dir = extracted_path.join("test_crate-1.0.0");
        fs::create_dir_all(&crate_dir).expect("Failed to create test directory");

        // Create src/lib.rs but no Cargo.toml
        fs::create_dir_all(crate_dir.join("src")).expect("Failed to create src directory");
        fs::write(crate_dir.join("src").join("lib.rs"), "pub fn test() {}")
            .expect("Failed to write lib.rs");

        let specifier = CrateSpecifier::from_str("test_crate@1.0.0").unwrap();
        let config = ReadConfig::default();

        let result = extract_documentation(&config, &specifier, &extracted_path).await;
        assert!(result.is_err(), "Should fail without Cargo.toml");

        let error_str = format!("{}", result.unwrap_err());
        assert!(error_str.contains("Cargo.toml not found"));

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_extract_documentation_missing_source() {
        let temp_dir = env::temp_dir().join(format!("crately_test_no_source_{}", rand::random::<u32>()));
        let extracted_path = temp_dir.join("test_crate-1.0.0");
        let crate_dir = extracted_path.join("test_crate-1.0.0");
        fs::create_dir_all(&crate_dir).expect("Failed to create test directory");

        // Create Cargo.toml but no lib.rs
        fs::write(
            crate_dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"\n",
        )
        .expect("Failed to write Cargo.toml");

        let specifier = CrateSpecifier::from_str("test_crate@1.0.0").unwrap();
        let config = ReadConfig::default();

        let result = extract_documentation(&config, &specifier, &extracted_path).await;
        assert!(result.is_err(), "Should fail without lib.rs");

        let error_str = format!("{}", result.unwrap_err());
        assert!(error_str.contains("No lib.rs or main.rs found"));

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_extract_documentation_without_readme() {
        let temp_dir = env::temp_dir().join(format!("crately_test_no_readme_{}", rand::random::<u32>()));
        let extracted_path = temp_dir.join("test_crate-1.0.0");
        let crate_dir = extracted_path.join("test_crate-1.0.0");
        fs::create_dir_all(crate_dir.join("src")).expect("Failed to create test directory");

        // Create Cargo.toml and lib.rs but no README
        fs::write(
            crate_dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"\n",
        )
        .expect("Failed to write Cargo.toml");

        fs::write(crate_dir.join("src").join("lib.rs"), "pub fn test() {}")
            .expect("Failed to write lib.rs");

        let specifier = CrateSpecifier::from_str("test_crate@1.0.0").unwrap();
        let config = ReadConfig::default();

        let result = extract_documentation(&config, &specifier, &extracted_path).await;
        assert!(result.is_ok(), "Should succeed without README");

        let (bytes, file_count) = result.unwrap();
        assert!(bytes > 0, "Should have extracted some bytes");
        assert_eq!(file_count, 2, "Should have extracted 2 files (Cargo.toml, lib.rs)");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_read_file_with_size_check_success() {
        let temp_dir = env::temp_dir().join(format!("crately_test_size_{}", rand::random::<u32>()));
        let _ = fs::create_dir_all(&temp_dir);

        let test_file = temp_dir.join("test.txt");
        let content = "Test content";
        fs::write(&test_file, content).expect("Failed to write test file");

        let config = ReadConfig::default();
        let result = read_file_with_size_check(&test_file, &config);
        assert!(result.is_ok(), "Should read file successfully");
        assert_eq!(result.unwrap(), content.len() as u64);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_read_file_with_size_check_too_large() {
        let temp_dir = env::temp_dir().join(format!("crately_test_large_{}", rand::random::<u32>()));
        let _ = fs::create_dir_all(&temp_dir);

        let test_file = temp_dir.join("large.txt");
        let content = "x".repeat(1000);
        fs::write(&test_file, content).expect("Failed to write test file");

        let config = ReadConfig {
            max_file_size_bytes: 100, // Set limit lower than file size
            ..Default::default()
        };

        let result = read_file_with_size_check(&test_file, &config);
        assert!(result.is_err(), "Should fail with file too large");

        let error_str = format!("{}", result.unwrap_err());
        assert!(error_str.contains("exceeds maximum"));

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_read_optional_file_exists() {
        let temp_dir = env::temp_dir().join(format!("crately_test_optional_{}", rand::random::<u32>()));
        let _ = fs::create_dir_all(&temp_dir);

        let test_file = temp_dir.join("test.txt");
        fs::write(&test_file, "content").expect("Failed to write test file");

        let config = ReadConfig::default();
        let result = read_optional_file(&test_file, &config);
        assert!(result.is_ok(), "Should read optional file successfully");
        assert!(result.unwrap().is_some(), "Should return Some(bytes)");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_read_optional_file_missing() {
        let temp_dir = env::temp_dir().join(format!("crately_test_optional_missing_{}", rand::random::<u32>()));
        let test_file = temp_dir.join("nonexistent.txt");

        let config = ReadConfig::default();
        let result = read_optional_file(&test_file, &config);
        assert!(result.is_ok(), "Should handle missing file gracefully");
        assert!(result.unwrap().is_none(), "Should return None for missing file");
    }

    #[test]
    fn test_classify_extraction_error_file_not_found() {
        let error = anyhow::anyhow!("File does not exist");
        let kind = classify_extraction_error(&error);
        assert_eq!(kind, ExtractionErrorKind::FileNotFound);
    }

    #[test]
    fn test_classify_extraction_error_file_too_large() {
        let error = anyhow::anyhow!("File exceeds maximum allowed size");
        let kind = classify_extraction_error(&error);
        assert_eq!(kind, ExtractionErrorKind::FileTooLarge);
    }

    #[test]
    fn test_classify_extraction_error_permission_denied() {
        let error = anyhow::anyhow!("Permission denied");
        let kind = classify_extraction_error(&error);
        assert_eq!(kind, ExtractionErrorKind::PermissionDenied);
    }

    #[test]
    fn test_classify_extraction_error_invalid_format() {
        let error = anyhow::anyhow!("Invalid file format");
        let kind = classify_extraction_error(&error);
        assert_eq!(kind, ExtractionErrorKind::InvalidFormat);
    }

    #[test]
    fn test_classify_extraction_error_other() {
        let error = anyhow::anyhow!("Unknown error");
        let kind = classify_extraction_error(&error);
        assert_eq!(kind, ExtractionErrorKind::Other);
    }
}
