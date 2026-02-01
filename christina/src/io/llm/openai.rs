use christina_core::error::CompletionError;
use christina_core::llm::{LlmRequest, LlmResponse, Role};
use llm::builder::{LLMBackend, LLMBuilder};
use llm::chat::ChatMessage as LLMChatMessage;

#[expect(dead_code, reason = "Public API for future use")]
pub async fn execute_openai_request(
    request: &LlmRequest,
    api_key: &str,
    base_url: Option<&str>,
) -> Result<LlmResponse, CompletionError> {
    let system_prompt = extract_system_prompt(&request.messages);

    let mut builder = LLMBuilder::new()
        .backend(LLMBackend::OpenAI)
        .api_key(api_key)
        .model("gpt-4")
        .max_tokens(request.max_tokens.get())
        .temperature(request.temperature);

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
