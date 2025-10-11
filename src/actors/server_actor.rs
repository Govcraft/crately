//! HTTP server actor for Crately.
//!
//! This module provides the `ServerActor` that manages the Axum HTTP server lifecycle
//! through message passing. The actor handles server start/stop operations and can be
//! controlled via keyboard events and configuration changes.

use acton_reactive::prelude::*;
use anyhow::Result;
use axum::{
    Router,
    extract::{Json, State},
    routing::post,
};
use crossterm::event::{KeyCode, KeyModifiers};
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::{
    actors::config::{Config, ConfigResponse, ReloadConfig},
    messages::{
        KeyPressed, PersistCrate, PrintError, PrintProgress, PrintSuccess, ServerReloaded,
        ServerStarted, StartServer, StopServer,
    },
    request::CrateRequest,
    response::CrateResponse,
};

/// Shared application state containing actor handles
#[derive(Clone)]
struct AppState {
    /// Thread-safe map of actor handles accessible by name
    actors: Arc<DashMap<String, AgentHandle>>,
}

/// Handle HTTP POST requests to the /crate endpoint
async fn handle_crate_request(
    State(state): State<AppState>,
    Json(payload): Json<CrateRequest>,
) -> Json<CrateResponse> {
    info!(
        "Received crate request: {} with features: {:?}",
        payload.specifier, payload.features
    );

    // CrateSpecifier is already validated during deserialization
    let specifier = &payload.specifier;

    debug!(
        "Processing crate: {} (version: {})",
        specifier.name(),
        specifier.version()
    );

    // Send persistence request to DatabaseActor
    if let Some(database) = state.actors.get("database") {
        database
            .send(PersistCrate {
                specifier: payload.specifier.clone(),
                features: payload.features.clone(),
            })
            .await;
    } else {
        warn!("DatabaseActor not found in registry, skipping persistence");
    }

    // Return success response
    let response = CrateResponse {
        name: specifier.name().to_string(),
        version: specifier.version().to_string(),
        features: payload.features.clone(),
        message: format!(
            "Successfully received crate {} with {} feature(s)",
            specifier,
            payload.features.len()
        ),
    };

    Json(response)
}

/// Actor that manages the HTTP server lifecycle.
///
/// The `ServerActor` encapsulates the Axum HTTP server and provides lifecycle
/// management through message passing. It can start, stop, and reload the server
/// in response to messages, and subscribes to keyboard events and configuration
/// changes for interactive control.
///
/// # Architecture
///
/// The server actor follows the actor model:
/// - Receives `StartServer` message to start the HTTP server
/// - Receives `StopServer` message to gracefully stop the server
/// - Subscribes to `ConfigResponse` for hot-reload capability
/// - Subscribes to `KeyPressed` for keyboard-driven control
/// - Stores shutdown handle internally for graceful termination
///
/// # Message Flow
///
/// ```text
/// KeyboardHandler → broadcasts KeyPressed('r')
///      |
///      v
/// ServerActor → receives KeyPressed → triggers reload
///
/// ConfigManager → broadcasts ConfigResponse
///      |
///      v
/// ServerActor → receives ConfigResponse → hot reload
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::server_actor::ServerActor;
/// use crately::messages::StartServer;
/// use crately::config::Config;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let mut runtime = ActonApp::launch();
///     let config = Config::default();
///     let actors = std::sync::Arc::new(dashmap::DashMap::new());
///
///     let server = ServerActor::spawn(&mut runtime, config, actors).await?;
///     server.send(StartServer).await;
///
///     runtime.shutdown_all().await?;
///     Ok(())
/// }
/// ```
#[acton_actor]
pub struct ServerActor {
    /// Current server configuration
    config: Config,
    /// Registry of all actor handles
    actors: Arc<DashMap<String, AgentHandle>>,
    /// Whether the server is currently running
    is_running: bool,
    /// Broadcast sender for shutdown signals (Clone-able, optional)
    ///
    /// This channel sender is cloneable and can be stored in the actor state.
    /// When we want to shut down the server, we send a signal through this channel.
    /// The receiver lives in the server task and triggers graceful shutdown.
    /// Created fresh each time the server starts.
    shutdown_broadcast: Option<broadcast::Sender<()>>,
}

