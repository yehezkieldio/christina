//! On-disk configuration schema and defaults.
//!
//! WHY explicit defaults: keep the config file optional for first-run while
//! guaranteeing stable operational limits (token caps, concurrency) that keep
//! latency predictable and protect providers from accidental overload.

use serde::{Deserialize, Serialize};

use crate::{
    profile::Profiles,
    types::{
        FreeTierLimits, TokenCount, UsageTier,
        commit::ValidationMode,
    },
};

/// Standard (common) configuration settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct StandardConfig {
    /// Active profile name (preferred over profiles.active)
    pub active_profile: Option<String>,

    /// Maximum commit message length (None = 72)
    pub commit_message_max_length: Option<usize>,

    /// Validation mode for commit message length
    pub commit_message_validation_mode: ValidationMode,

    /// Files to exclude from AI processing
    pub ignore_files: Vec<String>,
}

/// Advanced configuration settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct AdvancedConfig {
    /// Maximum tokens to include from lockfiles when truncating
    pub lockfile_token_limit: TokenCount,

    /// Whether to include commit history context in LLM prompts
    pub use_commit_history: bool,

    /// Number of recent commits to include for style analysis
    pub commit_history_depth: usize,

    /// Maximum concurrent LLM requests
    pub max_concurrent_requests: usize,

    /// Maximum allowed fraction of chunk failures before aborting map phase
    pub max_partial_failure_rate: f64,

    /// Failure rate threshold for prompting user confirmation
    pub prompt_failure_rate_threshold: f64,
}

/// Experimental configuration settings (opt-in).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct ExperimentalConfig {
    /// Enable experimental settings (default: false)
    pub use_experimental: bool,

    /// Usage tier for rate-limit-aware defaults
    pub usage_tier: UsageTier,

    /// Free-tier limits applied when usage_tier is set to free
    pub free_tier: FreeTierLimits,
}

/// On-disk configuration representation (serde-friendly)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct ConfigFile {
    /// Schema version for config file format migrations
    pub schema_version: u32,

    /// Standard settings
    pub standard: StandardConfig,

    /// Advanced settings
    pub advanced: AdvancedConfig,

    /// Experimental settings (opt-in)
    pub experimental: ExperimentalConfig,

    /// Provider profiles
    pub profiles: Profiles,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            // Version pins config migrations; bump only with a deliberate upgrade path.
            schema_version: 2,
            standard: StandardConfig::default(),
            advanced: AdvancedConfig::default(),
            experimental: ExperimentalConfig::default(),
            profiles: Profiles::new(),
        }
    }
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            // Conservative defaults balance throughput with provider limits.
            lockfile_token_limit: TokenCount::new_at_least_one(100),
            use_commit_history: true,
            commit_history_depth: 5,
            max_concurrent_requests: 4,
            // Low failure rates reduce silent degradation in map-phase fan-out.
            max_partial_failure_rate: 0.1,
            prompt_failure_rate_threshold: 0.05,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let config = ConfigFile::default();

        assert_eq!(config.standard.active_profile, None);
        assert!(config.profiles.definitions.is_empty());
        assert_eq!(config.standard.commit_message_max_length, None);
        assert_eq!(
            config.standard.commit_message_validation_mode,
            ValidationMode::Soft
        );
        assert!(config.advanced.use_commit_history);
        assert_eq!(config.advanced.commit_history_depth, 5);
        assert_eq!(config.advanced.max_concurrent_requests, 4);
        assert_eq!(config.advanced.prompt_failure_rate_threshold, 0.05);
        assert!(!config.experimental.use_experimental);
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = ConfigFile {
            standard: StandardConfig {
                active_profile: Some("default".to_string()),
                commit_message_max_length: Some(100),
                commit_message_validation_mode: ValidationMode::Strict,
                ..StandardConfig::default()
            },
            ..ConfigFile::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ConfigFile = serde_json::from_str(&json).unwrap();

        assert_eq!(
            deserialized.standard.active_profile,
            config.standard.active_profile
        );
        assert_eq!(
            deserialized.standard.commit_message_max_length,
            config.standard.commit_message_max_length
        );
        assert_eq!(
            deserialized.standard.commit_message_validation_mode,
            config.standard.commit_message_validation_mode
        );
        assert_eq!(
            deserialized.standard.ignore_files,
            config.standard.ignore_files
        );
    }

    #[test]
    fn test_optional_fields() {
        let config = ConfigFile {
            standard: StandardConfig {
                active_profile: None,
                commit_message_max_length: None,
                ignore_files: vec![],
                ..StandardConfig::default()
            },
            profiles: Profiles::new(),
            ..ConfigFile::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ConfigFile = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.standard.active_profile, None);
        assert_eq!(deserialized.standard.commit_message_max_length, None);
    }

    #[test]
    fn test_ignore_files_default() {
        let config = ConfigFile::default();

        assert!(config.standard.ignore_files.is_empty());
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_generate_json_schema() {
        use schemars::schema_for;

        let schema = schema_for!(ConfigFile);
        let schema_json = serde_json::to_string_pretty(&schema).unwrap();

        // Write to file for distribution
        let schema_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("config.schema.json");

        // Only write if the test is run with --ignored or the file doesn't exist
        // This prevents CI from failing when the file doesn't exist yet
        if !schema_path.exists() || std::env::var("UPDATE_SCHEMA").is_ok() {
            std::fs::write(&schema_path, &schema_json).unwrap();
        }

        // Validate that the schema is valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&schema_json).unwrap();
        assert!(parsed.is_object());

        // Check that the schema contains expected fields
        let obj = parsed.as_object().unwrap();
        assert!(obj.contains_key("$schema"));
        assert!(obj.contains_key("title"));
        assert!(obj.contains_key("type"));
    }
}
