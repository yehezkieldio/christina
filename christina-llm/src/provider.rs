use anyhow::Result;
use christina_core::{
    ProviderProfile,
    types::{ModelName, ProviderKind, TokenCount},
};
use url::Url;

use crate::providers::azure::parse_azure_url;

#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED:api-key]")
    }
}

impl From<String> for ApiKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ApiKey {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Role of a chat message participant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatRole {
    /// System message (instructions for the model)
    System,
    /// User message (input from the user)
    User,
}

/// A single message in a chat conversation.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Role of the message sender
    pub role: ChatRole,
    /// Content of the message
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }
}

/// Error type for completion provider operations.
#[derive(Debug, thiserror::Error)]
pub enum CompletionError {
    /// Unauthorized (401) - invalid or expired API key
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    /// Rate limited (429)
    #[error("Rate limited")]
    RateLimited,
    /// Request timed out
    #[error("Request timed out")]
    Timeout,
    /// Server error (500, 502, 503, 504)
    #[error("Server error: {0}")]
    ServerError(String),
    /// Network/connection error
    #[error("Network error: {0}")]
    NetworkError(String),
    /// Invalid or unparseable response
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

impl CompletionError {
    /// Check if this error is transient and should be retried.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            CompletionError::RateLimited
                | CompletionError::Timeout
                | CompletionError::ServerError(_)
                | CompletionError::NetworkError(_)
        )
    }

    /// Check if this error indicates a provider-level failure that should abort all processing.
    pub fn is_provider_error(&self) -> bool {
        matches!(
            self,
            CompletionError::Unauthorized(_)
                | CompletionError::RateLimited
                | CompletionError::ServerError(_)
        )
    }

    /// Create from an LLM library error string by parsing common patterns.
    pub fn from_api_error(msg: &str) -> Self {
        let lower = msg.to_lowercase();

        if lower.contains("401")
            || lower.contains("unauthorized")
            || lower.contains("invalid api key")
        {
            return CompletionError::Unauthorized(msg.to_string());
        }
        if lower.contains("429") || lower.contains("rate limit") || lower.contains("quota") {
            return CompletionError::RateLimited;
        }
        if lower.contains("timeout") || lower.contains("timed out") {
            return CompletionError::Timeout;
        }
        if lower.contains("500")
            || lower.contains("502")
            || lower.contains("503")
            || lower.contains("504")
            || lower.contains("server error")
            || lower.contains("overloaded")
        {
            return CompletionError::ServerError(msg.to_string());
        }
        if lower.contains("connection")
            || lower.contains("network")
            || lower.contains("dns")
            || lower.contains("resolve")
        {
            return CompletionError::NetworkError(msg.to_string());
        }

        // Default to server error for unknown API errors
        CompletionError::ServerError(msg.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub name: ProviderKind,
}

/// Registry of supported providers.
pub const SUPPORTED_PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        name: ProviderKind::OpenAI,
    },
    ProviderInfo {
        name: ProviderKind::Azure,
    },
];

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Missing required configuration: {0}")]
    MissingConfig(String),
}

#[derive(Debug)]
pub enum Provider {
    OpenAI {
        model: ModelName,
        api_key: ApiKey,
        base_url: Option<Url>,
        max_tokens: TokenCount,
        temperature: f32,
    },
    Azure {
        model: ModelName,
        api_key: ApiKey,
        endpoint: String,
        api_version: String,
        deployment_id: String,
        max_tokens: TokenCount,
        temperature: f32,
    },
    Mock {
        response: String,
        delay_ms: u64,
    },
    MockSequence {
        responses: std::sync::Arc<std::sync::Mutex<Vec<Result<String, CompletionError>>>>,
        delay_ms: u64,
    },
}

