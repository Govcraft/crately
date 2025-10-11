#![forbid(unsafe_code)]

mod actors;
mod cli;
mod colors;
mod commands;
mod crate_specifier;
mod logging;
mod messages;
mod request;
mod response;
pub mod runtime;

use clap::Parser;
use cli::{Cli, Commands};

use crate::colors::ColorConfig;
use crate::runtime::ActorSystem;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Dispatch to the appropriate command handler
    match cli.command {
        Commands::Cache(args) => {
            // Run the cache command
            commands::cache::run(args)?;
        }
        Commands::Doctor(args) => {
            // Run the doctor command
            commands::doctor::run(args)?;
        }
        Commands::Init(args) => {
            // Run the init command
            commands::init::run(args)?;
        }
        Commands::Serve => {
            // Initialize XDG-compliant file logging
            let (_guard, _log_dir) = logging::init().expect("Failed to initialize logging");

            // Create color configuration based on CLI color choice
            let color_config = ColorConfig::new(cli.color);

            // Initialize the actor system
            let actor_system = ActorSystem::initialize(color_config).await?;

            // Run the serve command with the initialized actor system
            commands::serve::run(actor_system).await?;
        }
        Commands::Batch(args) => {
            // Run the batch command
            commands::batch::run(args)?;
        }
        Commands::Warm(args) => {
            // Run the warm command
            commands::warm::run(args)?;
        }
    }

    Ok(())
}
