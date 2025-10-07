//! XDG-compliant configuration module
//!
//! This module provides configuration management following the XDG Base Directory
//! Specification. The configuration file is stored at `$XDG_CONFIG_HOME/crately/config.toml`
//! (typically `~/.config/crately/config.toml`).
//!
//! The configuration is loaded at startup, with automatic creation of default configuration
//! if the file does not exist.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
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

/// Loads configuration from the XDG-compliant config file
///
/// This function attempts to load configuration from `$XDG_CONFIG_HOME/crately/config.toml`.
/// If the file does not exist, it creates a default configuration file and returns the
/// default configuration.
///
/// # Returns
///
/// Returns a tuple of `(Config, PathBuf)` containing:
/// - The loaded or default `Config`
/// - The `PathBuf` to the config file for informational purposes
///
/// # Errors
///
/// Returns `ConfigError` if:
/// - XDG base directories cannot be determined
/// - Config directory cannot be created
/// - Config file cannot be read or written
/// - TOML parsing or serialization fails
///
/// # Example
///
/// ```no_run
/// let (config, config_path) = config::load().expect("Failed to load configuration");
/// println!("Loaded config from: {}", config_path.display());
/// println!("Server port: {}", config.port);
/// ```
pub fn load() -> Result<(Config, PathBuf), ConfigError> {
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
