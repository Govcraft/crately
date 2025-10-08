#![forbid(unsafe_code)]

mod cli;
mod commands;
mod config;
mod console;
mod crate_downloader;
mod crate_specifier;
mod logging;
mod messages;
mod request;
mod response;
pub mod runtime;

use clap::Parser;
use cli::{Cli, Commands};

use crate::console::Console;

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
            // Run the serve command
            commands::serve::run().await?;
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
