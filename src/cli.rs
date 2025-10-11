/// Command-line interface module for crately
///
/// This module defines the CLI structure using clap's derive API for parsing
/// command-line arguments and managing subcommands.
use clap::{Parser, Subcommand};

/// Color output control
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChoice {
    /// Always use colors
    Always,
    /// Automatically detect color support (default)
    Auto,
    /// Never use colors
    Never,
}

impl std::str::FromStr for ColorChoice {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "always" => Ok(Self::Always),
            "auto" => Ok(Self::Auto),
            "never" => Ok(Self::Never),
            _ => Err(format!(
                "Invalid color choice: '{}'. Valid options: always, auto, never",
                s
            )),
        }
    }
}

impl Default for ColorChoice {
    fn default() -> Self {
        Self::Auto
    }
}

/// Crately - Crate documentation processing and semantic search service
#[derive(Parser, Debug)]
#[command(name = "crately")]
#[command(author = "Govcraft <roland@govcraft.ai>")]
#[command(version)]
#[command(about = "Crate documentation processing and semantic search service", long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Control color output
    ///
    /// Controls when to use colors in terminal output:
    /// - always: Always use colors
    /// - auto: Auto-detect color support (default)
    /// - never: Never use colors
    ///
    /// Color support is auto-detected based on:
    /// - NO_COLOR environment variable (if set, colors are disabled)
    /// - CLICOLOR environment variable (if set to 0, colors are disabled)
    /// - Terminal TTY status (colors disabled if output is piped)
    #[arg(long, value_name = "WHEN", default_value = "auto", global = true)]
    pub color: ColorChoice,

    /// The subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI subcommands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage cached crate data
    ///
    /// Provides operations to inspect, analyze, and clean up cached crate
    /// documentation and metadata including:
    /// - List cached crates with detailed information
    /// - View cache usage statistics and analytics
    /// - Clean old or unused cache entries
    /// - Clear specific crates or all cached data
    Cache(CacheArgs),

    /// Run comprehensive system diagnostics
    ///
    /// Performs health checks on the crately installation including:
    /// - Configuration file validation
    /// - Database connectivity
    /// - File system permissions
    /// - Dependency verification
    /// - Runtime environment checks
    Doctor(DoctorArgs),

    /// Initialize crately configuration
    ///
    /// Runs an interactive wizard to guide through initial setup including:
    /// - OpenAI API key configuration
    /// - Cache configuration
    /// - Test run validation
    /// - Setup completion verification
    Init(InitArgs),

    /// Start the crately server
    ///
    /// Launches the HTTP server to process crate documentation requests
    /// and provide semantic search capabilities.
    Serve,

    /// Process multiple crates in batch
    ///
    /// Accepts multiple crate specifiers and processes them with configurable
    /// parallelism. Can also process all dependencies from a manifest file.
    Batch(BatchArgs),

    /// Pre-populate cache with popular crates
    ///
    /// Warms the cache by processing commonly used crates to improve
    /// query performance for popular dependencies.
    Warm(WarmArgs),
}

/// Arguments for the doctor subcommand
#[derive(Parser, Debug)]
pub struct DoctorArgs {
    /// Show detailed diagnostic information
    ///
    /// When enabled, displays verbose output including:
    /// - Full configuration details
    /// - Detailed error messages and stack traces
    /// - Debug-level diagnostic information
    /// - System resource utilization
    #[arg(short, long)]
    pub verbose: bool,

    /// Attempt automatic fixes for detected issues
    ///
    /// When enabled, the doctor command will attempt to automatically
    /// resolve common problems such as:
    /// - Creating missing configuration files
    /// - Fixing file permissions
    /// - Initializing database schema
    /// - Cleaning up temporary files
    #[arg(short, long)]
    pub fix: bool,
}

/// Arguments for the init subcommand
#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Skip interactive wizard and use defaults
    ///
    /// When enabled, bypasses the interactive setup wizard and initializes
    /// crately with default configuration values. Manual configuration
    /// will be required after initialization.
    #[arg(long)]
    pub no_interactive: bool,
}

/// Arguments for the cache subcommand
#[derive(Parser, Debug)]
pub struct CacheArgs {
    /// The cache operation to perform
    #[command(subcommand)]
    pub command: CacheCommands,

