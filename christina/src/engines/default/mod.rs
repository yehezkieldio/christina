//! Default provider implementations (OpenAI, Azure, Groq).
//!
//! WHY in one module: keeps provider-specific HTTP logic together while sharing
//! request construction and validation rules.

mod azure;
mod groq;
mod openai;

use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(test)]
use std::sync::{Arc, Mutex};

use anyhow::Result;
use tracing::Instrument;

use crate::config::profiles::ProviderProfile;

use christina_core::{
    error::{CompletionError, ProviderError},
    llm::{ChatMessage, LlmRequest, Role},
    types::backend_id::GenerationId,
    types::tokens::TokenCount,
    types::{ModelName, ProviderKind, Temperature},
};
use llm::chat::ChatMessage as LLMChatMessage;

// NOTE: AtomicU64::fetch_add wraps on overflow. Wraparound is acceptable here
// because IDs are used for logging/correlation, not long-term uniqueness.
static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Parse Azure OpenAI endpoint URL and extract api_version and deployment_id if present.
///
/// Azure endpoints typically follow:
/// https://{resource}.openai.azure.com/openai/deployments/{deployment_id}/chat/completions?api-version={version}
fn parse_azure_endpoint(url: &url::Url) -> (Option<String>, Option<String>) {
    let mut api_version = None;
    let mut deployment_id = None;

    // Extract api-version from query parameters (Azure often embeds it in the URL).
    for (key, value) in url.query_pairs() {
        if key == "api-version" {
            api_version = Some(value.to_string());
            break;
        }
    }

    // Extract deployment_id from path segments
    // Pattern: /openai/deployments/{deployment_id}/...
    let segments: Vec<&str> = url.path_segments().map_or(Vec::new(), |s| s.collect());
    if let Some(idx) = segments.iter().position(|&s| s == "deployments")
        && let Some(&id) = segments.get(idx + 1)
        && !id.is_empty()
    {
        deployment_id = Some(id.to_string());
    }

    (api_version, deployment_id)
}

fn has_azure_deployment_path(url: &url::Url) -> bool {
    url.path_segments()
        .map(|mut segments| segments.any(|segment| segment == "deployments"))
        .unwrap_or(false)
}

fn normalize_azure_endpoint(url: &url::Url) -> String {
    // Provider API expects the resource root, not the deployment path.
    let mut clean = url.clone();
    clean.set_path("");
    clean.set_query(None);
    clean.set_fragment(None);
    let mut endpoint = clean.to_string();
    if endpoint.ends_with('/') {
        endpoint.pop();
    }
    endpoint
}

