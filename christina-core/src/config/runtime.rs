use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::config::env::EnvConfig;
use crate::config::settings::{Settings, ValidationMode};
use crate::profile::{Profiles, ProviderProfile};
use crate::types::token_count::{MAX_INPUT, MAX_OUTPUT};
use crate::types::{ModelName, ProviderKind, TokenCount};

/// Runtime configuration after profile resolution and env var application.
///
/// This struct represents the fully-resolved configuration used at runtime.
/// Unlike Settings (which is the storage format), RuntimeConfig contains
/// the actual provider profile settings that will be used for LLM calls.
///
/// The flow is: Settings → RuntimeConfig (via RuntimeConfig::load())
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeConfig {
    /// The active provider profile (fully resolved from Settings)
    pub profile: ProviderProfile,

    /// Diff tool configuration
    pub diff: DiffConfig,

    /// Files to ignore when generating commits
    pub ignore_files: Vec<String>,

    /// Commit message settings
    pub commit_message: CommitMessageConfig,

    /// Whether to include commit history in context
    pub use_commit_history: bool,

    /// Number of commits to include in history
    pub commit_history_depth: usize,

    /// Max concurrent LLM requests (from env only, not persisted)
    pub concurrency_limit: u32,

    /// Debug mode (from env only, not persisted)
    pub debug: bool,
}

/// Diff tool configuration for runtime.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffConfig {
    /// The diff tool to use (delta, diff-so-fancy, etc.)
    pub tool: Option<String>,

    /// Whether to show a preview of the diff
    pub show_preview: bool,
}

/// Commit message configuration for runtime.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitMessageConfig {
    /// Maximum length for generated commit messages
    pub max_length: usize,

    /// Validation mode for commit messages
    pub validation_mode: ValidationMode,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            profile: ProviderProfile::new(
                "default".to_string(),
                ProviderKind::OpenAI,
                ModelName::from("gpt-4o-mini"),
            ),
            diff: DiffConfig {
                tool: None,
                show_preview: true,
            },
            ignore_files: vec![
                "Cargo.lock".to_string(),
                "package-lock.json".to_string(),
                "yarn.lock".to_string(),
                "pnpm-lock.yaml".to_string(),
                "*.lock".to_string(),
            ],
            commit_message: CommitMessageConfig {
                max_length: 72,
                validation_mode: ValidationMode::Strict,
            },
            use_commit_history: true,
            commit_history_depth: 5,
            concurrency_limit: 5,
            debug: false,
        }
    }
}

impl RuntimeConfig {
    /// Load configuration from files and environment.
    ///
    /// Configuration precedence (highest to lowest):
    /// 1. Environment variables (CHRISTINA_*)
    /// 2. Local config file (./christina.toml) - only safe fields
    /// 3. Global config file (~/.config/christina/config.toml)
    /// 4. Default values
    pub fn load() -> Result<Self> {
        let env_config = EnvConfig::from_env();

        // Start with defaults
        let mut runtime = Self::default();

        // Try to load global config
        if let Some(global_path) = Self::global_config_path() && global_path.exists() {
            let settings = Self::load_settings_file(&global_path)?;
            runtime.apply_settings(&settings);
        }

        // Try to load local config (only safe fields)
        if let Ok(local_settings) = Self::load_local_settings() {
            runtime.apply_safe_local_settings(&local_settings);
        }

        // Apply environment variable overrides (highest priority)
        runtime.apply_env_config(&env_config);

        // Validate the final configuration
        runtime.validate()?;

        Ok(runtime)
    }

    /// Load settings from a specific file path.
    pub fn load_settings_file(path: &Path) -> Result<Settings> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let mut settings: Settings = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        // Ensure profile names match their keys
        settings.profiles.fix_names();

        // Ensure we have an active profile
        settings.ensure_active_profile();

