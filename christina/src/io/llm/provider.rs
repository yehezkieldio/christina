#[cfg(test)]
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;

use christina_core::{
    error::{CompletionError, ProviderError},
    ids::GenerationId,
    llm::{ChatMessage, LlmRequest},
    profile::ProviderProfile,
    types::{ModelName, ProviderKind, TokenCount},
};

use crate::io::llm::{azure, groq, openai};

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Parse Azure OpenAI endpoint URL and extract api_version and deployment_id if present.
///
/// Azure endpoints typically follow:
/// https://{resource}.openai.azure.com/openai/deployments/{deployment_id}/chat/completions?api-version={version}
fn parse_azure_endpoint(url: &url::Url) -> (Option<String>, Option<String>) {
    let mut api_version = None;
    let mut deployment_id = None;

    // Extract api-version from query parameters
    for (key, value) in url.query_pairs() {
        if key == "api-version" {
            api_version = Some(value.to_string());
            break;
        }
    }

    // Extract deployment_id from path segments
    // Pattern: /openai/deployments/{deployment_id}/...
    let segments: Vec<&str> = url.path_segments().map_or(Vec::new(), |s| s.collect());
    if let Some(idx) = segments.iter().position(|&s| s == "deployments") {
        if let Some(&id) = segments.get(idx + 1) {
            if !id.is_empty() {
                deployment_id = Some(id.to_string());
            }
        }
    }

    (api_version, deployment_id)
}

#[derive(Clone, Copy, Debug)]
pub struct Temperature(f32);

impl Temperature {
    pub fn new(value: f32) -> Self {
        Self(value.clamp(0.0, 2.0))
    }

    pub fn value(self) -> f32 {
        self.0
    }
}

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

#[derive(Debug, Clone)]
pub enum Provider {
    OpenAI {
        model: ModelName,
        api_key: ApiKey,
        base_url: Option<url::Url>,
        max_tokens: TokenCount,
        temperature: Temperature,
    },
    Azure {
        model: ModelName,
        api_key: ApiKey,
        endpoint: String,
        api_version: String,
        deployment_id: String,
        max_tokens: TokenCount,
        temperature: Temperature,
    },
    Groq {
        model: ModelName,
        api_key: ApiKey,
        base_url: Option<url::Url>,
        max_tokens: TokenCount,
        temperature: Temperature,
    },
    #[cfg(test)]
    Mock { response: String, delay_ms: u64 },
    #[cfg(test)]
    MockSequence {
        responses: Arc<Mutex<Vec<Result<String, CompletionError>>>>,
        delay_ms: u64,
    },
}

impl Provider {
    pub fn from_profile(profile: &ProviderProfile, api_key: &str) -> Result<Self> {
        let temperature = Temperature::new(profile.temperature.unwrap_or(0.3));

        match profile.provider {
            ProviderKind::OpenAI => Ok(Provider::OpenAI {
                model: profile.model.clone(),
                api_key: ApiKey::new(api_key),
                base_url: profile.api_url.clone(),
                max_tokens: profile.max_output_tokens,
                temperature,
            }),
            ProviderKind::Azure => {
                let url = profile
                    .api_url
                    .as_ref()
                    .ok_or_else(|| {
                        ProviderError::MissingConfig(
                            "model_api_url (Azure endpoint required)".to_string(),
                        )
                    })?;

                // Try to extract from URL if not explicitly provided
                let (url_api_version, url_deployment_id) = parse_azure_endpoint(url);

                let api_version = profile
                    .azure_api_version
                    .clone()
                    .or(url_api_version)
                    .ok_or_else(|| {
                        ProviderError::MissingConfig(
                            "azure_api_version (not found in config or URL)".to_string(),
                        )
                    })?;

                let deployment_id = profile
                    .azure_deployment_id
                    .clone()
                    .or(url_deployment_id)
                    .ok_or_else(|| {
                        ProviderError::MissingConfig(
                            "azure_deployment_id (not found in config or URL)".to_string(),
                        )
                    })?;

                // Strip query parameters from endpoint if present
                let endpoint = if url.query().is_some() {
                    let mut clean_url = url.clone();
                    clean_url.set_query(None);
                    clean_url.to_string()
                } else {
                    url.to_string()
                };

                Ok(Provider::Azure {
                    model: profile.model.clone(),
                    api_key: ApiKey::new(api_key),
                    endpoint,
                    api_version,
                    deployment_id,
                    max_tokens: profile.max_output_tokens,
                    temperature,
                })
            }
            ProviderKind::Groq => Ok(Provider::Groq {
                model: profile.model.clone(),
                api_key: ApiKey::new(api_key),
                base_url: profile.api_url.clone(),
                max_tokens: profile.max_output_tokens,
                temperature,
            }),
        }
    }