impl ServerActor {
    /// Spawns, configures, and starts a new ServerActor
    ///
    /// This is the standard factory method for creating ServerActor instances.
    /// The actor is initialized with configuration and actor registry, then
    /// configured with message handlers for server control.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    /// * `config` - Initial server configuration
    /// * `actors` - Registry of actor handles for HTTP handlers
    ///
    /// # Returns
    ///
    /// Returns `AgentHandle` to the started ServerActor.
    ///
    /// # When to Use
    ///
    /// Call this during application startup after the actor system is initialized.
    /// The server will not begin serving until it receives a `StartServer` message.
    ///
    /// # Errors
    ///
    /// Returns an error if actor creation or initialization fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use acton_reactive::prelude::*;
    /// use crately::server_actor::ServerActor;
    /// use crately::config::Config;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut runtime = ActonApp::launch();
    ///     let config = Config::default();
    ///     let actors = std::sync::Arc::new(dashmap::DashMap::new());
    ///
    ///     let server = ServerActor::spawn(&mut runtime, config, actors).await?;
    ///     runtime.shutdown_all().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn spawn(
        runtime: &mut AgentRuntime,
        config: Config,
        actors: Arc<DashMap<String, AgentHandle>>,
    ) -> Result<AgentHandle> {
        let mut builder = runtime
            .new_agent_with_name::<ServerActor>("server".to_string())
            .await;

        // Initialize the actor state
        builder.model = ServerActor {
            config,
            actors,
            is_running: false,
            shutdown_broadcast: None,
        };

        // Handle StartServer messages
        builder.mutate_on::<StartServer>(|agent, _envelope| {
            if agent.model.is_running {
                warn!("Server is already running, ignoring StartServer message");
                return AgentReply::immediate();
            }

            info!("Starting HTTP server");

            let config = agent.model.config.clone();
            let actors = Arc::clone(&agent.model.actors);
            let console = actors.get("console").map(|h| h.clone());
            let broker = agent.broker().clone();

            // Create shared application state
            let app_state = AppState { actors };

            // Build the router
            let app = Router::new()
                .route("/crate", post(handle_crate_request))
                .with_state(app_state);

            let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

            // Create a new broadcast channel for this server instance
            let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

            // Store the sender for shutdown signaling
            agent.model.shutdown_broadcast = Some(shutdown_tx);

            // Spawn the server in a background task
            tokio::spawn(async move {
                info!("Binding server to {}", addr);

                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        error!("Failed to bind server to {}: {}", addr, e);
                        if let Some(console) = console.as_ref() {
                            console
                                .send(PrintError(format!("Failed to start server: {}", e)))
                                .await;
                        }
                        return;
                    }
                };

                if let Some(console) = console.as_ref() {
                    console
                        .send(PrintSuccess(format!("Server listening {} http://{}",
                            crate::actors::console::LOCATION, addr)))
                        .await;
                }

                // Broadcast ServerStarted event after successful bind
                broker.broadcast(ServerStarted).await;

                // Set up graceful shutdown: wait for shutdown signal from broadcast channel
                let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                    let _ = shutdown_rx.recv().await;
                    info!("Received shutdown signal, initiating graceful shutdown");
                });

                if let Err(e) = server.await {
                    error!("Server error: {}", e);
                    if let Some(console) = console.as_ref() {
                        console
                            .send(PrintError(format!("Server error: {}", e)))
                            .await;
                    }
                }
            });

            agent.model.is_running = true;
            AgentReply::immediate()
        });

        // Handle StopServer messages
        builder.mutate_on::<StopServer>(|agent, _envelope| {
            if !agent.model.is_running {
                warn!("Server is not running, ignoring StopServer message");
                return AgentReply::immediate();
            }

            info!("Stopping HTTP server");

            // Send shutdown signal through broadcast channel if it exists
            // This triggers the receiver in the server's graceful_shutdown handler
            // to complete, signaling Axum to stop accepting new connections
            // and drain existing requests before shutting down
            if let Some(sender) = &agent.model.shutdown_broadcast {
                let _ = sender.send(());
            }
            agent.model.shutdown_broadcast = None;
            agent.model.is_running = false;

            if let Some(console) = agent.model.actors.get("console") {
                let console = console.clone();
                return AgentReply::from_async(async move {
                    console
                        .send(PrintSuccess("Server stopped".to_string()))
                        .await;
                });
            }

            AgentReply::immediate()
        });

        // Handle configuration changes for hot reload
        builder.mutate_on::<ConfigResponse>(|agent, envelope| {
            info!("Received configuration update");

            let new_config = envelope.message().config.clone();
            let was_running = agent.model.is_running;

            // Stop the server if it's running by sending shutdown signal
            if agent.model.is_running {
                info!("Shutting down server for configuration reload");
                if let Some(sender) = &agent.model.shutdown_broadcast {
                    let _ = sender.send(());
                }
                agent.model.shutdown_broadcast = None;
                agent.model.is_running = false;
            }

            // Update configuration
            let config_port = new_config.port;
            agent.model.config = new_config;

            // Restart the server if it was running
            // We send a message to ourselves via the actor system, not a direct handle
            if was_running {
                let console = agent.model.actors.get("console").map(|h| h.clone());
                let broker = agent.broker().clone();
                let server_actor = agent.model.actors.get("server").map(|h| h.clone());

                if let (Some(console), Some(server)) = (console, server_actor) {
                    return AgentReply::from_async(async move {
                        console
                            .send(PrintProgress(
                                "Reloading server with new configuration...".to_string(),
                            ))
                            .await;

                        // Wait a moment for old server to drain connections
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                        // Restart the server with new configuration
                        server.send(StartServer).await;

                        // Broadcast ServerReloaded to notify Console of successful reload
                        broker.broadcast(ServerReloaded { port: config_port }).await;
                    });
                }
            }

            AgentReply::immediate()
        });

        // Handle keyboard events
        builder.act_on::<KeyPressed>(|agent, envelope| {
            let key_event = envelope.message();

            info!("ServerActor received KeyPressed event: key={:?}, modifiers={:?}",
                  key_event.key, key_event.modifiers);

            match (key_event.key, key_event.modifiers) {
                (KeyCode::Char('r'), KeyModifiers::NONE) => {
                    info!("Processing 'r' key - reload configuration request");

                    // Display progress message
                    if let Some(console) = agent.model.actors.get("console") {
                        let console = console.clone();
                        tokio::spawn(async move {
                            console
                                .send(PrintProgress(
                                    "Reload requested - reloading configuration...".to_string(),
                                ))
                                .await;
                        });
                    }

                    // Trigger actual reload
                    if let Some(config_manager) = agent.model.actors.get("config_manager") {
                        let config_manager = config_manager.clone();
                        return AgentReply::from_async(async move {
                            config_manager.send(ReloadConfig).await;
                        });
                    } else {
                        warn!("ConfigManager actor not found, cannot reload configuration");
                    }
                }
                (KeyCode::Char('s'), KeyModifiers::NONE) => {
                    info!("Processing 's' key - stop server request");
                    if let Some(console) = agent.model.actors.get("console") {
                        let console = console.clone();
                        return AgentReply::from_async(async move {
                            console
                                .send(PrintProgress("Stop requested via keyboard...".to_string()))
                                .await;
                        });
                    } else {
                        warn!("Console actor not found, cannot display stop message");
                    }
                }
                (KeyCode::Char('q'), KeyModifiers::NONE)
                | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                    info!("Received shutdown key (q or Ctrl+C) - deferring to main shutdown handler");
                }
                _ => {
                    debug!("Ignoring unhandled key: {:?} with modifiers {:?}",
                           key_event.key, key_event.modifiers);
                }
            }

            AgentReply::immediate()
        });

        // Subscribe to configuration changes and keyboard events
        let handle = builder.handle();
        handle.subscribe::<ConfigResponse>().await;
        handle.subscribe::<KeyPressed>().await;

        Ok(builder.start().await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_server_actor_spawn_succeeds() {
        let mut runtime = ActonApp::launch();
        let config = Config::default();
        let actors = Arc::new(DashMap::new());

        let result = ServerActor::spawn(&mut runtime, config, actors).await;
        assert!(result.is_ok(), "ServerActor should spawn successfully");

        let handle = result.unwrap();
        handle.stop().await.expect("Should stop cleanly");
        runtime
            .shutdown_all()
            .await
            .expect("Runtime should shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_server_actor_start_stop_cycle() {
        let mut runtime = ActonApp::launch();
        let config = Config { port: 0 }; // Use port 0 for random available port
        let actors = Arc::new(DashMap::new());

        let handle = ServerActor::spawn(&mut runtime, config, actors)
            .await
            .expect("Should spawn");

        // Start the server
        handle.send(StartServer).await;

        // Give it time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Stop the server
        handle.send(StopServer).await;

        // Give it time to stop
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        handle.stop().await.expect("Should stop cleanly");
        runtime
            .shutdown_all()
            .await
            .expect("Runtime should shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_server_actor_ignores_duplicate_start() {
        let mut runtime = ActonApp::launch();
        let config = Config { port: 0 };
        let actors = Arc::new(DashMap::new());

        let handle = ServerActor::spawn(&mut runtime, config, actors)
            .await
            .expect("Should spawn");

        // Start the server twice
        handle.send(StartServer).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        handle.send(StartServer).await;

        // Stop and cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        handle.send(StopServer).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        handle.stop().await.expect("Should stop cleanly");
        runtime
            .shutdown_all()
            .await
            .expect("Runtime should shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_server_shutdown_mechanism_works() {
        use std::io::ErrorKind;

        let mut runtime = ActonApp::launch();
        // Use a fixed test port that's unlikely to conflict
        let test_port = 38291;
        let config = Config { port: test_port };
        let actors = Arc::new(DashMap::new());

        let handle = ServerActor::spawn(&mut runtime, config, actors.clone())
            .await
            .expect("Should spawn");

        // Start the server
        handle.send(StartServer).await;

        // Wait for server to fully start and bind to port
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        // Verify server is actually listening on the port
        let listener_check = tokio::net::TcpListener::bind(("127.0.0.1", test_port)).await;
        assert!(
            listener_check.is_err(),
            "Port should be in use when server is running"
        );
        if let Err(e) = listener_check {
            assert_eq!(
                e.kind(),
                ErrorKind::AddrInUse,
                "Expected address-in-use error"
            );
        }

        // Stop the server
        handle.send(StopServer).await;

        // Wait for shutdown to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Verify server stopped by checking port is now available
        let listener_check = tokio::net::TcpListener::bind(("127.0.0.1", test_port)).await;
        assert!(
            listener_check.is_ok(),
            "Port should be available after server stops"
        );

        // Clean up the test listener
        drop(listener_check);

        handle.stop().await.expect("Should stop cleanly");
        runtime
            .shutdown_all()
            .await
            .expect("Runtime should shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_server_multiple_start_stop_cycles() {
        use std::io::ErrorKind;

        let mut runtime = ActonApp::launch();
        let test_port = 38292;
        let config = Config { port: test_port };
        let actors = Arc::new(DashMap::new());

        let handle = ServerActor::spawn(&mut runtime, config, actors.clone())
            .await
            .expect("Should spawn");

        // Cycle 1: Start and stop
        handle.send(StartServer).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let check1 = tokio::net::TcpListener::bind(("127.0.0.1", test_port)).await;
        assert!(check1.is_err() && check1.unwrap_err().kind() == ErrorKind::AddrInUse);

        handle.send(StopServer).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        // Cycle 2: Start again and verify it works
        handle.send(StartServer).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let check2 = tokio::net::TcpListener::bind(("127.0.0.1", test_port)).await;
        assert!(check2.is_err() && check2.unwrap_err().kind() == ErrorKind::AddrInUse);

        handle.send(StopServer).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        // Verify port is free after final stop
        let check3 = tokio::net::TcpListener::bind(("127.0.0.1", test_port)).await;
        assert!(check3.is_ok(), "Port should be available after second stop");
        drop(check3);

        handle.stop().await.expect("Should stop cleanly");
        runtime
            .shutdown_all()
            .await
            .expect("Runtime should shutdown");
    }
}
