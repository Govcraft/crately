//! Actor modules for Crately.
//!
//! This module contains all actor implementations organized in a dedicated namespace.
//! Each actor is defined in its own file following the actor factory pattern with `spawn` methods.
//!
//! # Organization Pattern
//!
//! All actors are located in `src/actors/` to match the organizational pattern used for
//! `src/messages/` and `src/commands/`, providing a consistent and discoverable structure.
//!
//! # Actor Categories
//!
//! - **Core Actors** (initialized in `ActorSystem::initialize()`):
//!   - [`Console`](console::Console) - Output formatting actor
//!   - [`ConfigManager`](config::ConfigManager) - Configuration management with hot-reload
//!   - [`DownloaderActor`](downloader_actor::DownloaderActor) - Stateless crate download worker
//!   - [`FileReaderActor`](file_reader_actor::FileReaderActor) - Stateless documentation extraction worker
//!   - [`ProcessorActor`](processor_actor::ProcessorActor) - Stateless documentation chunking worker
//!   - [`DatabaseActor`](database::DatabaseActor) - SurrealDB persistence management
//!
//! - **Server Actors** (initialized in `ActorSystem::initialize_server_actors()`):
//!   - [`KeyboardHandler`](keyboard_handler::KeyboardHandler) - Interactive keyboard input
//!   - [`ServerActor`](server_actor::ServerActor) - Axum HTTP server lifecycle management

pub mod config;
pub mod console;
pub mod database;
pub mod downloader_actor;
pub mod file_reader_actor;
pub mod keyboard_handler;
pub mod processor_actor;
pub mod retry_coordinator;
pub mod server_actor;
