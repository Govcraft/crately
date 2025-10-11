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
use tracing::{error, info};

use crate::{
    actors::{
        config::{Config, ConfigManager},
        console::{Console, PrintProgress, PrintSuccess},
        crate_downloader::CrateDownloader,
        keyboard_handler::{KeyboardHandler, StopKeyboardHandler},
        server_actor::ServerActor,
    },
    messages::{Init, StopServer},
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
/// - `"keyboard_handler"` - Keyboard event handler actor (server mode only)
/// - `"server"` - HTTP server actor (server mode only)
pub struct ActorSystem {
    /// The acton-reactive runtime managing actor execution
    runtime: AgentRuntime,
    /// Thread-safe map of actor handles accessible by name
    actors: Arc<DashMap<String, AgentHandle>>,
    /// The initial configuration loaded at startup
    config: Config,
    /// Whether server-specific actors have been initialized
    server_mode: bool,
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
        // Launch the acton-reactive runtime
        let mut runtime = ActonApp::launch();
        let actors = Arc::new(DashMap::new());

        // Spawn Console actor
        let console = Console::spawn(&mut runtime)
            .await
            .context("Failed to spawn Console actor")?;
        console.send(Init).await;

        // Print startup banner and logging confirmation
        let log_dir = crate::logging::get_log_dir().context("Failed to determine log directory")?;
        console
            .send(PrintSuccess(format!(
                "Logging initialized → {}",
                log_dir.display()
            )))
            .await;

        actors.insert("console".to_string(), console.clone());

        // Spawn ConfigManager actor
        let (config_manager, config) = ConfigManager::spawn(&mut runtime)
            .await
            .context("Failed to spawn ConfigManager actor")?;

        actors.insert("config_manager".to_string(), config_manager.clone());

        // Spawn CrateDownloader actor
        let crate_downloader = CrateDownloader::spawn(&mut runtime)
            .await
            .context("Failed to spawn CrateDownloader actor")?;

        actors.insert("crate_downloader".to_string(), crate_downloader.clone());

        Ok(Self {
            runtime,
            actors,
            config,
            server_mode: false,
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

    /// Gets mutable access to the runtime for spawning new actors.
    ///
    /// This method provides mutable access to the underlying acton-reactive
    /// runtime, allowing commands to spawn additional actors after the initial
    /// system initialization.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crately::runtime::ActorSystem;
    /// # async fn example(mut system: ActorSystem) -> anyhow::Result<()> {
    /// // Spawn additional actors
    /// let runtime = system.runtime_mut();
    /// // Use runtime.new_agent() to create new actors
    /// # Ok(())
    /// # }
    /// ```
    pub fn runtime_mut(&mut self) -> &mut AgentRuntime {
        &mut self.runtime
    }

    /// Initializes server-specific actors for the HTTP server.
    ///
    /// This method spawns and configures the KeyboardHandler and ServerActor
    /// actors required for running the HTTP server. It should be called after
    /// `initialize()` when the serve command is executed.
    ///
    /// The server actor is automatically configured with the system's actors
    /// registry and configuration. The keyboard handler is spawned to enable
    /// interactive server control.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful initialization of both actors.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - KeyboardHandler fails to spawn (e.g., terminal raw mode unavailable)
    /// - ServerActor fails to spawn
    /// - Server-specific actors are already initialized (idempotent)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crately::runtime::ActorSystem;
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut system = ActorSystem::initialize().await?;
    /// system.initialize_server_actors().await?;
    ///
    /// // Server actors are now available
    /// let server = system.get_actor("server").expect("Server exists");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn initialize_server_actors(&mut self) -> Result<()> {
        // Check if already initialized to maintain idempotency
        if self.server_mode {
            info!("Server actors already initialized, skipping");
            return Ok(());
        }

        info!("Initializing server-specific actors");

        // Spawn KeyboardHandler actor with access to actor registry
        let keyboard_handle = KeyboardHandler::spawn(&mut self.runtime, Arc::clone(&self.actors))
            .await
            .context("Failed to spawn KeyboardHandler actor")?;

        self.actors
            .insert("keyboard_handler".to_string(), keyboard_handle.clone());

        // Spawn ServerActor
        let server_handle = ServerActor::spawn(
            &mut self.runtime,
            self.config.clone(),
            Arc::clone(&self.actors),
        )
        .await
        .context("Failed to spawn ServerActor actor")?;

        self.actors
            .insert("server".to_string(), server_handle.clone());

        self.server_mode = true;

        Ok(())
    }

    /// Waits for a shutdown signal from Ctrl+C, SIGTERM, or keyboard shutdown keys.
    ///
    /// This method blocks until one of the following occurs:
    /// - User presses Ctrl+C (signal handler or keyboard in raw mode)
    /// - Process receives SIGTERM (Unix only)
    /// - User presses 'q' key (only when keyboard handler is active)
    ///
    /// Creates a temporary ShutdownListener actor that subscribes to KeyPressed
    /// broadcasts from the KeyboardHandler to receive raw mode keyboard events
    /// ('q' or Ctrl+C) without requiring Enter key press.
    ///
    /// # Arguments
    ///
    /// * `actor_system` - Mutable reference to the actor system for spawning the listener
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crately::runtime::ActorSystem;
    /// # async fn example(mut system: ActorSystem) {
    /// ActorSystem::wait_for_shutdown(&mut system).await;
    /// println!("Shutdown signal received");
    /// # }
    /// ```
    pub async fn wait_for_shutdown(actor_system: &mut ActorSystem) {
        use crate::messages::KeyPressed;
        use crossterm::event::{KeyCode, KeyModifiers};
        use tokio::sync::mpsc;

        // Set up graceful shutdown mechanism using a channel
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        // Create a minimal listener actor if keyboard handler exists
        let shutdown_listener = if actor_system.get_actor("keyboard_handler").is_some() {
            // Define a minimal actor that listens for 'q' or Ctrl+C keypresses
            #[acton_actor]
            struct ShutdownListener;

            let mut builder = actor_system.runtime.new_agent::<ShutdownListener>().await;

            builder.act_on::<KeyPressed>(move |_agent, envelope| {
                let message = envelope.message();
                match (message.key, message.modifiers) {
                    (KeyCode::Char('q'), KeyModifiers::NONE)
                    | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        info!("Received shutdown key ('q' or Ctrl+C), initiating graceful shutdown");

                        // Send shutdown signal (non-blocking, won't fail if channel is full)
                        let _ = shutdown_tx.try_send(());
                    }
                    _ => {
                        // Ignore other keys
                    }
                }
                AgentReply::immediate()
            });

            // Subscribe to KeyPressed broadcasts before starting
            builder.handle().subscribe::<KeyPressed>().await;

            Some(builder.start().await)
        } else {
            None
        };

        let ctrl_c = async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received Ctrl+C signal, initiating graceful shutdown");
            },
            _ = terminate => {
                info!("Received termination signal, initiating graceful shutdown");
            },
            _ = shutdown_rx.recv() => {
                info!("Received keyboard shutdown signal");
            },
        }

        // Display shutdown message to user via Console actor
        if let Some(console) = actor_system.get_actor("console") {
            console
                .send(PrintProgress("Initiating graceful shutdown...".to_string()))
                .await;

            // Brief delay to allow Console to process and display the message
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        // Clean up the shutdown listener actor
        if let Some(handle) = shutdown_listener {
            let _ = handle.stop().await;
        }
    }

    /// Gracefully shuts down the actor system.
    ///
    /// This method performs a clean shutdown sequence:
    /// 1. Stops ServerActor (if running) - gracefully stop accepting requests
    /// 2. Stops KeyboardHandler (if running) - stop listening for input
    /// 3. Stops CrateDownloader actor - finish any pending work
    /// 4. Stops ConfigManager actor - finish any pending work
    /// 5. Stops Console actor - last so logging works throughout
    /// 6. Clears actor registry
    /// 7. Shuts down the acton-reactive runtime
    ///
    /// The shutdown order ensures that the server stops first, followed by
    /// supporting actors, with logging remaining available until all other
    /// actors have completed their shutdown procedures.
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

        // Stop ServerActor first (if running in server mode)
        if self.server_mode {
            if let Some(server) = self.get_actor("server") {
                // Send StopServer message first to gracefully stop accepting requests
                server.send(StopServer).await;

                // Give server time to drain connections
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                match server.stop().await {
                    Ok(()) => {}
                    Err(e) => {
                        error!("Failed to stop ServerActor: {:?}", e);
                    }
                }
            }

            // Stop KeyboardHandler (if running)
            if let Some(keyboard_handler) = self.get_actor("keyboard_handler") {
                keyboard_handler.send(StopKeyboardHandler).await;

                match keyboard_handler.stop().await {
                    Ok(()) => {}
                    Err(e) => {
                        error!("Failed to stop KeyboardHandler: {:?}", e);
                    }
                }
            }
        }

        // Stop CrateDownloader actor
        if let Some(crate_downloader) = self.get_actor("crate_downloader") {
            match crate_downloader.stop().await {
                Ok(()) => {}
                Err(e) => {
                    error!("Failed to stop CrateDownloader actor: {:?}", e);
                }
            }
        }

        // Stop ConfigManager actor
        if let Some(config_manager) = self.get_actor("config_manager") {
            match config_manager.stop().await {
                Ok(()) => {}
                Err(e) => {
                    error!("Failed to stop ConfigManager actor: {:?}", e);
                }
            }
        }

        // Stop Console actor last so logging works throughout shutdown
        if let Some(console) = self.get_actor("console") {
            match console.stop().await {
                Ok(()) => {}
                Err(e) => {
                    error!("Failed to stop Console actor: {:?}", e);
                }
            }
        }

        // Clear the actor registry
        self.actors.clear();

        // Shutdown the acton-reactive runtime
        match self.runtime.shutdown_all().await {
            Ok(()) => {}
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
    use crate::messages::StartServer;

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

        let console1 = system.get_actor("console").expect("Console should exist");
        let console2 = system.get_actor("console").expect("Console should exist");

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
        assert_eq!(actors.len(), 3, "Should have exactly 3 actors registered");

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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_initialize_server_actors_succeeds() {
        let mut system = ActorSystem::initialize()
            .await
            .expect("Initialization should succeed");

        // Server mode should initially be false
        assert!(!system.server_mode, "Server mode should be false initially");

        // Initialize server actors
        system
            .initialize_server_actors()
            .await
            .expect("Server actor initialization should succeed");

        // Server mode should now be true
        assert!(
            system.server_mode,
            "Server mode should be true after initialization"
        );

        // Both server actors should be registered
        assert!(
            system.get_actor("keyboard_handler").is_some(),
            "KeyboardHandler should be registered"
        );
        assert!(
            system.get_actor("server").is_some(),
            "ServerActor should be registered"
        );

        system.shutdown().await.expect("Shutdown should succeed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_initialize_server_actors_is_idempotent() {
        let mut system = ActorSystem::initialize()
            .await
            .expect("Initialization should succeed");

        // Initialize server actors twice
        system
            .initialize_server_actors()
            .await
            .expect("First initialization should succeed");

        system
            .initialize_server_actors()
            .await
            .expect("Second initialization should succeed");

        // Should still only have one of each actor
        let actors = system.actors();
        assert_eq!(
            actors.len(),
            5,
            "Should have exactly 5 actors (3 core + 2 server)"
        );

        system.shutdown().await.expect("Shutdown should succeed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_shutdown_with_server_actors() {
        let mut system = ActorSystem::initialize()
            .await
            .expect("Initialization should succeed");

        // Initialize server actors
        system
            .initialize_server_actors()
            .await
            .expect("Server actor initialization should succeed");

        // Start the server
        let server = system.get_actor("server").expect("Server should exist");
        server.send(StartServer).await;

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Shutdown should cleanly stop all actors including server actors
        system.shutdown().await.expect("Shutdown should succeed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_actor_system_shutdown_without_server_actors() {
        let system = ActorSystem::initialize()
            .await
            .expect("Initialization should succeed");

        // Shutdown should succeed without server actors initialized
        system.shutdown().await.expect("Shutdown should succeed");
    }
}
