use serde::{Deserialize, Serialize};

use crate::{
    profile::Profiles,
    types::{
        FreeTierLimits, TokenCount, UsageTier,
        commit_message::ValidationMode,
    },
};

/// On-disk configuration representation (serde-friendly)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct ConfigFile {
    /// Schema version for config file format migrations
    pub schema_version: u32,

    /// Active profile name (legacy alias, preferred via profiles.active)
    pub active_profile: Option<String>,

    /// Provider profiles
    pub profiles: Profiles,

    /// Maximum commit message length (None = 72)
    pub commit_message_max_length: Option<usize>,

    /// Validation mode for commit message length
    pub commit_message_validation_mode: ValidationMode,

    /// Files to exclude from AI processing
    pub ignore_files: Vec<String>,

    /// Maximum tokens to include from lockfiles when truncating
    pub lockfile_token_limit: TokenCount,

    /// Usage tier for rate-limit-aware defaults
    pub usage_tier: UsageTier,

    /// Free-tier limits applied when usage_tier is set to free
    pub free_tier: FreeTierLimits,

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

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            schema_version: 1,
            active_profile: None,
            profiles: Profiles::new(),
            commit_message_max_length: None,
            commit_message_validation_mode: ValidationMode::default(),
            ignore_files: vec![
                "package-lock.json".to_string(),
                "yarn.lock".to_string(),
                "pnpm-lock.yaml".to_string(),
                "bun.lock".to_string(),
                "Cargo.lock".to_string(),
                "poetry.lock".to_string(),
                "Gemfile.lock".to_string(),
                "*.lock".to_string(),
            ],
            lockfile_token_limit: TokenCount::new_at_least_one(100),
            usage_tier: UsageTier::Standard,
            free_tier: FreeTierLimits::default(),
            use_commit_history: true,
            commit_history_depth: 5,
            max_concurrent_requests: 4,
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

        assert_eq!(config.active_profile, None);
        assert!(config.profiles.definitions.is_empty());
        assert_eq!(config.commit_message_max_length, None);
        assert_eq!(config.commit_message_validation_mode, ValidationMode::Soft);
        assert!(config.use_commit_history);
        assert_eq!(config.commit_history_depth, 5);
        assert_eq!(config.max_concurrent_requests, 4);
        assert_eq!(config.prompt_failure_rate_threshold, 0.05);
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = ConfigFile {
            active_profile: Some("default".to_string()),
            commit_message_max_length: Some(100),
            commit_message_validation_mode: ValidationMode::Strict,
            ..ConfigFile::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ConfigFile = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.active_profile, config.active_profile);
        assert_eq!(
            deserialized.commit_message_max_length,
            config.commit_message_max_length
        );
        assert_eq!(
            deserialized.commit_message_validation_mode,
            config.commit_message_validation_mode
        );
        assert_eq!(deserialized.ignore_files, config.ignore_files);
    }

    #[test]
    fn test_optional_fields() {
        let config = ConfigFile {
            active_profile: None,
            profiles: Profiles::new(),
            commit_message_max_length: None,
            ignore_files: vec![],
            ..ConfigFile::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ConfigFile = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.active_profile, None);
        assert_eq!(deserialized.commit_message_max_length, None);
    }

    #[test]
    fn test_ignore_files_default() {
        let config = ConfigFile::default();

        assert!(config.ignore_files.contains(&"Cargo.lock".to_string()));
        assert!(
            config
                .ignore_files
                .contains(&"package-lock.json".to_string())
        );
        assert!(config.ignore_files.contains(&"yarn.lock".to_string()));
        assert!(config.ignore_files.contains(&"pnpm-lock.yaml".to_string()));
        assert!(config.ignore_files.contains(&"*.lock".to_string()));
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
