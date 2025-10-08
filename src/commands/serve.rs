/// Serve command implementation for running the HTTP server
///
/// This module contains the server startup logic and graceful shutdown handling
/// for the crately HTTP service. The actor system is pre-initialized by main.rs
/// and passed as a parameter.
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::info;

use acton_reactive::prelude::*;

use crate::{
    keyboard_handler::{KeyboardHandler, StopKeyboardHandler},
    messages::{StartServer, StopServer},
    runtime::ActorSystem,
    server_actor::ServerActor,
};

/// Execute the serve command to start the HTTP server
///
/// Initializes keyboard handler and server actors, then waits for shutdown signal.
/// The ActorSystem must be pre-initialized by the caller (main.rs).
///
/// # Arguments
///
/// * `actor_system` - Pre-initialized actor system with core actors
///
/// # Returns
///
/// Returns `Ok(())` on successful shutdown, or an error if startup or
/// shutdown fails.
///
/// # Examples
///
/// ```no_run
/// use crately::commands::serve;
/// use crately::runtime::ActorSystem;
///
/// #[tokio::main]
/// async fn main() {
///     let actor_system = ActorSystem::initialize().await.expect("Failed to initialize");
///     serve::run(actor_system).await.expect("Serve command failed");
/// }
/// ```
pub async fn run(mut actor_system: ActorSystem) -> Result<()> {
    info!("Starting serve command");

    // Get references to needed actors
    let config = actor_system.config().clone();
    let actors = actor_system.actors();

    // Create a channel for coordinating shutdown
    let (_shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

    // Spawn keyboard handler actor (only for serve command)
    info!("Spawning keyboard handler");
    let keyboard_handle = KeyboardHandler::spawn(actor_system.runtime_mut()).await?;

    // Spawn server actor
    info!("Spawning server actor");
    let server_handle = ServerActor::spawn(actor_system.runtime_mut(), config, actors).await?;

    // Start the server
    info!("Sending StartServer message");
    server_handle.send(StartServer).await;

    eprintln!();
    eprintln!("Server is running. Press 'q' or Ctrl+C to shutdown gracefully");
    eprintln!("Press 'r' to reload configuration");
    eprintln!();

    // Wait for shutdown signal
    shutdown_signal(&mut shutdown_rx).await;

    info!("Shutdown signal received, stopping server");

    // Stop the server
    server_handle.send(StopServer).await;

    // Give server time to drain connections
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Stop keyboard handler
    keyboard_handle.send(StopKeyboardHandler).await;

    // Stop actors
    server_handle.stop().await?;
    keyboard_handle.stop().await?;

    info!("Serve command completed");

    Ok(())
}

/// Wait for shutdown signal (keyboard 'q' or Ctrl+C)
async fn shutdown_signal(shutdown_rx: &mut mpsc::Receiver<()>) {
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
            info!("Received shutdown signal from keyboard handler");
        },
    }
}
