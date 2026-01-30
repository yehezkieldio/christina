use clap::{Parser, Subcommand};

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
    /// Open configuration TUI
    Config,
    /// Generate commit message from staged changes (CLI mode)
    Generate {
        /// Additional context to help generate a better commit message
        #[arg(short, long)]
        context: Option<String>,
    },
}
