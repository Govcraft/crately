//! Actor responsible for downloading crate artifacts from crates.io
//!
//! The `DownloaderActor` is a stateless worker that subscribes to `CrateReceived` events
//! and downloads crate tarballs from crates.io. It extracts the tarball to the configured
//! cache directory and broadcasts success or failure events.

use acton_reactive::prelude::*;
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use reqwest::Client;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tar::Archive;

use crate::actors::config::DownloadConfig;
use crate::crate_specifier::CrateSpecifier;
use crate::messages::{CrateDownloadFailed, CrateDownloaded, CrateReceived, DownloadErrorKind};

/// Stateless actor for downloading crate tarballs from crates.io
///
/// This actor subscribes to `CrateReceived` events and performs the following:
/// 1. Downloads the crate tarball from crates.io using HTTP
/// 2. Extracts the tarball to the configured download directory
/// 3. Broadcasts `CrateDownloaded` on success or `CrateDownloadFailed` on error
///
/// # State
///
/// The actor is stateless and relies on configuration provided through the
/// `DownloadConfig` which is immutable after initialization.
///
/// # Message Flow
///
/// - Subscribes to: `CrateReceived`
/// - Broadcasts: `CrateDownloaded`, `CrateDownloadFailed`
#[acton_actor]
pub struct DownloaderActor {
    /// Download configuration (immutable)
    config: DownloadConfig,
    /// HTTP client for downloading tarballs (reusable across requests)
    http_client: Client,
}

impl DownloaderActor {
    /// Creates a new DownloaderActor with the given configuration
    ///
    /// This constructor encapsulates the initialization logic, creating the HTTP client
    /// and storing the immutable download configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Download configuration including cache directory and timeout
    ///
    /// # Returns
    ///
    /// Returns a new `DownloaderActor` instance ready to be spawned.
    pub fn new(config: DownloadConfig) -> Self {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .user_agent("crately/0.1.0") // Identify as crately for API compliance
            .build()
            .expect("Failed to build HTTP client");

        Self {
            config,
            http_client,
        }
    }

    /// Spawns, configures, and starts a new DownloaderActor
    ///
    /// This is the standard factory method for creating DownloaderActor actors.
    /// The DownloaderActor subscribes to `CrateReceived` events and downloads
    /// crate tarballs from crates.io, broadcasting success or failure events.
    ///
    /// This follows the simple actor pattern where only the handle is returned,
    /// as the DownloaderActor has no startup data to provide to the application.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    /// * `config` - Download configuration including cache directory and timeout
    ///
    /// # Returns
    ///
    /// Returns `AgentHandle` to the started DownloaderActor for message passing.
    ///
    /// # When to Use
    ///
    /// Call this during application startup to initialize the download subsystem.
    /// The actor will be ready to process `CrateReceived` events and coordinate
    /// with other actors in the crate processing pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if actor creation or initialization fails, or if the
    /// download directory cannot be created.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use acton_reactive::prelude::*;
    /// use crately::actors::downloader_actor::DownloaderActor;
    /// use crately::actors::config::DownloadConfig;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut runtime = ActonApp::launch();
    ///
    ///     // Spawn the DownloaderActor for handling downloads
    ///     let config = DownloadConfig::default();
    ///     let downloader = DownloaderActor::spawn(&mut runtime, config).await?;
    ///
    ///     runtime.shutdown_all().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn spawn(runtime: &mut AgentRuntime, config: DownloadConfig) -> Result<AgentHandle> {
        // Ensure download directory exists
        ensure_download_directory_exists(&config.download_dir)?;

        let mut builder = runtime
            .new_agent_with_name::<DownloaderActor>("downloader".to_string())
            .await;

        // Initialize actor with configuration
        builder.model = DownloaderActor::new(config);

        // Subscribe to CrateReceived events
        builder.mutate_on::<CrateReceived>(|agent, envelope| {
            let msg = envelope.message().clone();
            let broker = agent.broker().clone();
            let config = agent.model.config.clone();
            let http_client = agent.model.http_client.clone();

            AgentReply::from_async(async move {
                let start_time = Instant::now();

                match download_and_extract(&http_client, &config, &msg.specifier).await {
                    Ok(extracted_path) => {
                        let duration_ms = start_time.elapsed().as_millis() as u64;

                        broker
                            .broadcast(CrateDownloaded {
                                specifier: msg.specifier,
                                features: msg.features,
                                extracted_path,
                                download_duration_ms: duration_ms,
                            })
                            .await;
                    }
                    Err(e) => {
                        let error_kind = classify_error(&e);
                        let error_message = format!("{:#}", e);

                        broker
                            .broadcast(CrateDownloadFailed {
                                specifier: msg.specifier,
                                features: msg.features,
                                error_message,
                                error_kind,
                            })
                            .await;
                    }
                }
            })
        });

