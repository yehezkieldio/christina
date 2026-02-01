use christina_core::types::{ModelName, TokenCount};
use llm::builder::LLMBackend;
use url::Url;

use crate::{
    ChatMessage, CompletionError,
    providers::http::{LlmConfig, build_llm, convert_messages, extract_system_prompt},
};

pub async fn generate(
    model: &ModelName,
    api_key: &str,
    base_url: Option<&Url>,
    max_tokens: TokenCount,
    temperature: f32,
    messages: &[ChatMessage],
) -> Result<String, CompletionError> {
    let system_prompt = extract_system_prompt(messages);

    let llm = build_llm(LlmConfig {
        backend: LLMBackend::OpenAI,
        api_key,
        model: model.as_ref(),
        max_tokens: max_tokens.get(),
        temperature,
        base_url: base_url.map(Url::as_str),
        api_version: None,
        deployment_id: None,
        system_prompt,
    })
    .map_err(|e| CompletionError::from_api_error(&e.to_string()))?;

    let llm_messages = convert_messages(messages);

    let response = llm
        .chat(&llm_messages)
        .await
        .map_err(|e| CompletionError::from_api_error(&e.to_string()))?;

    response
        .text()
        .map(|s| s.to_string())
        .ok_or_else(|| CompletionError::InvalidResponse("No text in OpenAI response".to_string()))
}
