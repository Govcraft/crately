use acton_reactive::prelude::*;

/// Actor responsible for downloading crate artifacts from crates.io
///
/// The `CrateDownloader` actor manages the process of downloading crate files
/// (.crate archives) from the crates.io registry. It handles HTTP requests to
/// download specific versions of crates based on their specifier (name and version).
///
/// # State Management
///
/// This actor maintains the crate specifier that identifies which crate version
/// to download. The actor can be spawned with a specific crate specifier and
/// will handle download requests asynchronously.
///
/// # Actor Lifecycle
///
/// The actor is created using the acton-reactive framework and can be managed
/// by the `AgentRuntime`. Handler methods for download operations should be
/// implemented separately to process download requests and manage the download
/// lifecycle.
///
/// # Examples
///
/// ```no_run
/// use acton_reactive::prelude::*;
/// use crately::crate_downloader::CrateDownloader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut runtime = ActonApp::launch();
/// let downloader = CrateDownloader::spawn(&mut runtime).await?;
///
/// // Actor can send messages to trigger download operations
/// // Handler implementations would process download requests
/// # Ok(())
/// # }
/// ```
#[acton_actor]
pub struct CrateDownloader {}

/// Message to request downloading a crate from crates.io
///
/// This message triggers the download process for a specific crate version.
/// The actor will handle the HTTP request to fetch the .crate archive from
/// the crates.io registry and process it for documentation compilation.
#[acton_actor]
pub struct DownloadCrate {
    /// Crate name to download
    pub name: String,
    /// Crate version to download
    pub version: String,
}

impl CrateDownloader {
    /// Spawns, configures, and starts a new CrateDownloader actor
    ///
    /// This is the standard factory method for creating CrateDownloader actors.
    /// The CrateDownloader actor handles downloading crate artifacts from crates.io,
    /// managing the HTTP download process, and coordinating with downstream processing
    /// actors for documentation compilation and vectorization.
    ///
    /// This follows the simple actor pattern where only the handle is returned,
    /// as the CrateDownloader actor has no startup data to provide to the application.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    ///
    /// # Returns
    ///
    /// Returns `AgentHandle` to the started CrateDownloader actor for message passing.
    ///
    /// # When to Use
    ///
    /// Call this during application startup to initialize the download subsystem.
    /// The actor will be ready to process `DownloadCrate` messages and coordinate
    /// with other actors in the crate processing pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if actor creation or initialization fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use acton_reactive::prelude::*;
    /// use crately::crate_downloader::{CrateDownloader, DownloadCrate};
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut runtime = ActonApp::launch();
    ///
    ///     // Spawn the CrateDownloader actor for handling downloads
    ///     let downloader = CrateDownloader::spawn(&mut runtime).await?;
    ///
    ///     // Request a crate download
    ///     downloader.send(DownloadCrate {
    ///         name: "serde".to_string(),
    ///         version: "1.0.0".to_string(),
    ///     }).await;
    ///
    ///     runtime.shutdown_all().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn spawn(runtime: &mut AgentRuntime) -> anyhow::Result<AgentHandle> {
        use tracing::*;

        let mut builder = runtime
            .new_agent_with_name::<CrateDownloader>("crate_downloader".to_string())
            .await;

        // Configure message handler for download requests
        builder.mutate_on::<DownloadCrate>(|_agent, envelope| {
            let msg = envelope.message();
            info!(
                "Received download request for crate: {} version: {}",
                msg.name, msg.version
            );
            // TODO: Implement actual download logic
            // This will involve:
            // 1. HTTP request to crates.io to fetch .crate archive
            // 2. Validation of downloaded artifact
            // 3. Storage to local filesystem
            // 4. Coordination with documentation compilation actors
            AgentReply::immediate()
        });

        Ok(builder.start().await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_crate_downloader_spawn_creates_actor() {
        let mut runtime = ActonApp::launch();
        let result = CrateDownloader::spawn(&mut runtime).await;
        assert!(result.is_ok(), "CrateDownloader spawn should succeed");

        let handle = result.unwrap();
        // Verify the actor can be stopped gracefully
        let stop_result = handle.stop().await;
        assert!(
            stop_result.is_ok(),
            "CrateDownloader should stop gracefully"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_crate_downloader_handles_download_message() {
        let mut runtime = ActonApp::launch();
        let handle = CrateDownloader::spawn(&mut runtime)
            .await
            .expect("Failed to spawn CrateDownloader");

        // Send a download request
        let download_msg = DownloadCrate {
            name: "serde".to_string(),
            version: "1.0.0".to_string(),
        };

        // Verify message can be sent without errors
        handle.send(download_msg).await;

        // Allow time for message processing
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Clean shutdown
        handle.stop().await.expect("Failed to stop actor");
    }

    #[tokio::test]
    async fn test_download_crate_message_creation() {
        let msg = DownloadCrate {
            name: "tokio".to_string(),
            version: "1.35.0".to_string(),
        };

        assert_eq!(msg.name, "tokio");
        assert_eq!(msg.version, "1.35.0");
    }

    #[tokio::test]
    async fn test_download_crate_message_clone() {
        let msg1 = DownloadCrate {
            name: "axum".to_string(),
            version: "0.7.0".to_string(),
        };

        let msg2 = msg1.clone();
        assert_eq!(msg1.name, msg2.name);
        assert_eq!(msg1.version, msg2.version);
    }
}
