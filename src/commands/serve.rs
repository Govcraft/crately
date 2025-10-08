/// Serve command implementation for running the HTTP server
///
/// This module contains the server startup logic and graceful shutdown handling
/// for the crately HTTP service. Actor system initialization is handled by the
/// centralized runtime module.
use anyhow::Result;
use axum::{
    Router,
    extract::{Json, State},
    routing::post,
};
use chrono::Local;
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, info};

use acton_reactive::prelude::*;
use crate::{
    console::{PrintError, PrintProgress, PrintSuccess},
    request::CrateRequest,
    response::CrateResponse,
    runtime::ActorSystem,
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

    // TODO: Send download request to CrateDownloader actor
    // Access the crate downloader actor from the registry
    if let Some(_crate_downloader) = state.actors.get("crate_downloader") {
        // Future: send DownloadCrate message to the actor
    }

    // For now, just echo back with a success message
    let response = CrateResponse {
        name: specifier.name().to_string(),
        version: specifier.version().to_string(),
        features: payload.features.clone(),
        message: format!(
            "Successfully processed crate {} with {} feature(s)",
            specifier,
            payload.features.len()
        ),
    };

    Json(response)
}

/// Wait for shutdown signal (SIGTERM or Ctrl+C)
async fn shutdown_signal() {
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
    }
}

/// Execute the serve command to start the HTTP server
///
/// Initializes the actor system via `ActorSystem::initialize()`,
/// starts the HTTP server, and handles graceful shutdown.
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
///
/// #[tokio::main]
/// async fn main() {
///     serve::run().await.expect("Serve command failed");
/// }
/// ```
pub async fn run() -> Result<()> {
    // Initialize XDG-compliant file logging
    let (_guard, log_dir) = crate::logging::init().expect("Failed to initialize logging");

    // Initialize the centralized actor system
    debug!("Initializing actor system");
    let actor_system = ActorSystem::initialize().await?;

    // Get actor handles from the registry
    let console = actor_system
        .get_actor("console")
        .expect("Console actor should be registered");

    // Get configuration from the actor system
    let config = actor_system.config();

    // Create shared application state with the actor registry
    let app_state = AppState {
        actors: actor_system.actors(),
    };

    // Build our application with routes and state
    let app = Router::new()
        .route("/crate", post(handle_crate_request))
        .with_state(app_state.clone());

    // Run the server
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    info!("Starting server on {}", addr);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!();
            console
                .send(PrintError("Failed to start server".to_string()))
                .await;
            eprintln!("  Reason: {}", e);
            eprintln!("  Action: Check if port {} is already in use", addr.port());
            eprintln!(
                "  Logs: {}/crately.{}",
                log_dir.display(),
                Local::now().format("%Y-%m-%d")
            );
            std::process::exit(1);
        }
    };

    console
        .send(PrintSuccess(format!("Server listening → http://{}", addr)))
        .await;
    eprintln!();
    eprintln!("Press Ctrl+C to shutdown gracefully");
    eprintln!();

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!();
        console
            .send(PrintError("Server error during execution".to_string()))
            .await;
        eprintln!("  Reason: {}", e);
        eprintln!(
            "  Logs: {}/crately.{}",
            log_dir.display(),
            Local::now().format("%Y-%m-%d")
        );
        std::process::exit(1);
    }

    info!("Server shut down gracefully");

    // Clear the ^C that the terminal printed and move to start of line
    eprint!("\r");
    console
        .send(PrintProgress("Shutting down gracefully...".to_string()))
        .await;
    console
        .send(PrintSuccess("Server stopped".to_string()))
        .await;

    // Delegate shutdown to the ActorSystem
    actor_system.shutdown().await?;

    Ok(())
}