    pub async fn generate(&self, messages: &[ChatMessage]) -> Result<String, CompletionError> {
        match self {
            Provider::OpenAI {
                model,
                api_key,
                base_url,
                max_tokens,
                temperature,
            } => {
                let request = request_from_messages(messages, *max_tokens, temperature.value());
                let response = openai::execute_openai_request(
                    &request,
                    api_key.as_str(),
                    base_url.as_ref().map(|u| u.as_str()),
                    model.as_str(),
                )
                .await?;
                Ok(response.content)
            }
            Provider::Azure {
                model,
                api_key,
                endpoint,
                api_version,
                deployment_id,
                max_tokens,
                temperature,
            } => {
                let request = request_from_messages(messages, *max_tokens, temperature.value());
                let response = azure::execute_azure_request(
                    &request,
                    api_key.as_str(),
                    endpoint,
                    deployment_id,
                    api_version,
                    model.as_str(),
                )
                .await?;
                Ok(response.content)
            }
            Provider::Groq {
                model,
                api_key,
                base_url,
                max_tokens,
                temperature,
            } => {
                let request = request_from_messages(messages, *max_tokens, temperature.value());
                let response = groq::execute_groq_request(
                    &request,
                    api_key.as_str(),
                    base_url.as_ref().map(|u| u.as_str()),
                    model.as_str(),
                )
                .await?;
                Ok(response.content)
            }
            #[cfg(test)]
            Provider::Mock { response, delay_ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
                Ok(response.clone())
            }
            #[cfg(test)]
            Provider::MockSequence {
                responses,
                delay_ms,
            } => {
                tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
                let mut guard = responses
                    .lock()
                    .map_err(|_| CompletionError::NetworkError("mock lock poisoned".into()))?;
                if guard.is_empty() {
                    return Err(CompletionError::InvalidResponse(
                        "mock sequence exhausted".into(),
                    ));
                }
                guard.remove(0)
            }
        }
    }

    #[cfg(test)]
    pub fn mock(response: impl Into<String>) -> Self {
        Provider::Mock {
            response: response.into(),
            delay_ms: 100,
        }
    }

    #[cfg(test)]
    pub fn mock_sequence(responses: Vec<Result<String, CompletionError>>) -> Self {
        Provider::MockSequence {
            responses: Arc::new(Mutex::new(responses)),
            delay_ms: 0,
        }
    }

    #[cfg(test)]
    pub fn mock_sequence_with_delay(
        responses: Vec<Result<String, CompletionError>>,
        delay_ms: u64,
    ) -> Self {
        Provider::MockSequence {
            responses: Arc::new(Mutex::new(responses)),
            delay_ms,
        }
    }
}

fn request_from_messages(
    messages: &[ChatMessage],
    max_tokens: TokenCount,
    temperature: f32,
) -> LlmRequest {
    let mut mapped = Vec::with_capacity(messages.len());
    for msg in messages {
        mapped.push(ChatMessage {
            role: msg.role,
            content: msg.content.clone(),
        });
    }

    let id = REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed);

    LlmRequest {
        id: GenerationId::new(id),
        messages: mapped,
        max_tokens,
        temperature,
        system_prompt: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_azure_endpoint_full_url() {
        let url = url::Url::parse(
            "https://my-resource.openai.azure.com/openai/deployments/gpt-4/chat/completions?api-version=2024-02-15"
        ).unwrap();

        let (api_version, deployment_id) = parse_azure_endpoint(&url);

        assert_eq!(api_version, Some("2024-02-15".to_string()));
        assert_eq!(deployment_id, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_parse_azure_endpoint_missing_version() {
        let url = url::Url::parse(
            "https://my-resource.openai.azure.com/openai/deployments/gpt-4/chat/completions"
        )
        .unwrap();

        let (api_version, deployment_id) = parse_azure_endpoint(&url);

        assert_eq!(api_version, None);
        assert_eq!(deployment_id, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_parse_azure_endpoint_missing_deployment() {
        let url = url::Url::parse(
            "https://my-resource.openai.azure.com/?api-version=2024-02-15"
        )
        .unwrap();

        let (api_version, deployment_id) = parse_azure_endpoint(&url);

        assert_eq!(api_version, Some("2024-02-15".to_string()));
        assert_eq!(deployment_id, None);
    }

    #[test]
    fn test_parse_azure_endpoint_base_url_only() {
        let url = url::Url::parse("https://my-resource.openai.azure.com").unwrap();

        let (api_version, deployment_id) = parse_azure_endpoint(&url);

        assert_eq!(api_version, None);
        assert_eq!(deployment_id, None);
    }
}
