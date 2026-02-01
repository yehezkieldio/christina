use crate::error::ProviderError;
use crate::ids::GenerationId;
use crate::types::TokenCount;

/// Represents a single message in a chat conversation
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// The role of a chat message participant
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A request to generate a commit message using an LLM
#[derive(Debug, Clone)]
pub struct LlmRequest {
    /// Unique identifier for this generation request
    pub id: GenerationId,
    /// The messages to send to the LLM
    pub messages: Vec<ChatMessage>,
    /// Maximum tokens to generate in the response
    pub max_tokens: TokenCount,
    /// Temperature for sampling (0.0 to 2.0, typically)
    pub temperature: f32,
    /// Optional system prompt to prepend to messages
    pub system_prompt: Option<String>,
}

impl LlmRequest {
    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.messages.is_empty() {
            return Err(ProviderError::InvalidConfig(
                "LlmRequest must contain at least one message".to_string(),
            ));
        }

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

        if self.max_tokens.get() == 0 {
            return Err(ProviderError::InvalidConfig(
                "max_tokens must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Response from an LLM containing generated content
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// The generated content (typically a commit message)
    pub content: String,
    /// Optional token usage information
    pub tokens_used: Option<TokenCount>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_creation() {
        let msg = ChatMessage {
            role: Role::User,
            content: "Generate a commit message".to_string(),
        };

        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Generate a commit message");
    }

    #[test]
    fn role_equality() {
        assert_eq!(Role::System, Role::System);
        assert_eq!(Role::User, Role::User);
        assert_eq!(Role::Assistant, Role::Assistant);
        assert_ne!(Role::System, Role::User);
    }

    #[test]
    fn llm_request_creation() {
        let id = GenerationId::new(1);
        let messages = vec![ChatMessage {
            role: Role::User,
            content: "test".to_string(),
        }];
        let max_tokens = TokenCount::new(100).unwrap();

        let req = LlmRequest {
            id,
            messages,
            max_tokens,
            temperature: 0.7,
            system_prompt: None,
        };

        assert_eq!(req.id, id);
        assert_eq!(req.temperature, 0.7);
        assert!(req.system_prompt.is_none());
    }

    #[test]
    fn llm_response_creation() {
        let resp = LlmResponse {
            content: "feat: add feature".to_string(),
            tokens_used: TokenCount::new(42),
        };

        assert_eq!(resp.content, "feat: add feature");
        assert!(resp.tokens_used.is_some());
    }
}