impl Provider {
    pub fn from_profile(profile: &ProviderProfile, api_key: &str) -> Result<Self> {
        match profile.provider {
            ProviderKind::OpenAI => Ok(Provider::OpenAI {
                model: profile.model.clone(),
                api_key: ApiKey::new(api_key),
                base_url: profile.api_url.clone(),
                max_tokens: profile.max_output_tokens,
                temperature: 0.3,
            }),

            ProviderKind::Azure => {
                let parsed = profile
                    .api_url
                    .as_ref()
                    .and_then(|url| parse_azure_url(url.as_str()));

                let (endpoint, api_version, deployment_id) = if let Some(parsed) = parsed {
                    (
                        parsed.endpoint,
                        profile
                            .azure_api_version
                            .clone()
                            .unwrap_or(parsed.api_version),
                        profile
                            .azure_deployment_id
                            .clone()
                            .unwrap_or(parsed.deployment_id),
                    )
                } else {
                    let endpoint = profile
                        .api_url
                        .clone()
                        .ok_or_else(|| {
                            ProviderError::MissingConfig("model_api_url (Azure endpoint)".into())
                        })?
                        .to_string();

                    let api_version = profile
                        .azure_api_version
                        .clone()
                        .ok_or_else(|| ProviderError::MissingConfig("azure_api_version".into()))?;

                    let deployment_id = profile.azure_deployment_id.clone().ok_or_else(|| {
                        ProviderError::MissingConfig("azure_deployment_id".into())
                    })?;

                    (endpoint, api_version, deployment_id)
                };

                Ok(Provider::Azure {
                    model: profile.model.clone(),
                    api_key: ApiKey::new(api_key),
                    endpoint,
                    api_version,
                    deployment_id,
                    max_tokens: profile.max_output_tokens,
                    temperature: 0.3,
                })
            }
        }
    }

    pub async fn generate(&self, messages: &[ChatMessage]) -> Result<String, CompletionError> {
        match self {
            Provider::OpenAI {
                model,
                api_key,
                base_url,
                max_tokens,
                temperature,
            } => {
                crate::providers::openai::generate(
                    model,
                    api_key.as_str(),
                    base_url.as_ref(),
                    *max_tokens,
                    *temperature,
                    messages,
                )
                .await
            }

            Provider::Azure {
                model,
                api_key,
                endpoint,
                api_version,
                deployment_id,
                max_tokens,
                temperature,
            } => {
                crate::providers::azure::generate(crate::providers::azure::AzureGenRequest {
                    model,
                    api_key: api_key.as_str(),
                    endpoint,
                    api_version,
                    deployment_id,
                    max_tokens: *max_tokens,
                    temperature: *temperature,
                    messages,
                })
                .await
            }

            Provider::Mock { response, delay_ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
                Ok(response.clone())
            }
            Provider::MockSequence {
                responses,
                delay_ms,
            } => {
                tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;

                {
                    let mut guard = responses
                        .lock()
                        .map_err(|_| CompletionError::NetworkError("mock lock poisoned".into()))?;
                    if guard.is_empty() {
                        return Err(CompletionError::InvalidResponse(
                            "mock sequence exhausted".into(),
                        ));
                    }
                    guard.remove(0)
                }
            }
        }
    }

    pub fn mock(response: impl Into<String>) -> Self {
        Provider::Mock {
            response: response.into(),
            delay_ms: 100,
        }
    }

    pub fn mock_with_delay(response: impl Into<String>, delay_ms: u64) -> Self {
        Provider::Mock {
            response: response.into(),
            delay_ms,
        }
    }

    pub fn mock_sequence(responses: Vec<Result<String, CompletionError>>) -> Self {
        Provider::MockSequence {
            responses: std::sync::Arc::new(std::sync::Mutex::new(responses)),
            delay_ms: 0,
        }
    }

    pub fn mock_sequence_with_delay(
        responses: Vec<Result<String, CompletionError>>,
        delay_ms: u64,
    ) -> Self {
        Provider::MockSequence {
            responses: std::sync::Arc::new(std::sync::Mutex::new(responses)),
            delay_ms,
        }
    }
}
