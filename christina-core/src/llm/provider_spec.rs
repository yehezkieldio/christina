use crate::config::AzureEndpoint;
use crate::error::ProviderError;
use crate::types::{ModelName, ProviderKind, TokenCount};
use url::Url;

/// Configuration for an LLM provider
#[derive(Debug, Clone)]
pub struct ProviderSpec {
    /// The kind of provider (OpenAI, Azure, Groq, etc.)
    pub kind: ProviderKind,
    /// The model name to use
    pub model: ModelName,
    /// Provider-specific endpoint configuration
    pub endpoint: ProviderEndpoint,
    /// Maximum tokens to generate
    pub max_tokens: TokenCount,
    /// Temperature for sampling
    pub temperature: f32,
}

impl ProviderSpec {
    pub fn validate(&self) -> Result<(), ProviderError> {
        self.validate_endpoint_consistency()?;
        self.validate_temperature()?;
        self.validate_url_scheme()?;
        Ok(())
    }

    fn validate_endpoint_consistency(&self) -> Result<(), ProviderError> {
        match (&self.kind, &self.endpoint) {
            (ProviderKind::OpenAI, ProviderEndpoint::OpenAi { .. }) => Ok(()),
            (ProviderKind::Azure, ProviderEndpoint::AzureOpenAi { .. }) => Ok(()),
            (ProviderKind::Groq, ProviderEndpoint::Groq { .. }) => Ok(()),
            (kind, endpoint) => Err(ProviderError::InvalidConfig(format!(
                "Provider kind {:?} is incompatible with endpoint variant {:?}",
                kind, endpoint
            ))),
        }
    }

    fn validate_temperature(&self) -> Result<(), ProviderError> {
        if self.temperature.is_nan() {
            return Err(ProviderError::InvalidConfig(
                "Temperature must be a valid number".to_string(),
            ));
        }
        if self.temperature < 0.0 || self.temperature > 2.0 {
            return Err(ProviderError::InvalidConfig(format!(
                "Temperature must be between 0.0 and 2.0, got {}",
                self.temperature
            )));
        }
        Ok(())
    }

    fn validate_url_scheme(&self) -> Result<(), ProviderError> {
        let scheme = match &self.endpoint {
            ProviderEndpoint::OpenAi { base_url } => base_url.scheme(),
            ProviderEndpoint::Groq { base_url } => base_url.scheme(),
            ProviderEndpoint::AzureOpenAi { endpoint, .. } => {
                endpoint.endpoint.split("://").next().unwrap_or("https")
            }
        };

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
    /// OpenAI-compatible endpoint
    OpenAi { base_url: Url },
    /// Azure OpenAI endpoint with deployment details
    AzureOpenAi {
        endpoint: AzureEndpoint,
        api_version: String,
        deployment_id: String,
    },
    /// Groq endpoint
    Groq { base_url: Url },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn provider_spec_creation() {
        let model = ModelName::from("gpt-4");
        let max_tokens = TokenCount::new(2048).unwrap();
        let base_url = Url::parse("https://api.openai.com/v1").unwrap();

        let spec = ProviderSpec {
            kind: ProviderKind::OpenAI,
            model,
            endpoint: ProviderEndpoint::OpenAi { base_url },
            max_tokens,
            temperature: 0.7,
        };

        assert_eq!(spec.kind, ProviderKind::OpenAI);
        assert_eq!(spec.temperature, 0.7);
    }

    #[test]
    fn provider_endpoint_openai() {
        let url = Url::parse("https://api.openai.com/v1").unwrap();
        let endpoint = ProviderEndpoint::OpenAi { base_url: url };

        match endpoint {
            ProviderEndpoint::OpenAi { base_url } => {
                assert_eq!(base_url.host_str(), Some("api.openai.com"));
            }
            _ => panic!("Expected OpenAi variant"),
        }
    }

    #[test]
    fn provider_endpoint_groq() {
        let url = Url::parse("https://api.groq.com/v1").unwrap();
        let endpoint = ProviderEndpoint::Groq { base_url: url };

        match endpoint {
            ProviderEndpoint::Groq { base_url } => {
                assert_eq!(base_url.host_str(), Some("api.groq.com"));
            }
            _ => panic!("Expected Groq variant"),
        }
    }
}
