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
