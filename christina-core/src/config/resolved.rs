//! Runtime configuration after secret resolution.
//!
//! WHY separate from `ConfigFile`: runtime uses resolved secrets and concrete
//! defaults so downstream code never handles `Option` or placeholder secrets.

use std::collections::HashMap;

use crate::{config::SecretString, profile::ProviderProfile};

/// Runtime configuration with resolved secrets
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// Active profile name
    pub active_profile: Option<String>,

    /// Provider profiles with runtime-resolved secrets.
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
            // Align with conventional commit summary width.
            commit_message_max_length: 72,
            ignore_files: Vec::new(),
            include_file_diffs: false,
        }
    }
}

impl ResolvedConfig {
    /// Get the active profile, if any
    #[must_use]
    pub fn get_active_profile(&self) -> Option<&ProviderProfile<SecretString>> {
        self.active_profile
            .as_ref()
            .and_then(|name| self.profiles.get(name))
    }

    /// Get a profile by name
    #[must_use]
    pub fn get_profile(&self, name: &str) -> Option<&ProviderProfile<SecretString>> {
        self.profiles.get(name)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
    fn test_default_values() {
        let config = ResolvedConfig::default();

        assert_eq!(config.active_profile, None);
        assert!(config.profiles.is_empty());
        assert_eq!(config.commit_message_max_length, 72);
        assert!(!config.include_file_diffs);
        assert!(config.ignore_files.is_empty());
    }
}
