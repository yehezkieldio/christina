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

impl Default for Provider {
    fn default() -> Self {
        #[cfg(test)]
        {
            Provider::Mock {
                response: "feat(core): implement async TUI event loop".to_string(),
                delay_ms: 0,
            }
        }
        #[cfg(not(test))]
        {
            Provider::OpenAI {
                model: ModelName::from("gpt-4"),
                api_key: ApiKey::new(""),
                base_url: None,
                max_tokens: TokenCount::new_saturating(1024),
                temperature: 0.3,
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn base_profile(provider: ProviderKind) -> ProviderProfile {
        ProviderProfile::new("test".to_string(), provider, ModelName::from("gpt-4"))
    }

    #[test]
    fn azure_missing_config() {
        let profile = base_profile(ProviderKind::Azure);

        let result = Provider::from_profile(&profile, "key");
        assert!(result.is_err());

        if let Err(e) = result {
            let err_msg = e.to_string();
            assert!(err_msg.contains("model_api_url"));
        }
    }

    #[tokio::test]
    async fn mock_provider_response() {
        let provider = Provider::mock("test response");
        let messages = vec![ChatMessage::user("Hello")];

        let result = provider
            .generate(&messages)
            .await
            .unwrap_or_else(|e| panic!("generate should succeed, got error: {}", e));
        assert_eq!(result, "test response");
    }

    #[tokio::test]
    async fn mock_provider_default() {
        let provider = Provider::default();
        let messages = vec![ChatMessage::user("Hello")];

        let result = provider
            .generate(&messages)
            .await
            .unwrap_or_else(|e| panic!("generate should succeed, got error: {}", e));
        assert!(result.contains("feat"));
    }

    #[test]
    fn from_api_error_401_status() {
        let err = CompletionError::from_api_error("Error 401: Unauthorized");
        assert!(matches!(err, CompletionError::Unauthorized(_)));
    }

    #[test]
    fn from_api_error_401_status_lowercase() {
        let err = CompletionError::from_api_error("HTTP 401 error");
        assert!(matches!(err, CompletionError::Unauthorized(_)));
    }

    #[test]
    fn from_api_error_unauthorized_keyword() {
        let err = CompletionError::from_api_error("Unauthorized access denied");
        assert!(matches!(err, CompletionError::Unauthorized(_)));
    }

    #[test]
    fn from_api_error_unauthorized_case_insensitive() {
        let err = CompletionError::from_api_error("UNAUTHORIZED REQUEST FAILED");
        assert!(matches!(err, CompletionError::Unauthorized(_)));
    }

    #[test]
    fn from_api_error_invalid_api_key() {
        let err = CompletionError::from_api_error("Invalid API key provided");
        assert!(matches!(err, CompletionError::Unauthorized(_)));
    }

    #[test]
    fn from_api_error_invalid_api_key_case_insensitive() {
        let err = CompletionError::from_api_error("INVALID API KEY");
        assert!(matches!(err, CompletionError::Unauthorized(_)));
    }

    #[test]
    fn from_api_error_unauthorized_preserves_message() {
        let msg = "401: Invalid or expired API key";
        let err = CompletionError::from_api_error(msg);
        if let CompletionError::Unauthorized(msg_content) = err {
            assert_eq!(msg_content, msg);
        } else {
            panic!("Expected Unauthorized variant");
        }
    }

    #[test]
    fn from_api_error_429_status() {
        let err = CompletionError::from_api_error("Error 429: Too Many Requests");
        assert!(matches!(err, CompletionError::RateLimited));
    }

    #[test]
    fn from_api_error_429_status_lowercase() {
        let err = CompletionError::from_api_error("http 429");
        assert!(matches!(err, CompletionError::RateLimited));
    }

    #[test]
    fn from_api_error_rate_limit_keyword() {
        let err = CompletionError::from_api_error("Rate limit exceeded");
        assert!(matches!(err, CompletionError::RateLimited));
    }

    #[test]
    fn from_api_error_rate_limit_case_insensitive() {
        let err = CompletionError::from_api_error("RATE LIMITING APPLIED");
        assert!(matches!(err, CompletionError::RateLimited));
    }

    #[test]
    fn from_api_error_quota_exceeded() {
        let err = CompletionError::from_api_error("Quota exceeded for this month");
        assert!(matches!(err, CompletionError::RateLimited));
    }

    #[test]
    fn from_api_error_quota_case_insensitive() {
        let err = CompletionError::from_api_error("YOUR QUOTA HAS BEEN EXCEEDED");
        assert!(matches!(err, CompletionError::RateLimited));
    }

    #[test]
    fn from_api_error_timeout_keyword() {
        let err = CompletionError::from_api_error("Request timeout");
        assert!(matches!(err, CompletionError::Timeout));
    }

    #[test]
    fn from_api_error_timeout_case_insensitive() {
        let err = CompletionError::from_api_error("TIMEOUT OCCURRED");
        assert!(matches!(err, CompletionError::Timeout));
    }

    #[test]
    fn from_api_error_timed_out() {
        let err = CompletionError::from_api_error("Connection timed out waiting for response");
        assert!(matches!(err, CompletionError::Timeout));
    }

    #[test]
    fn from_api_error_timed_out_case_insensitive() {
        let err = CompletionError::from_api_error("REQUEST TIMED OUT");
        assert!(matches!(err, CompletionError::Timeout));
    }

    #[test]
    fn from_api_error_500_status() {
        let err = CompletionError::from_api_error("HTTP 500 Internal Server Error");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_500_status_lowercase() {
        let err = CompletionError::from_api_error("error 500");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_502_status() {
        let err = CompletionError::from_api_error("HTTP 502 Bad Gateway");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_502_status_lowercase() {
        let err = CompletionError::from_api_error("error 502");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_503_status() {
        let err = CompletionError::from_api_error("HTTP 503 Service Unavailable");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_503_status_lowercase() {
        let err = CompletionError::from_api_error("error 503");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_504_status() {
        let err = CompletionError::from_api_error("HTTP 504 Bad Gateway");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_504_status_lowercase() {
        let err = CompletionError::from_api_error("error 504");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_server_error_keyword() {
        let err = CompletionError::from_api_error("Server error occurred");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_server_error_case_insensitive() {
        let err = CompletionError::from_api_error("SERVER ERROR");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_overloaded() {
        let err = CompletionError::from_api_error("Service is currently overloaded");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_overloaded_case_insensitive() {
        let err = CompletionError::from_api_error("THE SERVER IS OVERLOADED");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_server_error_preserves_message() {
        let msg = "HTTP 500: Internal Server Error";
        let err = CompletionError::from_api_error(msg);
        if let CompletionError::ServerError(msg_content) = err {
            assert_eq!(msg_content, msg);
        } else {
            panic!("Expected ServerError variant");
        }
    }

    #[test]
    fn from_api_error_connection_error() {
        let err = CompletionError::from_api_error("Connection refused");
        assert!(matches!(err, CompletionError::NetworkError(_)));
    }

    #[test]
    fn from_api_error_connection_case_insensitive() {
        let err = CompletionError::from_api_error("CONNECTION FAILED");
        assert!(matches!(err, CompletionError::NetworkError(_)));
    }

    #[test]
    fn from_api_error_network_error() {
        let err = CompletionError::from_api_error("Network error: unable to connect");
        assert!(matches!(err, CompletionError::NetworkError(_)));
    }

    #[test]
    fn from_api_error_network_case_insensitive() {
        let err = CompletionError::from_api_error("NETWORK UNREACHABLE");
        assert!(matches!(err, CompletionError::NetworkError(_)));
    }

    #[test]
    fn from_api_error_dns_error() {
        let err = CompletionError::from_api_error("DNS resolution failed");
        assert!(matches!(err, CompletionError::NetworkError(_)));
    }

    #[test]
    fn from_api_error_dns_case_insensitive() {
        let err = CompletionError::from_api_error("DNS LOOKUP FAILURE");
        assert!(matches!(err, CompletionError::NetworkError(_)));
    }

    #[test]
    fn from_api_error_resolve_error() {
        let err = CompletionError::from_api_error("Failed to resolve hostname");
        assert!(matches!(err, CompletionError::NetworkError(_)));
    }

    #[test]
    fn from_api_error_resolve_case_insensitive() {
        let err = CompletionError::from_api_error("CANNOT RESOLVE NAME");
        assert!(matches!(err, CompletionError::NetworkError(_)));
    }

    #[test]
    fn from_api_error_network_error_preserves_message() {
        let msg = "Network error: connection refused";
        let err = CompletionError::from_api_error(msg);
        if let CompletionError::NetworkError(msg_content) = err {
            assert_eq!(msg_content, msg);
        } else {
            panic!("Expected NetworkError variant");
        }
    }

    #[test]
    fn from_api_error_unknown_error_defaults_to_server_error() {
        let err = CompletionError::from_api_error("Some unknown error message");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_empty_string_defaults_to_server_error() {
        let err = CompletionError::from_api_error("");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_generic_error_message() {
        let err = CompletionError::from_api_error("An error occurred");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_numeric_unknown_status() {
        let err = CompletionError::from_api_error("HTTP 418 I'm a teapot");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_unknown_preserves_message() {
        let msg = "Unrecognized error format";
        let err = CompletionError::from_api_error(msg);
        if let CompletionError::ServerError(msg_content) = err {
            assert_eq!(msg_content, msg);
        } else {
            panic!("Expected ServerError variant");
        }
    }

    #[test]
    fn from_api_error_multiple_keywords_first_match_wins() {
        // 401 comes before 429 in the logic, so it should match 401 first
        let err = CompletionError::from_api_error("401 Unauthorized and also rate limited");
        assert!(matches!(err, CompletionError::Unauthorized(_)));
    }

    #[test]
    fn from_api_error_priority_timeout_over_network() {
        // timeout check comes before network check, so timeout should win
        let err = CompletionError::from_api_error("timeout and network error");
        assert!(matches!(err, CompletionError::Timeout));
    }

    #[test]
    fn from_api_error_500_before_network() {
        // server error check comes before network check
        let err = CompletionError::from_api_error("500 Server Error and network problem");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn from_api_error_unauthorized_is_provider_error() {
        let err = CompletionError::from_api_error("401 Unauthorized");
        assert!(err.is_provider_error());
    }

    #[test]
    fn from_api_error_unauthorized_not_transient() {
        let err = CompletionError::from_api_error("401 Unauthorized");
        assert!(!err.is_transient());
    }

    #[test]
    fn from_api_error_rate_limited_is_transient() {
        let err = CompletionError::from_api_error("429 Rate Limited");
        assert!(err.is_transient());
    }

    #[test]
    fn from_api_error_rate_limited_is_provider_error() {
        let err = CompletionError::from_api_error("429 Rate Limited");
        assert!(err.is_provider_error());
    }

    #[test]
    fn from_api_error_timeout_is_transient() {
        let err = CompletionError::from_api_error("timeout");
        assert!(err.is_transient());
    }

    #[test]
    fn from_api_error_timeout_not_provider_error() {
        let err = CompletionError::from_api_error("timeout");
        assert!(!err.is_provider_error());
    }

    #[test]
    fn from_api_error_server_error_is_transient() {
        let err = CompletionError::from_api_error("500 server error");
        assert!(err.is_transient());
    }

    #[test]
    fn from_api_error_server_error_is_provider_error() {
        let err = CompletionError::from_api_error("500 server error");
        assert!(err.is_provider_error());
    }

    #[test]
    fn from_api_error_network_error_is_transient() {
        let err = CompletionError::from_api_error("network error");
        assert!(err.is_transient());
    }

    #[test]
    fn from_api_error_network_error_not_provider_error() {
        let err = CompletionError::from_api_error("network error");
        assert!(!err.is_provider_error());
    }

    #[test]
    fn from_api_error_unauthorized_display() {
        let err = CompletionError::from_api_error("401 Unauthorized");
        let msg = format!("{}", err);
        assert!(msg.contains("Unauthorized:"));
        assert!(msg.contains("401"));
    }

    #[test]
    fn from_api_error_rate_limited_display() {
        let err = CompletionError::from_api_error("429 Rate Limited");
        let msg = format!("{}", err);
        assert_eq!(msg, "Rate limited");
    }

    #[test]
    fn from_api_error_timeout_display() {
        let err = CompletionError::from_api_error("timeout");
        let msg = format!("{}", err);
        assert_eq!(msg, "Request timed out");
    }

    #[test]
    fn from_api_error_server_error_display() {
        let err = CompletionError::from_api_error("500 server error");
        let msg = format!("{}", err);
        assert!(msg.contains("Server error:"));
        assert!(msg.contains("500"));
    }

    #[test]
    fn from_api_error_network_error_display() {
        let err = CompletionError::from_api_error("network error");
        let msg = format!("{}", err);
        assert!(msg.contains("Network error:"));
        assert!(msg.contains("network"));
    }
}
