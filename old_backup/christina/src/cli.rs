use clap::{Args, Parser, Subcommand};

/// AI-powered commit message generator
#[derive(Parser)]
#[command(
    name = "christina",
    about = "Automated Conventional Commit Generator Powered By LLMs",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Configuration management
    #[command(subcommand)]
    Config(ConfigCommands),
    /// Profile management
    #[command(subcommand)]
    Profile(ProfileCommands),
}

#[derive(Subcommand)]
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
    /// Open configuration TUI
    Tui,
}

#[derive(Subcommand)]
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
        /// Model provider (openai, azure, etc.)
        #[arg(long)]
        provider: Option<String>,
        /// Model name
        #[arg(long)]
        model: Option<String>,
        /// API key
        #[arg(long)]
        api_key: Option<String>,
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
        /// Model provider (openai, azure, etc.)
        #[arg(long)]
        provider: Option<String>,
        /// Model name
        #[arg(long)]
        model: Option<String>,
        /// API key
        #[arg(long)]
        api_key: Option<String>,
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
    /// Open profile management TUI
    Tui,
}

/// Generate commit messages (default command when no subcommand provided)
#[derive(Args)]
pub struct GenerateArgs {
    /// Dry run - generate without committing
    #[arg(long)]
    pub dry_run: bool,
    /// Specify commit message directly
    #[arg(short, long)]
    pub message: Option<String>,
}
