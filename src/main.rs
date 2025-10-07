mod crate_downloader;
mod crate_specifier;
mod logging;
mod request;
mod response;

use acton_reactive::prelude::*;
use axum::{
    Router,
    extract::{Json, State},
    routing::post,
};
use request::CrateRequest;
use response::CrateResponse;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{info, error, debug};

use crate::crate_downloader::CrateDownloader;

/// Shared application state containing the acton-reactive app instance
#[derive(Clone)]
struct AppState {
    /// The acton-reactive runtime for managing reactive agents
    acton_runtime: Arc<AgentRuntime>,
}

async fn handle_crate_request(
    State(state): State<AppState>,
    Json(payload): Json<CrateRequest>,
) -> Json<CrateResponse> {
    info!(
        "Received crate request: {} with features: {:?}",
        payload.specifier, payload.features
    );

    // Access the acton-reactive runtime (available for future use)
    let _runtime = &state.acton_runtime;

    // CrateSpecifier is already validated during deserialization
    let specifier = &payload.specifier;

    debug!(
        "Processing crate: {} (version: {})",
        specifier.name(),
        specifier.version()
    );

    // TODO: Send download request to CrateDownloader actor via runtime
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

#[tokio::main]
async fn main() {
    // Initialize XDG-compliant file logging
    let _guard = logging::init().expect("Failed to initialize logging");
    info!("Logging initialized successfully");

    // Launch the acton-reactive runtime
    info!("Launching acton-reactive runtime");
    let mut acton_runtime = ActonApp::launch();
    info!("Acton-reactive runtime launched successfully");

    // Create and start the CrateDownloader actor
    debug!("Creating CrateDownloader actor");
    let crate_downloader = acton_runtime.new_agent::<CrateDownloader>().await;

    debug!("Starting CrateDownloader actor");
    let crate_downloader_handle = crate_downloader.start().await;
    info!("CrateDownloader actor started successfully");

    // Create shared application state
    let app_state = AppState {
        acton_runtime: Arc::new(acton_runtime),
    };

    // Build our application with routes and state
    let app = Router::new()
        .route("/crate", post(handle_crate_request))
        .with_state(app_state.clone());

    // Run the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server failed during execution");

    info!("Server shut down gracefully");

    // Shutdown sequence: stop individual actors first, then shutdown runtime
    info!("Initiating graceful shutdown sequence");

    // Step 1: Stop the CrateDownloader actor
    debug!("Stopping CrateDownloader actor");
    match crate_downloader_handle.stop().await {
        Ok(()) => {
            info!("CrateDownloader actor stopped successfully");
        }
        Err(e) => {
            error!("Failed to stop CrateDownloader actor: {:?}", e);
        }
    }

    // Step 2: Shutdown the acton-reactive runtime
    debug!("Shutting down acton-reactive runtime");
    match Arc::try_unwrap(app_state.acton_runtime) {
        Ok(mut runtime) => {
            match runtime.shutdown_all().await {
                Ok(()) => {
                    info!("Acton-reactive runtime shut down successfully");
                }
                Err(e) => {
                    error!("Failed to shut down acton-reactive runtime: {:?}", e);
                }
            }
        }
        Err(_arc) => {
            error!(
                "Failed to unwrap acton runtime Arc - multiple references still exist. \
                 This indicates the runtime may not be properly cleaned up."
            );
        }
    }

    info!("Application shutdown complete");
}