    /// Preview changes without executing them
    ///
    /// When enabled, displays what would be done without actually
    /// performing any modifications to the cache.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Output results in JSON format
    ///
    /// Formats output as structured JSON for programmatic consumption
    /// rather than human-readable text.
    #[arg(long, global = true)]
    pub json: bool,
}

/// Cache management operations
#[derive(Subcommand, Debug)]
pub enum CacheCommands {
    /// List all cached crates with details
    ///
    /// Displays information about cached crates including:
    /// - Crate name and version
    /// - Cache creation date
    /// - Storage size
    /// - Last access time
    List,

    /// Display cache usage statistics and analytics
    ///
    /// Provides comprehensive analytics including:
    /// - Total cache size and entry count
    /// - Storage utilization by crate
    /// - Access patterns and frequency
    /// - Cache hit/miss rates
    Stats,

    /// Remove old or unused cache entries
    ///
    /// Selectively cleans cache entries based on age criteria.
    /// Requires the --older-than flag to specify the retention period.
    Clean(CleanArgs),

    /// Delete specific cache or all cached data
    ///
    /// Removes cache entries either for a specific crate (e.g., tokio@1.35)
    /// or all cached data when used with the --all flag.
    Clear(ClearArgs),
}

/// Arguments for the cache clean subcommand
#[derive(Parser, Debug)]
pub struct CleanArgs {
    /// Remove caches older than the specified duration
    ///
    /// Accepts durations in the format: 30d, 6w, 3m, 1y
    /// Examples:
    /// - 30d: 30 days
    /// - 6w: 6 weeks
    /// - 3m: 3 months
    /// - 1y: 1 year
    #[arg(long, value_name = "DURATION")]
    pub older_than: String,
}

/// Arguments for the cache clear subcommand
#[derive(Parser, Debug)]
pub struct ClearArgs {
    /// Specific crate to clear from cache
    ///
    /// Format: name@version (e.g., serde@1.0.0)
    /// If not specified, requires --all flag.
    #[arg(value_name = "CRATE")]
    pub crate_spec: Option<String>,

    /// Clear all cached data
    ///
    /// When enabled, removes all cached crate data.
    /// Cannot be used together with a specific crate argument.
    #[arg(long, conflicts_with = "crate_spec")]
    pub all: bool,
}

/// Arguments for the batch subcommand
#[derive(Parser, Debug)]
pub struct BatchArgs {
    /// Crate specifiers to process
    ///
    /// Format: name@version (e.g., serde@^1.0 tokio@1.35)
    /// Multiple crates can be specified as space-separated arguments.
    #[arg(value_name = "CRATES")]
    pub crates: Vec<String>,

    /// Process all dependencies from a manifest file
    ///
    /// Reads dependencies from the specified file (e.g., Cargo.toml)
    /// and processes all listed crates. Cannot be used with positional
    /// crate arguments.
    #[arg(long, value_name = "FILE", conflicts_with = "crates")]
    pub from: Option<String>,

    /// Number of concurrent processing tasks
    ///
    /// Controls how many crates are processed in parallel.
    /// Default is determined by system capabilities.
    #[arg(long, value_name = "N")]
    pub parallel: Option<usize>,
}

/// Arguments for the warm subcommand
#[derive(Parser, Debug)]
pub struct WarmArgs {
    /// Warm cache with popular crates
    ///
    /// When enabled, processes a curated list of commonly used crates
    /// to pre-populate the cache for better query performance.
    #[arg(long)]
    pub popular: bool,

    /// Number of concurrent processing tasks
    ///
    /// Controls how many crates are processed in parallel.
    /// Default is determined by system capabilities.
    #[arg(long, value_name = "N")]
    pub parallel: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_doctor_no_flags() {
        let cli = Cli::try_parse_from(["crately", "doctor"]).expect("Failed to parse CLI");
        match cli.command {
            Commands::Doctor(args) => {
                assert!(!args.verbose, "verbose should default to false");
                assert!(!args.fix, "fix should default to false");
            }
            _ => panic!("Expected Doctor command"),
        }
    }

    #[test]
    fn test_cli_doctor_verbose_flag() {
        let cli =
            Cli::try_parse_from(["crately", "doctor", "--verbose"]).expect("Failed to parse CLI");
        match cli.command {
            Commands::Doctor(args) => {
                assert!(args.verbose, "verbose should be true");
                assert!(!args.fix, "fix should be false");
            }
            _ => panic!("Expected Doctor command"),
        }
    }

