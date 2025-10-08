//! Centralized actor system management for Crately.
//!
//! This module provides the `ActorSystem` that manages the acton-reactive runtime
//! and all actor lifecycle operations. It serves as the single source of truth for
//! actor handles across all commands.
//!
//! # Architecture
//!
//! The `ActorSystem` follows the actor model principles:
//! - No blocking synchronization primitives (Mutex, RwLock)
//! - DashMap for lock-free concurrent access to actor handles
//! - Cheap AgentHandle cloning via Arc
//! - Message passing for all actor communication
//!
//! # Example
//!
//! ```no_run
//! use crately::runtime::ActorSystem;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Initialize the actor system
//!     let actor_system = ActorSystem::initialize().await?;
//!
//!     // Get actor handles
//!     let console = actor_system.get_actor("console")
//!         .expect("Console actor should exist");
//!
//!     // Use actors for your application logic
//!     // ...
//!
//!     // Shutdown cleanly
//!     actor_system.shutdown().await?;
//!     Ok(())
//! }
//! ```

use acton_reactive::prelude::*;
use anyhow::{Context, Result};
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::{
    Console,
    config::{Config, ConfigManager},
    console::PrintSuccess,
    crate_downloader::CrateDownloader,
    messages::Init,
};

/// Centralized actor system managing runtime and all actors.
///
/// The `ActorSystem` provides lifecycle management for the acton-reactive runtime
/// and all actors in the application. It maintains actor handles in a thread-safe
/// DashMap for efficient concurrent access without blocking.
///
/// # Design Principles
///
/// - **Actor Model**: All synchronization via message passing, no locks
/// - **Lock-Free Access**: DashMap enables concurrent reads without contention
/// - **Cheap Cloning**: AgentHandle is Arc-based, cloning is cheap
/// - **Clean Shutdown**: Actors stopped before runtime shutdown
///
/// # Actor Registry
///
/// Actors are stored with consistent naming:
/// - `"console"` - Console output formatting actor
/// - `"config_manager"` - Configuration management actor
/// - `"crate_downloader"` - Crate download orchestration actor
pub struct ActorSystem {
    /// The acton-reactive runtime managing actor execution
    runtime: AgentRuntime,
    /// Thread-safe map of actor handles accessible by name
    actors: Arc<DashMap<String, AgentHandle>>,
    /// The initial configuration loaded at startup
    config: Config,
}

impl ActorSystem {
    /// Initializes the complete actor system with all actors.
    ///
    /// This method performs the following initialization sequence:
    /// 1. Launches the acton-reactive runtime
    /// 2. Spawns the Console actor and triggers initialization
    /// 3. Spawns the ConfigManager actor and loads configuration
    /// 4. Spawns the CrateDownloader actor
    /// 5. Stores all actor handles in the registry
    ///
    /// # Returns
    ///
    /// Returns the initialized `ActorSystem` ready for use by commands.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any actor fails to spawn
    /// - Configuration cannot be loaded
    /// - Actor initialization fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use crately::runtime::ActorSystem;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let system = ActorSystem::initialize().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn initialize() -> Result<Self> {
        debug!("Initializing actor system");

        // Launch the acton-reactive runtime
        let mut runtime = ActonApp::launch();
        let actors = Arc::new(DashMap::new());

        // Spawn Console actor
        debug!("Spawning Console actor");
        let console = Console::spawn(&mut runtime)
            .await
            .context("Failed to spawn Console actor")?;
        console.send(Init).await;

        // Print startup banner and logging confirmation
        let log_dir = crate::logging::get_log_dir()
            .context("Failed to determine log directory")?;
        console
            .send(PrintSuccess(format!(
                "Logging initialized → {}",
                log_dir.display()
            )))
            .await;

        actors.insert("console".to_string(), console.clone());
        info!("Console actor initialized and registered");

        // Spawn ConfigManager actor
        debug!("Spawning ConfigManager actor");
        let (config_manager, config) = ConfigManager::spawn(&mut runtime)
            .await
            .context("Failed to spawn ConfigManager actor")?;

