use std::env;

/// Unified environment variable configuration.
///
/// This struct provides a single source of truth for all environment
/// variables used to configure the application. All env var parsing
/// happens in one place, making it easy to see what can be configured
/// via environment and ensuring consistent naming.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnvConfig {
    /// CHRISTINA_MAX_INPUT_TOKENS - Maximum tokens for input context
    pub max_input_tokens: Option<u32>,

    /// CHRISTINA_MAX_OUTPUT_TOKENS - Maximum tokens for LLM output
    pub max_output_tokens: Option<u32>,

    /// CHRISTINA_MODEL_PROVIDER - LLM provider (openai, azure)
    pub model_provider: Option<String>,

    /// CHRISTINA_MODEL - Model name/identifier
    pub model: Option<String>,

    /// CHRISTINA_MODEL_API_KEY - API key for the provider
    pub model_api_key: Option<String>,

    /// CHRISTINA_MODEL_API_URL - Custom API endpoint URL
    pub model_api_url: Option<String>,

    /// CHRISTINA_AZURE_API_VERSION - Azure API version
    pub azure_api_version: Option<String>,

    /// CHRISTINA_AZURE_DEPLOYMENT_ID - Azure deployment ID
    pub azure_deployment_id: Option<String>,

    /// CHRISTINA_MODEL_TEMPERATURE - Temperature for LLM sampling (0.0-2.0)
    pub model_temperature: Option<f32>,

    /// CHRISTINA_USE_COMMIT_HISTORY - Whether to include commit history
    pub use_commit_history: Option<bool>,

    /// CHRISTINA_COMMIT_HISTORY_DEPTH - Number of commits to include
    pub commit_history_depth: Option<usize>,

    /// CHRISTINA_DIFF_TOOL - Diff tool to use (delta, diff-so-fancy, etc.)
    pub diff_tool: Option<String>,

    /// CHRISTINA_DIFF_SHOW_PREVIEW - Whether to show diff preview
    pub diff_show_preview: Option<bool>,

    /// CHRISTINA_CONCURRENCY_LIMIT - Max concurrent LLM requests (1-20)
    pub concurrency_limit: Option<u32>,

    /// CHRISTINA_DEBUG - Enable debug mode
    pub debug: Option<bool>,
}

impl EnvConfig {
    /// Load all configuration from environment variables.
    ///
    /// This is the single entry point for environment-based configuration.
    /// All env vars are parsed here with consistent error handling.
    pub fn from_env() -> Self {
        Self {
            max_input_tokens: parse_env_u32("CHRISTINA_MAX_INPUT_TOKENS"),
            max_output_tokens: parse_env_u32("CHRISTINA_MAX_OUTPUT_TOKENS"),
            model_provider: env::var("CHRISTINA_MODEL_PROVIDER").ok(),
            model: env::var("CHRISTINA_MODEL").ok(),
            model_api_key: env::var("CHRISTINA_MODEL_API_KEY").ok(),
            model_api_url: env::var("CHRISTINA_MODEL_API_URL").ok(),
            azure_api_version: env::var("CHRISTINA_AZURE_API_VERSION").ok(),
            azure_deployment_id: env::var("CHRISTINA_AZURE_DEPLOYMENT_ID").ok(),
            model_temperature: parse_env_f32("CHRISTINA_MODEL_TEMPERATURE"),
            use_commit_history: parse_env_bool("CHRISTINA_USE_COMMIT_HISTORY"),
            commit_history_depth: parse_env_usize("CHRISTINA_COMMIT_HISTORY_DEPTH"),
            diff_tool: env::var("CHRISTINA_DIFF_TOOL").ok(),
            diff_show_preview: parse_env_bool("CHRISTINA_DIFF_SHOW_PREVIEW"),
            concurrency_limit: parse_env_u32("CHRISTINA_CONCURRENCY_LIMIT"),
            debug: parse_env_bool("CHRISTINA_DEBUG"),
        }
    }

    /// Check if any environment variables are set.
    pub fn has_any(&self) -> bool {
        self.max_input_tokens.is_some()
            || self.max_output_tokens.is_some()
            || self.model_provider.is_some()
            || self.model.is_some()
            || self.model_api_key.is_some()
            || self.model_api_url.is_some()
            || self.azure_api_version.is_some()
            || self.azure_deployment_id.is_some()
            || self.model_temperature.is_some()
            || self.use_commit_history.is_some()
            || self.commit_history_depth.is_some()
            || self.diff_tool.is_some()
            || self.diff_show_preview.is_some()
            || self.concurrency_limit.is_some()
            || self.debug.is_some()
    }
}

fn parse_env_u32(name: &str) -> Option<u32> {
    env::var(name).ok().and_then(|s| s.parse().ok())
}

fn parse_env_usize(name: &str) -> Option<usize> {
    env::var(name).ok().and_then(|s| s.parse().ok())
}

fn parse_env_f32(name: &str) -> Option<f32> {
    env::var(name).ok().and_then(|s| s.parse().ok())
}

fn parse_env_bool(name: &str) -> Option<bool> {
    env::var(name)
        .ok()
        .map(|s| matches!(s.to_lowercase().as_str(), "true" | "1" | "yes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_bool() {
        // Note: We can't actually set env vars in tests without affecting other tests,
        // so we just test the parsing logic indirectly
        assert_eq!(parse_env_bool("NONEXISTENT"), None);
    }

    #[test]
    fn test_env_config_default() {
        let config = EnvConfig::default();
        assert!(!config.has_any());
    }
}
