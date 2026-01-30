use serde::{Deserialize, Serialize};

use crate::profile::{Profiles, ProviderProfile};
use crate::types::{ModelName, ProviderKind};

/// Serializable settings format for storage.
///
/// This struct represents the on-disk format for configuration files.
/// It separates storage concerns from runtime state, allowing the storage
/// format to evolve independently from runtime needs.
///
/// Key design decisions:
/// - Provider-specific settings live in Profiles, not duplicated here
/// - Only global/non-provider settings are at the top level
/// - Profile resolution happens during load(), producing RuntimeConfig
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Profile management - includes definitions and active profile name
    pub profiles: Profiles,

    /// Diff tool configuration
    pub diff: DiffSettings,

    /// Files to ignore when generating commits
    pub ignore_files: Vec<String>,

    /// Commit message settings
    pub commit_message: CommitMessageSettings,

    /// Whether to include commit history in context
    pub use_commit_history: bool,

    /// Number of commits to include in history
    pub commit_history_depth: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            profiles: Profiles::new(),
            diff: DiffSettings::default(),
            ignore_files: vec![
                "Cargo.lock".to_string(),
                "package-lock.json".to_string(),
                "yarn.lock".to_string(),
                "pnpm-lock.yaml".to_string(),
                "*.lock".to_string(),
            ],
            commit_message: CommitMessageSettings::default(),
            use_commit_history: true,
            commit_history_depth: 5,
        }
    }
}

impl Settings {
    /// Create a new Settings with a default profile.
    ///
    /// This is used when no configuration file exists yet.
    pub fn with_default_profile() -> Self {
        let mut settings = Self::default();

        // Create a default OpenAI profile
        let default_profile = ProviderProfile::new(
            "default".to_string(),
            ProviderKind::OpenAI,
            ModelName::from("gpt-4o-mini"),
        );

        // Add the profile and set it as active
        // We use insert directly since we know the profiles map is empty
        settings
            .profiles
            .definitions
            .insert("default".to_string(), default_profile);
        settings.profiles.active = Some("default".to_string());

        settings
    }

    /// Get the active profile, or create a default one if none exists.
    pub fn ensure_active_profile(&mut self) -> &ProviderProfile {
        if self.profiles.get_active().is_none() {
            // No active profile, create default
            let default_profile = ProviderProfile::new(
                "default".to_string(),
                ProviderKind::OpenAI,
                ModelName::from("gpt-4o-mini"),
            );

            self.profiles.active = Some("default".to_string());
            self.profiles
                .definitions
                .insert("default".to_string(), default_profile);
        }

        // We just ensured an active profile exists, so this is safe
        #[allow(clippy::expect_used)]
        self.profiles
            .get_active()
            .expect("Active profile should exist after ensure_active_profile()")
    }
}

/// Settings for the diff tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DiffSettings {
    /// The diff tool to use (delta, diff-so-fancy, etc.)
    pub tool: Option<String>,

    /// Whether to show a preview of the diff
    pub show_preview: bool,
}

impl Default for DiffSettings {
    fn default() -> Self {
        Self {
            tool: None,
            show_preview: true,
        }
    }
}

/// Settings for commit message generation and validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CommitMessageSettings {
    /// Maximum length for generated commit messages
    pub max_length: usize,

    /// Validation mode for commit messages
    pub validation_mode: ValidationMode,
}

impl Default for CommitMessageSettings {
    fn default() -> Self {
        Self {
            max_length: 72,
            validation_mode: ValidationMode::Strict,
        }
    }
}

/// Validation mode for commit messages.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ValidationMode {
    /// Strict validation - enforces all rules
    #[default]
    Strict,
    /// Soft validation - warns but allows
    Soft,
    /// No validation
    Disabled,
}

impl ValidationMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValidationMode::Strict => "strict",
            ValidationMode::Soft => "soft",
            ValidationMode::Disabled => "disabled",
        }
    }
}

impl std::str::FromStr for ValidationMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "strict" => Ok(ValidationMode::Strict),
            "soft" => Ok(ValidationMode::Soft),
            "disabled" => Ok(ValidationMode::Disabled),
            _ => Err(format!("Unknown validation mode: {}", s)),
        }
    }
}

impl std::fmt::Display for ValidationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_default() {
        let settings = Settings::default();
        assert!(settings.profiles.definitions.is_empty());
        assert_eq!(settings.commit_history_depth, 5);
        assert!(settings.use_commit_history);
    }

    #[test]
    fn test_settings_with_default_profile() {
        let settings = Settings::with_default_profile();
        assert!(settings.profiles.definitions.contains_key("default"));
        assert_eq!(settings.profiles.active, Some("default".to_string()));
    }

    #[test]
    fn test_ensure_active_profile() {
        let mut settings = Settings::default();
        assert!(settings.profiles.get_active().is_none());

        let profile = settings.ensure_active_profile();
        assert_eq!(profile.name, "default");
        assert_eq!(settings.profiles.active, Some("default".to_string()));
    }

    #[test]
    fn test_validation_mode_roundtrip() {
        for mode in [
            ValidationMode::Strict,
            ValidationMode::Soft,
            ValidationMode::Disabled,
        ] {
            let s = mode.as_str();
            let parsed: ValidationMode = s.parse().unwrap();
            assert_eq!(mode, parsed);
        }
    }
}
