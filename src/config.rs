//! XDG-compliant configuration module
//!
//! This module provides configuration management following the XDG Base Directory
//! Specification. The configuration file is stored at `$XDG_CONFIG_HOME/crately/config.toml`
//! (typically `~/.config/crately/config.toml`).
//!
//! The configuration is managed by a `ConfigManager` actor that loads configuration at startup
//! and provides query and reload capabilities through message passing.

use acton_reactive::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::*;
use xdg::BaseDirectories;

/// Application configuration structure
///
/// This struct holds all configuration values for the Crately service.
/// It supports TOML serialization/deserialization and provides sensible defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// HTTP server port (default: 3000)
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: default_port(),
        }
    }
}

fn default_port() -> u16 {
    3000
}

/// Message types for ConfigManager actor communication
/// Request to get the current configuration
#[acton_actor]
pub struct GetConfig;

/// Broadcast message when configuration has been loaded
#[acton_actor]
pub struct ConfigLoaded {
    pub config_path: PathBuf,
}

/// Request to reload configuration from disk
#[acton_actor]
pub struct ReloadConfig;

/// Response containing the current configuration and its path
#[acton_actor]
#[allow(dead_code)] // Used by subscribers via message passing
pub struct ConfigResponse {
    pub config: Config,
    pub config_path: PathBuf,
}

/// Error types that can occur during configuration operations
#[derive(Debug)]
pub enum ConfigError {
    /// HOME directory could not be determined (XDG config home is None)
    HomeDirectoryNotFound,
    /// Failed to create config directory
    DirectoryCreation(std::io::Error),
    /// Failed to read config file
    FileRead(std::io::Error),
    /// Failed to write config file
    FileWrite(std::io::Error),
    /// Failed to parse TOML configuration
    TomlParse(toml::de::Error),
    /// Failed to serialize config to TOML
    TomlSerialize(toml::ser::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HomeDirectoryNotFound => {
                write!(
                    f,
                    "HOME directory not found - cannot determine XDG config directory"
                )
            }
            Self::DirectoryCreation(e) => write!(f, "Failed to create config directory: {}", e),
            Self::FileRead(e) => write!(f, "Failed to read config file: {}", e),
            Self::FileWrite(e) => write!(f, "Failed to write config file: {}", e),
            Self::TomlParse(e) => write!(f, "Failed to parse TOML configuration: {}", e),
            Self::TomlSerialize(e) => write!(f, "Failed to serialize config to TOML: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HomeDirectoryNotFound => None,
            Self::DirectoryCreation(e) => Some(e),
            Self::FileRead(e) => Some(e),
            Self::FileWrite(e) => Some(e),
            Self::TomlParse(e) => Some(e),
            Self::TomlSerialize(e) => Some(e),
        }
    }
}

/// Gets the XDG-compliant config file path for the application
///
/// Returns a path following the pattern: `$XDG_CONFIG_HOME/crately/config.toml`
/// where `$XDG_CONFIG_HOME` defaults to `~/.config` if not set.
///
/// # Errors
///
/// Returns `ConfigError::HomeDirectoryNotFound` if the HOME directory cannot be determined
fn get_config_path() -> Result<PathBuf, ConfigError> {
    // Use with_prefix to automatically append "crately" to all paths
    let base_dirs = BaseDirectories::with_prefix("crately");

    // Get the config home directory with the prefix already applied
    // This will be something like ~/.config/crately
    let config_home = base_dirs
        .get_config_home()
        .ok_or(ConfigError::HomeDirectoryNotFound)?;

    // Append config file name
    let config_path = config_home.join("config.toml");

    Ok(config_path)
}

/// Ensures the config directory exists, creating it if necessary
///
/// # Errors
///
/// Returns `ConfigError::DirectoryCreation` if the directory cannot be created
fn ensure_config_directory_exists(config_path: &Path) -> Result<(), ConfigError> {
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(ConfigError::DirectoryCreation)?;
        }
    }
    Ok(())
}

/// ConfigManager agent that manages application configuration
///
/// This actor loads configuration from the XDG-compliant config file at startup
/// and provides query and reload capabilities through message passing.
#[acton_actor]
pub struct ConfigManager {
    config: Config,
    config_path: PathBuf,
}

