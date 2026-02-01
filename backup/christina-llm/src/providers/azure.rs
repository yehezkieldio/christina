use christina_core::types::{ModelName, TokenCount};
use llm::builder::LLMBackend;
use url::Url;

use crate::{
    ChatMessage, CompletionError,
    providers::http::{LlmConfig, build_llm, convert_messages, extract_system_prompt},
};

#[derive(Debug, Clone)]
pub struct ParsedAzureConfig {
    pub endpoint: String,
    pub deployment_id: String,
    pub api_version: String,
}

pub fn parse_azure_url(url: &str) -> Option<ParsedAzureConfig> {
    if !url.contains("cognitiveservices.azure.com") && !url.contains("openai.azure.com") {
        return None;
    }

    let url_parsed = Url::parse(url).ok()?;
    let endpoint = format!("{}://{}", url_parsed.scheme(), url_parsed.host_str()?);

    let path = url_parsed.path();
    let deployment_id = path
        .strip_prefix("/openai/deployments/")?
        .split('/')
        .next()?
        .to_string();

    if deployment_id.is_empty() {
        return None;
    }

    let api_version = url_parsed
        .query_pairs()
        .find(|(key, _)| key == "api-version")
        .map(|(_, value): (_, std::borrow::Cow<'_, str>)| value.to_string())
        .unwrap_or_else(|| "2024-12-01-preview".to_string());

    Some(ParsedAzureConfig {
        endpoint,
        deployment_id,
        api_version,
    })
}

pub struct AzureGenRequest<'a> {
    pub model: &'a ModelName,
    pub api_key: &'a str,
    pub endpoint: &'a str,
    pub api_version: &'a str,
    pub deployment_id: &'a str,
    pub max_tokens: TokenCount,
    pub temperature: f32,
    pub messages: &'a [ChatMessage],
}

pub async fn generate(req: AzureGenRequest<'_>) -> Result<String, CompletionError> {
    let system_prompt = extract_system_prompt(req.messages);

    let llm = build_llm(LlmConfig {
        backend: LLMBackend::AzureOpenAI,
        api_key: req.api_key,
        model: req.model.as_ref(),
        max_tokens: req.max_tokens.get(),
        temperature: req.temperature,
        base_url: Some(req.endpoint),
        api_version: Some(req.api_version),
        deployment_id: Some(req.deployment_id),
        system_prompt,
    })
    .map_err(|e| CompletionError::from_api_error(&e.to_string()))?;

    let llm_messages = convert_messages(req.messages);

    let response = llm
        .chat(&llm_messages)
        .await
        .map_err(|e| CompletionError::from_api_error(&e.to_string()))?;

    response.text().map(|s| s.to_string()).ok_or_else(|| {
        CompletionError::InvalidResponse("No text in Azure OpenAI response".to_string())
    })
}

#[cfg(test)]
#[expect(
    clippy::panic,
    reason = "test assertions use panic for failure reporting"
)]
mod tests {
    use super::*;

    #[test]
    fn parse_azure_url_full() {
        let url = "https://myresource.cognitiveservices.azure.com/openai/deployments/gpt-4/chat/completions?api-version=2024-12-01-preview";
        let parsed = parse_azure_url(url);

        let p = parsed.unwrap_or_else(|| panic!("Valid Azure URL should parse: {}", url));
        assert_eq!(p.endpoint, "https://myresource.cognitiveservices.azure.com");
        assert_eq!(p.deployment_id, "gpt-4");
        assert_eq!(p.api_version, "2024-12-01-preview");
    }

    #[test]
    fn parse_azure_url_openai_azure_domain() {
        let url = "https://myresource.openai.azure.com/openai/deployments/gpt-4.1-mini/chat/completions?api-version=2025-01-01";
        let parsed = parse_azure_url(url);

        let p = parsed.unwrap_or_else(|| panic!("Valid Azure OpenAI URL should parse: {}", url));
        assert_eq!(p.endpoint, "https://myresource.openai.azure.com");
        assert_eq!(p.deployment_id, "gpt-4.1-mini");
        assert_eq!(p.api_version, "2025-01-01");
    }

    #[test]
    fn parse_azure_url_non_azure() {
        let url = "https://api.openai.com/v1/chat/completions";
        let parsed = parse_azure_url(url);

        assert!(parsed.is_none());
    }

    #[test]
    fn parse_azure_url_defaults_api_version() {
        let url = "https://myresource.cognitiveservices.azure.com/openai/deployments/gpt-4/chat/completions";
        let parsed = parse_azure_url(url);

        let p =
            parsed.unwrap_or_else(|| panic!("Azure URL without api-version should parse: {}", url));
        assert_eq!(p.api_version, "2024-12-01-preview");
    }
}
