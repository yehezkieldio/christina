use crate::provider::{ChatMessage, ChatRole};
use anyhow::{Context, Result};
use llm::LLMProvider;
use llm::builder::{LLMBackend, LLMBuilder};
use llm::chat::ChatMessage as LLMChatMessage;

pub(super) struct LlmConfig<'a> {
    pub backend: LLMBackend,
    pub api_key: &'a str,
    pub model: &'a str,
    pub max_tokens: u32,
    pub temperature: f32,
    pub base_url: Option<&'a str>,
    pub api_version: Option<&'a str>,
    pub deployment_id: Option<&'a str>,
    pub system_prompt: Option<&'a str>,
}

pub(super) fn build_llm(config: LlmConfig<'_>) -> Result<Box<dyn LLMProvider>> {
    let mut builder = LLMBuilder::new()
        .backend(config.backend)
        .api_key(config.api_key)
        .model(config.model)
        .max_tokens(config.max_tokens)
        .temperature(config.temperature);

    if let Some(url) = config.base_url {
        builder = builder.base_url(url);
    }

    if let Some(version) = config.api_version {
        builder = builder.api_version(version);
    }

    if let Some(id) = config.deployment_id {
        builder = builder.deployment_id(id);
    }

    if let Some(system) = config.system_prompt {
        builder = builder.system(system);
    }

    builder.build().context("Failed to build LLM provider")
}

pub fn convert_messages(messages: &[ChatMessage]) -> Vec<LLMChatMessage> {
    messages
        .iter()
        .filter_map(|msg| match msg.role {
            ChatRole::User => Some(LLMChatMessage::user().content(&msg.content).build()),
            ChatRole::System => None, // Filtered out, handled via system_prompt
        })
        .collect()
}

pub fn extract_system_prompt(messages: &[ChatMessage]) -> Option<&str> {
    messages
        .iter()
        .find(|m| m.role == ChatRole::System)
        .map(|m| m.content.as_str())
}
