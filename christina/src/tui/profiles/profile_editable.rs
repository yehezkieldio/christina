use crate::tui::form::editable::{Editable, FieldDef, FieldType};
use anyhow::{anyhow, Result};
use christina_core::{profile::ProviderProfile, types::ProviderKind};

impl Editable for ProviderProfile {
    fn fields(&self) -> Vec<FieldDef> {
        let mut fields = vec![
            FieldDef::new("name", "Profile Name")
                .help("Unique identifier for this profile")
                .required(),
            FieldDef::new("provider", "Provider")
                .help("AI provider: openai, azure, groq, anthropic, etc.")
                .required(),
            FieldDef::new("model", "Model")
                .help("Model name (e.g., gpt-4, claude-3-5-sonnet)")
                .required(),
            FieldDef::new("max_input_tokens", "Max Input Tokens")
                .help("Maximum input tokens for this profile")
                .field_type(FieldType::Number {
                    min: Some(1),
                    max: Some(128000),
                })
                .required(),
            FieldDef::new("max_output_tokens", "Max Output Tokens")
                .help("Maximum output tokens for this profile")
                .field_type(FieldType::Number {
                    min: Some(1),
                    max: Some(4096),
                })
                .required(),
            FieldDef::new("api_url", "API URL")
                .help("Custom API endpoint (optional, for proxies or self-hosted)"),
        ];

        // Add Azure-specific fields if provider is Azure
        if self.provider == ProviderKind::Azure {
            fields.push(
                FieldDef::new("azure_api_version", "Azure API Version")
                    .help("Azure OpenAI API version (e.g., 2024-12-01-preview)"),
            );
            fields.push(
                FieldDef::new("azure_deployment_id", "Azure Deployment ID")
                    .help("Azure deployment/model name")
                    .required(),
            );
        }

        fields
    }

    fn get_field(&self, key: &str) -> Option<String> {
        match key {
            "name" => Some(self.name.clone()),
            "provider" => Some(self.provider.to_string()),
            "model" => Some(self.model.as_str().to_string()),
            "api_url" => self.api_url.as_ref().map(|u| u.to_string()),
            "api_key" => match &self.api_key {
                christina_core::config::Secret::Value(key) => Some(key.clone()),
                christina_core::config::Secret::EnvVar(name) => Some(format!("env:{}", name)),
                christina_core::config::Secret::Keyring(key) => Some(format!("keyring:{}", key)),
            },
            "max_input_tokens" => Some(self.max_input_tokens.get().to_string()),
            "max_output_tokens" => Some(self.max_output_tokens.get().to_string()),
            "azure_api_version" => self.azure_api_version.clone(),
            "azure_deployment_id" => self.azure_deployment_id.clone(),
            _ => None,
        }
    }

    fn set_field(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "name" => {
                if value.trim().is_empty() {
                    return Err(anyhow!("Profile name cannot be empty"));
                }
                self.name = value.to_string();
            }
            "provider" => {
                self.provider = value
                    .parse()
                    .map_err(|_| anyhow!("Invalid provider: {}", value))?;
            }
            "model" => {
                self.model = christina_core::types::ModelName::from(value);
            }
            "api_url" => {
                if value.is_empty() {
                    self.api_url = None;
                } else {
                    self.api_url = Some(
                        value
                            .parse()
                            .map_err(|_| anyhow!("Invalid API URL: {}", value))?,
                    );
                }
            }
            "api_key" => {
                if value.is_empty() {
                    self.api_key = christina_core::config::Secret::Value(String::new());
                } else {
                    self.api_key = christina_core::config::Secret::Value(value.to_string());
                }
            }
            "max_input_tokens" => {
                let val = value
                    .parse::<u32>()
                    .map_err(|_| anyhow!("Invalid token count: {}", value))?;
                self.max_input_tokens = christina_core::types::TokenCount::new_saturating(val);
            }
            "max_output_tokens" => {
                let val = value
                    .parse::<u32>()
                    .map_err(|_| anyhow!("Invalid token count: {}", value))?;
                self.max_output_tokens = christina_core::types::TokenCount::new_saturating(val);
            }
            "azure_api_version" => {
                if value.is_empty() {
                    self.azure_api_version = None;
                } else {
                    self.azure_api_version = Some(value.to_string());
                }
            }
            "azure_deployment_id" => {
                if value.is_empty() {
                    self.azure_deployment_id = None;
                } else {
                    self.azure_deployment_id = Some(value.to_string());
                }
            }
            _ => return Err(anyhow!("Unknown field: {}", key)),
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(anyhow!("Profile name cannot be empty"));
        }

        if self.model.as_str().trim().is_empty() {
            return Err(anyhow!("Model cannot be empty"));
        }

        let input_tokens = self.max_input_tokens.get();
        if input_tokens == 0 || input_tokens > 128_000 {
            return Err(anyhow!(
                "Max input tokens must be between 1 and 128000, got {}",
                input_tokens
            ));
        }

        let output_tokens = self.max_output_tokens.get();
        if output_tokens == 0 || output_tokens > 4096 {
            return Err(anyhow!(
                "Max output tokens must be between 1 and 4096, got {}",
                output_tokens
            ));
        }

        if self.provider == ProviderKind::Azure
            && self
                .azure_deployment_id
                .as_ref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
        {
            return Err(anyhow!(
                "Azure deployment ID is required when using Azure provider"
            ));
        }

        Ok(())
    }
}