    #[test]
    fn test_cli_doctor_fix_flag() {
        let cli = Cli::try_parse_from(["crately", "doctor", "--fix"]).expect("Failed to parse CLI");
        match cli.command {
            Commands::Doctor(args) => {
                assert!(!args.verbose, "verbose should be false");
                assert!(args.fix, "fix should be true");
            }
            _ => panic!("Expected Doctor command"),
        }
    }

    #[test]
    fn test_cli_doctor_both_flags() {
        let cli = Cli::try_parse_from(["crately", "doctor", "--verbose", "--fix"])
            .expect("Failed to parse CLI");
        match cli.command {
            Commands::Doctor(args) => {
                assert!(args.verbose, "verbose should be true");
                assert!(args.fix, "fix should be true");
            }
            _ => panic!("Expected Doctor command"),
        }
    }

    #[test]
    fn test_cli_doctor_short_flags() {
        let cli =
            Cli::try_parse_from(["crately", "doctor", "-v", "-f"]).expect("Failed to parse CLI");
        match cli.command {
            Commands::Doctor(args) => {
                assert!(args.verbose, "verbose should be true with -v");
                assert!(args.fix, "fix should be true with -f");
            }
            _ => panic!("Expected Doctor command"),
        }
    }

    #[test]
    fn test_cli_serve_command() {
        let cli = Cli::try_parse_from(["crately", "serve"]).expect("Failed to parse CLI");
        match cli.command {
            Commands::Serve => {
                // Successfully parsed serve command
            }
            _ => panic!("Expected Serve command"),
        }
    }

    #[test]
    fn test_cli_version_flag() {
        // Version flag should cause parse to fail with exit, which is expected behavior
        // We just verify it doesn't panic
        let result = Cli::try_parse_from(["crately", "--version"]);
        assert!(result.is_err(), "Version flag should cause early exit");
    }

    #[test]
    fn test_cli_help_flag() {
        // Help flag should cause parse to fail with exit, which is expected behavior
        let result = Cli::try_parse_from(["crately", "--help"]);
        assert!(result.is_err(), "Help flag should cause early exit");
    }

