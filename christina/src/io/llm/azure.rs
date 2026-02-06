use std::sync::Arc;

use christina_core::error::CompletionError;
use christina_core::llm::{LlmRequest, LlmResponse, Role};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::retry::{RetryPolicy, retry_with_backoff};

/// Execute an Azure OpenAI request with retry logic and exponential backoff.
pub async fn execute_azure_request(
    request: &LlmRequest,
    api_key: &str,
    endpoint: &str,
    deployment_id: &str,
    api_version: &str,
    model: &str,
) -> Result<LlmResponse, CompletionError> {
    execute_azure_request_with_retry(
        request,
        api_key,
        endpoint,
        deployment_id,
        api_version,
        model,
        &RetryPolicy::default(),
    )
    .await
}

/// Execute an Azure OpenAI request with custom retry policy.
pub async fn execute_azure_request_with_retry(
    request: &LlmRequest,
    api_key: &str,
    endpoint: &str,
    deployment_id: &str,
    api_version: &str,
    model: &str,
    retry_policy: &RetryPolicy,
) -> Result<LlmResponse, CompletionError> {
    let request = Arc::new(request.clone());
    let api_key: Arc<str> = Arc::from(api_key);
    let endpoint: Arc<str> = Arc::from(endpoint);
    let deployment_id: Arc<str> = Arc::from(deployment_id);
    let api_version: Arc<str> = Arc::from(api_version);
    let model: Arc<str> = Arc::from(model);

    retry_with_backoff(retry_policy, || {
        let request = Arc::clone(&request);
        let api_key = Arc::clone(&api_key);
        let endpoint = Arc::clone(&endpoint);
        let deployment_id = Arc::clone(&deployment_id);
        let api_version = Arc::clone(&api_version);
        let model = Arc::clone(&model);

        async move {
            execute_azure_request_inner(
                request.as_ref(),
                api_key.as_ref(),
                endpoint.as_ref(),
                deployment_id.as_ref(),
                api_version.as_ref(),
                model.as_ref(),
            )
            .await
        }
    })
    .await
}

fn requires_completion_tokens_param(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.starts_with("o1")
        || m.starts_with("o3")
        || m.starts_with("o4")
        || m.starts_with("gpt-5")
}

#[derive(Serialize, Debug)]
struct AzureChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize, Debug)]
struct AzureChatRequest<'a> {
    model: &'a str,
    messages: Vec<AzureChatMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    stream: bool,
}

#[derive(Deserialize, Debug)]
struct AzureChatResponse {
    choices: Vec<AzureChatChoice>,
}

#[derive(Deserialize, Debug)]
struct AzureChatChoice {
    message: AzureChatMsg,
}

#[derive(Deserialize, Debug)]
struct AzureChatMsg {
    #[allow(dead_code)]
    role: String,
    content: Option<String>,
}

/// Inner implementation without retry logic.
///
/// Bypasses the `llm` crate's Azure backend to directly construct the HTTP
/// request. This allows using `max_completion_tokens` for models that require
/// it (o1, o3, gpt-5 families) while keeping `max_tokens` for older models.
async fn execute_azure_request_inner(
    request: &LlmRequest,
    api_key: &str,
    endpoint: &str,
    deployment_id: &str,
    api_version: &str,
    model: &str,
) -> Result<LlmResponse, CompletionError> {
    let messages: Vec<AzureChatMessage<'_>> = request
        .messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            AzureChatMessage {
                role,
                content: &msg.content,
            }
        })
        .collect();

    let use_completion_tokens = requires_completion_tokens_param(model);
    let max_tokens_value = request.max_tokens.get();

    let body = AzureChatRequest {
        model,
        messages,
        max_tokens: if use_completion_tokens {
            None
        } else {
            Some(max_tokens_value)
        },
        max_completion_tokens: if use_completion_tokens {
            Some(max_tokens_value)
        } else {
            None
        },
        temperature: Some(request.temperature.value()),
        stream: false,
    };

    if tracing::enabled!(tracing::Level::TRACE)
        && let Ok(json) = serde_json::to_string(&body)
    {
        let truncated = truncate_for_log(&json, 500);
        tracing::trace!("Azure OpenAI request payload: {truncated}");
    }

    let url = format!(
        "{endpoint}/openai/deployments/{deployment_id}/chat/completions?api-version={api_version}"
    );

    let client = Client::new();
    let response = client
        .post(&url)
        .header("api-key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| CompletionError::from_api_error(&e.to_string()))?;

    let status = response.status();
    tracing::debug!("Azure OpenAI HTTP status: {status}");

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read body>".to_string());
        return Err(CompletionError::InvalidResponse(format!(
            "Response Format Error: OpenAI API returned error status: {status}. Raw response: {error_text}"
        )));
    }

    let resp_text = response
        .text()
        .await
        .map_err(|e| CompletionError::from_api_error(&e.to_string()))?;

    let parsed: AzureChatResponse = serde_json::from_str(&resp_text).map_err(|e| {
        CompletionError::InvalidResponse(format!(
            "Failed to decode Azure OpenAI response: {e}. Raw: {resp_text}"
        ))
    })?;

    let content = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .ok_or_else(|| {
            CompletionError::InvalidResponse("No text in Azure OpenAI response".to_string())
        })?;

    Ok(LlmResponse {
        content,
        tokens_used: None,
    })
}

