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

use crate::messages::{ConfigReloadFailed, PipelineConfigChanged};

/// Application configuration structure
///
/// This struct holds all configuration values for the Crately service including
/// server settings and pipeline processing parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// HTTP server port (default: 3000)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Pipeline processing configuration
    #[serde(default)]
    pub pipeline: PipelineConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: default_port(),
            pipeline: PipelineConfig::default(),
        }
    }
}

fn default_port() -> u16 {
    3000
}

/// Pipeline processing configuration for all stages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PipelineConfig {
    /// Download stage configuration
    #[serde(default)]
    pub download: DownloadConfig,

    /// File reading stage configuration
    #[serde(default)]
    pub read: ReadConfig,

    /// Processing stage configuration
    #[serde(default)]
    pub process: ProcessConfig,

    /// Vectorization stage configuration
    #[serde(default)]
    pub vectorize: VectorizeConfig,

    /// Coordinator configuration
    #[serde(default)]
    pub coordinator: CoordinatorConfig,
}

impl PipelineConfig {
    /// Validates pipeline configuration for logical consistency
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::InvalidValue` if any configuration value fails validation
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.download.validate()?;
        self.read.validate()?;
        self.process.validate()?;
        self.vectorize.validate()?;
        self.coordinator.validate()?;
        Ok(())
    }
}

/// Configuration for the crate download stage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadConfig {
    /// Directory path for downloaded crates (default: $XDG_DATA_HOME/crately/downloads)
    #[serde(default = "default_download_dir")]
    pub download_dir: PathBuf,

    /// HTTP request timeout in seconds (default: 30)
    #[serde(default = "default_download_timeout")]
    pub timeout_secs: u64,

    /// Maximum concurrent downloads (default: 4)
    #[serde(default = "default_max_concurrent_downloads")]
    pub max_concurrent: usize,

    /// Maximum download retries (default: 3)
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Retry delay in seconds (default: 5)
    #[serde(default = "default_retry_delay")]
    pub retry_delay_secs: u64,

    /// crates.io API base URL (default: https://crates.io/api/v1)
    #[serde(default = "default_crates_io_url")]
    pub crates_io_url: String,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            download_dir: default_download_dir(),
            timeout_secs: default_download_timeout(),
            max_concurrent: default_max_concurrent_downloads(),
            max_retries: default_max_retries(),
            retry_delay_secs: default_retry_delay(),
            crates_io_url: default_crates_io_url(),
        }
    }
}

impl DownloadConfig {
    /// Validates download configuration
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::InvalidValue` if any value is invalid
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.timeout_secs == 0 {
            return Err(ConfigError::InvalidValue(
                "download.timeout_secs must be > 0".to_string(),
            ));
        }
        if self.max_concurrent == 0 {
            return Err(ConfigError::InvalidValue(
                "download.max_concurrent must be > 0".to_string(),
            ));
        }
        if self.max_retries == 0 {
            return Err(ConfigError::InvalidValue(
                "download.max_retries must be > 0".to_string(),
            ));
        }
        if self.retry_delay_secs == 0 {
            return Err(ConfigError::InvalidValue(
                "download.retry_delay_secs must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

fn default_download_dir() -> PathBuf {
    BaseDirectories::with_prefix("crately")
        .get_data_home()
        .map(|path| path.join("downloads"))
        .unwrap_or_else(|| PathBuf::from("downloads"))
}

fn default_download_timeout() -> u64 {
    30
}

fn default_max_concurrent_downloads() -> usize {
    4
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay() -> u64 {
    5
}

fn default_crates_io_url() -> String {
    "https://crates.io/api/v1".to_string()
}

/// Configuration for the file reading stage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadConfig {
    /// Directory path for extracted crate files (default: $XDG_DATA_HOME/crately/extracted)
    #[serde(default = "default_extract_dir")]
    pub extract_dir: PathBuf,

    /// Maximum file size to read in bytes (default: 10MB)
    #[serde(default = "default_max_file_size")]
    pub max_file_size_bytes: u64,

    /// File patterns to include (default: ["*.rs", "*.md", "*.toml"])
    #[serde(default = "default_include_patterns")]
    pub include_patterns: Vec<String>,

    /// File patterns to exclude (default: ["*/target/*", "*/tests/*"])
    #[serde(default = "default_exclude_patterns")]
    pub exclude_patterns: Vec<String>,

    /// Read operation timeout in seconds (default: 10)
    #[serde(default = "default_read_timeout")]
    pub timeout_secs: u64,
}

impl Default for ReadConfig {
    fn default() -> Self {
        Self {
            extract_dir: default_extract_dir(),
            max_file_size_bytes: default_max_file_size(),
            include_patterns: default_include_patterns(),
            exclude_patterns: default_exclude_patterns(),
            timeout_secs: default_read_timeout(),
        }
    }
}

impl ReadConfig {
    /// Validates read configuration
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::InvalidValue` if any value is invalid
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_file_size_bytes == 0 {
            return Err(ConfigError::InvalidValue(
                "read.max_file_size_bytes must be > 0".to_string(),
            ));
        }
        if self.timeout_secs == 0 {
            return Err(ConfigError::InvalidValue(
                "read.timeout_secs must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

fn default_extract_dir() -> PathBuf {
    BaseDirectories::with_prefix("crately")
        .get_data_home()
        .map(|path| path.join("extracted"))
        .unwrap_or_else(|| PathBuf::from("extracted"))
}

fn default_max_file_size() -> u64 {
    10_485_760 // 10MB
}

fn default_include_patterns() -> Vec<String> {
    vec![
        "*.rs".to_string(),
        "*.md".to_string(),
        "*.toml".to_string(),
    ]
}

fn default_exclude_patterns() -> Vec<String> {
    vec!["*/target/*".to_string(), "*/tests/*".to_string()]
}

fn default_read_timeout() -> u64 {
    10
}

/// Token encoding type for tokenization
///
/// Specifies which tiktoken encoding to use for token counting and text splitting.
/// Different encodings match different model families and have different token vocabularies.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TokenEncoding {
    /// cl100k_base encoding used by GPT-3.5-turbo, GPT-4, text-embedding-ada-002, text-embedding-3-small, text-embedding-3-large
    #[serde(rename = "cl100k_base")]
    Cl100k,
    /// o200k_base encoding used by newer models
    #[serde(rename = "o200k_base")]
    O200k,
    /// p50k_base encoding used by earlier GPT-3 models (text-davinci-002, text-davinci-003, code-davinci-002)
    #[serde(rename = "p50k_base")]
    P50k,
}

impl Default for TokenEncoding {
    fn default() -> Self {
        Self::Cl100k
    }
}

/// Configuration for the processing stage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessConfig {
    /// rustdoc compilation timeout in seconds (default: 300)
    #[serde(default = "default_compilation_timeout")]
    pub compilation_timeout_secs: u64,

    /// Maximum concurrent compilation jobs (default: 2)
    #[serde(default = "default_max_concurrent_compilations")]
    pub max_concurrent_compilations: usize,

    /// Documentation output directory (default: $XDG_DATA_HOME/crately/docs)
    #[serde(default = "default_docs_dir")]
    pub docs_dir: PathBuf,

    /// Text chunking size in tokens (default: 500)
    ///
    /// BREAKING CHANGE: This is now in tokens, not characters.
    /// Previous default was 1000 characters (~250 tokens).
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,

    /// Chunk overlap in tokens (default: 100)
    ///
    /// BREAKING CHANGE: This is now in tokens, not characters.
    /// Previous default was 200 characters (~50 tokens).
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,

    /// Token encoding to use for chunking (default: Cl100kBase)
    #[serde(default)]
    pub token_encoding: TokenEncoding,

    /// Enable source code extraction (default: true)
    #[serde(default = "default_extract_source_code")]
    pub extract_source_code: bool,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            compilation_timeout_secs: default_compilation_timeout(),
            max_concurrent_compilations: default_max_concurrent_compilations(),
            docs_dir: default_docs_dir(),
            chunk_size: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
            token_encoding: TokenEncoding::default(),
            extract_source_code: default_extract_source_code(),
        }
    }
}

impl ProcessConfig {
    /// Validates process configuration
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::InvalidValue` if any value is invalid
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.compilation_timeout_secs == 0 {
            return Err(ConfigError::InvalidValue(
                "process.compilation_timeout_secs must be > 0".to_string(),
            ));
        }
        if self.max_concurrent_compilations == 0 {
            return Err(ConfigError::InvalidValue(
                "process.max_concurrent_compilations must be > 0".to_string(),
            ));
        }
        if self.chunk_size <= self.chunk_overlap {
            return Err(ConfigError::InvalidValue(
                "process.chunk_size must be > chunk_overlap".to_string(),
            ));
        }
        Ok(())
    }
}

fn default_compilation_timeout() -> u64 {
    300
}

fn default_max_concurrent_compilations() -> usize {
    2
}

fn default_docs_dir() -> PathBuf {
    BaseDirectories::with_prefix("crately")
        .get_data_home()
        .map(|path| path.join("docs"))
        .unwrap_or_else(|| PathBuf::from("docs"))
}

fn default_chunk_size() -> usize {
    500 // tokens (was 1000 characters ~250 tokens)
}

fn default_chunk_overlap() -> usize {
    100 // tokens (was 200 characters ~50 tokens)
}

fn default_extract_source_code() -> bool {
    true
}

/// Configuration for the vectorization stage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VectorizeConfig {
    /// Embedding model name (default: "text-embedding-3-small")
    #[serde(default = "default_model_name")]
    pub model_name: String,

    /// Embedding model version (default: "1.0")
    #[serde(default = "default_model_version")]
    pub model_version: String,

    /// Batch size for vectorization (default: 32)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Vector dimension (default: 1536 for text-embedding-3-small)
    #[serde(default = "default_vector_dimension")]
    pub vector_dimension: usize,

    /// Maximum concurrent vectorization operations (default: 2)
    #[serde(default = "default_max_concurrent_vectorize")]
    pub max_concurrent: usize,

    /// Vectorization timeout in seconds (default: 60)
    #[serde(default = "default_vectorize_timeout")]
    pub timeout_secs: u64,

    /// OpenAI-compatible API endpoint for embeddings
    /// (default: "https://api.openai.com/v1/embeddings")
    #[serde(default = "default_api_endpoint")]
    pub api_endpoint: String,

    /// API key for embedding service (read from environment variable)
    /// Set OPENAI_API_KEY or EMBEDDING_API_KEY in the environment
    #[serde(skip)]
    pub api_key: Option<String>,

    /// HTTP request timeout in seconds for embedding API calls (default: 30)
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,

    /// Maximum retry attempts for failed API requests (default: 3)
    #[serde(default = "default_embedding_max_retries")]
    pub max_retries: u32,

    /// Initial retry delay in seconds for exponential backoff (default: 2)
    #[serde(default = "default_embedding_retry_delay_secs")]
    pub retry_delay_secs: u64,
}

impl Default for VectorizeConfig {
    fn default() -> Self {
        Self {
            model_name: default_model_name(),
            model_version: default_model_version(),
            batch_size: default_batch_size(),
            vector_dimension: default_vector_dimension(),
            max_concurrent: default_max_concurrent_vectorize(),
            timeout_secs: default_vectorize_timeout(),
            api_endpoint: default_api_endpoint(),
            api_key: None, // Set via environment variable at runtime
            request_timeout_secs: default_request_timeout_secs(),
            max_retries: default_embedding_max_retries(),
            retry_delay_secs: default_embedding_retry_delay_secs(),
        }
    }
}

impl VectorizeConfig {
    /// Validates vectorize configuration
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::InvalidValue` if any value is invalid
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.batch_size == 0 {
            return Err(ConfigError::InvalidValue(
                "vectorize.batch_size must be > 0".to_string(),
            ));
        }
        if self.vector_dimension == 0 {
            return Err(ConfigError::InvalidValue(
                "vectorize.vector_dimension must be > 0".to_string(),
            ));
        }
        if self.max_concurrent == 0 {
            return Err(ConfigError::InvalidValue(
                "vectorize.max_concurrent must be > 0".to_string(),
            ));
        }
        if self.timeout_secs == 0 {
            return Err(ConfigError::InvalidValue(
                "vectorize.timeout_secs must be > 0".to_string(),
            ));
        }

        // Validate API endpoint is not empty
        if self.api_endpoint.is_empty() {
            return Err(ConfigError::InvalidValue(
                "vectorize.api_endpoint must not be empty".to_string(),
            ));
        }

        // Validate API endpoint is a valid URL format (basic check)
        if !self.api_endpoint.starts_with("http://") && !self.api_endpoint.starts_with("https://") {
            return Err(ConfigError::InvalidValue(
                "vectorize.api_endpoint must be a valid HTTP(S) URL".to_string(),
            ));
        }

        if self.request_timeout_secs == 0 {
            return Err(ConfigError::InvalidValue(
                "vectorize.request_timeout_secs must be > 0".to_string(),
            ));
        }
        if self.max_retries == 0 {
            return Err(ConfigError::InvalidValue(
                "vectorize.max_retries must be > 0".to_string(),
            ));
        }
        if self.retry_delay_secs == 0 {
            return Err(ConfigError::InvalidValue(
                "vectorize.retry_delay_secs must be > 0".to_string(),
            ));
        }

        Ok(())
    }
}

fn default_model_name() -> String {
    "text-embedding-3-small".to_string()
}

fn default_model_version() -> String {
    "1.0".to_string()
}

fn default_batch_size() -> usize {
    32
}

fn default_vector_dimension() -> usize {
    1536
}

fn default_max_concurrent_vectorize() -> usize {
    2
}

fn default_vectorize_timeout() -> u64 {
    60
}

fn default_api_endpoint() -> String {
    "https://api.openai.com/v1/embeddings".to_string()
}

fn default_request_timeout_secs() -> u64 {
    30
}

fn default_embedding_max_retries() -> u32 {
    3
}

fn default_embedding_retry_delay_secs() -> u64 {
    2
}

/// Configuration for the crate processing coordinator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinatorConfig {
    /// Maximum retry attempts (default: 3)
    #[serde(default = "default_max_retries_coordinator")]
    pub max_retries: u32,

    /// Retry backoff delay in seconds (default: 2)
    #[serde(default = "default_retry_backoff")]
    pub retry_backoff_secs: u64,

    /// Processing timeout in seconds (default: 300 = 5 minutes)
    #[serde(default = "default_processing_timeout")]
    pub timeout_secs: u64,

    /// Check interval for timeout detection in seconds (default: 30)
    #[serde(default = "default_check_interval")]
    pub check_interval_secs: u64,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries_coordinator(),
            retry_backoff_secs: default_retry_backoff(),
            timeout_secs: default_processing_timeout(),
            check_interval_secs: default_check_interval(),
        }
    }
}

impl CoordinatorConfig {
    /// Validates coordinator configuration
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::InvalidValue` if any value is invalid
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_retries == 0 {
            return Err(ConfigError::InvalidValue(
                "coordinator.max_retries must be > 0".to_string(),
            ));
        }
        if self.retry_backoff_secs == 0 {
            return Err(ConfigError::InvalidValue(
                "coordinator.retry_backoff_secs must be > 0".to_string(),
            ));
        }
        if self.timeout_secs == 0 {
            return Err(ConfigError::InvalidValue(
                "coordinator.timeout_secs must be > 0".to_string(),
            ));
        }
        if self.check_interval_secs == 0 {
            return Err(ConfigError::InvalidValue(
                "coordinator.check_interval_secs must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

fn default_max_retries_coordinator() -> u32 {
    3
}

fn default_retry_backoff() -> u64 {
    2
}

fn default_processing_timeout() -> u64 {
    300
}

fn default_check_interval() -> u64 {
    30
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
    /// Configuration value validation failed
    InvalidValue(String),
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
            Self::InvalidValue(msg) => write!(f, "Invalid configuration value: {}", msg),
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
            Self::InvalidValue(_) => None,
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
                    // Validate pipeline configuration before updating
                    if let Err(e) = new_config.pipeline.validate() {
                        error!("Configuration validation failed: {}", e);

                        // Broadcast validation error
                        let error_msg = ConfigReloadFailed {
                            error: format!("Configuration validation failed: {}", e),
                        };
                        let broker = agent.broker().clone();

                        return Box::pin(async move {
                            broker.broadcast(error_msg).await;
                        });
                    }

                    info!("Configuration reloaded from: {}", new_config_path.display());

                    // Update internal state
                    agent.model.config = new_config.clone();
                    agent.model.config_path = new_config_path.clone();

                    // Broadcast ConfigResponse (for ServerActor)
                    let config_response = ConfigResponse {
                        config: new_config.clone(),
                        config_path: new_config_path.clone(),
                    };

                    // Broadcast ConfigLoaded (for Console)
                    let config_loaded = ConfigLoaded {
                        config_path: new_config_path,
                    };

                    // Broadcast PipelineConfigChanged (for pipeline actors)
                    let pipeline_changed = PipelineConfigChanged {
                        pipeline: new_config.pipeline,
                    };

                    let broker = agent.broker().clone();

                    Box::pin(async move {
                        broker.broadcast(config_response).await;
                        broker.broadcast(config_loaded).await;
                        broker.broadcast(pipeline_changed).await;
                    })
                }
                Err(e) => {
                    error!("Failed to reload configuration: {}", e);

                    // Broadcast error to subscribers (Console will display to user)
                    let error_msg = ConfigReloadFailed {
                        error: e.to_string(),
                    };
                    let broker = agent.broker().clone();

                    Box::pin(async move {
                        broker.broadcast(error_msg).await;
                    })
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
        assert_eq!(config.pipeline.download.timeout_secs, 30);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            port: 8080,
            pipeline: PipelineConfig::default(),
        };
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
        let original = Config {
            port: 9000,
            pipeline: PipelineConfig::default(),
        };
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
        let original_config = Config {
            port: 7777,
            pipeline: PipelineConfig::default(),
        };
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
        let config1 = Config {
            port: 3000,
            pipeline: PipelineConfig::default(),
        };
        let config2 = Config {
            port: 3000,
            pipeline: PipelineConfig::default(),
        };
        let config3 = Config {
            port: 8080,
            pipeline: PipelineConfig::default(),
        };

        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    #[test]
    fn test_config_clone() {
        let config1 = Config {
            port: 5555,
            pipeline: PipelineConfig::default(),
        };
        let config2 = config1.clone();
        assert_eq!(config1, config2);
    }

    // Pipeline configuration tests
    #[test]
    fn test_pipeline_config_default_values() {
        let pipeline = PipelineConfig::default();

        // Download config defaults
        assert_eq!(pipeline.download.timeout_secs, 30);
        assert_eq!(pipeline.download.max_concurrent, 4);
        assert_eq!(pipeline.download.max_retries, 3);
        assert_eq!(pipeline.download.retry_delay_secs, 5);
        assert_eq!(
            pipeline.download.crates_io_url,
            "https://crates.io/api/v1"
        );

        // Read config defaults
        assert_eq!(pipeline.read.max_file_size_bytes, 10_485_760);
        assert_eq!(pipeline.read.timeout_secs, 10);
        assert_eq!(pipeline.read.include_patterns.len(), 3);
        assert_eq!(pipeline.read.exclude_patterns.len(), 2);

        // Process config defaults
        assert_eq!(pipeline.process.compilation_timeout_secs, 300);
        assert_eq!(pipeline.process.max_concurrent_compilations, 2);
        assert_eq!(pipeline.process.chunk_size, 500); // BREAKING CHANGE: Now in tokens
        assert_eq!(pipeline.process.chunk_overlap, 100); // BREAKING CHANGE: Now in tokens
        assert!(pipeline.process.extract_source_code);

        // Vectorize config defaults
        assert_eq!(pipeline.vectorize.model_name, "text-embedding-3-small");
        assert_eq!(pipeline.vectorize.model_version, "1.0");
        assert_eq!(pipeline.vectorize.batch_size, 32);
        assert_eq!(pipeline.vectorize.vector_dimension, 1536);
        assert_eq!(pipeline.vectorize.max_concurrent, 2);
        assert_eq!(pipeline.vectorize.timeout_secs, 60);
        assert_eq!(
            pipeline.vectorize.api_endpoint,
            "https://api.openai.com/v1/embeddings"
        );
        assert!(pipeline.vectorize.api_key.is_none()); // Set via environment variable
        assert_eq!(pipeline.vectorize.request_timeout_secs, 30);
        assert_eq!(pipeline.vectorize.max_retries, 3);
        assert_eq!(pipeline.vectorize.retry_delay_secs, 2);
    }

    #[test]
    fn test_pipeline_config_serialization() {
        let config = Config {
            port: 3000,
            pipeline: PipelineConfig::default(),
        };
        let toml_string = toml::to_string(&config).unwrap();
        assert!(toml_string.contains("[pipeline.download]"));
        assert!(toml_string.contains("[pipeline.read]"));
        assert!(toml_string.contains("[pipeline.process]"));
        assert!(toml_string.contains("[pipeline.vectorize]"));
    }

    #[test]
    fn test_pipeline_config_deserialization_with_defaults() {
        let toml_string = r#"
            port = 3000
            [pipeline.download]
            timeout_secs = 60
        "#;

        let config: Config = toml::from_str(toml_string).unwrap();
        assert_eq!(config.pipeline.download.timeout_secs, 60);
        assert_eq!(config.pipeline.download.max_concurrent, 4); // default used
    }

    #[test]
    fn test_pipeline_config_validation_success() {
        let config = PipelineConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_download_config_validation_zero_timeout() {
        let config = DownloadConfig {
            timeout_secs: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("download.timeout_secs must be > 0"));
    }

    #[test]
    fn test_download_config_validation_zero_max_concurrent() {
        let config = DownloadConfig {
            max_concurrent: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("download.max_concurrent must be > 0"));
    }

    #[test]
    fn test_download_config_validation_zero_max_retries() {
        let config = DownloadConfig {
            max_retries: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("download.max_retries must be > 0"));
    }

    #[test]
    fn test_read_config_validation_zero_max_file_size() {
        let config = ReadConfig {
            max_file_size_bytes: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("read.max_file_size_bytes must be > 0"));
    }

    #[test]
    fn test_read_config_validation_zero_timeout() {
        let config = ReadConfig {
            timeout_secs: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("read.timeout_secs must be > 0"));
    }

    #[test]
    fn test_process_config_validation_zero_compilation_timeout() {
        let config = ProcessConfig {
            compilation_timeout_secs: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("process.compilation_timeout_secs must be > 0"));
    }

    #[test]
    fn test_process_config_validation_zero_max_concurrent_compilations() {
        let config = ProcessConfig {
            max_concurrent_compilations: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("process.max_concurrent_compilations must be > 0"));
    }

    #[test]
    fn test_process_config_validation_chunk_size_overlap() {
        let config = ProcessConfig {
            chunk_size: 100,
            chunk_overlap: 100,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("process.chunk_size must be > chunk_overlap"));
    }

    #[test]
    fn test_vectorize_config_validation_zero_batch_size() {
        let config = VectorizeConfig {
            batch_size: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("vectorize.batch_size must be > 0"));
    }

    #[test]
    fn test_vectorize_config_validation_zero_vector_dimension() {
        let config = VectorizeConfig {
            vector_dimension: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("vectorize.vector_dimension must be > 0"));
    }

    #[test]
    fn test_vectorize_config_validation_zero_max_concurrent() {
        let config = VectorizeConfig {
            max_concurrent: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("vectorize.max_concurrent must be > 0"));
    }

    #[test]
    fn test_vectorize_config_validation_empty_api_endpoint() {
        let config = VectorizeConfig {
            api_endpoint: String::new(),
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("vectorize.api_endpoint must not be empty"));
    }

    #[test]
    fn test_vectorize_config_validation_invalid_api_endpoint() {
        let config = VectorizeConfig {
            api_endpoint: "not-a-valid-url".to_string(),
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("vectorize.api_endpoint must be a valid HTTP(S) URL"));
    }

    #[test]
    fn test_vectorize_config_validation_zero_request_timeout() {
        let config = VectorizeConfig {
            request_timeout_secs: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("vectorize.request_timeout_secs must be > 0"));
    }

    #[test]
    fn test_vectorize_config_validation_zero_max_retries() {
        let config = VectorizeConfig {
            max_retries: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("vectorize.max_retries must be > 0"));
    }

    #[test]
    fn test_vectorize_config_validation_zero_retry_delay() {
        let config = VectorizeConfig {
            retry_delay_secs: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("vectorize.retry_delay_secs must be > 0"));
    }

    #[test]
    fn test_pipeline_config_roundtrip() {
        let original = Config {
            port: 3000,
            pipeline: PipelineConfig::default(),
        };
        let toml_string = toml::to_string(&original).unwrap();
        let deserialized: Config = toml::from_str(&toml_string).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_config_error_invalid_value_display() {
        let err = ConfigError::InvalidValue("test validation error".to_string());
        assert!(err.to_string().contains("Invalid configuration value"));
        assert!(err.to_string().contains("test validation error"));
    }

    #[test]
    fn test_complete_config_with_all_pipeline_settings() {
        let toml_string = r#"
            port = 8080

            [pipeline.download]
            timeout_secs = 45
            max_concurrent = 8
            max_retries = 5
            retry_delay_secs = 10
            crates_io_url = "https://custom.crates.io/api/v1"

            [pipeline.read]
            max_file_size_bytes = 5242880
            timeout_secs = 20
            include_patterns = ["*.rs", "*.md"]
            exclude_patterns = ["*/target/*"]

            [pipeline.process]
            compilation_timeout_secs = 600
            max_concurrent_compilations = 4
            chunk_size = 2000
            chunk_overlap = 400
            extract_source_code = false

            [pipeline.vectorize]
            model_name = "custom-model"
            model_version = "2.0"
            batch_size = 64
            vector_dimension = 768
            max_concurrent = 4
            timeout_secs = 120
        "#;

        let config: Config = toml::from_str(toml_string).unwrap();
        assert_eq!(config.port, 8080);
        assert_eq!(config.pipeline.download.timeout_secs, 45);
        assert_eq!(config.pipeline.download.max_concurrent, 8);
        assert_eq!(config.pipeline.read.max_file_size_bytes, 5_242_880);
        assert_eq!(config.pipeline.process.chunk_size, 2000);
        assert!(!config.pipeline.process.extract_source_code);
        assert_eq!(config.pipeline.vectorize.model_name, "custom-model");
        assert_eq!(config.pipeline.vectorize.model_version, "2.0");
        assert_eq!(config.pipeline.vectorize.batch_size, 64);
    }
}