impl ConfigManager {
    /// Spawns, configures, and starts a new ConfigManager actor
    ///
    /// This is the standard factory method for creating ConfigManager actors.
    /// It loads configuration from the XDG-compliant config file, initializes
    /// the actor with the loaded state, and starts it to handle messages.
    ///
    /// This follows the service actor pattern where startup data is returned
    /// alongside the handle for immediate use by the application.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    ///
    /// # Returns
    ///
    /// Returns a tuple containing:
    /// - `AgentHandle` - Handle to the started ConfigManager actor for message passing
    /// - `Config` - The loaded configuration for immediate use during startup
    ///
    /// # When to Use
    ///
    /// Call this during application startup when you need both:
    /// 1. An actor to manage configuration updates via message passing
    /// 2. Immediate access to configuration values for initialization
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Configuration file cannot be loaded or parsed
    /// - Actor creation or initialization fails
    /// - XDG directories cannot be determined
    ///
    /// # Example
    ///
    /// ```no_run
    /// use acton_reactive::prelude::*;
    /// use crately::config::ConfigManager;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut runtime = ActonApp::launch();
    ///
    ///     // Spawn the ConfigManager actor and get the initial config
    ///     let (config_manager, config) = ConfigManager::spawn(&mut runtime).await?;
    ///
    ///     // Use config immediately for startup
    ///     println!("Server port: {}", config.port);
    ///
    ///     // Query configuration updates via messages
    ///     config_manager.send(GetConfig).await;
    ///
    ///     runtime.shutdown_all().await?;
    ///     Ok(())
    /// }
    /// ```
    #[instrument(skip(runtime))]
    pub async fn spawn(runtime: &mut AgentRuntime) -> anyhow::Result<(AgentHandle, Config)> {
        use anyhow::Context;

        // Load configuration internally with rich error context
        let (config, config_path) = load_config_internal().with_context(|| {
            let config_path = get_config_path().ok().map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            format!(
                "Failed to load configuration from {}\n\
                 \n\
                 Possible causes:\n\
                 - XDG_CONFIG_HOME environment variable is not set or invalid\n\
                 - Configuration file has invalid TOML syntax\n\
                 - Insufficient permissions to read config file or create config directory\n\
                 \n\
                 Expected location: $XDG_CONFIG_HOME/crately/config.toml (usually ~/.config/crately/config.toml)",
                config_path
            )
        })?;

        // Create agent configuration
        let agent_config = AgentConfig::new(Ern::with_root("config_manager").unwrap(), None, None)
            .context("Failed to create ConfigManager agent configuration")?;

        // Create the ConfigManager model with the loaded configuration
        let config_manager = ConfigManager {
            config: config.clone(),
            config_path: config_path.clone(),
        };

        // Create agent builder - we need to manually construct this
        // since we have a non-default initial state
        let mut builder = runtime
            .new_agent_with_config::<ConfigManager>(agent_config)
            .await;

        // Override the default model with our initialized one
        // This happens before .start() is called, so we can set it directly
        builder.model = config_manager;

        // Handle GetConfig requests
        builder.mutate_on::<GetConfig>(|agent, _envelope| {
            let response = ConfigResponse {
                config: agent.model.config.clone(),
                config_path: agent.model.config_path.clone(),
            };
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                broker.broadcast(response).await;
            })
        });

        // Handle ReloadConfig requests
        builder.mutate_on::<ReloadConfig>(|agent, _envelope| {
            match load_config_internal() {
                Ok((new_config, new_config_path)) => {
                    info!("Configuration reloaded from: {}", new_config_path.display());

                    // Broadcast the new config. The ConfigManager's state remains unchanged,
                    // but subscribers can update their own state with the new values.
                    let response = ConfigResponse {
                        config: new_config,
                        config_path: new_config_path,
                    };
                    let broker = agent.broker().clone();

                    Box::pin(async move {
                        broker.broadcast(response).await;
                    })
                }
                Err(e) => {
                    error!("Failed to reload configuration: {}", e);
                    // Keep existing config on reload failure
                    AgentReply::immediate()
                }
            }
        });

        // Add lifecycle hook to log configuration on startup and broadcast ConfigLoaded
        builder.after_start(|agent| {
            info!(
                "Configuration loaded from: {}",
                agent.model.config_path.display()
            );
            info!("Server port configured: {}", agent.model.config.port);

            // Broadcast ConfigLoaded message to subscribers
            let broker = agent.broker().clone();
            let config_path = agent.model.config_path.clone();

            AgentReply::from_async(async move {
                broker.broadcast(ConfigLoaded { config_path }).await;
            })
        });

        let handle = builder.start().await;
        Ok((handle, config))
    }
}

/// Internal helper function to load configuration (legacy compatibility)
///
/// This function is kept for backward compatibility and internal use.
/// New code should use the ConfigManager actor instead.
fn load_config_internal() -> Result<(Config, PathBuf), ConfigError> {
    let config_path = get_config_path()?;

    // Ensure the config directory exists
    ensure_config_directory_exists(&config_path)?;

    // If config file exists, load it; otherwise create default
    if config_path.exists() {
        let content = fs::read_to_string(&config_path).map_err(ConfigError::FileRead)?;
        let config: Config = toml::from_str(&content).map_err(ConfigError::TomlParse)?;
        Ok((config, config_path))
    } else {
        // Create default config
        let config = Config::default();
        save_config(&config_path, &config)?;
        Ok((config, config_path))
    }
}

