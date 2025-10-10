/// Serve command implementation for running the HTTP server
///
/// This module provides a thin wrapper around ActorSystem that initializes
/// server-specific actors, starts the HTTP server, and coordinates shutdown.
/// All actor lifecycle management is delegated to the ActorSystem.
use acton_reactive::prelude::*;
use anyhow::Result;
use tracing::info;

use crate::{messages::StartServer, runtime::ActorSystem};

/// Execute the serve command to start the HTTP server
///
/// This method initializes server-specific actors (KeyboardHandler and ServerActor),
/// starts the HTTP server, waits for a shutdown signal, and then triggers graceful
/// shutdown via the ActorSystem.
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

    // Initialize server-specific actors (KeyboardHandler and ServerActor)
    actor_system.initialize_server_actors().await?;

    // Get the server actor and start it
    let server = actor_system
        .get_actor("server")
        .expect("Server actor should exist after initialization");

    server.send(StartServer).await;

    // Wait for shutdown signal (Ctrl+C, SIGTERM, or keyboard 'q')
    ActorSystem::wait_for_shutdown(&mut actor_system).await;

    // Explicit shutdown to ensure proper cleanup (disables raw mode, stops actors)
    actor_system.shutdown().await?;

    info!("Serve command completed");

    Ok(())
}
