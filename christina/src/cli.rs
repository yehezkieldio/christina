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
    Config,
}
