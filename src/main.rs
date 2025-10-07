mod crate_specifier;
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
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

    info!(
        "Processing crate: {} (version: {})",
        specifier.name(),
        specifier.version()
    );

    // TODO: Do something useful with the crate specifier and features
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
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "crately=debug,tower_http=debug,axum=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Launch the acton-reactive app instance
    info!("Launching acton-reactive runtime");
    let acton_runtime = ActonApp::launch();
    info!("Acton-reactive runtime launched successfully");

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

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    info!("Server shut down gracefully");

    // Shutdown the acton-reactive runtime after server shutdown
    info!("Initiating acton-reactive runtime shutdown");

    // Extract the runtime from the Arc. Since we cloned app_state for the router,
    // we still have one reference here. We need to get mutable access.
    match Arc::try_unwrap(app_state.acton_runtime) {
        Ok(mut runtime) => {
            match runtime.shutdown_all().await {
                Ok(()) => {
                    info!("Acton-reactive runtime shut down successfully");
                }
                Err(e) => {
                    tracing::error!("Failed to shut down acton-reactive runtime: {:?}", e);
                }
            }
        }
        Err(_) => {
            tracing::error!(
                "Failed to unwrap acton runtime Arc - multiple references still exist"
            );
        }
    }

    info!("Application shutdown complete");
}
