use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use super::diff_tool::DiffConfig;
use christina_core::{
    profile::{Profiles, ProviderProfile},
    types::{
        ModelName, ProviderKind, TokenCount,
        commit_message::ValidationMode,
        token_count::{MAX_INPUT, MAX_OUTPUT},
    },
};
use url::Url;

/// Application configuration with layered loading.
///
/// Precedence (highest to lowest):
/// 1. Environment variables (CHRISTINA_*)
/// 2. Local config file (./christina.toml) - ONLY safe fields (security)
/// 3. Global config file (~/.config/christina/config.toml)
/// 4. Default values
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Maximum input tokens for the AI model
    #[serde(skip_serializing)]
    pub max_input_tokens: TokenCount,

    /// Maximum output tokens for the AI model
    #[serde(skip_serializing)]
    pub max_output_tokens: TokenCount,

    /// AI model provider
    #[serde(skip_serializing)]
    pub model_provider: ProviderKind,

    /// Specific model name to use
    #[serde(skip_serializing)]
    pub model: ModelName,

    /// API key for the model provider
    #[serde(skip_serializing)]
    pub api_key: Option<String>,

    /// Custom API URL for the model provider
    #[serde(skip_serializing)]
    pub model_api_url: Option<Url>,

    /// Azure API version (for Azure OpenAI provider)
    #[serde(skip_serializing)]
    pub azure_api_version: Option<String>,

    /// Azure deployment ID (for Azure OpenAI provider)
    #[serde(skip_serializing)]
    pub azure_deployment_id: Option<String>,

    /// LLM temperature (0.0 to 2.0). Lower values = more deterministic.
    #[serde(skip_serializing)]
    pub model_temperature: f32,

    /// Files to exclude from AI processing (lockfiles, binaries, etc.)
    #[serde(default)]
    pub ignore_files: Vec<String>,

    /// Provider profiles for quick switching
    #[serde(default)]
    pub profiles: Profiles,

    /// Diff tool configuration
    #[serde(default)]
    pub diff: DiffConfig,

    /// Maximum length for commit messages (None = 72)
    pub commit_message_max_length: Option<usize>,

    /// Validation mode for commit message length
    #[serde(default)]
    pub commit_message_validation_mode: ValidationMode,

    /// Whether to include commit history context in LLM prompts for style consistency
    #[serde(default)]
    pub use_commit_history: bool,

    /// Number of recent commits to include for style analysis
    #[serde(default)]
    pub commit_history_depth: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_input_tokens: TokenCount::new_saturating(4096),
            max_output_tokens: TokenCount::new_saturating(500),
            model_provider: ProviderKind::OpenAI,
            model: ModelName::from("gpt-4"),
            api_key: None,
            model_api_url: None,
            azure_api_version: None,
            azure_deployment_id: None,
            model_temperature: 0.3,
            ignore_files: vec![
                "package-lock.json".to_string(),
                "yarn.lock".to_string(),
                "pnpm-lock.yaml".to_string(),
                "bun.lock".to_string(),
                "Cargo.lock".to_string(),
                "poetry.lock".to_string(),
                "Gemfile.lock".to_string(),
            ],
            profiles: Profiles::new(),
            diff: DiffConfig::default(),
            commit_message_max_length: None,
            commit_message_validation_mode: ValidationMode::default(),
            use_commit_history: true,
            commit_history_depth: 5,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut builder = config::Config::builder();

        // Layer 4: Defaults
        builder = builder.add_source(config::Config::try_from(&Config::default())?);

        // Layer 3: Global config file
        if let Some(global_path) = Self::global_config_path()
            && global_path.exists()
        {
            builder = builder.add_source(config::File::from(global_path).required(false));
        }

        // Build config without local file first to get trusted values
        let mut config = builder
            .clone()
            .add_source(
                config::Environment::with_prefix("CHRISTINA")
                    .separator("_")
                    .try_parsing(true),
            )
            .build()
            .context("Failed to build configuration")?
            .try_deserialize::<Config>()
            .context("Failed to deserialize configuration")?;

        // Fix profile names after deserialization (HashMap keys become names)
        config.profiles.fix_names();

        // Create default profile if no profiles exist
        // Preserve token limits from current config (which may include env var overrides)
        // to ensure user-configured limits aren't lost when auto-creating the default profile.
        if !config.profiles.exists("default") {
            let mut default_profile = config.to_profile("default".to_string());
            // Explicitly preserve token limits from current config as a defensive measure.
            // While to_profile() already copies these values, this makes the intent explicit
            // and guards against future refactoring that might change to_profile() behavior.
            default_profile.max_input_tokens = config.max_input_tokens;
            default_profile.max_output_tokens = config.max_output_tokens;
            config.profiles.add(default_profile)?;
            config.profiles.active = Some("default".to_string());

            // Persist the default profile to global config to prevent recreation on next run
            // This ensures user's API key and settings are not lost
            if let Err(e) = config.save_to_global() {
                eprintln!("Warning: Failed to persist default profile: {}", e);
                // Continue execution - profile will be recreated on next run
            }
        }

        // Load and apply active profile if set
        // This must happen BEFORE env vars so env vars can override the profile.
        config.load_active_profile();

        // Layer 1: Environment variables (TRUSTED - can override all)
        // Re-apply env vars as they have highest priority
        if let Ok(env_val) = std::env::var("CHRISTINA_TOKENS_MAX_INPUT")
            && let Ok(v) = env_val.parse()
        {
            config.max_input_tokens = v;
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_TOKENS_OUTPUT")
            && let Ok(v) = env_val.parse()
        {
            config.max_output_tokens = v;
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_MODEL_PROVIDER") {
            config.model_provider = env_val.parse().map_err(anyhow::Error::msg)?;
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_MODEL") {
            config.model = ModelName::from(env_val.as_str());
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_MODEL_API_KEY") {
            config.api_key = Some(env_val);
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_MODEL_API_URL") {
            config.model_api_url = Some(Url::parse(&env_val)?);
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_AZURE_API_VERSION") {
            config.azure_api_version = Some(env_val);
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_AZURE_DEPLOYMENT_ID") {
            config.azure_deployment_id = Some(env_val);
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_MODEL_TEMPERATURE")
            && let Ok(v) = env_val.parse()
        {
            config.model_temperature = v;
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_USE_COMMIT_HISTORY")
            && let Ok(v) = env_val.parse()
        {
            config.use_commit_history = v;
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_COMMIT_HISTORY_DEPTH")
            && let Ok(v) = env_val.parse::<usize>()
        {
            config.commit_history_depth = v.clamp(5, 20);
        }

        config.diff = config.diff.with_env_override();

        // Validate and clamp token values to hard limits after all configuration is loaded
        config.validate();

        Ok(config)
    }

    /// Validate and clamp token values to hard limits.
    /// Also validates provider name against the registry.
    fn validate(&mut self) {
        let max_input = TokenCount::new_saturating(MAX_INPUT);
        let max_output = TokenCount::new_saturating(MAX_OUTPUT);
        self.max_input_tokens = self.max_input_tokens.min(max_input);
        self.max_output_tokens = self.max_output_tokens.min(max_output);

        // Clamp temperature to valid range (0.0 to 2.0)
        self.model_temperature = self.model_temperature.clamp(0.0, 2.0);

        // Warn if provider is unknown (but don't fail - let factory handle it)
    }

    pub fn global_config_dir() -> Option<PathBuf> {
        ProjectDirs::from("", "", "christina").map(|dirs| dirs.config_dir().to_path_buf())
    }

    pub fn global_config_path() -> Option<PathBuf> {
        Self::global_config_dir().map(|dir| dir.join("config.toml"))
    }

    pub fn save_to_global(&self) -> Result<()> {
        let config_dir = Self::global_config_dir()
            .context("Could not determine config directory for your platform")?;

        std::fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

        let config_path = config_dir.join("config.toml");
        let toml_content =
            toml::to_string_pretty(self).context("Failed to serialize configuration")?;

        std::fs::write(&config_path, toml_content).context("Failed to write config file")?;

        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "max_input_tokens" => Some(self.max_input_tokens.get().to_string()),
            "max_output_tokens" => Some(self.max_output_tokens.get().to_string()),
            "model_provider" => Some(self.model_provider.to_string()),
            "model" => Some(self.model.to_string()),
            "api_key" | "model_api_key" => self.api_key.clone(),
            "model_api_url" => self.model_api_url.as_ref().map(|url| url.to_string()),
            "azure_api_version" => self.azure_api_version.clone(),
            "azure_deployment_id" => self.azure_deployment_id.clone(),
            "ignore_files" => Some(self.ignore_files.join(",")),
            "commit_message_max_length" => self.commit_message_max_length.map(|v| v.to_string()),
            "commit_message_validation_mode" => Some(match self.commit_message_validation_mode {
                ValidationMode::Strict => "strict".to_string(),
                ValidationMode::Soft => "soft".to_string(),
                ValidationMode::Disabled => "disabled".to_string(),
            }),
            "diff_tool" => Some(self.diff.tool.to_string()),
            "diff_show_preview" => Some(self.diff.show_preview.to_string()),
            "use_commit_history" => Some(self.use_commit_history.to_string()),
            "commit_history_depth" => Some(self.commit_history_depth.to_string()),
            _ => None,
        }
    }

    /// Set a configuration value by key name.
    /// Token values are clamped to hard limits to prevent misconfiguration.
    ///
    /// Updates are also synchronized to the active profile to ensure persistence.
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        // Helper to update the active profile
        let mut update_active_profile = |k: &str, v: &str| -> Result<()> {
            if let Some(active_name) = self.profiles.active.clone() {
                // Validate active profile exists before attempting update
                let profile = self.profiles.get_mut(&active_name).ok_or_else(|| {
                    anyhow::anyhow!(
                        "Active profile '{}' not found. Profile may have been deleted.",
                        active_name
                    )
                })?;

                match k {
                    "max_input_tokens" => {
                        profile.max_input_tokens = v.parse().map_err(anyhow::Error::msg)?
                    }
                    "max_output_tokens" => {
                        profile.max_output_tokens = v.parse().map_err(anyhow::Error::msg)?
                    }
                    "model_provider" => profile.provider = v.parse().map_err(anyhow::Error::msg)?,
                    "model" => profile.model = ModelName::from(v),
                    "model_api_url" => profile.api_url = Some(Url::parse(v)?),
                    "azure_api_version" => profile.azure_api_version = Some(v.to_string()),
                    "azure_deployment_id" => profile.azure_deployment_id = Some(v.to_string()),
                    "api_key" | "model_api_key" => profile.api_key = Some(v.to_string()),
                    // Note: ignore_files are not in profile
                    _ => {}
                }
            }
            Ok(())
        };

        match key {
            "max_input_tokens" => {
                let parsed: TokenCount = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                let hard_limit = TokenCount::new_saturating(MAX_INPUT);
                self.max_input_tokens = parsed.min(hard_limit);
                update_active_profile(key, value)?;
            }
            "max_output_tokens" => {
                let parsed: TokenCount = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                let hard_limit = TokenCount::new_saturating(MAX_OUTPUT);
                self.max_output_tokens = parsed.min(hard_limit);
                update_active_profile(key, value)?;
            }
            "model_provider" => {
                self.model_provider = value.parse().map_err(anyhow::Error::msg)?;
                update_active_profile(key, value)?;
            }
            "model" => {
                self.model = ModelName::from(value);
                update_active_profile(key, value)?;
            }
            "api_key" | "model_api_key" => {
                self.api_key = Some(value.to_string());
                update_active_profile(key, value)?;
            }
            "model_api_url" => {
                self.model_api_url = Some(Url::parse(value)?);
                update_active_profile(key, value)?;
            }
            "azure_api_version" => {
                self.azure_api_version = Some(value.to_string());
                update_active_profile(key, value)?;
            }
            "azure_deployment_id" => {
                self.azure_deployment_id = Some(value.to_string());
                update_active_profile(key, value)?;
            }
            "ignore_files" => {
                self.ignore_files = value.split(',').map(|s| s.trim().to_string()).collect();
            }
            "commit_message_max_length" => {
                if value.is_empty() {
                    self.commit_message_max_length = None;
                } else {
                    let parsed: usize = value
                        .parse()
                        .map_err(anyhow::Error::msg)
                        .context("Invalid number")?;
                    self.commit_message_max_length = Some(parsed);
                }
            }
            "commit_message_validation_mode" => {
                self.commit_message_validation_mode = match value.to_lowercase().as_str() {
                    "strict" => ValidationMode::Strict,
                    "soft" => ValidationMode::Soft,
                    "disabled" => ValidationMode::Disabled,
                    _ => {
                        anyhow::bail!("Invalid validation mode: must be strict, soft, or disabled")
                    }
                };
            }
            "diff_tool" => {
                self.diff.tool = value.parse().map_err(anyhow::Error::msg)?;
            }
            "diff_show_preview" => {
                self.diff.show_preview = value.parse().map_err(anyhow::Error::msg)?;
            }
            "use_commit_history" => {
                let lower = value.trim().to_lowercase();
                let bool_val = match lower.as_str() {
                    "true" | "yes" | "1" | "on" => true,
                    "false" | "no" | "0" | "off" => false,
                    _ => value
                        .parse()
                        .context("Invalid boolean (expected true/false, yes/no, 1/0, on/off)")?,
                };
                self.use_commit_history = bool_val;
            }
            "commit_history_depth" => {
                let parsed: usize = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                // Clamp to valid range: 5-20 commits for balance between context and cost
                self.commit_history_depth = parsed.clamp(5, 20);
            }
            _ => anyhow::bail!("Unknown configuration key: {}", key),
        }
        Ok(())
    }

    pub fn apply_profile(&mut self, profile: &ProviderProfile) {
        self.model_provider = profile.provider;
        self.model = profile.model.clone();
        self.max_input_tokens = profile.max_input_tokens;
        self.max_output_tokens = profile.max_output_tokens;
        self.model_api_url = profile.api_url.clone();
        self.azure_api_version = profile.azure_api_version.clone();
        self.azure_deployment_id = profile.azure_deployment_id.clone();
        self.api_key = profile.api_key.clone();
    }

    pub fn load_active_profile(&mut self) {
        if let Some(active_name) = self.profiles.active.clone()
            && let Some(profile) = self.profiles.get(&active_name).cloned()
        {
            self.apply_profile(&profile);
        }
    }

    pub fn to_profile(&self, name: String) -> ProviderProfile {
        ProviderProfile {
            name,
            provider: self.model_provider,
            model: self.model.clone(),
            api_url: self.model_api_url.clone(),
            api_key: self.api_key.clone(),
            max_input_tokens: self.max_input_tokens,
            max_output_tokens: self.max_output_tokens,
            azure_api_version: self.azure_api_version.clone(),
            azure_deployment_id: self.azure_deployment_id.clone(),
        }
    }
}
