//! LLM provider profiles and configuration management.
//!
//! ## Generic Parameter Pattern
//!
//! Both `ProviderProfile<S>` and `Profiles<S>` use a generic parameter `S` to support
//! different secret storage strategies at compile time. This enables type-safe handling
//! of secrets across serialization boundaries.
//!
//! ### Use Cases
//!
//! **Disk Storage (`S = String` or `S = SecretRef`)**:
//! - Used in config files loaded from disk
//! - Secrets stored as references (env var names, keyring keys)
//! - Fully serializable via serde
//! - Example: `Profiles<String>` in `Config::profiles`
//!
//! **Runtime Resolution (`S = SecretString`)**:
//! - Used after resolving secret references to actual values
//! - Holds sensitive data in memory
//! - NOT serializable (SecretString deliberately omits Serialize)
//! - NOT comparable (SecretString deliberately omits PartialEq)
//! - Example: `ResolvedConfig::profiles` internally uses SecretString
//!
//! ### Design Rationale
//!
//! The generic pattern enforces a clean separation:
//! 1. Config files NEVER contain literal secrets (type system prevents serialization)
//! 2. Runtime secrets NEVER leak into config files (no Serialize impl)
//! 3. Secret references flow: Disk → SecretRef → resolve() → SecretString → use
//!
//! ### Common Pitfalls
//!
//! - **Cloning profiles**: Cloning `ProviderProfile<SecretString>` clones sensitive data
//! - **Serializing runtime profiles**: Attempting to serialize profiles with SecretString
//!   will fail at compile time (no Serialize implementation)
//! - **Comparing resolved secrets**: SecretString intentionally has no PartialEq to prevent
//!   timing attacks through comparison
//!
//! ### Migration Path
//!
//! To convert between storage and runtime representations:
//! ```ignore
//! // Disk → Runtime
//! let disk_profile: ProviderProfile<String> = load_from_file();
//! let resolved: ProviderProfile<SecretString> = resolve_profile(&disk_profile)?;
//!
//! // Runtime → Disk (loses secret values, only stores references)
//! let runtime_profile: ProviderProfile<SecretString> = /* ... */;
//! // Cannot directly serialize - must convert back to SecretRef representation
//! ```

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::{
    config::Secret,
    types::{
        ModelName, ProviderKind, TokenCount,
        tokens::{MAX_INPUT, MAX_OUTPUT},
    },
};

/// Configuration for an LLM provider (API credentials, model, token limits).
///
/// Generic parameter `S` determines secret storage strategy:
/// - `S = String` or `S = SecretRef`: Disk storage, fully serializable
/// - `S = SecretString`: Runtime secrets, NOT serializable
///
/// See module-level documentation for detailed usage patterns.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct ProviderProfile<S = String> {
    pub name: String,
    pub provider: ProviderKind,
    pub model: ModelName,
    #[cfg_attr(feature = "jsonschema", schemars(with = "Option<String>"))]
    pub api_url: Option<url::Url>,
    pub api_key: Secret<S>,
    pub max_input_tokens: TokenCount,
    pub max_output_tokens: TokenCount,
    pub azure_api_version: Option<String>,
    pub azure_deployment_id: Option<String>,
    pub temperature: Option<f32>,
}

impl<S> ProviderProfile<S> {
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("Profile name cannot be empty"));
        }

        if self.max_input_tokens.get() > MAX_INPUT {
            return Err(anyhow!("Max input tokens cannot exceed {}", MAX_INPUT));
        }

        if self.max_output_tokens.get() > MAX_OUTPUT {
            return Err(anyhow!("Max output tokens cannot exceed {}", MAX_OUTPUT));
        }

        Ok(())
    }
}

impl ProviderProfile<String> {
    pub fn new(name: String, provider: ProviderKind, model: ModelName) -> Self {
        Self {
            name,
            provider,
            model,
            api_url: None,
            api_key: Secret::EnvVar(provider.default_api_key_env_var().to_string()),
            max_input_tokens: TokenCount::new_at_least_one(128000),
            max_output_tokens: TokenCount::new_at_least_one(2048),
            azure_api_version: Some("2024-12-01-preview".to_string()),
            azure_deployment_id: None,
            temperature: None,
        }
    }
}

/// Collection of provider profiles with an active profile selection.
///
/// Generic parameter `S` determines secret storage strategy:
/// - `S = String`: Direct string values, serializable
/// - `S = SecretRef`: References to secrets (env vars, keyring), serializable
/// - `S = SecretString`: Resolved secret values, NOT serializable
///
/// The `definitions` field uses serde's `flatten` attribute to serialize profiles
/// directly into the parent config structure, enabling TOML files like:
/// ```toml
/// [profiles.default]
/// provider = "openai"
/// model = "gpt-4"
/// ```
///
/// See module-level documentation for usage patterns and best practices.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct Profiles<S = String> {
    pub active: Option<String>,
    #[serde(flatten)]
    pub definitions: HashMap<String, ProviderProfile<S>>,
}