        Ok(settings)
    }

    /// Load local settings from ./christina.toml if it exists.
    fn load_local_settings() -> Result<Settings> {
        let local_path = PathBuf::from("christina.toml");
        if local_path.exists() {
            Self::load_settings_file(&local_path)
        } else {
            anyhow::bail!("No local config file found")
        }
    }

    /// Apply settings from a Settings object.
    fn apply_settings(&mut self, settings: &Settings) {
        // Apply the active profile
        if let Some(profile) = settings.profiles.get_active() {
            self.profile = profile.clone();
        }

        // Apply diff settings
        self.diff.tool = settings.diff.tool.clone();
        self.diff.show_preview = settings.diff.show_preview;

        // Apply global settings
        self.ignore_files = settings.ignore_files.clone();
        self.use_commit_history = settings.use_commit_history;
        self.commit_history_depth = settings.commit_history_depth;
        self.commit_message.max_length = settings.commit_message.max_length;
        self.commit_message.validation_mode = settings.commit_message.validation_mode;
    }

    /// Apply only "safe" local settings (non-sensitive fields).
    ///
    /// Security-sensitive fields like api_key should not be loaded from local configs.
    fn apply_safe_local_settings(&mut self, settings: &Settings) {
        // Only apply non-sensitive fields from local config
        self.ignore_files = settings.ignore_files.clone();
        self.use_commit_history = settings.use_commit_history;
        self.commit_history_depth = settings.commit_history_depth;
        self.diff.show_preview = settings.diff.show_preview;
        // Note: We intentionally do NOT apply api_key, diff.tool, or profile settings
        // from local config for security
    }

    /// Apply environment variable overrides.
    fn apply_env_config(&mut self, env: &EnvConfig) {
        // Apply to profile
        if let Some(provider) = env.model_provider.as_ref()
            && let Ok(kind) = provider.parse::<ProviderKind>()
        {
            self.profile.provider = kind;
        }

        if let Some(model) = env.model.as_ref() {
            self.profile.model = ModelName::from(model.as_str());
        }

        if let Some(api_key) = env.model_api_key.as_ref() {
            self.profile.api_key = Some(api_key.clone());
        }

        if let Some(api_url) = env.model_api_url.as_ref()
            && let Ok(url) = api_url.parse()
        {
            self.profile.api_url = Some(url);
        }

        if let Some(tokens) = env.max_input_tokens {
            self.profile.max_input_tokens = TokenCount::new_saturating(tokens);
        }

        if let Some(tokens) = env.max_output_tokens {
            self.profile.max_output_tokens = TokenCount::new_saturating(tokens);
        }

        if let Some(version) = env.azure_api_version.as_ref() {
            self.profile.azure_api_version = Some(version.clone());
        }

        if let Some(deployment) = env.azure_deployment_id.as_ref() {
            self.profile.azure_deployment_id = Some(deployment.clone());
        }

        if let Some(temp) = env.model_temperature {
            self.profile.temperature = temp.clamp(0.0, 2.0);
        }

        // Apply to diff config
        if let Some(tool) = env.diff_tool.as_ref() {
            self.diff.tool = Some(tool.clone());
        }

        if let Some(show) = env.diff_show_preview {
            self.diff.show_preview = show;
        }

        // Apply to global settings
        if let Some(use_history) = env.use_commit_history {
            self.use_commit_history = use_history;
        }

        if let Some(depth) = env.commit_history_depth {
            self.commit_history_depth = depth;
        }

        if let Some(limit) = env.concurrency_limit {
            self.concurrency_limit = limit.clamp(1, 20);
        }

        if let Some(debug) = env.debug {
            self.debug = debug;
        }
    }

    /// Validate the runtime configuration.
    pub fn validate(&self) -> Result<()> {
        // Validate the profile
        self.profile
            .validate()
            .context("Invalid provider profile configuration")?;

        // Validate token counts are within bounds
        if self.profile.max_input_tokens.get() > MAX_INPUT {
            anyhow::bail!("Max input tokens cannot exceed {}", MAX_INPUT);
        }

        if self.profile.max_output_tokens.get() > MAX_OUTPUT {
            anyhow::bail!("Max output tokens cannot exceed {}", MAX_OUTPUT);
        }

        // Validate temperature
        if !(0.0..=2.0).contains(&self.profile.temperature) {
            anyhow::bail!("Temperature must be between 0.0 and 2.0");
        }

        Ok(())
    }

    /// Get the global configuration directory path.
    pub fn global_config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("christina"))
    }

    /// Get the global configuration file path.
    pub fn global_config_path() -> Option<PathBuf> {
        Self::global_config_dir().map(|d| d.join("config.toml"))
    }

    /// Convert back to Settings for saving.
    ///
    /// This creates a Settings object that can be serialized back to disk.
    /// Note: Some runtime-only fields (concurrency_limit, debug) are not persisted.
    pub fn to_settings(&self) -> Settings {
        let mut profiles = Profiles::new();

        // Create a profile from the current runtime profile
        let profile = self.profile.clone();
        let profile_name = profile.name.clone();
        profiles.definitions.insert(profile_name.clone(), profile);
        profiles.active = Some(profile_name);

        Settings {
            profiles,
            diff: crate::config::settings::DiffSettings {
                tool: self.diff.tool.clone(),
                show_preview: self.diff.show_preview,
            },
            ignore_files: self.ignore_files.clone(),
            commit_message: crate::config::settings::CommitMessageSettings {
                max_length: self.commit_message.max_length,
                validation_mode: self.commit_message.validation_mode,
            },
            use_commit_history: self.use_commit_history,
            commit_history_depth: self.commit_history_depth,
        }
    }

    /// Save the current configuration to the global config file.
    pub fn save_to_global(&self) -> Result<()> {
        let settings = self.to_settings();

        let config_dir =
            Self::global_config_dir().context("Could not determine config directory")?;

        std::fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

        let config_path = config_dir.join("config.toml");
        let toml = toml::to_string_pretty(&settings).context("Failed to serialize config")?;

        std::fs::write(&config_path, toml)
            .with_context(|| format!("Failed to write config to {}", config_path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_config_default() {
        let config = RuntimeConfig::default();
        assert_eq!(config.profile.name, "default");
        assert_eq!(config.concurrency_limit, 5);
        assert!(!config.debug);
    }

    #[test]
    fn test_runtime_config_validate() {
        let config = RuntimeConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_to_settings_roundtrip() {
        let config = RuntimeConfig::default();
        let settings = config.to_settings();

        // Create new runtime from settings
        let mut runtime = RuntimeConfig::default();
        runtime.apply_settings(&settings);

        // Profile should match
        assert_eq!(runtime.profile.name, config.profile.name);
        assert_eq!(runtime.diff.show_preview, config.diff.show_preview);
    }
}