    #[test]
    fn test_cli_init_no_flags() {
        let cli = Cli::try_parse_from(["crately", "init"]).expect("Failed to parse CLI");
        match cli.command {
            Commands::Init(args) => {
                assert!(
                    !args.no_interactive,
                    "no_interactive should default to false"
                );
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_cli_init_with_no_interactive_flag() {
        let cli = Cli::try_parse_from(["crately", "init", "--no-interactive"])
            .expect("Failed to parse CLI");
        match cli.command {
            Commands::Init(args) => {
                assert!(args.no_interactive, "no_interactive should be true");
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_cache_list_command() {
        let cli = Cli::try_parse_from(["crately", "cache", "list"]).expect("Failed to parse CLI");
        match cli.command {
            Commands::Cache(args) => {
                assert!(!args.dry_run, "dry_run should default to false");
                assert!(!args.json, "json should default to false");
                match args.command {
                    CacheCommands::List => {}
                    _ => panic!("Expected List subcommand"),
                }
            }
            _ => panic!("Expected Cache command"),
        }
    }

    #[test]
    fn test_cache_list_with_json_flag() {
        let cli =
            Cli::try_parse_from(["crately", "cache", "--json", "list"]).expect("Failed to parse");
        match cli.command {
            Commands::Cache(args) => {
                assert!(args.json, "json should be true");
                assert!(!args.dry_run, "dry_run should be false");
            }
            _ => panic!("Expected Cache command"),
        }
    }

    #[test]
    fn test_cache_list_with_dry_run_flag() {
        let cli = Cli::try_parse_from(["crately", "cache", "--dry-run", "list"])
            .expect("Failed to parse");
        match cli.command {
            Commands::Cache(args) => {
                assert!(args.dry_run, "dry_run should be true");
                assert!(!args.json, "json should be false");
            }
            _ => panic!("Expected Cache command"),
        }
    }

    #[test]
    fn test_cache_stats_command() {
        let cli = Cli::try_parse_from(["crately", "cache", "stats"]).expect("Failed to parse CLI");
        match cli.command {
            Commands::Cache(args) => match args.command {
                CacheCommands::Stats => {}
                _ => panic!("Expected Stats subcommand"),
            },
            _ => panic!("Expected Cache command"),
        }
    }

    #[test]
    fn test_cache_clean_with_older_than() {
        let cli = Cli::try_parse_from(["crately", "cache", "clean", "--older-than", "30d"])
            .expect("Failed to parse");
        match cli.command {
            Commands::Cache(args) => match args.command {
                CacheCommands::Clean(clean_args) => {
                    assert_eq!(clean_args.older_than, "30d", "older_than should be '30d'");
                }
                _ => panic!("Expected Clean subcommand"),
            },
            _ => panic!("Expected Cache command"),
        }
    }

    #[test]
    fn test_cache_clean_requires_older_than() {
        let result = Cli::try_parse_from(["crately", "cache", "clean"]);
        assert!(
            result.is_err(),
            "Clean command should require --older-than flag"
        );
    }

    #[test]
    fn test_cache_clear_with_crate_spec() {
        let cli = Cli::try_parse_from(["crately", "cache", "clear", "serde@1.0.0"])
            .expect("Failed to parse");
        match cli.command {
            Commands::Cache(args) => match args.command {
                CacheCommands::Clear(clear_args) => {
                    assert_eq!(
                        clear_args.crate_spec,
                        Some("serde@1.0.0".to_string()),
                        "crate_spec should be 'serde@1.0.0'"
                    );
                    assert!(!clear_args.all, "all should be false");
                }
                _ => panic!("Expected Clear subcommand"),
            },
            _ => panic!("Expected Cache command"),
        }
    }

    #[test]
    fn test_cache_clear_with_all_flag() {
        let cli =
            Cli::try_parse_from(["crately", "cache", "clear", "--all"]).expect("Failed to parse");
        match cli.command {
            Commands::Cache(args) => match args.command {
                CacheCommands::Clear(clear_args) => {
                    assert!(clear_args.all, "all should be true");
                    assert_eq!(clear_args.crate_spec, None, "crate_spec should be None");
                }
                _ => panic!("Expected Clear subcommand"),
            },
            _ => panic!("Expected Cache command"),
        }
    }

    #[test]
    fn test_cache_clear_rejects_both_crate_and_all() {
        let result = Cli::try_parse_from(["crately", "cache", "clear", "serde@1.0.0", "--all"]);
        assert!(
            result.is_err(),
            "Clear command should reject both crate spec and --all"
        );
    }

    #[test]
    fn test_cache_clear_requires_crate_or_all() {
        let cli = Cli::try_parse_from(["crately", "cache", "clear"]).expect("Failed to parse");
        match cli.command {
            Commands::Cache(args) => match args.command {
                CacheCommands::Clear(clear_args) => {
                    assert_eq!(clear_args.crate_spec, None, "crate_spec should be None");
                    assert!(!clear_args.all, "all should be false");
                }
                _ => panic!("Expected Clear subcommand"),
            },
            _ => panic!("Expected Cache command"),
        }
    }

    #[test]
    fn test_cache_with_both_global_flags() {
        let cli = Cli::try_parse_from(["crately", "cache", "--dry-run", "--json", "list"])
            .expect("Failed to parse");
        match cli.command {
            Commands::Cache(args) => {
                assert!(args.dry_run, "dry_run should be true");
                assert!(args.json, "json should be true");
            }
            _ => panic!("Expected Cache command"),
        }
    }

    #[test]
    fn test_cache_clean_with_global_flags() {
        let cli = Cli::try_parse_from([
            "crately",
            "cache",
            "--dry-run",
            "--json",
            "clean",
            "--older-than",
            "1y",
        ])
        .expect("Failed to parse");
        match cli.command {
            Commands::Cache(args) => {
                assert!(args.dry_run, "dry_run should be true");
                assert!(args.json, "json should be true");
                match args.command {
                    CacheCommands::Clean(clean_args) => {
                        assert_eq!(clean_args.older_than, "1y", "older_than should be '1y'");
                    }
                    _ => panic!("Expected Clean subcommand"),
                }
            }
            _ => panic!("Expected Cache command"),
        }
    }

    #[test]
    fn test_batch_with_multiple_crates() {
        let cli = Cli::try_parse_from([
            "crately",
            "batch",
            "serde@^1.0",
            "tokio@1.35",
            "actix-web@4.4",
        ])
        .expect("Failed to parse");
        match cli.command {
            Commands::Batch(args) => {
                assert_eq!(args.crates.len(), 3, "should have 3 crate specifiers");
                assert_eq!(args.crates[0], "serde@^1.0");
                assert_eq!(args.crates[1], "tokio@1.35");
                assert_eq!(args.crates[2], "actix-web@4.4");
                assert_eq!(args.from, None, "from should be None");
                assert_eq!(args.parallel, None, "parallel should be None");
            }
            _ => panic!("Expected Batch command"),
        }
    }

    #[test]
    fn test_batch_with_single_crate() {
        let cli =
            Cli::try_parse_from(["crately", "batch", "serde@1.0.0"]).expect("Failed to parse");
        match cli.command {
            Commands::Batch(args) => {
                assert_eq!(args.crates.len(), 1, "should have 1 crate specifier");
                assert_eq!(args.crates[0], "serde@1.0.0");
            }
            _ => panic!("Expected Batch command"),
        }
    }

    #[test]
    fn test_batch_with_from_flag() {
        let cli = Cli::try_parse_from(["crately", "batch", "--from", "Cargo.toml"])
            .expect("Failed to parse");
        match cli.command {
            Commands::Batch(args) => {
                assert_eq!(args.crates.len(), 0, "crates should be empty");
                assert_eq!(
                    args.from,
                    Some("Cargo.toml".to_string()),
                    "from should be Cargo.toml"
                );
                assert_eq!(args.parallel, None, "parallel should be None");
            }
            _ => panic!("Expected Batch command"),
        }
    }

    #[test]
    fn test_batch_with_parallel_flag() {
        let cli = Cli::try_parse_from(["crately", "batch", "--parallel", "4", "serde@1.0.0"])
            .expect("Failed to parse");
        match cli.command {
            Commands::Batch(args) => {
                assert_eq!(args.crates.len(), 1);
                assert_eq!(args.parallel, Some(4), "parallel should be 4");
            }
            _ => panic!("Expected Batch command"),
        }
    }

    #[test]
    fn test_batch_with_from_and_parallel() {
        let cli = Cli::try_parse_from([
            "crately",
            "batch",
            "--from",
            "Cargo.toml",
            "--parallel",
            "8",
        ])
        .expect("Failed to parse");
        match cli.command {
            Commands::Batch(args) => {
                assert_eq!(
                    args.from,
                    Some("Cargo.toml".to_string()),
                    "from should be Cargo.toml"
                );
                assert_eq!(args.parallel, Some(8), "parallel should be 8");
            }
            _ => panic!("Expected Batch command"),
        }
    }

    #[test]
    fn test_batch_rejects_crates_and_from() {
        let result =
            Cli::try_parse_from(["crately", "batch", "serde@1.0.0", "--from", "Cargo.toml"]);
        assert!(
            result.is_err(),
            "Batch should reject both crates and --from"
        );
    }

    #[test]
    fn test_batch_allows_no_arguments() {
        let cli = Cli::try_parse_from(["crately", "batch"]).expect("Failed to parse");
        match cli.command {
            Commands::Batch(args) => {
                assert_eq!(args.crates.len(), 0, "crates should be empty");
                assert_eq!(args.from, None, "from should be None");
                assert_eq!(args.parallel, None, "parallel should be None");
            }
            _ => panic!("Expected Batch command"),
        }
    }

    #[test]
    fn test_warm_with_popular_flag() {
        let cli = Cli::try_parse_from(["crately", "warm", "--popular"]).expect("Failed to parse");
        match cli.command {
            Commands::Warm(args) => {
                assert!(args.popular, "popular should be true");
                assert_eq!(args.parallel, None, "parallel should be None");
            }
            _ => panic!("Expected Warm command"),
        }
    }

    #[test]
    fn test_warm_with_parallel_flag() {
        let cli =
            Cli::try_parse_from(["crately", "warm", "--parallel", "6"]).expect("Failed to parse");
        match cli.command {
            Commands::Warm(args) => {
                assert!(!args.popular, "popular should be false");
                assert_eq!(args.parallel, Some(6), "parallel should be 6");
            }
            _ => panic!("Expected Warm command"),
        }
    }

    #[test]
    fn test_warm_with_both_flags() {
        let cli = Cli::try_parse_from(["crately", "warm", "--popular", "--parallel", "12"])
            .expect("Failed to parse");
        match cli.command {
            Commands::Warm(args) => {
                assert!(args.popular, "popular should be true");
                assert_eq!(args.parallel, Some(12), "parallel should be 12");
            }
            _ => panic!("Expected Warm command"),
        }
    }

    #[test]
    fn test_warm_with_no_flags() {
        let cli = Cli::try_parse_from(["crately", "warm"]).expect("Failed to parse");
        match cli.command {
            Commands::Warm(args) => {
                assert!(!args.popular, "popular should default to false");
                assert_eq!(args.parallel, None, "parallel should be None");
            }
            _ => panic!("Expected Warm command"),
        }
    }

    #[test]
    fn test_batch_parallel_accepts_positive_numbers() {
        let cli = Cli::try_parse_from(["crately", "batch", "--parallel", "100"])
            .expect("Failed to parse");
        match cli.command {
            Commands::Batch(args) => {
                assert_eq!(args.parallel, Some(100), "parallel should be 100");
            }
            _ => panic!("Expected Batch command"),
        }
    }

    #[test]
    fn test_warm_parallel_accepts_positive_numbers() {
        let cli =
            Cli::try_parse_from(["crately", "warm", "--parallel", "50"]).expect("Failed to parse");
        match cli.command {
            Commands::Warm(args) => {
                assert_eq!(args.parallel, Some(50), "parallel should be 50");
            }
            _ => panic!("Expected Warm command"),
        }
    }

    #[test]
    fn test_cli_color_flag_default() {
        let cli = Cli::try_parse_from(["crately", "serve"]).expect("Failed to parse");
        assert_eq!(
            cli.color,
            ColorChoice::Auto,
            "color should default to Auto"
        );
    }

    #[test]
    fn test_cli_color_flag_always() {
        let cli =
            Cli::try_parse_from(["crately", "--color", "always", "serve"]).expect("Failed to parse");
        assert_eq!(
            cli.color,
            ColorChoice::Always,
            "color should be Always when flag is set"
        );
    }

    #[test]
    fn test_cli_color_flag_never() {
        let cli =
            Cli::try_parse_from(["crately", "--color", "never", "serve"]).expect("Failed to parse");
        assert_eq!(
            cli.color,
            ColorChoice::Never,
            "color should be Never when flag is set"
        );
    }

    #[test]
    fn test_cli_color_flag_auto() {
        let cli =
            Cli::try_parse_from(["crately", "--color", "auto", "serve"]).expect("Failed to parse");
        assert_eq!(
            cli.color,
            ColorChoice::Auto,
            "color should be Auto when flag is set"
        );
    }

    #[test]
    fn test_cli_color_flag_with_other_commands() {
        let cli = Cli::try_parse_from(["crately", "--color", "never", "doctor"])
            .expect("Failed to parse");
        assert_eq!(
            cli.color,
            ColorChoice::Never,
            "color should work with any command"
        );
        match cli.command {
            Commands::Doctor(_) => {}
            _ => panic!("Expected Doctor command"),
        }
    }

    #[test]
    fn test_cli_color_flag_case_insensitive() {
        let cli =
            Cli::try_parse_from(["crately", "--color", "ALWAYS", "serve"]).expect("Failed to parse");
        assert_eq!(
            cli.color,
            ColorChoice::Always,
            "color flag should be case insensitive"
        );
    }

    #[test]
    fn test_cli_color_flag_invalid_value() {
        let result = Cli::try_parse_from(["crately", "--color", "invalid", "serve"]);
        assert!(result.is_err(), "Invalid color value should cause parse error");
    }

    #[test]
    fn test_color_choice_from_str() {
        assert_eq!("always".parse::<ColorChoice>().unwrap(), ColorChoice::Always);
        assert_eq!("auto".parse::<ColorChoice>().unwrap(), ColorChoice::Auto);
        assert_eq!("never".parse::<ColorChoice>().unwrap(), ColorChoice::Never);
        assert_eq!("ALWAYS".parse::<ColorChoice>().unwrap(), ColorChoice::Always);
        assert!("invalid".parse::<ColorChoice>().is_err());
    }
}