        let handle = builder.start().await;

        // Subscribe to CrateReceived events through the broker
        handle.subscribe::<CrateReceived>().await;

        Ok(handle)
    }
}

/// Ensures the download directory exists, creating it if necessary
///
/// # Errors
///
/// Returns an error if the directory cannot be created
fn ensure_download_directory_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)
            .with_context(|| format!("Failed to create download directory: {}", path.display()))?;
    }
    Ok(())
}

/// Downloads a crate tarball from crates.io and extracts it to the cache directory
///
/// This function performs the complete download and extraction workflow:
/// 1. Constructs the download URL based on crate name and version
/// 2. Downloads the .crate tarball via HTTP
/// 3. Extracts the tarball to the cache directory
/// 4. Returns the path to the extracted crate
///
/// # Arguments
///
/// * `client` - HTTP client for downloading
/// * `config` - Download configuration
/// * `specifier` - Crate name and version to download
///
/// # Returns
///
/// Returns the path to the extracted crate directory on success
///
/// # Errors
///
/// Returns an error if:
/// - HTTP request fails
/// - Tarball extraction fails
/// - Directory creation fails
async fn download_and_extract(
    client: &Client,
    config: &DownloadConfig,
    specifier: &CrateSpecifier,
) -> Result<PathBuf> {
    // Construct crates.io download URL
    // Format: https://crates.io/api/v1/crates/{crate}/{version}/download
    let download_url = format!(
        "{}/crates/{}/{}/download",
        config.crates_io_url,
        specifier.name(),
        specifier.version()
    );

    // Download tarball
    let response = client
        .get(&download_url)
        .send()
        .await
        .with_context(|| format!("Failed to download crate from {}", download_url))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "HTTP request failed with status {}: {}",
            response.status(),
            download_url
        );
    }

    let tarball_bytes = response
        .bytes()
        .await
        .context("Failed to read response body")?;

    // Create extraction directory: download_dir/crate-version/
    let extract_dir = config
        .download_dir
        .join(format!("{}-{}", specifier.name(), specifier.version()));

    // Create extraction directory
    fs::create_dir_all(&extract_dir)
        .with_context(|| format!("Failed to create extraction directory: {}", extract_dir.display()))?;

    // Extract tarball
    extract_tarball(&tarball_bytes, &extract_dir)
        .context("Failed to extract tarball")?;

    Ok(extract_dir)
}

/// Extracts a gzipped tarball to the specified directory
///
/// # Arguments
///
/// * `tarball_bytes` - Raw bytes of the .crate tarball (gzipped tar)
/// * `extract_to` - Directory where files should be extracted
///
/// # Returns
///
/// Returns `Ok(())` on successful extraction
///
/// # Errors
///
/// Returns an error if:
/// - Gzip decompression fails
/// - Tar extraction fails
/// - File I/O operations fail
fn extract_tarball(tarball_bytes: &[u8], extract_to: &Path) -> Result<()> {
    // Decompress gzip
    let tar_decoder = GzDecoder::new(tarball_bytes);

    // Extract tar archive
    let mut archive = Archive::new(tar_decoder);

    archive
        .unpack(extract_to)
        .with_context(|| format!("Failed to unpack tarball to {}", extract_to.display()))?;

    Ok(())
}

