use christina_core::types::{ModelName, ProviderKind, TokenCount};
use url::Url;

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
