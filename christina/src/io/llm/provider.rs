#[cfg(test)]
use std::sync::{Arc, Mutex};

use anyhow::Result;

use christina_core::{
    error::{CompletionError, ProviderError},
    ids::GenerationId,
    llm::{ChatMessage, LlmRequest},
    profile::ProviderProfile,
    types::{ModelName, ProviderKind, TokenCount},
};

use crate::io::llm::{azure, groq, openai};

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
        temperature: f32,
    },
    Azure {
        model: ModelName,
        api_key: ApiKey,
        endpoint: String,
        api_version: String,
        deployment_id: String,
        max_tokens: TokenCount,
        temperature: f32,
    },
    Groq {
        model: ModelName,
        api_key: ApiKey,
        base_url: Option<url::Url>,
        max_tokens: TokenCount,
        temperature: f32,
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
        let temperature = profile.temperature.unwrap_or(0.3).clamp(0.0, 2.0);

        match profile.provider {
            ProviderKind::OpenAI => Ok(Provider::OpenAI {
                model: profile.model.clone(),
                api_key: ApiKey::new(api_key),
                base_url: profile.api_url.clone(),
                max_tokens: profile.max_output_tokens,
                temperature,
            }),
            ProviderKind::Azure => {
                let endpoint = profile
                    .api_url
                    .as_ref()
                    .ok_or_else(|| ProviderError::MissingConfig("model_api_url".to_string()))?
                    .to_string();
                let api_version = profile
                    .azure_api_version
                    .as_ref()
                    .ok_or_else(|| ProviderError::MissingConfig("azure_api_version".to_string()))?
                    .clone();
                let deployment_id = profile
                    .azure_deployment_id
                    .as_ref()
                    .ok_or_else(|| ProviderError::MissingConfig("azure_deployment_id".to_string()))?
                    .clone();

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
                let request = request_from_messages(messages, *max_tokens, *temperature);
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
                let request = request_from_messages(messages, *max_tokens, *temperature);
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
                let request = request_from_messages(messages, *max_tokens, *temperature);
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

    LlmRequest {
        id: GenerationId::new(0),
        messages: mapped,
        max_tokens,
        temperature,
        system_prompt: None,
    }
}
