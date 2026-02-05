use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{config::SecretRef, profile::ProviderProfile};

/// On-disk configuration representation (serde-friendly)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(default)]
pub struct ConfigFile {
    /// Active profile name
    pub active_profile: Option<String>,

    /// Provider profiles (with SecretRef for file storage)
    pub profiles: HashMap<String, ProviderProfile<SecretRef>>,

    /// Maximum commit message length (None = 72)
    pub commit_message_max_length: Option<usize>,

    /// Files to exclude from AI processing
    pub ignore_files: Vec<String>,

    /// Whether to enable file diffs in output
    pub include_file_diffs: bool,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            active_profile: None,
            profiles: HashMap::new(),
            commit_message_max_length: None,
            ignore_files: vec![
                "Cargo.lock".to_string(),
                "package-lock.json".to_string(),
                "yarn.lock".to_string(),
                "pnpm-lock.yaml".to_string(),
                "*.lock".to_string(),
            ],
            include_file_diffs: false,
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
        assert!(config.profiles.is_empty());
        assert_eq!(config.commit_message_max_length, None);
        assert!(!config.include_file_diffs);
        assert_eq!(config.ignore_files.len(), 5);
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = ConfigFile {
            active_profile: Some("default".to_string()),
            commit_message_max_length: Some(100),
            include_file_diffs: true,
            ..ConfigFile::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ConfigFile = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.active_profile, config.active_profile);
        assert_eq!(
            deserialized.commit_message_max_length,
            config.commit_message_max_length
        );
        assert_eq!(deserialized.include_file_diffs, config.include_file_diffs);
        assert_eq!(deserialized.ignore_files, config.ignore_files);
    }

    #[test]
    fn test_optional_fields() {
        let config = ConfigFile {
            active_profile: None,
            profiles: HashMap::new(),
            commit_message_max_length: None,
            ignore_files: vec![],
            include_file_diffs: false,
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
