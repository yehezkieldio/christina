//! Provider configuration shapes with validation rules.
//!
//! WHY validate here: keeps provider constraints close to the data model so
//! CLI/config parsing can fail fast before any network calls.

use crate::config::AzureEndpoint;
use crate::error::ProviderError;
use crate::types::{ModelName, ProviderKind, Temperature, TokenCount};

/// Configuration for an LLM provider
#[derive(Debug, Clone)]
pub struct ProviderSpec {
    /// The kind of provider (Azure, etc.)
    pub kind: ProviderKind,
    /// The model name to use
    pub model: ModelName,
    /// Provider-specific endpoint configuration
    pub endpoint: ProviderEndpoint,
    /// Maximum tokens to generate
    pub max_tokens: TokenCount,
    /// Temperature for sampling
    pub temperature: Temperature,
}

impl ProviderSpec {
    pub fn validate(&self) -> Result<(), ProviderError> {
        self.validate_url_scheme()?;
        Ok(())
    }

    fn validate_url_scheme(&self) -> Result<(), ProviderError> {
        let scheme = match &self.endpoint {
            ProviderEndpoint::AzureOpenAi { endpoint, .. } => {
                endpoint.endpoint.split("://").next().unwrap_or("https")
            }
        };

        // HTTPS-only avoids accidental credential leakage on insecure transport.
        if scheme != "https" {
            return Err(ProviderError::InvalidConfig(format!(
                "URL scheme must be HTTPS for security, got {}",
                scheme
            )));
        }

        Ok(())
    }
}

/// Provider-specific endpoint configuration
#[derive(Debug, Clone)]
pub enum ProviderEndpoint {
    /// Azure OpenAI endpoint with deployment details
    AzureOpenAi {
        endpoint: AzureEndpoint,
        api_version: String,
        deployment_id: String,
    },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn validate_temperature_nan() {
        assert!(Temperature::try_new(f32::NAN).is_err());
    }

    #[test]
    fn validate_temperature_out_of_range_low() {
        assert!(Temperature::try_new(-0.5).is_err());
    }

    #[test]
    fn validate_temperature_out_of_range_high() {
        assert!(Temperature::try_new(3.0).is_err());
    }

    #[test]
    fn validate_temperature_boundary_values() {
        assert!(Temperature::try_new(0.0).is_ok());
        assert!(Temperature::try_new(2.0).is_ok());
    }

    #[test]
    fn validate_endpoint_consistency_azure_ok() {
        let spec = ProviderSpec {
            kind: ProviderKind::Azure,
            model: ModelName::from("gpt-4"),
            endpoint: ProviderEndpoint::AzureOpenAi {
                endpoint: AzureEndpoint {
                    endpoint: "https://test.openai.azure.com".to_string(),
                    api_version: "2024-02-15".to_string(),
                    deployment_id: "gpt-4".to_string(),
                },
                api_version: "2024-02-15".to_string(),
                deployment_id: "gpt-4".to_string(),
            },
            max_tokens: TokenCount::new(2048).unwrap(),
            temperature: Temperature::try_new(0.7).unwrap(),
        };

        assert!(spec.validate().is_ok());
    }

    #[test]
    fn validate_url_scheme_https_ok() {
        let spec = ProviderSpec {
            kind: ProviderKind::Azure,
            model: ModelName::from("gpt-4"),
            endpoint: ProviderEndpoint::AzureOpenAi {
                endpoint: AzureEndpoint {
                    endpoint: "https://test.openai.azure.com".to_string(),
                    api_version: "2024-02-15".to_string(),
                    deployment_id: "gpt-4".to_string(),
                },
                api_version: "2024-02-15".to_string(),
                deployment_id: "gpt-4".to_string(),
            },
            max_tokens: TokenCount::new(2048).unwrap(),
            temperature: Temperature::try_new(0.7).unwrap(),
        };

        assert!(spec.validate().is_ok());
    }

    #[test]
    fn validate_url_scheme_http_rejected() {
        let spec = ProviderSpec {
            kind: ProviderKind::Azure,
            model: ModelName::from("gpt-4"),
            endpoint: ProviderEndpoint::AzureOpenAi {
                endpoint: AzureEndpoint {
                    endpoint: "http://test.openai.azure.com".to_string(), // Using http
                    api_version: "2024-02-15".to_string(),
                    deployment_id: "gpt-4".to_string(),
                },
                api_version: "2024-02-15".to_string(),
                deployment_id: "gpt-4".to_string(),
            },
            max_tokens: TokenCount::new(2048).unwrap(),
            temperature: Temperature::try_new(0.7).unwrap(),
        };

        let result = spec.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("HTTPS"));
    }
}
