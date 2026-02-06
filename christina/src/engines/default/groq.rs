use christina_core::error::CompletionError;
use christina_core::llm::{LlmRequest, LlmResponse, Role};
use llm::builder::{LLMBackend, LLMBuilder};
use llm::chat::ChatMessage as LLMChatMessage;

use crate::orchestrator::retry::{RetryPolicy, retry_with_backoff};

/// Execute a Groq request with retry logic and exponential backoff.
pub async fn execute_groq_request(
    request: &LlmRequest,
    api_key: &str,
    base_url: Option<&str>,
    model: &str,
) -> Result<LlmResponse, CompletionError> {
    execute_groq_request_with_retry(request, api_key, base_url, model, &RetryPolicy::default())
        .await
}

/// Execute a Groq request with custom retry policy.
pub async fn execute_groq_request_with_retry(
    request: &LlmRequest,
    api_key: &str,
    base_url: Option<&str>,
    model: &str,
    retry_policy: &RetryPolicy,
) -> Result<LlmResponse, CompletionError> {
    let request_clone = request.clone();
    let api_key = api_key.to_string();
    let base_url = base_url.map(String::from);
    let model = model.to_string();

    retry_with_backoff(retry_policy, || {
        let request = request_clone.clone();
        let api_key = api_key.clone();
        let base_url = base_url.clone();
        let model = model.clone();

        async move {
            execute_groq_request_inner(&request, &api_key, base_url.as_deref(), &model).await
        }
    })
    .await
}

/// Inner implementation without retry logic.
///
/// HTTP Timeout Configuration:
/// The llm crate v1.3.7 supports HTTP-level timeouts via LLMBuilder::timeout_seconds().
/// However, this codebase relies on orchestrator-level tokio::timeout wrapping all
/// LLM calls (see orchestrator.rs:generate_with_retry). The orchestrator applies
/// progressive timeouts (30s initial, 60s retry, 120s final) that wrap the entire
/// HTTP request, providing more precise control than backend-level timeouts.
///
/// If future versions need backend-level timeout configuration (e.g., to distinguish
/// connection vs. read timeouts), call .timeout_seconds(60) on the builder.
async fn execute_groq_request_inner(
    request: &LlmRequest,
    api_key: &str,
    base_url: Option<&str>,
    model: &str,
) -> Result<LlmResponse, CompletionError> {
    let system_prompt = extract_system_prompt(&request.messages);

    let mut builder = LLMBuilder::new()
        .backend(LLMBackend::Groq)
        .api_key(api_key)
        .model(model)
        .max_tokens(request.max_tokens.get())
        .temperature(request.temperature.value());

    if let Some(url) = base_url {
        builder = builder.base_url(url);
    }

    if let Some(system) = system_prompt {
        builder = builder.system(system);
    }

    let llm = builder
        .build()
        .map_err(|e| CompletionError::from_api_error(&e.to_string()))?;

    let llm_messages = convert_messages(&request.messages);

    let response = llm
        .chat(&llm_messages)
        .await
        .map_err(|e| CompletionError::from_api_error(&e.to_string()))?;

    let content = response
        .text()
        .ok_or_else(|| CompletionError::InvalidResponse("No text in Groq response".to_string()))?
        .to_string();

    Ok(LlmResponse {
        content,
        tokens_used: None,
    })
}

fn convert_messages(messages: &[christina_core::llm::ChatMessage]) -> Vec<LLMChatMessage> {
    messages
        .iter()
        .filter_map(|msg| match msg.role {
            Role::User => Some(LLMChatMessage::user().content(&msg.content).build()),
            Role::Assistant => Some(LLMChatMessage::assistant().content(&msg.content).build()),
            Role::System => None,
        })
        .collect()
}

fn extract_system_prompt(messages: &[christina_core::llm::ChatMessage]) -> Option<&str> {
    messages
        .iter()
        .find(|m| m.role == Role::System)
        .map(|m| m.content.as_str())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use christina_core::llm::ChatMessage;

    #[test]
    fn convert_messages_filters_system() {
        let messages = vec![
            ChatMessage {
                role: Role::System,
                content: "You are a commit message generator".to_string(),
            },
            ChatMessage {
                role: Role::User,
                content: "Generate a commit message".to_string(),
            },
            ChatMessage {
                role: Role::Assistant,
                content: "feat: add feature".to_string(),
            },
        ];

        let result = convert_messages(&messages);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn extract_system_prompt_found() {
        let messages = vec![
            ChatMessage {
                role: Role::System,
                content: "You are a commit message generator".to_string(),
            },
            ChatMessage {
                role: Role::User,
                content: "Generate a commit message".to_string(),
            },
        ];

        let result = extract_system_prompt(&messages);
        assert_eq!(result, Some("You are a commit message generator"));
    }

    #[test]
    fn extract_system_prompt_not_found() {
        let messages = vec![ChatMessage {
            role: Role::User,
            content: "Generate a commit message".to_string(),
        }];

        let result = extract_system_prompt(&messages);
        assert!(result.is_none());
    }
}
