use crate::config::AzureEndpoint;
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