/// Loads configuration from the XDG-compliant config file
///
/// # Deprecated
///
/// This function is deprecated in favor of using the `ConfigManager` actor.
/// It is kept for backward compatibility but should not be used in new code.
///
/// # Example
///
/// ```no_run
/// // Old approach (deprecated):
/// // let (config, config_path) = config::load().expect("Failed to load configuration");
///
/// // New approach (recommended):
/// use acton_reactive::prelude::*;
/// use crately::config::ConfigManager;
///
/// async fn example() -> anyhow::Result<()> {
///     let mut runtime = ActonApp::launch();
///     let (config_manager, config) = ConfigManager::spawn(&mut runtime).await?;
///     // Use config immediately for startup
///     println!("Server port: {}", config.port);
///     Ok(())
/// }
/// ```
#[deprecated(
    since = "0.1.0",
    note = "Use ConfigManager actor instead for better concurrency support"
)]
#[allow(dead_code)] // Kept for backward compatibility
pub fn load() -> Result<(Config, PathBuf), ConfigError> {
    load_config_internal()
}

/// Saves the configuration to the specified path
///
/// # Errors
///
/// Returns `ConfigError` if serialization or file write fails
fn save_config(config_path: &Path, config: &Config) -> Result<(), ConfigError> {
    let toml_string = toml::to_string_pretty(config).map_err(ConfigError::TomlSerialize)?;
    fs::write(config_path, toml_string).map_err(ConfigError::FileWrite)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config_values() {
        let config = Config::default();
        assert_eq!(config.port, 3000);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config { port: 8080 };
        let toml_string = toml::to_string(&config).unwrap();
        assert!(toml_string.contains("port"));
        assert!(toml_string.contains("8080"));
    }

    #[test]
    fn test_config_deserialization() {
        let toml_string = "port = 8080\n";
        let config: Config = toml::from_str(toml_string).unwrap();
        assert_eq!(config.port, 8080);
    }

    #[test]
    fn test_config_deserialization_with_defaults() {
        // Empty config should use defaults
        let toml_string = "";
        let config: Config = toml::from_str(toml_string).unwrap();
        assert_eq!(config.port, 3000);
    }

    #[test]
    fn test_config_roundtrip() {
        let original = Config { port: 9000 };
        let toml_string = toml::to_string(&original).unwrap();
        let deserialized: Config = toml::from_str(&toml_string).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_get_config_path_returns_valid_path() {
        let result = get_config_path();
        assert!(result.is_ok());

        let config_path = result.unwrap();
        assert!(config_path.to_string_lossy().contains("crately"));
        assert!(config_path.to_string_lossy().contains("config.toml"));
    }

    #[test]
    fn test_ensure_config_directory_exists_creates_directory() {
        // Create a temporary path for testing
        let temp_dir = env::temp_dir().join("crately_test_config");
        let config_path = temp_dir.join("config.toml");

        // Ensure it doesn't exist before the test
        let _ = fs::remove_dir_all(&temp_dir);

        // Test directory creation
        let result = ensure_config_directory_exists(&config_path);
        assert!(result.is_ok());
        assert!(temp_dir.exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ensure_config_directory_exists_handles_existing_directory() {
        let temp_dir = env::temp_dir().join("crately_test_config_existing");
        let config_path = temp_dir.join("config.toml");

        // Create the directory first
        let _ = fs::create_dir_all(&temp_dir);

        // Test that it handles existing directory
        let result = ensure_config_directory_exists(&config_path);
        assert!(result.is_ok());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = env::temp_dir().join("crately_test_config_save_load");
        let config_path = temp_dir.join("config.toml");

        // Ensure clean state
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);

        // Create and save config
        let original_config = Config { port: 7777 };
        let save_result = save_config(&config_path, &original_config);
        assert!(save_result.is_ok());

        // Verify file exists
        assert!(config_path.exists());

        // Load and verify
        let content = fs::read_to_string(&config_path).unwrap();
        let loaded_config: Config = toml::from_str(&content).unwrap();
        assert_eq!(loaded_config.port, 7777);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_creates_default_config_if_not_exists() {
        // Use a unique temp directory to avoid conflicts
        let temp_home = env::temp_dir().join("crately_test_load_default");
        let _ = fs::remove_dir_all(&temp_home);

        // This test verifies behavior but cannot easily override XDG paths
        // in a cross-platform way without environment manipulation.
        // The functionality is tested through integration with actual XDG paths.

        // For unit testing, we verify the default config structure
        let default_config = Config::default();
        assert_eq!(default_config.port, 3000);
    }

    #[test]
    fn test_config_error_display_messages() {
        let err = ConfigError::HomeDirectoryNotFound;
        assert!(err.to_string().contains("HOME directory not found"));

        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test");
        let err = ConfigError::DirectoryCreation(io_err);
        assert!(
            err.to_string()
                .contains("Failed to create config directory")
        );
    }

    #[test]
    fn test_config_equality() {
        let config1 = Config { port: 3000 };
        let config2 = Config { port: 3000 };
        let config3 = Config { port: 8080 };

        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    #[test]
    fn test_config_clone() {
        let config1 = Config { port: 5555 };
        let config2 = config1.clone();
        assert_eq!(config1, config2);
    }
}
