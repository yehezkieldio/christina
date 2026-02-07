//! LLM request/response shapes shared across providers.
//!
//! WHY keep provider-agnostic: lets orchestration and tests construct requests
//! without importing any client SDKs or transport-specific types.

use crate::error::ProviderError;
use crate::types::backend_id::GenerationId;
use crate::types::{Temperature, tokens::TokenCount};

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
    pub temperature: Temperature,
    /// Optional system prompt to prepend to messages
    ///
    /// Kept separate so providers that already embed a system message can
    /// inject it without mutating the original message list.
    pub system_prompt: Option<String>,
}

impl LlmRequest {
    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.messages.is_empty() {
            return Err(ProviderError::InvalidConfig(
                "LlmRequest must contain at least one message".to_string(),
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
            temperature: Temperature::try_new(0.7).unwrap(),
            system_prompt: None,
        };

        assert_eq!(req.id, id);
        assert_eq!(req.temperature.value(), 0.7);
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

    #[test]
    fn chat_message_system() {
        let msg = ChatMessage::system("You are a helpful assistant");
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.content, "You are a helpful assistant");
    }

    #[test]
    fn chat_message_user() {
        let msg = ChatMessage::user("Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn chat_message_assistant() {
        let msg = ChatMessage::assistant("Hi there!");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content, "Hi there!");
    }

    #[test]
    fn validate_request_valid() {
        let req = LlmRequest {
            id: GenerationId::new(1),
            messages: vec![ChatMessage::user("test")],
            max_tokens: TokenCount::new(100).unwrap(),
            temperature: Temperature::try_new(0.7).unwrap(),
            system_prompt: None,
        };

        assert!(req.validate().is_ok());
    }

    #[test]
    fn validate_request_empty_messages() {
        let req = LlmRequest {
            id: GenerationId::new(1),
            messages: vec![],
            max_tokens: TokenCount::new(100).unwrap(),
            temperature: Temperature::try_new(0.7).unwrap(),
            system_prompt: None,
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("at least one message")
        );
    }

    #[test]
    fn validate_request_temperature_nan() {
        assert!(Temperature::try_new(f32::NAN).is_err());
    }

    #[test]
    fn validate_request_temperature_negative() {
        assert!(Temperature::try_new(-0.5).is_err());
    }

    #[test]
    fn validate_request_temperature_too_high() {
        assert!(Temperature::try_new(3.0).is_err());
    }

    #[test]
    fn validate_request_temperature_boundaries() {
        assert!(Temperature::try_new(0.0).is_ok());
        assert!(Temperature::try_new(2.0).is_ok());
    }

    #[test]
    fn validate_request_with_system_prompt() {
        let req = LlmRequest {
            id: GenerationId::new(1),
            messages: vec![ChatMessage::user("test")],
            max_tokens: TokenCount::new(100).unwrap(),
            temperature: Temperature::try_new(0.7).unwrap(),
            system_prompt: Some("You are a helpful assistant".to_string()),
        };

        assert!(req.validate().is_ok());
    }

    #[test]
    fn validate_request_multiple_messages() {
        let req = LlmRequest {
            id: GenerationId::new(1),
            messages: vec![
                ChatMessage::system("System context"),
                ChatMessage::user("User query"),
                ChatMessage::assistant("Assistant response"),
                ChatMessage::user("Follow-up"),
            ],
            max_tokens: TokenCount::new(100).unwrap(),
            temperature: Temperature::try_new(0.7).unwrap(),
            system_prompt: None,
        };

        assert!(req.validate().is_ok());
    }

    #[test]
    fn role_is_copy() {
        let role1 = Role::User;
        let role2 = role1;
        assert_eq!(role1, role2);
    }
}
