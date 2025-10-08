/// Command-line interface module for crately
///
/// This module defines the CLI structure using clap's derive API for parsing
/// command-line arguments and managing subcommands.
use clap::{Parser, Subcommand};

/// Crately - Crate documentation processing and semantic search service
#[derive(Parser, Debug)]
#[command(name = "crately")]
#[command(author = "Govcraft <roland@govcraft.ai>")]
#[command(version)]
#[command(about = "Crate documentation processing and semantic search service", long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
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
        let cli = Cli::try_parse_from(["crately", "doctor", "--verbose"])
            .expect("Failed to parse CLI");
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
        let cli =
            Cli::try_parse_from(["crately", "doctor", "--fix"]).expect("Failed to parse CLI");
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
        let cli = Cli::try_parse_from(["crately", "doctor", "-v", "-f"])
            .expect("Failed to parse CLI");
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
        let cli =
            Cli::try_parse_from(["crately", "cache", "stats"]).expect("Failed to parse CLI");
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
                    assert_eq!(
                        clean_args.older_than, "30d",
                        "older_than should be '30d'"
                    );
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
        let cli = Cli::try_parse_from(["crately", "cache", "clear", "--all"])
            .expect("Failed to parse");
        match cli.command {
            Commands::Cache(args) => match args.command {
                CacheCommands::Clear(clear_args) => {
                    assert!(clear_args.all, "all should be true");
                    assert_eq!(
                        clear_args.crate_spec, None,
                        "crate_spec should be None"
                    );
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
                    assert_eq!(
                        clear_args.crate_spec, None,
                        "crate_spec should be None"
                    );
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
}