/// API key wrapper with secure defaults.
///
/// The inner `String` is private to prevent accidental exposure through pattern matching
/// or direct field access. Use `as_str()` to access the key value, which makes secret
/// handling explicit and visible in code.
///
/// `#[non_exhaustive]` prevents external code from constructing or destructuring this type
/// without going through the provided API.
#[derive(Clone)]
#[non_exhaustive]
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
        let temperature = Temperature::new_clamped(profile.temperature.unwrap_or(0.3));

        match profile.provider {
            ProviderKind::OpenAI => Ok(Provider::OpenAI {
                model: profile.model.clone(),
                api_key: ApiKey::new(api_key),
                base_url: profile.api_url.clone(),
                max_tokens: profile.max_output_tokens,
                temperature,
            }),
            ProviderKind::Azure => {
                let url = profile.api_url.as_ref().ok_or_else(|| {
                    ProviderError::MissingConfig(
                        "model_api_url (Azure endpoint required)".to_string(),
                    )
                })?;

                // Try to extract from URL if not explicitly provided
                let (url_api_version, url_deployment_id) = parse_azure_endpoint(url);
                let has_deployment_path = has_azure_deployment_path(url);

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
                    .or(url_deployment_id.clone())
                    .ok_or_else(|| {
                        ProviderError::MissingConfig(
                            "azure_deployment_id (not found in config or URL)".to_string(),
                        )
                    })?;

                if profile.azure_deployment_id.is_some() && url_deployment_id.is_some() {
                    return Err(ProviderError::InvalidConfig(
                        "azure_deployment_id provided in config and URL; use one source only"
                            .to_string(),
                    )
                    .into());
                }

                if profile.azure_deployment_id.is_some() && has_deployment_path {
                    return Err(ProviderError::InvalidConfig(
                        "model_api_url contains /openai/deployments while azure_deployment_id is set; \
                         use a resource root URL or remove azure_deployment_id"
                            .to_string(),
                    )
                    .into());
                }

                let endpoint = normalize_azure_endpoint(url);

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
        let request = match self {
            Provider::OpenAI {
                model,
                max_tokens,
                temperature,
                ..
            }
            | Provider::Groq {
                model,
                max_tokens,
                temperature,
                ..
            }
            | Provider::Azure {
                model,
                max_tokens,
                temperature,
                ..
            } => {
                let req = request_from_messages(messages, *max_tokens, *temperature);
                let gen_id = req.id;
                let span = tracing::info_span!(
                    "llm_generate",
                    generation_id = %gen_id,
                    model = %model.as_str(),
                    provider = ?self.provider_kind()
                );
                Some((req, span))
            }
            #[cfg(test)]
            _ => None,
        };

        let generate_impl = async {
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
        };

        match request {
            Some((_, span)) => generate_impl.instrument(span).await,
            None => generate_impl.await,
        }
    }

    fn provider_kind(&self) -> &'static str {
        match self {
            Provider::OpenAI { .. } => "openai",
            Provider::Azure { .. } => "azure",
            Provider::Groq { .. } => "groq",
            #[cfg(test)]
            Provider::Mock { .. } => "mock",
            #[cfg(test)]
            Provider::MockSequence { .. } => "mock_sequence",
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
    temperature: Temperature,
) -> LlmRequest {
    let mut mapped = Vec::with_capacity(messages.len());
    for msg in messages {
        mapped.push(ChatMessage {
            role: msg.role,
            content: msg.content.clone(),
        });
    }

    let id = REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    // ID is for logging/correlation only; wraparound is acceptable.

    LlmRequest {
        id: GenerationId::new(id),
        messages: mapped,
        max_tokens,
        temperature,
        system_prompt: None,
    }
}

fn convert_messages(messages: &[ChatMessage]) -> Vec<LLMChatMessage> {
    messages
        .iter()
        .filter_map(|msg| match msg.role {
            Role::User => Some(LLMChatMessage::user().content(&msg.content).build()),
            Role::Assistant => Some(LLMChatMessage::assistant().content(&msg.content).build()),
            Role::System => None,
        })
        .collect()
}

fn extract_system_prompt(messages: &[ChatMessage]) -> Option<&str> {
    messages
        .iter()
        .find(|m| m.role == Role::System)
        .map(|m| m.content.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::expect_used)]
    fn test_parse_azure_endpoint_full_url() {
        let url = url::Url::parse(
            "https://my-resource.openai.azure.com/openai/deployments/gpt-4/chat/completions?api-version=2024-02-15"
        ).expect("valid URL");

        let (api_version, deployment_id) = parse_azure_endpoint(&url);

        assert_eq!(api_version, Some("2024-02-15".to_string()));
        assert_eq!(deployment_id, Some("gpt-4".to_string()));
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_parse_azure_endpoint_missing_version() {
        let url = url::Url::parse(
            "https://my-resource.openai.azure.com/openai/deployments/gpt-4/chat/completions",
        )
        .expect("valid URL");

        let (api_version, deployment_id) = parse_azure_endpoint(&url);

        assert_eq!(api_version, None);
        assert_eq!(deployment_id, Some("gpt-4".to_string()));
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_parse_azure_endpoint_missing_deployment() {
        let url = url::Url::parse("https://my-resource.openai.azure.com/?api-version=2024-02-15")
            .expect("valid URL");

        let (api_version, deployment_id) = parse_azure_endpoint(&url);

        assert_eq!(api_version, Some("2024-02-15".to_string()));
        assert_eq!(deployment_id, None);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_parse_azure_endpoint_base_url_only() {
        let url = url::Url::parse("https://my-resource.openai.azure.com").expect("valid URL");

        let (api_version, deployment_id) = parse_azure_endpoint(&url);

        assert_eq!(api_version, None);
        assert_eq!(deployment_id, None);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_parse_azure_endpoint_with_multiple_query_params() {
        let url = url::Url::parse(
            "https://my-resource.openai.azure.com/openai/deployments/gpt-4/chat/completions?api-version=2024-02-15&param=value"
        ).expect("valid URL");

        let (api_version, deployment_id) = parse_azure_endpoint(&url);

        assert_eq!(api_version, Some("2024-02-15".to_string()));
        assert_eq!(deployment_id, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_temperature_new_clamps() {
        let temp = Temperature::new_clamped(-1.0);
        assert_eq!(temp.value(), 0.0);

        let temp = Temperature::new_clamped(3.0);
        assert_eq!(temp.value(), 2.0);

        let temp = Temperature::new_clamped(1.0);
        assert_eq!(temp.value(), 1.0);
    }

    #[test]
    fn test_api_key_debug_redacts() {
        let key = ApiKey::new("sk-secret123");
        let debug_str = format!("{:?}", key);
        assert!(debug_str.contains("REDACTED"));
        assert!(!debug_str.contains("secret"));
    }

    #[test]
    fn test_api_key_as_str() {
        let key = ApiKey::new("sk-test");
        assert_eq!(key.as_str(), "sk-test");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_provider_from_profile_openai() {
        let mut profile = ProviderProfile::new(
            "test".to_string(),
            ProviderKind::OpenAI,
            ModelName::from("gpt-4"),
        );
        profile.temperature = Some(0.5);

        let provider = Provider::from_profile(&profile, "sk-test").unwrap();

        match provider {
            Provider::OpenAI {
                model,
                temperature,
                max_tokens,
                ..
            } => {
                assert_eq!(model, ModelName::from("gpt-4"));
                assert_eq!(temperature.value(), 0.5);
                assert!(max_tokens.get() > 0);
            }
            _ => panic!("Expected OpenAI provider"),
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_provider_from_profile_azure_with_url() {
        let mut profile = ProviderProfile::new(
            "test".to_string(),
            ProviderKind::Azure,
            ModelName::from("gpt-4"),
        );
        profile.api_url =
            Some(url::Url::parse("https://test.openai.azure.com/?api-version=2024-02-15").unwrap());
        profile.azure_api_version = Some("2024-02-15".to_string());
        profile.azure_deployment_id = Some("gpt-4".to_string());

        let provider = Provider::from_profile(&profile, "sk-test").unwrap();

        match provider {
            Provider::Azure {
                endpoint,
                api_version,
                deployment_id,
                ..
            } => {
                assert!(endpoint.contains("azure.com"));
                assert!(!endpoint.contains("openai/deployments"));
                assert!(!endpoint.contains("api-version"));
                assert_eq!(api_version, "2024-02-15");
                assert_eq!(deployment_id, "gpt-4");
            }
            _ => panic!("Expected Azure provider"),
        }
    }

    #[test]
    fn test_provider_from_profile_azure_missing_url() {
        let profile = ProviderProfile::new(
            "test".to_string(),
            ProviderKind::Azure,
            ModelName::from("gpt-4"),
        );

        let result = Provider::from_profile(&profile, "sk-test");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Azure endpoint required")
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_provider_from_profile_azure_extracts_from_url() {
        let mut profile = ProviderProfile::new(
            "test".to_string(),
            ProviderKind::Azure,
            ModelName::from("gpt-4"),
        );
        profile.api_url = Some(
            url::Url::parse(
                "https://test.openai.azure.com/openai/deployments/my-deployment/chat/completions?api-version=2023-12-01"
            )
            .unwrap(),
        );
        // Clear defaults so URL values are extracted
        profile.azure_api_version = None;
        profile.azure_deployment_id = None;

        let provider = Provider::from_profile(&profile, "sk-test").unwrap();

        match provider {
            Provider::Azure {
                api_version,
                deployment_id,
                ..
            } => {
                assert_eq!(api_version, "2023-12-01");
                assert_eq!(deployment_id, "my-deployment");
            }
            _ => panic!("Expected Azure provider"),
        }
    }

    #[test]
    fn test_provider_from_profile_azure_rejects_conflicting_deployment() {
        let mut profile = ProviderProfile::new(
            "test".to_string(),
            ProviderKind::Azure,
            ModelName::from("gpt-4"),
        );
        profile.api_url = Some(
            url::Url::parse(
                "https://test.openai.azure.com/openai/deployments/my-deployment/chat/completions?api-version=2024-02-15"
            )
            .unwrap(),
        );
        profile.azure_api_version = Some("2024-02-15".to_string());
        profile.azure_deployment_id = Some("explicit".to_string());

        let result = Provider::from_profile(&profile, "sk-test");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("azure_deployment_id")
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_provider_from_profile_groq() {
        let profile = ProviderProfile::new(
            "test".to_string(),
            ProviderKind::Groq,
            ModelName::from("llama-3"),
        );

        let provider = Provider::from_profile(&profile, "gsk-test").unwrap();

        match provider {
            Provider::Groq { model, .. } => {
                assert_eq!(model, ModelName::from("llama-3"));
            }
            _ => panic!("Expected Groq provider"),
        }
    }

    #[test]
    fn test_request_from_messages() {
        let messages = vec![
            ChatMessage::system("You are a helpful assistant"),
            ChatMessage::user("Hello"),
        ];
        let max_tokens = TokenCount::new_at_least_one(100);

        let request =
            request_from_messages(&messages, max_tokens, Temperature::try_new(0.7).unwrap());

        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.max_tokens, max_tokens);
        assert_eq!(request.temperature.value(), 0.7);
    }

    #[tokio::test]
    async fn test_provider_mock() {
        let provider = Provider::mock("test response");
        let messages = vec![ChatMessage::user("test")];

        let result = provider.generate(&messages).await.unwrap();
        assert_eq!(result, "test response");
    }

    #[tokio::test]
    async fn test_provider_mock_sequence() {
        let responses = vec![
            Ok("first".to_string()),
            Ok("second".to_string()),
            Err(CompletionError::RateLimited { retry_after: None }),
        ];
        let provider = Provider::mock_sequence(responses);
        let messages = vec![ChatMessage::user("test")];

        assert_eq!(provider.generate(&messages).await.unwrap(), "first");
        assert_eq!(provider.generate(&messages).await.unwrap(), "second");
        assert!(provider.generate(&messages).await.is_err());
    }

    #[tokio::test]
    async fn test_provider_mock_sequence_exhausted() {
        let provider = Provider::mock_sequence(vec![Ok("only".to_string())]);
        let messages = vec![ChatMessage::user("test")];

        let _ = provider.generate(&messages).await;
        let result = provider.generate(&messages).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("sequence exhausted")
        );
    }

    #[tokio::test]
    async fn test_provider_error_timeout() {
        let provider = Provider::mock_sequence(vec![Err(CompletionError::Timeout)]);
        let messages = vec![ChatMessage::user("test")];

        let result = provider.generate(&messages).await;
        assert!(matches!(result, Err(CompletionError::Timeout)));
    }

    #[tokio::test]
    async fn test_provider_error_server_error() {
        let provider = Provider::mock_sequence(vec![Err(CompletionError::ServerError(
            "500 Internal Server Error".to_string(),
        ))]);
        let messages = vec![ChatMessage::user("test")];

        let result = provider.generate(&messages).await;
        assert!(matches!(result, Err(CompletionError::ServerError(_))));
    }

    #[tokio::test]
    async fn test_provider_error_network_error() {
        let provider = Provider::mock_sequence(vec![Err(CompletionError::NetworkError(
            "Connection reset".to_string(),
        ))]);
        let messages = vec![ChatMessage::user("test")];

        let result = provider.generate(&messages).await;
        assert!(matches!(result, Err(CompletionError::NetworkError(_))));
    }

    #[tokio::test]
    async fn test_provider_error_unauthorized() {
        let provider = Provider::mock_sequence(vec![Err(CompletionError::Unauthorized(
            "Invalid API key".to_string(),
        ))]);
        let messages = vec![ChatMessage::user("test")];

        let result = provider.generate(&messages).await;
        assert!(matches!(result, Err(CompletionError::Unauthorized(_))));
    }

    #[tokio::test]
    async fn test_provider_error_invalid_response() {
        let provider = Provider::mock_sequence(vec![Err(CompletionError::InvalidResponse(
            "Malformed JSON".to_string(),
        ))]);
        let messages = vec![ChatMessage::user("test")];

        let result = provider.generate(&messages).await;
        assert!(matches!(result, Err(CompletionError::InvalidResponse(_))));
    }

    #[tokio::test]
    async fn test_provider_error_sequence_mixed() {
        let provider = Provider::mock_sequence(vec![
            Ok("success".to_string()),
            Err(CompletionError::RateLimited { retry_after: None }),
            Ok("recovered".to_string()),
            Err(CompletionError::Timeout),
        ]);
        let messages = vec![ChatMessage::user("test")];

        assert_eq!(provider.generate(&messages).await.unwrap(), "success");
        assert!(matches!(
            provider.generate(&messages).await,
            Err(CompletionError::RateLimited { .. })
        ));
        assert_eq!(provider.generate(&messages).await.unwrap(), "recovered");
        assert!(matches!(
            provider.generate(&messages).await,
            Err(CompletionError::Timeout)
        ));
    }
}
