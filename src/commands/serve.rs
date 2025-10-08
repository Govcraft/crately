/// Serve command implementation for running the HTTP server
///
/// This module contains the server startup logic, actor system initialization,
/// and graceful shutdown handling for the crately HTTP service.
use acton_reactive::prelude::*;
use anyhow::Result;
use axum::{
    Router,
    extract::{Json, State},
    routing::post,
};
use chrono::Local;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::{
    Console,
    config::ConfigManager,
    console::{PrintError, PrintProgress, PrintSuccess},
    crate_downloader::CrateDownloader,
    messages::Init,
    request::CrateRequest,
    response::CrateResponse,
};

/// Shared application state containing the acton-reactive runtime
#[derive(Clone)]
struct AppState {
    /// The acton-reactive runtime for managing reactive agents
    acton_runtime: Arc<AgentRuntime>,
    /// Handle to the configuration manager actor
    config_manager: AgentHandle,
    /// Handle to the console actor for output formatting
    console: AgentHandle,
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
/// Initializes the acton-reactive runtime, creates actor instances,
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

    // Launch the acton-reactive runtime
    let mut acton_runtime = ActonApp::launch();
    let console = Console::spawn(&mut acton_runtime).await?;
    console.send(Init).await;

    // Print startup banner and logging confirmation
    console
        .send(PrintSuccess(format!(
            "Logging initialized → {}",
            log_dir.display()
        )))
        .await;

    // Create and start the ConfigManager actor (it loads its own configuration)
    debug!("Creating ConfigManager actor");
    let (config_manager, config) = ConfigManager::spawn(&mut acton_runtime).await?;

    // Create and start the CrateDownloader actor using spawn pattern
    debug!("Creating and starting CrateDownloader actor");
    let crate_downloader_handle = CrateDownloader::spawn(&mut acton_runtime).await?;
    info!("CrateDownloader actor started successfully");

    // Create shared application state
    let app_state = AppState {
        acton_runtime: Arc::new(acton_runtime),
        config_manager: config_manager.clone(),
        console: console.clone(),
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
            app_state
                .console
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

    app_state
        .console
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
        app_state
            .console
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
    app_state
        .console
        .send(PrintProgress("Shutting down gracefully...".to_string()))
        .await;
    app_state
        .console
        .send(PrintSuccess("Server stopped".to_string()))
        .await;

    // Shutdown sequence: stop individual actors first, then shutdown runtime
    info!("Initiating graceful shutdown sequence");

    // Step 1: Stop the CrateDownloader actor
    debug!("Stopping CrateDownloader actor");
    match crate_downloader_handle.stop().await {
        Ok(()) => {
            info!("CrateDownloader actor stopped successfully");
            app_state
                .console
                .send(PrintSuccess("CrateDownloader actor stopped".to_string()))
                .await;
        }
        Err(e) => {
            error!("Failed to stop CrateDownloader actor: {:?}", e);
        }
    }

    // Step 1.5: Stop the ConfigManager actor
    debug!("Stopping ConfigManager actor");
    match app_state.config_manager.stop().await {
        Ok(()) => {
            info!("ConfigManager actor stopped successfully");
            app_state
                .console
                .send(PrintSuccess("ConfigManager actor stopped".to_string()))
                .await;
        }
        Err(e) => {
            error!("Failed to stop ConfigManager actor: {:?}", e);
        }
    }

    // Step 2: Stop the Console actor before shutting down runtime
    debug!("Stopping Console actor");
    match app_state.console.stop().await {
        Ok(()) => {
            info!("Console actor stopped successfully");
        }
        Err(e) => {
            error!("Failed to stop Console actor: {:?}", e);
        }
    }

    // Step 3: Shutdown the acton-reactive runtime
    debug!("Shutting down acton-reactive runtime");
    match Arc::try_unwrap(app_state.acton_runtime) {
        Ok(mut runtime) => match runtime.shutdown_all().await {
            Ok(()) => {
                info!("Acton-reactive runtime shut down successfully");
            }
            Err(e) => {
                error!("Failed to shut down acton-reactive runtime: {:?}", e);
            }
        },
        Err(_arc) => {
            error!(
                "Failed to unwrap acton runtime Arc - multiple references still exist. \
                 This indicates the runtime may not be properly cleaned up."
            );
        }
    }

    // Log session end separator
    info!("========================================");
    info!("= APPLICATION SHUTDOWN COMPLETE");
    info!("========================================");
    Ok(())
}