impl<S> Profiles<S> {
    pub fn new() -> Self {
        Self {
            active: None,
            definitions: HashMap::new(),
        }
    }

    pub fn fix_names(&mut self) {
        for (key, profile) in &mut self.definitions {
            if profile.name.is_empty() {
                profile.name = key.clone();
            }
        }
    }

    pub fn add(&mut self, profile: ProviderProfile<S>) -> Result<()> {
        profile.validate()?;

        if self.definitions.contains_key(&profile.name) {
            return Err(anyhow!("Profile '{}' already exists", profile.name));
        }

        let name = profile.name.clone();
        self.definitions.insert(name, profile);

        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Result<()> {
        if self.definitions.remove(name).is_none() {
            return Err(anyhow!("Profile '{}' not found", name));
        }

        if self.active.as_deref() == Some(name) {
            self.active = None;
        }

        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&ProviderProfile<S>> {
        self.definitions.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut ProviderProfile<S>> {
        self.definitions.get_mut(name)
    }

    pub fn set_active(&mut self, name: &str) -> Result<()> {
        if !self.definitions.contains_key(name) {
            return Err(anyhow!("Profile '{}' not found", name));
        }
        self.active = Some(name.to_string());
        Ok(())
    }

    pub fn get_active(&self) -> Option<&ProviderProfile<S>> {
        self.active.as_ref().and_then(|name| self.get(name))
    }

    pub fn list_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.definitions.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn exists(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    pub fn update(&mut self, name: &str, profile: ProviderProfile<S>) -> Result<()> {
        profile.validate()?;

        if !self.definitions.contains_key(name) {
            return Err(anyhow!("Profile '{}' not found", name));
        }

        if profile.name != name {
            return Err(anyhow!(
                "Profile name mismatch: expected '{}', got '{}'. Use remove() + add() to rename.",
                name,
                profile.name
            ));
        }

        self.definitions.insert(name.to_string(), profile);
        Ok(())
    }
}

impl<S> Default for Profiles<S> {
    fn default() -> Self {
        Self {
            active: None,
            definitions: HashMap::new(),
        }
    }
}

// Compile-time assertions to verify generic parameter behavior.
//
// These assertions ensure that the generic pattern maintains expected properties:
// - Disk storage variants (String) are serializable
// - SecretString is Clone but deliberately NOT PartialEq or Serialize
#[allow(dead_code)]
const _: () = {
    use crate::config::SecretString;

    // Verify that String variant implements Serialize + Deserialize
    const fn assert_serde<T: Serialize + for<'de> Deserialize<'de>>() {}

    const fn _check_string_variant() {
        assert_serde::<ProviderProfile<String>>();
        assert_serde::<Profiles<String>>();
    }

    // Verify SecretString is Clone (needed for practical use cases)
    const fn assert_clone<T: Clone>() {}

    const fn _check_secretstring_clone() {
        assert_clone::<SecretString>();
    }

    // Note: We cannot directly assert !Serialize or !PartialEq in stable Rust,
    // but SecretString deliberately omits these traits to prevent accidental
    // serialization or timing-attack-vulnerable comparisons. The design ensures
    // that attempting to serialize ProviderProfile<SecretString> will fail at
    // compile time due to the missing Serialize bound on SecretString.
    //
    // SecretRef is also serializable but doesn't have Default, so we omit it
    // from these assertions. In practice, Profiles<SecretRef> works correctly
    // when constructed non-default, which is the normal usage pattern.
};

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn profile_validation() {
        let profile = ProviderProfile::new(
            "test".to_string(),
            ProviderKind::OpenAI,
            ModelName::from("gpt-5-nano"),
        );
        assert!(profile.validate().is_ok());

        let invalid = ProviderProfile {
            name: "".to_string(),
            ..profile
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn profiles_manager() {
        let mut manager = Profiles::new();
        let profile = ProviderProfile::new(
            "test".to_string(),
            ProviderKind::OpenAI,
            ModelName::from("gpt-4.1-mini"),
        );

        assert!(manager.add(profile.clone()).is_ok());
        assert!(manager.exists("test"));
        assert_eq!(manager.get("test").unwrap().name, "test");

        assert!(manager.set_active("test").is_ok());
        assert_eq!(manager.get_active().unwrap().name, "test");

        assert!(manager.remove("test").is_ok());
        assert!(!manager.exists("test"));
        assert!(manager.get_active().is_none());
    }
}
