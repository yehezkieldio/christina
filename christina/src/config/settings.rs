use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tracing;

use christina_core::{
    ConfigFile,
    profile::{Profiles, ProviderProfile},
    types::{
        FreeTierLimits, ModelName, ProviderKind, TokenCount, UsageTier,
        commit_message::ValidationMode,
        token_count::{MAX_INPUT, MAX_OUTPUT},
    },
};
use url::Url;

const MIN_PARTIAL_FAILURE_RATE: f64 = 0.01;
const MAX_PARTIAL_FAILURE_RATE: f64 = 0.50;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LocalConfigSafe {
    ignore_files: Option<Vec<String>>,
    lockfile_token_limit: Option<TokenCount>,
    commit_message_max_length: Option<usize>,
    commit_message_validation_mode: Option<ValidationMode>,
    use_commit_history: Option<bool>,
    commit_history_depth: Option<usize>,
}

/// Default schema version for config files
fn default_schema_version() -> u32 {
    2
}

fn default_lockfile_token_limit() -> TokenCount {
    TokenCount::new_at_least_one(100)
}

fn clamp_partial_failure_rate(value: f64) -> (f64, Vec<String>) {
    let mut warnings = Vec::new();

    if (value - 0.0).abs() < f64::EPSILON {
        warnings.push(
            "max_partial_failure_rate set to 0.0 causes any chunk failure to abort processing"
                .to_string(),
        );
    }

    if (value - 1.0).abs() < f64::EPSILON {
        warnings.push(
            "max_partial_failure_rate set to 1.0 allows all chunk failures to pass"
                .to_string(),
        );
    }

    let clamped = value.clamp(MIN_PARTIAL_FAILURE_RATE, MAX_PARTIAL_FAILURE_RATE);
    if (clamped - value).abs() > f64::EPSILON {
        warnings.push(format!(
            "max_partial_failure_rate clamped from {} to {} (recommended range {}-{})",
            value, clamped, MIN_PARTIAL_FAILURE_RATE, MAX_PARTIAL_FAILURE_RATE
        ));
    }

    (clamped, warnings)
}

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

    /// Maximum tokens to include from lockfiles when truncating
    #[serde(default = "default_lockfile_token_limit")]
    pub lockfile_token_limit: TokenCount,

    /// Usage tier for rate-limit-aware defaults (standard or free)
    #[serde(default)]
    pub usage_tier: UsageTier,

    /// Free-tier limits applied when usage_tier is set to free
    #[serde(default)]
    pub free_tier: FreeTierLimits,

    /// Enable experimental settings (default: false)
    #[serde(default)]
    pub use_experimental: bool,

    /// Provider profiles for quick switching
    #[serde(default)]
    pub profiles: Profiles,

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

    /// Maximum concurrent LLM requests (1-10, default: 4)
    #[serde(default)]
    pub max_concurrent_requests: usize,

    /// Maximum partial failure rate before aborting (0.0-1.0, default: 0.10)
    #[serde(default)]
    pub max_partial_failure_rate: f64,

    /// Failure rate threshold for prompting user confirmation (0.0-1.0, default: 0.05)
    #[serde(default)]
    pub prompt_failure_rate_threshold: f64,

    /// Schema version for config file format migrations
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_input_tokens: TokenCount::new_at_least_one(4096),
            max_output_tokens: TokenCount::new_at_least_one(500),
            model_provider: ProviderKind::OpenAI,
            model: ModelName::from("gpt-4.1-mini"),
            api_key: None,
            model_api_url: None,
            azure_api_version: None,
            azure_deployment_id: None,
            model_temperature: 0.3,
            ignore_files: Vec::new(),
            lockfile_token_limit: default_lockfile_token_limit(),
            usage_tier: UsageTier::Standard,
            free_tier: FreeTierLimits::default(),
            use_experimental: false,
            profiles: Profiles::new(),
            commit_message_max_length: None,
            commit_message_validation_mode: ValidationMode::default(),
            use_commit_history: true,
            commit_history_depth: 5,
            max_concurrent_requests: 4,
            max_partial_failure_rate: 0.10,
            prompt_failure_rate_threshold: 0.05,
            schema_version: default_schema_version(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut config = Config::default();

        // Layer 3: Global config file
        if let Some(global_path) = Self::global_config_path()
            && global_path.exists()
        {
            let content =
                std::fs::read_to_string(&global_path).context("Failed to read global config")?;
            let config_file: ConfigFile =
                toml::from_str(&content).context("Failed to parse global config")?;
            config.apply_config_file(config_file);
        }

        // Fix profile names after deserialization (HashMap keys become names)
        config.profiles.fix_names();

        let local_path = std::path::Path::new("./christina.toml");
        if local_path.exists() {
            let local_content = std::fs::read_to_string(local_path)
                .context("Failed to read local config (christina.toml)")?;
            let local_safe: LocalConfigSafe = toml::from_str(&local_content)
                .context("Failed to parse local config (christina.toml)")?;
            config.apply_local_safe_overrides(local_safe);
        }

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
                tracing::warn!(
                    "Failed to persist default profile (read-only config?): {}",
                    e
                );
                if let Some(path) = Self::global_config_path() {
                    eprintln!(
                        "Warning: unable to persist default profile to {}. {}. Check permissions or update the config path.",
                        path.display(),
                        e
                    );
                } else {
                    eprintln!(
                        "Warning: unable to persist default profile. {}. Check permissions or update the config path.",
                        e
                    );
                }
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
            config.commit_history_depth = v.clamp(0, 50);
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_CONCURRENCY_LIMIT")
            && let Ok(v) = env_val.parse::<usize>()
        {
            config.max_concurrent_requests = v.clamp(1, 20);
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_MAX_FAILURE_RATE")
            && let Ok(v) = env_val.parse::<f64>()
        {
            config.max_partial_failure_rate = v;
        }

        // Validate and clamp token values to hard limits after all configuration is loaded
        let warnings = config.validate();
        for warning in warnings {
            tracing::warn!("{}", warning);
            eprintln!("Warning: {}", warning);
        }

        Ok(config)
    }

    /// Async-friendly configuration loader that offloads blocking work.
    pub async fn load_async() -> Result<Self> {
        tokio::task::spawn_blocking(Self::load)
            .await
            .map_err(|e| anyhow::anyhow!("Config load task failed: {}", e))?
    }

    fn apply_config_file(&mut self, file: ConfigFile) {
        self.schema_version = file.schema_version;
        self.profiles = file.profiles;
        if self.profiles.active.is_none() && file.standard.active_profile.is_some() {
            self.profiles.active = file.standard.active_profile;
        }
        self.commit_message_max_length = file.standard.commit_message_max_length;
        self.commit_message_validation_mode = file.standard.commit_message_validation_mode;
        self.ignore_files = file.standard.ignore_files;
        self.lockfile_token_limit = file.advanced.lockfile_token_limit;
        self.use_commit_history = file.advanced.use_commit_history;
        self.commit_history_depth = file.advanced.commit_history_depth;
        self.max_concurrent_requests = file.advanced.max_concurrent_requests;
        self.max_partial_failure_rate = file.advanced.max_partial_failure_rate;
        self.prompt_failure_rate_threshold = file.advanced.prompt_failure_rate_threshold;
        self.use_experimental = file.experimental.use_experimental;
        self.usage_tier = file.experimental.usage_tier;
        self.free_tier = file.experimental.free_tier;
    }

    fn to_config_file(&self) -> ConfigFile {
        let mut profiles = self.profiles.clone();
        let active_profile = profiles.active.clone();
        profiles.active = None;

        ConfigFile {
            schema_version: self.schema_version,
            standard: christina_core::config::StandardConfig {
                active_profile,
                commit_message_max_length: self.commit_message_max_length,
                commit_message_validation_mode: self.commit_message_validation_mode,
                ignore_files: self.ignore_files.clone(),
            },
            advanced: christina_core::config::AdvancedConfig {
                lockfile_token_limit: self.lockfile_token_limit,
                use_commit_history: self.use_commit_history,
                commit_history_depth: self.commit_history_depth,
                max_concurrent_requests: self.max_concurrent_requests,
                max_partial_failure_rate: self.max_partial_failure_rate,
                prompt_failure_rate_threshold: self.prompt_failure_rate_threshold,
            },
            experimental: christina_core::config::ExperimentalConfig {
                use_experimental: self.use_experimental,
                usage_tier: self.usage_tier,
                free_tier: self.free_tier.clone(),
            },
            profiles,
        }
    }

    /// Validate and clamp token values to hard limits.
    /// Also validates provider name against the registry.
    fn validate(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();
        let max_input = TokenCount::new_at_least_one(MAX_INPUT);
        let max_output = TokenCount::new_at_least_one(MAX_OUTPUT);

        if self.max_input_tokens > max_input {
            warnings.push(format!(
                "max_input_tokens clamped from {} to {}",
                self.max_input_tokens.get(),
                max_input.get()
            ));
            self.max_input_tokens = max_input;
        }

        if self.max_output_tokens > max_output {
            warnings.push(format!(
                "max_output_tokens clamped from {} to {}",
                self.max_output_tokens.get(),
                max_output.get()
            ));
            self.max_output_tokens = max_output;
        }

        // Clamp temperature to valid range (0.0 to 2.0)
        let original_temperature = self.model_temperature;
        self.model_temperature = self.model_temperature.clamp(0.0, 2.0);
        if (self.model_temperature - original_temperature).abs() > f32::EPSILON {
            warnings.push(format!(
                "model_temperature clamped from {} to {}",
                original_temperature, self.model_temperature
            ));
        }

        if self.free_tier.max_input_tokens > max_input {
            warnings.push(format!(
                "free_tier.max_input_tokens clamped from {} to {}",
                self.free_tier.max_input_tokens.get(),
                max_input.get()
            ));
            self.free_tier.max_input_tokens = max_input;
        }

        if self.free_tier.max_output_tokens > max_output {
            warnings.push(format!(
                "free_tier.max_output_tokens clamped from {} to {}",
                self.free_tier.max_output_tokens.get(),
                max_output.get()
            ));
            self.free_tier.max_output_tokens = max_output;
        }

        let original_free_concurrency = self.free_tier.max_concurrent_requests;
        self.free_tier.max_concurrent_requests =
            self.free_tier.max_concurrent_requests.clamp(1, 20);
        if self.free_tier.max_concurrent_requests != original_free_concurrency {
            warnings.push(format!(
                "free_tier.max_concurrent_requests clamped from {} to {}",
                original_free_concurrency, self.free_tier.max_concurrent_requests
            ));
        }

        let original_free_history = self.free_tier.commit_history_depth;
        self.free_tier.commit_history_depth =
            self.free_tier.commit_history_depth.clamp(0, 50);
        if self.free_tier.commit_history_depth != original_free_history {
            warnings.push(format!(
                "free_tier.commit_history_depth clamped from {} to {}",
                original_free_history, self.free_tier.commit_history_depth
            ));
        }

        let original_failure_rate = self.max_partial_failure_rate;
        let (clamped_failure_rate, mut failure_warnings) =
            clamp_partial_failure_rate(original_failure_rate);
        if (clamped_failure_rate - original_failure_rate).abs() > f64::EPSILON {
            self.max_partial_failure_rate = clamped_failure_rate;
        }
        warnings.append(&mut failure_warnings);

        if !self.use_experimental && self.usage_tier != UsageTier::Standard {
            warnings.push(
                "usage_tier set but experimental settings disabled; set use_experimental=true to apply"
                    .to_string(),
            );
        }

        // Warn if provider is unknown (but don't fail - let factory handle it)
        warnings
    }

    pub fn global_config_dir() -> Option<PathBuf> {
        ProjectDirs::from("", "", "christina").map(|dirs| dirs.config_dir().to_path_buf())
    }

    pub fn global_config_path() -> Option<PathBuf> {
        Self::global_config_dir().map(|dir| dir.join("config.toml"))
    }

    pub fn save_to_global(&self) -> Result<()> {
        use anyhow::Context;
        use fs2::FileExt;
        use std::fs::{File, OpenOptions};
        use std::io::Write;

        let config_dir = Self::global_config_dir()
            .context("Could not determine config directory for your platform")?;

        std::fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

        let config_path = config_dir.join("config.toml");
        let temp_path = config_dir.join("config.toml.tmp");

        let toml_content = self.render_config_toml()?;

        {
            let mut temp_file =
                File::create(&temp_path).context("Failed to create temporary config file")?;
            temp_file
                .write_all(toml_content.as_bytes())
                .context("Failed to write to temporary config file")?;
            temp_file
                .sync_all()
                .context("Failed to sync temporary config file")?;
        }

        let target_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&config_path)
            .context("Failed to open target config file for locking")?;
        target_file
            .lock_exclusive()
            .context("Failed to acquire exclusive lock on config file")?;

        std::fs::rename(&temp_path, &config_path)
            .context("Failed to atomically replace config file")?;

        let dir_file =
            File::open(&config_dir).context("Failed to open config directory for sync")?;
        dir_file
            .sync_all()
            .context("Failed to sync config directory")?;

        Ok(())
    }

    fn render_config_toml(&self) -> Result<String> {
        let config_file = self.to_config_file();
        render_config_file_with_comments(&config_file)
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
            "lockfile_token_limit" => Some(self.lockfile_token_limit.get().to_string()),
            "usage_tier" => Some(match self.usage_tier {
                UsageTier::Standard => "standard".to_string(),
                UsageTier::Free => "free".to_string(),
            }),
            "use_experimental" => Some(self.use_experimental.to_string()),
            "free_tier_max_input_tokens" => Some(self.free_tier.max_input_tokens.get().to_string()),
            "free_tier_max_output_tokens" => Some(self.free_tier.max_output_tokens.get().to_string()),
            "free_tier_max_concurrent_requests" => {
                Some(self.free_tier.max_concurrent_requests.to_string())
            }
            "free_tier_commit_history_depth" => {
                Some(self.free_tier.commit_history_depth.to_string())
            }
            "commit_message_max_length" => self.commit_message_max_length.map(|v| v.to_string()),
            "commit_message_validation_mode" => Some(match self.commit_message_validation_mode {
                ValidationMode::Strict => "strict".to_string(),
                ValidationMode::Soft => "soft".to_string(),
                ValidationMode::Disabled => "disabled".to_string(),
            }),
            "use_commit_history" => Some(self.use_commit_history.to_string()),
            "commit_history_depth" => Some(self.commit_history_depth.to_string()),
            "max_concurrent_requests" => Some(self.max_concurrent_requests.to_string()),
            "max_partial_failure_rate" => Some(self.max_partial_failure_rate.to_string()),
            "prompt_failure_rate_threshold" => Some(self.prompt_failure_rate_threshold.to_string()),
            "model_temperature" => Some(self.model_temperature.to_string()),
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
                    "api_key" | "model_api_key" => {
                        profile.api_key = christina_core::config::Secret::Value(v.to_string())
                    }
                    "model_temperature" => {
                        profile.temperature = Some(v.parse().map_err(anyhow::Error::msg)?)
                    }
                    // Note: ignore_files are not in profile
                    _ => {}
                }
            }
            Ok(())
        };

        // Helper for consistent boolean parsing
        let parse_bool = |v: &str| -> Result<bool> {
            let lower = v.trim().to_lowercase();
            match lower.as_str() {
                "true" | "yes" | "1" | "on" => Ok(true),
                "false" | "no" | "0" | "off" => Ok(false),
                _ => v
                    .parse()
                    .context("Invalid boolean (expected true/false, yes/no, 1/0, on/off)"),
            }
        };

        match key {
            "max_input_tokens" => {
                let parsed: TokenCount = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                let hard_limit = TokenCount::new_at_least_one(MAX_INPUT);
                self.max_input_tokens = parsed.min(hard_limit);
                update_active_profile(key, value)?;
            }
            "max_output_tokens" => {
                let parsed: TokenCount = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                let hard_limit = TokenCount::new_at_least_one(MAX_OUTPUT);
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
                self.ignore_files = value
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
            }
            "lockfile_token_limit" => {
                let parsed: TokenCount = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                self.lockfile_token_limit = parsed;
            }
            "usage_tier" => {
                self.usage_tier = match value.to_lowercase().as_str() {
                    "standard" => UsageTier::Standard,
                    "free" => UsageTier::Free,
                    _ => anyhow::bail!("Invalid usage_tier: must be standard or free"),
                };
            }
            "use_experimental" => {
                self.use_experimental = parse_bool(value)?;
            }
            "free_tier_max_input_tokens" => {
                let parsed: TokenCount = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                self.free_tier.max_input_tokens = parsed;
            }
            "free_tier_max_output_tokens" => {
                let parsed: TokenCount = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                self.free_tier.max_output_tokens = parsed;
            }
            "free_tier_max_concurrent_requests" => {
                let parsed: usize = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                self.free_tier.max_concurrent_requests = parsed;
            }
            "free_tier_commit_history_depth" => {
                let parsed: usize = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                self.free_tier.commit_history_depth = parsed;
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
            "use_commit_history" => {
                self.use_commit_history = parse_bool(value)?;
            }
            "commit_history_depth" => {
                let parsed: usize = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                self.commit_history_depth = parsed.clamp(0, 50);
            }
            "max_concurrent_requests" => {
                let parsed: usize = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                self.max_concurrent_requests = parsed.clamp(1, 10);
            }
            "max_partial_failure_rate" => {
                let parsed: f64 = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                let (clamped, warnings) = clamp_partial_failure_rate(parsed);
                self.max_partial_failure_rate = clamped;
                for warning in warnings {
                    tracing::warn!("{}", warning);
                    eprintln!("Warning: {}", warning);
                }
            }
            "prompt_failure_rate_threshold" => {
                let parsed: f64 = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid number")?;
                self.prompt_failure_rate_threshold = parsed.clamp(0.0, 1.0);
            }
            "model_temperature" => {
                let parsed: f32 = value
                    .parse()
                    .map_err(anyhow::Error::msg)
                    .context("Invalid temperature value")?;
                self.model_temperature = parsed.clamp(0.0, 2.0);
                update_active_profile(key, value)?;
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
        self.api_key = match &profile.api_key {
            christina_core::config::Secret::Value(key) => Some(key.clone()),
            christina_core::config::Secret::EnvVar(name) => std::env::var(name).ok(),
            #[cfg(feature = "keyring-support")]
            christina_core::config::Secret::Keyring(key) => keyring::Entry::new("christina", key)
                .and_then(|e| e.get_password())
                .ok(),
            #[cfg(not(feature = "keyring-support"))]
            christina_core::config::Secret::Keyring(_) => None,
        };
    }

    fn apply_local_safe_overrides(&mut self, local: LocalConfigSafe) {
        if let Some(ignore_files) = local.ignore_files {
            self.ignore_files = ignore_files;
        }

        if let Some(limit) = local.lockfile_token_limit {
            self.lockfile_token_limit = limit;
        }

        if let Some(max_len) = local.commit_message_max_length {
            self.commit_message_max_length = Some(max_len);
        }

        if let Some(mode) = local.commit_message_validation_mode {
            self.commit_message_validation_mode = mode;
        }

        if let Some(use_history) = local.use_commit_history {
            self.use_commit_history = use_history;
        }

        if let Some(depth) = local.commit_history_depth {
            self.commit_history_depth = depth.clamp(0, 50);
        }
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
            api_key: self
                .api_key
                .as_ref()
                .map(|k| christina_core::config::Secret::Value(k.clone()))
                .unwrap_or_else(|| {
                    christina_core::config::Secret::EnvVar(
                        self.model_provider.default_api_key_env_var().to_string(),
                    )
                }),
            max_input_tokens: self.max_input_tokens,
            max_output_tokens: self.max_output_tokens,
            azure_api_version: self.azure_api_version.clone(),
            azure_deployment_id: self.azure_deployment_id.clone(),
            temperature: None,
        }
    }
}

