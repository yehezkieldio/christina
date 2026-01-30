use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// AI-powered commit message generator
#[derive(Parser)]
#[command(
    name = "christina",
    about = "Automated Conventional Commit Generator Powered By LLMs",
    version
)]
pub struct Cli {
    /// Maximum tokens for input context
    #[arg(long, env = "CHRISTINA_MAX_INPUT_TOKENS")]
    pub max_input_tokens: Option<u32>,

    /// Maximum tokens for LLM output
    #[arg(long, env = "CHRISTINA_MAX_OUTPUT_TOKENS")]
    pub max_output_tokens: Option<u32>,

    /// LLM provider (openai, azure)
    #[arg(long, env = "CHRISTINA_MODEL_PROVIDER")]
    pub model_provider: Option<String>,

    /// Model name/identifier
    #[arg(long, env = "CHRISTINA_MODEL")]
    pub model: Option<String>,

    /// API key for the provider
    #[arg(long, env = "CHRISTINA_MODEL_API_KEY")]
    pub model_api_key: Option<String>,

    /// Custom API endpoint URL
    #[arg(long, env = "CHRISTINA_MODEL_API_URL")]
    pub model_api_url: Option<String>,

    /// Azure API version
    #[arg(long, env = "CHRISTINA_AZURE_API_VERSION")]
    pub azure_api_version: Option<String>,

    /// Azure deployment ID
    #[arg(long, env = "CHRISTINA_AZURE_DEPLOYMENT_ID")]
    pub azure_deployment_id: Option<String>,

    /// Temperature for LLM sampling (0.0-2.0)
    #[arg(long, env = "CHRISTINA_MODEL_TEMPERATURE")]
    pub model_temperature: Option<f32>,

    /// Whether to include commit history in context
    #[arg(long, env = "CHRISTINA_USE_COMMIT_HISTORY")]
    pub use_commit_history: Option<bool>,

    /// Number of commits to include in history
    #[arg(long, env = "CHRISTINA_COMMIT_HISTORY_DEPTH")]
    pub commit_history_depth: Option<usize>,

    /// Diff tool to use (delta, diff-so-fancy, etc.)
    #[arg(long, env = "CHRISTINA_DIFF_TOOL")]
    pub diff_tool: Option<String>,

    /// Whether to show diff preview
    #[arg(long, env = "CHRISTINA_DIFF_SHOW_PREVIEW")]
    pub diff_show_preview: Option<bool>,

    /// Max concurrent LLM requests (1-20)
    #[arg(long, env = "CHRISTINA_CONCURRENCY_LIMIT")]
    pub concurrency_limit: Option<u32>,

    /// Enable debug mode
    #[arg(long, env = "CHRISTINA_DEBUG")]
    pub debug: Option<bool>,

    /// Path to configuration file
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Open the configuration UI
    Config,
}
