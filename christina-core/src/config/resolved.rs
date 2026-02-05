use std::collections::HashMap;

use crate::{config::SecretString, profile::ProviderProfile};

/// Runtime configuration with resolved secrets
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// Active profile name
    pub active_profile: Option<String>,

    /// Provider profiles (with SecretString for runtime)
    pub profiles: HashMap<String, ProviderProfile<SecretString>>,

    /// Maximum commit message length
    pub commit_message_max_length: usize,

    /// Files to exclude from AI processing
    pub ignore_files: Vec<String>,

    /// Whether to enable file diffs in output
    pub include_file_diffs: bool,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            active_profile: None,
            profiles: HashMap::new(),
            commit_message_max_length: 72,
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

impl ResolvedConfig {
    /// Get the active profile, if any
    pub fn get_active_profile(&self) -> Option<&ProviderProfile<SecretString>> {
        self.active_profile
            .as_ref()
            .and_then(|name| self.profiles.get(name))
    }

    /// Get a profile by name
    pub fn get_profile(&self, name: &str) -> Option<&ProviderProfile<SecretString>> {
        self.profiles.get(name)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        config::SecretString,
        profile::ProviderProfile,
        types::{ModelName, ProviderKind},
    };

    #[test]
    fn test_get_active_profile() {
        let mut config = ResolvedConfig::default();
        let profile = ProviderProfile {
            name: "test".to_string(),
            provider: ProviderKind::OpenAI,
            model: ModelName::from("gpt-4"),
            api_url: None,
            api_key: crate::config::Secret::Value(SecretString::new("key".to_string())),
            max_input_tokens: crate::types::TokenCount::new_at_least_one(128000),
            max_output_tokens: crate::types::TokenCount::new_at_least_one(2048),
            azure_api_version: None,
            azure_deployment_id: None,
            temperature: None,
        };

        config.profiles.insert("test".to_string(), profile.clone());
        config.active_profile = Some("test".to_string());

        let active = config.get_active_profile();
        assert!(active.is_some());
        assert_eq!(active.unwrap().name, "test");
    }

    #[test]
    fn test_get_active_profile_missing() {
        let config = ResolvedConfig::default();

        let active = config.get_active_profile();
        assert!(active.is_none());
    }

    #[test]
    fn test_get_active_profile_not_found() {
        let config = ResolvedConfig {
            active_profile: Some("nonexistent".to_string()),
            ..ResolvedConfig::default()
        };

        let active = config.get_active_profile();
        assert!(active.is_none());
    }

    #[test]
    fn test_get_profile() {
        let mut config = ResolvedConfig::default();
        let profile = ProviderProfile {
            name: "myprofile".to_string(),
            provider: ProviderKind::OpenAI,
            model: ModelName::from("gpt-4"),
            api_url: None,
            api_key: crate::config::Secret::Value(SecretString::new("key".to_string())),
            max_input_tokens: crate::types::TokenCount::new_at_least_one(128000),
            max_output_tokens: crate::types::TokenCount::new_at_least_one(2048),
            azure_api_version: None,
            azure_deployment_id: None,
            temperature: None,
        };

        config
            .profiles
            .insert("myprofile".to_string(), profile.clone());

        let found = config.get_profile("myprofile");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "myprofile");

        let not_found = config.get_profile("other");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_default_values() {
        let config = ResolvedConfig::default();

        assert_eq!(config.active_profile, None);
        assert!(config.profiles.is_empty());
        assert_eq!(config.commit_message_max_length, 72);
        assert!(!config.include_file_diffs);
        assert_eq!(config.ignore_files.len(), 5);

        assert!(config.ignore_files.contains(&"Cargo.lock".to_string()));
        assert!(
            config
                .ignore_files
                .contains(&"package-lock.json".to_string())
        );
    }
}