fn render_config_file_with_comments(config: &ConfigFile) -> Result<String> {
    use std::fmt::Write;

    let mut out = String::new();

    writeln!(out, "# Christina Configuration")?;
    writeln!(
        out,
        "# Generated by Christina. Edit values as needed."
    )?;
    writeln!(out)?;

    writeln!(out, "# Schema version for config migrations")?;
    writeln!(out, "schema_version = {}", config.schema_version)?;

    writeln!(out)?;
    writeln!(out, "# Standard settings (common defaults)")?;
    writeln!(out, "[standard]")?;
    writeln!(out, "# Active profile to use by default")?;
    if let Some(active) = &config.standard.active_profile {
        writeln!(out, "active_profile = {}", toml_string(active))?;
    } else {
        writeln!(out, "# active_profile = \"default\"")?;
    }
    writeln!(
        out,
        "# Maximum length for commit messages (default: 72)"
    )?;
    match config.standard.commit_message_max_length {
        Some(max_len) => writeln!(out, "commit_message_max_length = {}", max_len)?,
        None => writeln!(out, "# commit_message_max_length = 72")?,
    }
    writeln!(
        out,
        "# Validation mode for commit message length: soft | strict | disabled"
    )?;
    writeln!(
        out,
        "commit_message_validation_mode = {}",
        toml_string(validation_mode_to_str(
            config.standard.commit_message_validation_mode
        ))
    )?;
    writeln!(
        out,
        "# Files to exclude from AI processing (empty = include everything)"
    )?;
    writeln!(
        out,
        "ignore_files = {}",
        toml_value(&config.standard.ignore_files)?
    )?;

    writeln!(out)?;
    writeln!(out, "# Advanced settings")?;
    writeln!(out, "[advanced]")?;
    writeln!(
        out,
        "# Maximum tokens to include from lockfiles when truncating"
    )?;
    writeln!(
        out,
        "lockfile_token_limit = {}",
        config.advanced.lockfile_token_limit.get()
    )?;
    writeln!(
        out,
        "# Whether to include commit history context in prompts"
    )?;
    writeln!(
        out,
        "use_commit_history = {}",
        config.advanced.use_commit_history
    )?;
    writeln!(out, "# Number of recent commits to include")?;
    writeln!(
        out,
        "commit_history_depth = {}",
        config.advanced.commit_history_depth
    )?;
    writeln!(out, "# Maximum concurrent LLM requests")?;
    writeln!(
        out,
        "max_concurrent_requests = {}",
        config.advanced.max_concurrent_requests
    )?;
    writeln!(
        out,
        "# Maximum allowed chunk failure rate before aborting map phase"
    )?;
    writeln!(
        out,
        "max_partial_failure_rate = {}",
        config.advanced.max_partial_failure_rate
    )?;
    writeln!(
        out,
        "# Failure rate threshold for prompting user confirmation"
    )?;
    writeln!(
        out,
        "prompt_failure_rate_threshold = {}",
        config.advanced.prompt_failure_rate_threshold
    )?;

    writeln!(out)?;
    writeln!(out, "# Experimental settings (opt-in)")?;
    writeln!(out, "[experimental]")?;
    writeln!(out, "# Enable experimental settings")?;
    writeln!(out, "use_experimental = {}", config.experimental.use_experimental)?;
    writeln!(
        out,
        "# Usage tier for rate-limit-aware defaults: standard | free"
    )?;
    writeln!(
        out,
        "usage_tier = {}",
        toml_string(&config.experimental.usage_tier.to_string())
    )?;

    writeln!(out)?;
    writeln!(out, "[experimental.free_tier]")?;
    writeln!(out, "max_input_tokens = {}", config.experimental.free_tier.max_input_tokens.get())?;
    writeln!(
        out,
        "max_output_tokens = {}",
        config.experimental.free_tier.max_output_tokens.get()
    )?;
    writeln!(
        out,
        "max_concurrent_requests = {}",
        config.experimental.free_tier.max_concurrent_requests
    )?;
    writeln!(
        out,
        "commit_history_depth = {}",
        config.experimental.free_tier.commit_history_depth
    )?;

    if config.profiles.definitions.is_empty() {
        writeln!(out)?;
        writeln!(out, "# No profiles configured.")?;
        return Ok(out);
    }

    writeln!(out)?;
    writeln!(out, "# Provider profiles")?;

    let mut names: Vec<&String> = config.profiles.definitions.keys().collect();
    names.sort();

    for name in names {
        let profile = &config.profiles.definitions[name];
        writeln!(out)?;
        writeln!(out, "[profiles.{}]", name)?;
        writeln!(out, "name = {}", toml_string(&profile.name))?;
        writeln!(
            out,
            "provider = {}",
            toml_string(&profile.provider.to_string())
        )?;
        writeln!(out, "model = {}", toml_string(profile.model.as_ref()))?;
        writeln!(out, "api_key = {}", render_secret_inline(&profile.api_key))?;
        writeln!(
            out,
            "max_input_tokens = {}",
            profile.max_input_tokens.get()
        )?;
        writeln!(
            out,
            "max_output_tokens = {}",
            profile.max_output_tokens.get()
        )?;
        if let Some(api_url) = &profile.api_url {
            writeln!(out, "api_url = {}", toml_string(api_url.as_str()))?;
        }
        if let Some(version) = &profile.azure_api_version {
            writeln!(out, "azure_api_version = {}", toml_string(version))?;
        }
        if let Some(deployment) = &profile.azure_deployment_id {
            writeln!(out, "azure_deployment_id = {}", toml_string(deployment))?;
        }
        if let Some(temp) = profile.temperature {
            writeln!(out, "temperature = {}", temp)?;
        }
    }

    Ok(out)
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

fn toml_value<T: serde::Serialize>(value: &T) -> Result<String> {
    let rendered = toml::Value::try_from(value)
        .map_err(|err| anyhow::anyhow!("Failed to render TOML value: {}", err))?;
    Ok(rendered.to_string())
}

fn render_secret_inline(secret: &christina_core::config::Secret<String>) -> String {
    match secret {
        christina_core::config::Secret::Value(value) => {
            format!("{{ value = {} }}", toml_string(value))
        }
        christina_core::config::Secret::EnvVar(name) => {
            format!("{{ env = {} }}", toml_string(name))
        }
        christina_core::config::Secret::Keyring(name) => {
            format!("{{ keyring = {} }}", toml_string(name))
        }
    }
}

fn validation_mode_to_str(mode: ValidationMode) -> &'static str {
    match mode {
        ValidationMode::Strict => "strict",
        ValidationMode::Soft => "soft",
        ValidationMode::Disabled => "disabled",
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::field_reassign_with_default
)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = Config::default();
        assert_eq!(config.max_input_tokens.get(), 4096);
        assert_eq!(config.max_output_tokens.get(), 500);
        assert_eq!(config.model_provider, ProviderKind::OpenAI);
        assert_eq!(config.model, ModelName::from("gpt-4.1-mini"));
        assert!(config.api_key.is_none());
    }

    #[test]
    fn config_get_set() {
        let mut config = Config::default();
        config
            .set("model", "gpt-4.1-mini")
            .expect("setting config should succeed");
        assert_eq!(config.get("model"), Some("gpt-4.1-mini".to_string()));
    }

    #[test]
    fn set_max_input_tokens_valid() {
        let mut config = Config::default();
        config
            .set("max_input_tokens", "8192")
            .expect("should set valid token count");
        assert_eq!(config.max_input_tokens.get(), 8192);
    }

    #[test]
    fn set_max_input_tokens_clamping() {
        let mut config = Config::default();
        let over_limit = (MAX_INPUT + 1).to_string();
        config
            .set("max_input_tokens", &over_limit)
            .expect("should accept but clamp");
        assert_eq!(
            config.max_input_tokens.get(),
            MAX_INPUT,
            "should clamp to hard limit"
        );
    }

    #[test]
    fn set_max_input_tokens_invalid() {
        let mut config = Config::default();
        assert!(
            config.set("max_input_tokens", "not_a_number").is_err(),
            "should reject invalid number"
        );
        assert!(
            config.set("max_input_tokens", "-100").is_err(),
            "should reject negative number"
        );
    }

    #[test]
    fn set_max_output_tokens_valid() {
        let mut config = Config::default();
        config
            .set("max_output_tokens", "2048")
            .expect("should set valid token count");
        assert_eq!(config.max_output_tokens.get(), 2048);
    }

    #[test]
    fn set_max_output_tokens_clamping() {
        let mut config = Config::default();
        let over_limit = (MAX_OUTPUT + 1).to_string();
        config
            .set("max_output_tokens", &over_limit)
            .expect("should accept but clamp");
        assert_eq!(
            config.max_output_tokens.get(),
            MAX_OUTPUT,
            "should clamp to hard limit"
        );
    }

    #[test]
    fn set_max_output_tokens_invalid() {
        let mut config = Config::default();
        assert!(
            config.set("max_output_tokens", "invalid").is_err(),
            "should reject invalid number"
        );
    }

    #[test]
    fn set_model_provider_valid() {
        let mut config = Config::default();
        config
            .set("model_provider", "openai")
            .expect("should set openai provider");
        assert_eq!(config.model_provider, ProviderKind::OpenAI);

        config
            .set("model_provider", "azure")
            .expect("should set valid provider");
        assert_eq!(config.model_provider, ProviderKind::Azure);
    }

    #[test]
    fn set_model_provider_invalid() {
        let mut config = Config::default();
        assert!(
            config.set("model_provider", "invalid_provider").is_err(),
            "should reject unknown provider"
        );
    }

    #[test]
    fn set_model() {
        let mut config = Config::default();
        config
            .set("model", "gpt-5-nano")
            .expect("should set model name");
        assert_eq!(config.model, ModelName::from("gpt-5-nano"));

        config
            .set("model", "claude-4.5-sonnet")
            .expect("should set claude model");
        assert_eq!(config.model, ModelName::from("claude-4.5-sonnet"));
    }

    #[test]
    fn set_api_key() {
        let mut config = Config::default();
        config
            .set("api_key", "sk-test123")
            .expect("should set api_key");
        assert_eq!(config.api_key, Some("sk-test123".to_string()));

        config
            .set("model_api_key", "sk-test456")
            .expect("should set via model_api_key");
        assert_eq!(config.api_key, Some("sk-test456".to_string()));
    }

    #[test]
    fn set_model_api_url_valid() {
        let mut config = Config::default();
        config
            .set("model_api_url", "https://api.example.com/v1")
            .expect("should set valid URL");
        assert_eq!(
            config
                .model_api_url
                .expect("model_api_url should be set")
                .as_str(),
            "https://api.example.com/v1"
        );
    }

    #[test]
    fn set_model_api_url_invalid() {
        let mut config = Config::default();
        assert!(
            config.set("model_api_url", "not a url").is_err(),
            "should reject malformed URL"
        );
    }

    #[test]
    fn set_azure_api_version() {
        let mut config = Config::default();
        config
            .set("azure_api_version", "2024-02-15-preview")
            .expect("should set azure api version");
        assert_eq!(
            config.azure_api_version,
            Some("2024-02-15-preview".to_string())
        );
    }

    #[test]
    fn set_azure_deployment_id() {
        let mut config = Config::default();
        config
            .set("azure_deployment_id", "my-deployment")
            .expect("should set azure deployment id");
        assert_eq!(
            config.azure_deployment_id,
            Some("my-deployment".to_string())
        );
    }

    #[test]
    fn set_ignore_files() {
        let mut config = Config::default();
        config
            .set("ignore_files", "file1.txt,file2.lock,file3.bin")
            .expect("should parse CSV");
        assert_eq!(
            config.ignore_files,
            vec!["file1.txt", "file2.lock", "file3.bin"]
        );
    }

    #[test]
    fn set_ignore_files_with_spaces() {
        let mut config = Config::default();
        config
            .set("ignore_files", "file1.txt, file2.lock , file3.bin")
            .expect("should trim spaces");
        assert_eq!(
            config.ignore_files,
            vec!["file1.txt", "file2.lock", "file3.bin"]
        );
    }

    #[test]
    fn set_ignore_files_empty() {
        let mut config = Config {
            ignore_files: vec!["test.txt".to_string()],
            ..Config::default()
        };
        config
            .set("ignore_files", "")
            .expect("should accept empty string");
        assert_eq!(config.ignore_files, Vec::<String>::new());
    }

    #[test]
    fn set_commit_message_max_length() {
        let mut config = Config::default();
        config
            .set("commit_message_max_length", "100")
            .expect("should set max length");
        assert_eq!(config.commit_message_max_length, Some(100));
    }

    #[test]
    fn set_commit_message_max_length_empty() {
        let mut config = Config {
            commit_message_max_length: Some(100),
            ..Config::default()
        };
        config
            .set("commit_message_max_length", "")
            .expect("should clear when empty");
        assert_eq!(config.commit_message_max_length, None);
    }

    #[test]
    fn set_commit_message_max_length_invalid() {
        let mut config = Config::default();
        assert!(
            config
                .set("commit_message_max_length", "not_a_number")
                .is_err()
        );
    }

    #[test]
    fn set_commit_message_validation_mode() {
        let mut config = Config::default();

        config
            .set("commit_message_validation_mode", "strict")
            .expect("should set strict mode");
        assert_eq!(
            config.commit_message_validation_mode,
            ValidationMode::Strict
        );

        config
            .set("commit_message_validation_mode", "soft")
            .expect("should set soft mode");
        assert_eq!(config.commit_message_validation_mode, ValidationMode::Soft);

        config
            .set("commit_message_validation_mode", "disabled")
            .expect("should set disabled mode");
        assert_eq!(
            config.commit_message_validation_mode,
            ValidationMode::Disabled
        );

        config
            .set("commit_message_validation_mode", "STRICT")
            .expect("should be case insensitive");
        assert_eq!(
            config.commit_message_validation_mode,
            ValidationMode::Strict
        );
    }

    #[test]
    fn set_commit_message_validation_mode_invalid() {
        let mut config = Config::default();
        assert!(
            config
                .set("commit_message_validation_mode", "invalid_mode")
                .is_err()
        );
    }

    #[test]
    fn set_unknown_key() {
        let mut config = Config::default();
        let result = config.set("unknown_key", "value");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unknown configuration key")
        );
    }

    #[test]
    fn get_max_input_tokens() {
        let config = Config::default();
        assert_eq!(config.get("max_input_tokens"), Some("4096".to_string()));
    }

    #[test]
    fn get_max_output_tokens() {
        let config = Config::default();
        assert_eq!(config.get("max_output_tokens"), Some("500".to_string()));
    }

    #[test]
    fn env_var_parsing_max_input_tokens() {
        let val = "16384";
        let parsed: Result<TokenCount, _> = val.parse();
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().get(), 16384);
    }

    #[test]
    fn env_var_parsing_model_provider() {
        let val = "azure";
        let parsed: Result<ProviderKind, _> = val.parse();
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), ProviderKind::Azure);
    }

    #[test]
    fn env_var_parsing_use_keyring() {
        let val = "true";
        let parsed: Result<bool, _> = val.parse();
        assert!(parsed.is_ok());
        assert!(parsed.unwrap());
    }

    #[test]
    fn get_model() {
        let config = Config::default();
        assert_eq!(config.get("model"), Some("gpt-4.1-mini".to_string()));
    }

    #[test]
    fn get_api_key_none() {
        let config = Config::default();
        assert_eq!(config.get("api_key"), None);
        assert_eq!(config.get("model_api_key"), None);
    }

    #[test]
    fn get_api_key_some() {
        let mut config = Config::default();
        config.api_key = Some("sk-test".to_string());
        assert_eq!(config.get("api_key"), Some("sk-test".to_string()));
        assert_eq!(config.get("model_api_key"), Some("sk-test".to_string()));
    }

    #[test]
    fn get_model_api_url() {
        let mut config = Config::default();
        config.model_api_url = Some(Url::parse("https://api.example.com").unwrap());
        assert_eq!(
            config.get("model_api_url"),
            Some("https://api.example.com/".to_string())
        );
    }

    #[test]
    fn get_azure_fields() {
        let mut config = Config::default();
        config.azure_api_version = Some("2024-02-15".to_string());
        config.azure_deployment_id = Some("my-deployment".to_string());

        assert_eq!(
            config.get("azure_api_version"),
            Some("2024-02-15".to_string())
        );
        assert_eq!(
            config.get("azure_deployment_id"),
            Some("my-deployment".to_string())
        );
    }

    #[test]
    fn get_ignore_files() {
        let config = Config::default();
        let ignore_files = config.get("ignore_files").unwrap();
        assert!(ignore_files.is_empty());
    }

    #[test]
    fn get_commit_message_fields() {
        let mut config = Config::default();
        assert_eq!(config.get("commit_message_max_length"), None);

        config.commit_message_max_length = Some(100);
        assert_eq!(
            config.get("commit_message_max_length"),
            Some("100".to_string())
        );

        assert_eq!(
            config.get("commit_message_validation_mode"),
            Some("soft".to_string())
        );
    }

    #[test]
    fn get_unknown_key() {
        let config = Config::default();
        assert_eq!(config.get("unknown_key"), None);
    }

    #[test]
    fn validate_clamps_temperature() {
        let mut config = Config::default();
        config.model_temperature = 3.0;
        let warnings = config.validate();
        assert_eq!(config.model_temperature, 2.0);
        assert!(warnings.iter().any(|w| w.contains("model_temperature")));

        config.model_temperature = -1.0;
        let warnings = config.validate();
        assert_eq!(config.model_temperature, 0.0);
        assert!(warnings.iter().any(|w| w.contains("model_temperature")));

        config.model_temperature = 1.5;
        let warnings = config.validate();
        assert_eq!(config.model_temperature, 1.5);
        assert!(warnings.is_empty());
    }

    #[test]
    fn validate_clamps_token_limits() {
        let mut config = Config::default();

        config.max_input_tokens = TokenCount::new_at_least_one(MAX_INPUT + 1000);
        config.max_output_tokens = TokenCount::new_at_least_one(MAX_OUTPUT + 1000);

        let warnings = config.validate();

        assert_eq!(
            config.max_input_tokens.get(),
            MAX_INPUT,
            "should clamp input tokens to hard limit"
        );
        assert_eq!(
            config.max_output_tokens.get(),
            MAX_OUTPUT,
            "should clamp output tokens to hard limit"
        );
        assert!(warnings.iter().any(|w| w.contains("max_input_tokens")));
        assert!(warnings.iter().any(|w| w.contains("max_output_tokens")));
    }

    #[test]
    fn config_serialize_deserialize() {
        let config = Config::default();
        let toml_str = toml::to_string(&config.to_config_file()).expect("should serialize to TOML");

        let toml_value: toml::Value =
            toml::from_str(&toml_str).expect("should deserialize TOML into value");
        assert!(toml_value.get("max_input_tokens").is_none());
        assert!(toml_value.get("max_output_tokens").is_none());
        assert!(toml_value.get("api_key").is_none());
        assert!(toml_value.get("model_api_key").is_none());
        assert!(toml_value.get("standard").is_some());
        assert!(toml_value.get("advanced").is_some());
        assert!(toml_value.get("experimental").is_some());

        let deserialized: ConfigFile =
            toml::from_str(&toml_str).expect("should deserialize from TOML");
        let mut roundtrip = Config::default();
        roundtrip.apply_config_file(deserialized);
        assert_eq!(roundtrip.ignore_files, config.ignore_files);
    }

    #[test]
    fn config_deserialize_with_missing_fields() {
        let minimal_toml = r#"
        [standard]
        ignore_files = ["test.lock"]
        "#;
        let config_file: ConfigFile =
            toml::from_str(minimal_toml).expect("should use defaults");
        let mut config = Config::default();
        config.apply_config_file(config_file);
        assert_eq!(config.ignore_files, vec!["test.lock"]);
        assert_eq!(config.max_input_tokens.get(), 4096);
    }

    #[test]
    fn apply_profile() {
        let mut config = Config::default();
        let profile = ProviderProfile {
            name: "test-profile".to_string(),
            provider: ProviderKind::Azure,
            model: ModelName::from("gpt-4.1-mini"),
            api_url: Some(Url::parse("https://api.azure.com").unwrap()),
            api_key: christina_core::config::Secret::Value("sk-azure-test".to_string()),
            max_input_tokens: TokenCount::new_at_least_one(8192),
            max_output_tokens: TokenCount::new_at_least_one(4096),
            azure_api_version: Some("test-version".to_string()),
            azure_deployment_id: Some("test-deployment".to_string()),
            temperature: None,
        };

        config.apply_profile(&profile);

        assert_eq!(config.model_provider, ProviderKind::Azure);
        assert_eq!(config.model, ModelName::from("gpt-4.1-mini"));
        assert_eq!(config.max_input_tokens.get(), 8192);
        assert_eq!(config.max_output_tokens.get(), 4096);
        assert_eq!(config.api_key, Some("sk-azure-test".to_string()));
        assert_eq!(
            config.model_api_url.unwrap().as_str(),
            "https://api.azure.com/"
        );
        assert_eq!(config.azure_api_version, Some("test-version".to_string()));
        assert_eq!(
            config.azure_deployment_id,
            Some("test-deployment".to_string())
        );
    }

    #[test]
    fn to_profile() {
        let mut config = Config::default();
        config.model_provider = ProviderKind::OpenAI;
        config.model = ModelName::from("gpt-5.2");
        config.api_key = Some("sk-openai-test".to_string());
        config.max_input_tokens = TokenCount::new_at_least_one(16384);
        config.max_output_tokens = TokenCount::new_at_least_one(2048);

        let profile = config.to_profile("openai-profile".to_string());

        assert_eq!(profile.name, "openai-profile");
        assert_eq!(profile.provider, ProviderKind::OpenAI);
        assert_eq!(profile.model, ModelName::from("gpt-5.2"));
        assert_eq!(
            profile.api_key,
            christina_core::config::Secret::Value("sk-openai-test".to_string())
        );
        assert_eq!(profile.max_input_tokens.get(), 16384);
        assert_eq!(profile.max_output_tokens.get(), 2048);
    }
}