/// Classifies an error into a specific `DownloadErrorKind` for programmatic handling
///
/// # Arguments
///
/// * `error` - The error to classify
///
/// # Returns
///
/// Returns the appropriate `DownloadErrorKind` based on error characteristics
fn classify_error(error: &anyhow::Error) -> DownloadErrorKind {
    let error_str = format!("{:#}", error);

    if error_str.contains("404") || error_str.contains("not found") {
        DownloadErrorKind::NotFound
    } else if error_str.contains("timeout") || error_str.contains("timed out") {
        DownloadErrorKind::Timeout
    } else if error_str.contains("extract") || error_str.contains("unpack") || error_str.contains("gzip") {
        DownloadErrorKind::ExtractionError
    } else if error_str.contains("network") || error_str.contains("connection") || error_str.contains("DNS") {
        DownloadErrorKind::NetworkError
    } else {
        DownloadErrorKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_downloader_actor_spawn_creates_actor() {
        let mut runtime = ActonApp::launch();
        let config = DownloadConfig::default();
        let result = DownloaderActor::spawn(&mut runtime, config).await;
        assert!(result.is_ok(), "DownloaderActor spawn should succeed");

        let handle = result.unwrap();
        let stop_result = handle.stop().await;
        assert!(
            stop_result.is_ok(),
            "DownloaderActor should stop gracefully"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_downloader_actor_creates_download_directory() {
        use std::env;

        let temp_dir = env::temp_dir().join(format!("crately_test_download_{}", rand::random::<u32>()));
        let config = DownloadConfig {
            download_dir: temp_dir.clone(),
            ..Default::default()
        };

        let mut runtime = ActonApp::launch();
        let result = DownloaderActor::spawn(&mut runtime, config).await;
        assert!(result.is_ok(), "DownloaderActor should spawn successfully");

        // Verify directory was created
        assert!(temp_dir.exists(), "Download directory should exist");

        // Cleanup
        let handle = result.unwrap();
        handle.stop().await.expect("Failed to stop actor");
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_downloader_actor_new() {
        let config = DownloadConfig::default();
        let actor = DownloaderActor::new(config.clone());
        assert_eq!(actor.config.timeout_secs, config.timeout_secs);
    }

    #[test]
    fn test_ensure_download_directory_exists_creates_directory() {
        use std::env;

        let temp_dir = env::temp_dir().join(format!("crately_test_ensure_{}", rand::random::<u32>()));
        let _ = fs::remove_dir_all(&temp_dir);

        let result = ensure_download_directory_exists(&temp_dir);
        assert!(result.is_ok(), "Should create directory successfully");
        assert!(temp_dir.exists(), "Directory should exist");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ensure_download_directory_exists_handles_existing_directory() {
        use std::env;

        let temp_dir = env::temp_dir().join(format!("crately_test_existing_{}", rand::random::<u32>()));
        let _ = fs::create_dir_all(&temp_dir);

        let result = ensure_download_directory_exists(&temp_dir);
        assert!(result.is_ok(), "Should handle existing directory");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_classify_error_not_found() {
        let error = anyhow::anyhow!("HTTP 404 not found");
        let kind = classify_error(&error);
        assert_eq!(kind, DownloadErrorKind::NotFound);
    }

    #[test]
    fn test_classify_error_timeout() {
        let error = anyhow::anyhow!("Request timed out");
        let kind = classify_error(&error);
        assert_eq!(kind, DownloadErrorKind::Timeout);
    }

    #[test]
    fn test_classify_error_extraction() {
        let error = anyhow::anyhow!("Failed to extract tarball");
        let kind = classify_error(&error);
        assert_eq!(kind, DownloadErrorKind::ExtractionError);
    }

    #[test]
    fn test_classify_error_network() {
        let error = anyhow::anyhow!("Network connection failed");
        let kind = classify_error(&error);
        assert_eq!(kind, DownloadErrorKind::NetworkError);
    }

    #[test]
    fn test_classify_error_other() {
        let error = anyhow::anyhow!("Unknown error");
        let kind = classify_error(&error);
        assert_eq!(kind, DownloadErrorKind::Other);
    }

    #[test]
    fn test_extract_tarball_with_invalid_data() {
        use std::env;

        let temp_dir = env::temp_dir().join(format!("crately_test_invalid_{}", rand::random::<u32>()));
        let _ = fs::create_dir_all(&temp_dir);

        let invalid_bytes = b"not a valid tarball";
        let result = extract_tarball(invalid_bytes, &temp_dir);
        assert!(result.is_err(), "Should fail with invalid tarball data");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_download_url_format() {
        let config = DownloadConfig::default();
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();

        let expected_url = format!(
            "{}/crates/{}/{}/download",
            config.crates_io_url,
            specifier.name(),
            specifier.version()
        );

        assert_eq!(
            expected_url,
            "https://crates.io/api/v1/crates/serde/1.0.0/download"
        );
    }

    #[test]
    fn test_extract_directory_format() {
        let config = DownloadConfig::default();
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();

        let extract_dir = config
            .download_dir
            .join(format!("{}-{}", specifier.name(), specifier.version()));

        let path_str = extract_dir.to_string_lossy();
        assert!(path_str.contains("tokio-1.35.0"));
    }
}
