//! OpenAI integration via the shared llm crate.
//!
//! WHY use llm crate here: standard OpenAI endpoints map directly and benefit
//! from shared request formatting and error normalization.

use std::sync::Arc;

use christina_core::error::CompletionError;
use christina_core::llm::{LlmRequest, LlmResponse, Role};
use llm::builder::{LLMBackend, LLMBuilder};
use llm::chat::ChatMessage as LLMChatMessage;

use crate::orchestrator::retry::{RetryPolicy, retry_with_backoff};

/// Execute an OpenAI request with retry logic and exponential backoff.
///
/// This function wraps the underlying API call with retry logic that:
/// - Retries on transient errors (rate limits, timeouts, server errors)
/// - Uses exponential backoff with jitter to prevent thundering herd
/// - Fails fast on non-transient errors (auth, invalid request)
pub async fn execute_openai_request(
    request: &LlmRequest,
    api_key: &str,
    base_url: Option<&str>,
    model: &str,
) -> Result<LlmResponse, CompletionError> {
    execute_openai_request_with_retry(request, api_key, base_url, model, &RetryPolicy::default())
        .await
}

/// Execute an OpenAI request with custom retry policy.
pub async fn execute_openai_request_with_retry(
    request: &LlmRequest,
    api_key: &str,
    base_url: Option<&str>,
    model: &str,
    retry_policy: &RetryPolicy,
) -> Result<LlmResponse, CompletionError> {
    let request = Arc::new(request.clone());
    let api_key: Arc<str> = Arc::from(api_key);
    let base_url: Option<Arc<str>> = base_url.map(Arc::from);
    let model: Arc<str> = Arc::from(model);

    retry_with_backoff(retry_policy, || {
        let request = Arc::clone(&request);
        let api_key = Arc::clone(&api_key);
        let base_url = base_url.clone();
        let model = Arc::clone(&model);

        async move {
            execute_openai_request_inner(
                request.as_ref(),
                api_key.as_ref(),
                base_url.as_deref(),
                model.as_ref(),
            )
            .await
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
async fn execute_openai_request_inner(
    request: &LlmRequest,
    api_key: &str,
    base_url: Option<&str>,
    model: &str,
) -> Result<LlmResponse, CompletionError> {
    let system_prompt = extract_system_prompt(&request.messages);

    let mut builder = LLMBuilder::new()
        .backend(LLMBackend::OpenAI)
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
        .ok_or_else(|| CompletionError::InvalidResponse("No text in OpenAI response".to_string()))?
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