fn truncate_for_log(s: &str, max_chars: usize) -> std::borrow::Cow<'_, str> {
    if s.len() <= max_chars {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut end = max_chars;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    std::borrow::Cow::Owned(format!("{}…<truncated, {} total>", &s[..end], s.len()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use christina_core::llm::ChatMessage;

    #[test]
    fn requires_completion_tokens_gpt5() {
        assert!(requires_completion_tokens_param("gpt-5-nano"));
        assert!(requires_completion_tokens_param("gpt-5"));
        assert!(requires_completion_tokens_param("gpt-5-mini"));
        assert!(requires_completion_tokens_param("GPT-5-Nano"));
    }

    #[test]
    fn requires_completion_tokens_o_series() {
        assert!(requires_completion_tokens_param("o1"));
        assert!(requires_completion_tokens_param("o1-mini"));
        assert!(requires_completion_tokens_param("o1-preview"));
        assert!(requires_completion_tokens_param("o3"));
        assert!(requires_completion_tokens_param("o3-mini"));
        assert!(requires_completion_tokens_param("o4-mini"));
    }

    #[test]
    fn does_not_require_completion_tokens_gpt4() {
        assert!(!requires_completion_tokens_param("gpt-4"));
        assert!(!requires_completion_tokens_param("gpt-4o"));
        assert!(!requires_completion_tokens_param("gpt-4o-mini"));
        assert!(!requires_completion_tokens_param("gpt-4.1"));
        assert!(!requires_completion_tokens_param("gpt-4.1-nano"));
    }

    #[test]
    fn request_body_uses_max_completion_tokens_for_gpt5() {
        let body = AzureChatRequest {
            model: "gpt-5-nano",
            messages: vec![],
            max_tokens: None,
            max_completion_tokens: Some(512),
            temperature: Some(0.3),
            stream: false,
        };
        let json = serde_json::to_value(&body).unwrap();
        assert!(json.get("max_tokens").is_none());
        assert_eq!(json["max_completion_tokens"], 512);
    }

    #[test]
    fn request_body_uses_max_tokens_for_gpt4() {
        let body = AzureChatRequest {
            model: "gpt-4o",
            messages: vec![],
            max_tokens: Some(512),
            max_completion_tokens: None,
            temperature: Some(0.3),
            stream: false,
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["max_tokens"], 512);
        assert!(json.get("max_completion_tokens").is_none());
    }

    #[test]
    fn azure_chat_message_role_mapping() {
        let messages = [
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

        let mapped: Vec<AzureChatMessage<'_>> = messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };
                AzureChatMessage {
                    role,
                    content: &msg.content,
                }
            })
            .collect();

        assert_eq!(mapped.len(), 3);
        assert_eq!(mapped[0].role, "system");
        assert_eq!(mapped[1].role, "user");
        assert_eq!(mapped[2].role, "assistant");
    }
}
