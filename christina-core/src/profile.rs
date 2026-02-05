use std::collections::HashMap;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::{
    config::Secret,
    types::{
        ModelName, ProviderKind, TokenCount,
        token_count::{MAX_INPUT, MAX_OUTPUT},
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderProfile<S = String> {
    pub name: String,
    pub provider: ProviderKind,
    pub model: ModelName,
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
            api_key: Secret::Value(String::new()),
            max_input_tokens: TokenCount::new_at_least_one(128000),
            max_output_tokens: TokenCount::new_at_least_one(2048),
            azure_api_version: Some("2024-12-01-preview".to_string()),
            azure_deployment_id: None,
            temperature: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