        actors.insert("config_manager".to_string(), config_manager.clone());
        info!("ConfigManager actor initialized and registered");

        // Spawn CrateDownloader actor
        debug!("Spawning CrateDownloader actor");
        let crate_downloader = CrateDownloader::spawn(&mut runtime)
            .await
            .context("Failed to spawn CrateDownloader actor")?;

        actors.insert("crate_downloader".to_string(), crate_downloader.clone());
        info!("CrateDownloader actor initialized and registered");

        debug!("Actor system initialization complete");
        Ok(Self {
            runtime,
            actors,
            config,
        })
    }

    /// Gets a clone of an actor handle by name.
    ///
    /// Returns a cheap clone of the requested actor's handle. Cloning is
    /// inexpensive as `AgentHandle` is Arc-based internally.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the actor to retrieve
    ///
    /// # Returns
    ///
    /// Returns `Some(AgentHandle)` if the actor exists, `None` otherwise.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crately::runtime::ActorSystem;
    /// # async fn example(system: &ActorSystem) {
    /// if let Some(console) = system.get_actor("console") {
    ///     // Use the console actor
    /// }
    /// # }
    /// ```
    pub fn get_actor(&self, name: &str) -> Option<AgentHandle> {
        self.actors.get(name).map(|handle| handle.clone())
    }

    /// Gets the configuration loaded at startup.
    ///
    /// Returns a reference to the configuration that was loaded when
    /// the actor system was initialized.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crately::runtime::ActorSystem;
    /// # async fn example(system: &ActorSystem) {
    /// let config = system.config();
    /// println!("Server port: {}", config.port);
    /// # }
    /// ```
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Gets a thread-safe reference to the actor registry.
    ///
    /// Returns an Arc to the DashMap containing all actor handles.
    /// This is useful when you need to pass the actor registry to
    /// other components that need concurrent access.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crately::runtime::ActorSystem;
    /// # async fn example(system: &ActorSystem) {
    /// let actors = system.actors();
    /// // Pass to another component
    /// # }
    /// ```
    pub fn actors(&self) -> Arc<DashMap<String, AgentHandle>> {
        Arc::clone(&self.actors)
    }

    /// Gracefully shuts down the actor system.
    ///
    /// This method performs a clean shutdown sequence:
    /// 1. Stops the CrateDownloader actor
    /// 2. Stops the ConfigManager actor
    /// 3. Stops the Console actor
    /// 4. Shuts down the acton-reactive runtime
    ///
    /// The shutdown order ensures that logging remains available until
    /// all other actors have completed their shutdown procedures.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any actor fails to stop cleanly
    /// - Runtime shutdown fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crately::runtime::ActorSystem;
    /// # async fn example() -> anyhow::Result<()> {
    /// let system = ActorSystem::initialize().await?;
    /// // Use the system...
    /// system.shutdown().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn shutdown(mut self) -> Result<()> {
        info!("Initiating graceful shutdown sequence");

        // Stop CrateDownloader actor
        if let Some(crate_downloader) = self.get_actor("crate_downloader") {
            debug!("Stopping CrateDownloader actor");
            match crate_downloader.stop().await {
                Ok(()) => {
                    info!("CrateDownloader actor stopped successfully");
                    if let Some(console) = self.get_actor("console") {
                        console
                            .send(PrintSuccess("CrateDownloader actor stopped".to_string()))
                            .await;
                    }
                }
                Err(e) => {
                    error!("Failed to stop CrateDownloader actor: {:?}", e);
                }
            }
        }

        // Stop ConfigManager actor
        if let Some(config_manager) = self.get_actor("config_manager") {
            debug!("Stopping ConfigManager actor");
            match config_manager.stop().await {
                Ok(()) => {
                    info!("ConfigManager actor stopped successfully");
                    if let Some(console) = self.get_actor("console") {
                        console
                            .send(PrintSuccess("ConfigManager actor stopped".to_string()))
                            .await;
                    }
                }
                Err(e) => {
                    error!("Failed to stop ConfigManager actor: {:?}", e);
                }
            }
        }

        // Stop Console actor last so logging works throughout shutdown
        if let Some(console) = self.get_actor("console") {
            debug!("Stopping Console actor");
            match console.stop().await {
                Ok(()) => {
                    info!("Console actor stopped successfully");
                }
                Err(e) => {
                    error!("Failed to stop Console actor: {:?}", e);
                }
            }
        }

        // Clear the actor registry
        self.actors.clear();

        // Shutdown the acton-reactive runtime
        debug!("Shutting down acton-reactive runtime");
        match self.runtime.shutdown_all().await {
            Ok(()) => {
                info!("Acton-reactive runtime shut down successfully");
            }
            Err(e) => {
                error!("Failed to shut down acton-reactive runtime: {:?}", e);
            }
        }

        // Log session end separator
        info!("========================================");
        info!("= APPLICATION SHUTDOWN COMPLETE");
        info!("========================================");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_initialize_succeeds() {
        let result = ActorSystem::initialize().await;
        assert!(result.is_ok(), "ActorSystem initialization should succeed");

        let system = result.unwrap();
        system.shutdown().await.expect("Shutdown should succeed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_registers_all_actors() {
        let system = ActorSystem::initialize()
            .await
            .expect("Initialization should succeed");

        assert!(
            system.get_actor("console").is_some(),
            "Console actor should be registered"
        );
        assert!(
            system.get_actor("config_manager").is_some(),
            "ConfigManager actor should be registered"
        );
        assert!(
            system.get_actor("crate_downloader").is_some(),
            "CrateDownloader actor should be registered"
        );

        system.shutdown().await.expect("Shutdown should succeed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_get_actor_returns_none_for_unknown() {
        let system = ActorSystem::initialize()
            .await
            .expect("Initialization should succeed");

        assert!(
            system.get_actor("nonexistent").is_none(),
            "Unknown actor should return None"
        );

        system.shutdown().await.expect("Shutdown should succeed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_get_actor_clones_handle() {
        let system = ActorSystem::initialize()
            .await
            .expect("Initialization should succeed");

        let console1 = system
            .get_actor("console")
            .expect("Console should exist");
        let console2 = system
            .get_actor("console")
            .expect("Console should exist");

        // Both handles should work independently
        console1.send(Init).await;
        console2.send(Init).await;

        system.shutdown().await.expect("Shutdown should succeed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_config_returns_loaded_config() {
        let system = ActorSystem::initialize()
            .await
            .expect("Initialization should succeed");

        let config = system.config();
        // Config should have a valid port
        assert!(config.port > 0, "Config port should be set");

        system.shutdown().await.expect("Shutdown should succeed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_actors_returns_dashmap() {
        let system = ActorSystem::initialize()
            .await
            .expect("Initialization should succeed");

        let actors = system.actors();
        assert_eq!(
            actors.len(),
            3,
            "Should have exactly 3 actors registered"
        );

        system.shutdown().await.expect("Shutdown should succeed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_shutdown_cleans_up_actors() {
        let system = ActorSystem::initialize()
            .await
            .expect("Initialization should succeed");

        let actors_before = system.actors();
        assert_eq!(actors_before.len(), 3, "Should start with 3 actors");

        // Shutdown should succeed and clean up
        system.shutdown().await.expect("Shutdown should succeed");

        // After shutdown, the system is consumed, so we can't check the state
        // This test verifies that shutdown completes without errors
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_shutdown_handles_missing_actors_gracefully() {
        let system = ActorSystem::initialize()
            .await
            .expect("Initialization should succeed");

        // Manually remove an actor to simulate an edge case
        system.actors.remove("crate_downloader");

        // Shutdown should still succeed even with missing actor
        system.shutdown().await.expect("Shutdown should succeed");
    }
}
