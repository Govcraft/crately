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
}
