//! CLI argument parsing for the christina binary.
//!
//! WHY centralized: all command shapes live here so help text stays consistent
//! and subcommands can be tested without instantiating runtime dependencies.

use clap::{Parser, Subcommand};
use clap_complete::Shell;

pub mod commit;
pub mod config;
pub mod profile;

/// AI-powered commit message generator
#[derive(Parser)]
#[command(
    name = "christina",
    about = "Automated Conventional Commit Generator Powered By LLMs",
    version
)]
pub struct Cli {
    /// Increase logging verbosity (-v, -vv, -vvv, etc.)
    #[arg(short, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Enable full pipeline tracing with detailed telemetry output
    #[arg(long)]
    pub trace: bool,

    /// Skip interactive confirmations (non-interactive mode)
    #[arg(long)]
    pub yes: bool,

    /// Additional user-provided context appended to prompts
    #[arg(short, long)]
    pub context: Option<String>,

    /// Generate commit message without creating the commit (preview mode)
    #[arg(long)]
    pub dry_run: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Commands {
    /// Configuration management
    #[command(subcommand)]
    Config(ConfigCommands),
    /// Profile management
    #[command(subcommand)]
    Profile(ProfileCommands),
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum ConfigCommands {
    /// Get a configuration value
    Get {
        /// Configuration key to retrieve
        key: String,
    },
    /// Set a configuration value
    Set {
        /// Configuration key to set
        key: String,
        /// Value to set
        value: String,
    },
    /// List all configuration values
    List,
    /// Show configuration file path
    Path,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum ProfileCommands {
    /// List all profiles
    List,
    /// Show profile details
    Show {
        /// Profile name
        name: String,
    },
    /// Create a new profile
    Create {
        /// Profile name
        name: String,
        /// Model provider (azure, etc.)
        #[arg(long)]
        provider: Option<String>,
        /// Model name
        #[arg(long)]
        model: Option<String>,
        /// API key
        #[arg(long)]
        api_key: Option<String>,
        /// Allow storing plaintext API keys in config
        #[arg(long)]
        allow_plaintext: bool,
        /// API URL
        #[arg(long)]
        api_url: Option<String>,
        /// Max input tokens
        #[arg(long)]
        max_input_tokens: Option<usize>,
        /// Max output tokens
        #[arg(long)]
        max_output_tokens: Option<usize>,
        /// Azure API version
        #[arg(long)]
        azure_api_version: Option<String>,
        /// Azure deployment ID
        #[arg(long)]
        azure_deployment_id: Option<String>,
    },
    /// Edit a profile
    Edit {
        /// Profile name
        name: String,
        /// Model provider (azure, etc.)
        #[arg(long)]
        provider: Option<String>,
        /// Model name
        #[arg(long)]
        model: Option<String>,
        /// API key
        #[arg(long)]
        api_key: Option<String>,
        /// Allow storing plaintext API keys in config
        #[arg(long)]
        allow_plaintext: bool,
        /// API URL
        #[arg(long)]
        api_url: Option<String>,
        /// Max input tokens
        #[arg(long)]
        max_input_tokens: Option<usize>,
        /// Max output tokens
        #[arg(long)]
        max_output_tokens: Option<usize>,
        /// Azure API version
        #[arg(long)]
        azure_api_version: Option<String>,
        /// Azure deployment ID
        #[arg(long)]
        azure_deployment_id: Option<String>,
    },
    /// Delete a profile
    Delete {
        /// Profile name
        name: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
    /// Switch active profile
    Switch {
        /// Profile name
        name: String,
    },
    /// Duplicate a profile
    Duplicate {
        /// Source profile name
        source: String,
        /// New profile name
        new_name: String,
    },
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_default_no_command() {
        let cli = Cli::try_parse_from(["christina"]).expect("Failed to parse CLI");
        assert_eq!(cli.command, None, "Should have no command");
        assert_eq!(cli.verbose, 0, "Verbose count should be 0");
    }

    #[test]
    fn test_cli_verbose_single() {
        let cli = Cli::try_parse_from(["christina", "-v"]).expect("Failed to parse CLI");
        assert_eq!(cli.verbose, 1, "Verbose count should be 1");
        assert_eq!(cli.command, None);
    }

    #[test]
    fn test_cli_verbose_double() {
        let cli = Cli::try_parse_from(["christina", "-vv"]).expect("Failed to parse CLI");
        assert_eq!(cli.verbose, 2, "Verbose count should be 2");
        assert_eq!(cli.command, None);
    }

    #[test]
    fn test_cli_verbose_triple() {
        let cli = Cli::try_parse_from(["christina", "-vvv"]).expect("Failed to parse CLI");
        assert_eq!(cli.verbose, 3, "Verbose count should be 3");
        assert_eq!(cli.command, None);
    }

    #[test]
    fn test_cli_verbose_separate_flags() {
        let cli = Cli::try_parse_from(["christina", "-v", "-v"]).expect("Failed to parse CLI");
        assert_eq!(
            cli.verbose, 2,
            "Verbose count should be 2 when flags are separate"
        );
        assert_eq!(cli.command, None);
    }

    #[test]
    fn test_cli_verbose_with_command() {
        let cli = Cli::try_parse_from(["christina", "-v", "config", "list"])
            .expect("Failed to parse CLI");
        assert_eq!(cli.verbose, 1, "Verbose count should be 1");
        assert!(
            matches!(cli.command, Some(Commands::Config(_))),
            "Should have config command"
        );
    }

    #[test]
    fn test_cli_trace_flag() {
        let cli = Cli::try_parse_from(["christina", "--trace"]).expect("Failed to parse CLI");
        assert!(cli.trace, "Trace flag should be enabled");
        assert_eq!(cli.verbose, 0, "Verbose count should be 0 by default");
    }

    #[test]
    fn test_subcommand_config_list() {
        let cli =
            Cli::try_parse_from(["christina", "config", "list"]).expect("Failed to parse CLI");
        assert!(
            matches!(cli.command, Some(Commands::Config(ConfigCommands::List))),
            "Should parse config list subcommand"
        );
    }

    #[test]
    fn test_subcommand_config_get() {
        let cli = Cli::try_parse_from(["christina", "config", "get", "theme"])
            .expect("Failed to parse CLI");
        match cli.command {
            Some(Commands::Config(ConfigCommands::Get { key })) => {
                assert_eq!(key, "theme");
            }
            _ => panic!("Expected config get command"),
        }
    }

    #[test]
    fn test_subcommand_config_set() {
        let cli = Cli::try_parse_from(["christina", "config", "set", "theme", "dark"])
            .expect("Failed to parse CLI");
        match cli.command {
            Some(Commands::Config(ConfigCommands::Set { key, value })) => {
                assert_eq!(key, "theme");
                assert_eq!(value, "dark");
            }
            _ => panic!("Expected config set command"),
        }
    }

    #[test]
    fn test_subcommand_config_path() {
        let cli =
            Cli::try_parse_from(["christina", "config", "path"]).expect("Failed to parse CLI");
        assert!(
            matches!(cli.command, Some(Commands::Config(ConfigCommands::Path))),
            "Should parse config path subcommand"
        );
    }

    #[test]
    fn test_subcommand_profile_list() {
        let cli =
            Cli::try_parse_from(["christina", "profile", "list"]).expect("Failed to parse CLI");
        assert!(
            matches!(cli.command, Some(Commands::Profile(ProfileCommands::List))),
            "Should parse profile list subcommand"
        );
    }

    #[test]
    fn test_subcommand_profile_show() {
        let cli = Cli::try_parse_from(["christina", "profile", "show", "default"])
            .expect("Failed to parse CLI");
        match cli.command {
            Some(Commands::Profile(ProfileCommands::Show { name })) => {
                assert_eq!(name, "default");
            }
            _ => panic!("Expected profile show command"),
        }
    }

    #[test]
    fn test_subcommand_profile_create() {
        let cli = Cli::try_parse_from(["christina", "profile", "create", "my-profile"])
            .expect("Failed to parse CLI");
        match cli.command {
            Some(Commands::Profile(ProfileCommands::Create {
                name,
                provider: None,
                ..
            })) => {
                assert_eq!(name, "my-profile");
            }
            _ => panic!("Expected profile create command"),
        }
    }

    #[test]
    fn test_subcommand_profile_create_with_options() {
        let cli = Cli::try_parse_from([
            "christina",
            "profile",
            "create",
            "my-profile",
            "--provider",
            "azure",
            "--model",
            "gpt-4o",
        ])
        .expect("Failed to parse CLI");

        match cli.command {
            Some(Commands::Profile(ProfileCommands::Create {
                name,
                provider,
                model,
                ..
            })) => {
                assert_eq!(name, "my-profile");
                assert_eq!(provider, Some("azure".to_string()));
                assert_eq!(model, Some("gpt-4o".to_string()));
            }
            _ => panic!("Expected profile create command"),
        }
    }

    #[test]
    fn test_subcommand_profile_edit() {
        let cli = Cli::try_parse_from(["christina", "profile", "edit", "default"])
            .expect("Failed to parse CLI");
        match cli.command {
            Some(Commands::Profile(ProfileCommands::Edit { name, .. })) => {
                assert_eq!(name, "default");
            }
            _ => panic!("Expected profile edit command"),
        }
    }

    #[test]
    fn test_subcommand_profile_delete() {
        let cli = Cli::try_parse_from(["christina", "profile", "delete", "old-profile"])
            .expect("Failed to parse CLI");
        match cli.command {
            Some(Commands::Profile(ProfileCommands::Delete { name, force: false })) => {
                assert_eq!(name, "old-profile");
            }
            _ => panic!("Expected profile delete command"),
        }
    }

    #[test]
    fn test_subcommand_profile_delete_force() {
        let cli = Cli::try_parse_from(["christina", "profile", "delete", "old-profile", "--force"])
            .expect("Failed to parse CLI");
        match cli.command {
            Some(Commands::Profile(ProfileCommands::Delete { name, force: true })) => {
                assert_eq!(name, "old-profile");
            }
            _ => panic!("Expected profile delete command with force flag"),
        }
    }

    #[test]
    fn test_subcommand_profile_switch() {
        let cli = Cli::try_parse_from(["christina", "profile", "switch", "production"])
            .expect("Failed to parse CLI");
        match cli.command {
            Some(Commands::Profile(ProfileCommands::Switch { name })) => {
                assert_eq!(name, "production");
            }
            _ => panic!("Expected profile switch command"),
        }
    }

    #[test]
    fn test_subcommand_profile_duplicate() {
        let cli = Cli::try_parse_from(["christina", "profile", "duplicate", "source", "new"])
            .expect("Failed to parse CLI");
        match cli.command {
            Some(Commands::Profile(ProfileCommands::Duplicate { source, new_name })) => {
                assert_eq!(source, "source");
                assert_eq!(new_name, "new");
            }
            _ => panic!("Expected profile duplicate command"),
        }
    }

    #[test]
    fn test_invalid_subcommand() {
        let result = Cli::try_parse_from(["christina", "invalid"]);
        assert!(result.is_err(), "Should error on invalid subcommand");
    }

    #[test]
    fn test_config_get_missing_key() {
        let result = Cli::try_parse_from(["christina", "config", "get"]);
        assert!(
            result.is_err(),
            "Should error when config get key is missing"
        );
    }

    #[test]
    fn test_config_set_missing_value() {
        let result = Cli::try_parse_from(["christina", "config", "set", "key"]);
        assert!(
            result.is_err(),
            "Should error when config set value is missing"
        );
    }

    #[test]
    fn test_profile_create_missing_name() {
        let result = Cli::try_parse_from(["christina", "profile", "create"]);
        assert!(result.is_err(), "Should error when profile name is missing");
    }

    #[test]
    fn test_verbose_with_config_get() {
        let cli = Cli::try_parse_from(["christina", "-vv", "config", "get", "key"])
            .expect("Failed to parse CLI");
        assert_eq!(cli.verbose, 2);
        match cli.command {
            Some(Commands::Config(ConfigCommands::Get { key })) => {
                assert_eq!(key, "key");
            }
            _ => panic!("Expected config get command"),
        }
    }
}
