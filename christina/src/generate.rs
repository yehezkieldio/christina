use anyhow::Result;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::event_loop::Event;
use christina_core::ids::GenerationId;
use christina_core::llm::{ChatMessage, LlmRequest, Role};
use christina_core::types::ProviderKind;
use christina_core::ProviderProfile;
use christina_core::types::CommitMessage;

// TODO: Reimplement using new core LLM types
// Temporarily stubbed out - christina_llm crate has been removed
// use christina_llm::Provider;
// use christina_llm::{AIOrchestrator, GenerationResult};
// use christina_llm::{TokenBudget, get_tokenizer};

/// Temporary stub for GenerationResult - to be reimplemented
#[derive(Debug, Clone)]
pub struct GenerationResult {
    pub message: CommitMessage,
    pub warnings: Vec<String>,
}

impl GenerationResult {
    /// Get a summary of warnings
    pub fn warning_summary(&self) -> Option<String> {
        if self.warnings.is_empty() {
            None
        } else {
            Some(self.warnings.join("\n"))
        }
    }
}

fn config_to_profile(config: &Config) -> ProviderProfile {
    ProviderProfile {
        name: "active".to_string(),
        provider: config.model_provider,
        model: config.model.clone(),
        api_url: config.model_api_url.clone(),
        api_key: match &config.api_key {
            Some(key) => christina_core::config::Secret::Value(key.clone()),
            None => christina_core::config::Secret::Value(String::new()),
        },
        max_input_tokens: config.max_input_tokens,
        max_output_tokens: config.max_output_tokens,
        azure_api_version: config.azure_api_version.clone(),
        azure_deployment_id: config.azure_deployment_id.clone(),
        temperature: None,
    }
}

pub async fn generate_commit_message_with_progress(
    config: Config,
    diff: String,
    progress_tx: mpsc::Sender<Event>,
    generation_id: u64,
    user_context: Option<String>,
) -> Result<GenerationResult> {
    let profile = config_to_profile(&config);

    progress_tx
        .send(Event::GenerationProgress {
            stage: "Building request...".to_string(),
            generation_id,
        })
        .await
        .ok();

    let system_prompt = build_system_prompt();
    let user_message = build_user_message(&diff, user_context.as_deref());

    let request = LlmRequest {
        id: GenerationId::new(generation_id),
        messages: vec![
            ChatMessage {
                role: Role::System,
                content: system_prompt.clone(),
            },
            ChatMessage {
                role: Role::User,
                content: user_message,
            },
        ],
        temperature: profile.temperature.unwrap_or(0.7),
        max_tokens: profile.max_output_tokens,
        system_prompt: Some(system_prompt),
    };

    progress_tx
        .send(Event::GenerationProgress {
            stage: format!("Calling {} API...", profile.provider),
            generation_id,
        })
        .await
        .ok();

    let api_key = match profile.api_key {
        christina_core::config::Secret::Value(ref key) if !key.is_empty() => key.as_str(),
        _ => return Err(anyhow::anyhow!("API key not configured")),
    };

    let model_str = profile.model.as_str();
    let api_url_str = profile.api_url.as_ref().map(|u| u.as_str());

    let response = match profile.provider {
        ProviderKind::OpenAI => {
            crate::io::llm::openai::execute_openai_request(&request, api_key, api_url_str, model_str)
                .await?
        }
        ProviderKind::Azure => {
            let endpoint = profile
                .api_url
                .as_ref()
                .map(|u| u.as_str())
                .ok_or_else(|| anyhow::anyhow!("Azure endpoint not configured"))?;
            let deployment_id = profile
                .azure_deployment_id
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Azure deployment ID not configured"))?;
            let api_version = profile
                .azure_api_version
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Azure API version not configured"))?;

            crate::io::llm::azure::execute_azure_request(
                &request,
                api_key,
                endpoint,
                deployment_id,
                api_version,
                model_str,
            )
            .await?
        }
    };

    progress_tx
        .send(Event::GenerationProgress {
            stage: "Processing response...".to_string(),
            generation_id,
        })
        .await
        .ok();

    let commit_text = response.content.trim().to_string();
    let message = CommitMessage::try_from(commit_text.clone())
        .map_err(|e| anyhow::anyhow!("Failed to parse commit message: {}", e))?;

    let mut warnings = Vec::new();

    if !commit_text.contains(':') {
        warnings.push("Message may not follow conventional commit format".to_string());
    }

    if response.tokens_used.is_none() {
        warnings.push("Token count not available from provider".to_string());
    }

    Ok(GenerationResult { message, warnings })
}

fn build_system_prompt() -> String {
    r#"You are an expert at writing concise, conventional commit messages.
Generate a commit message following these rules:
1. Use conventional commit format: <type>: <description>
2. Type must be one of: feat, fix, docs, style, refactor, test, chore
3. Description must be lowercase, no period at end
4. Keep total message under 72 characters
5. Be specific about what changed, not how
6. Focus on the intent, not implementation details"#
        .to_string()
}

fn build_user_message(diff: &str, user_context: Option<&str>) -> String {
    let mut message = format!(
        "Generate a conventional commit message for these changes:\n\n{}",
        diff
    );

    if let Some(context) = user_context {
        message.push_str(&format!("\n\nAdditional context: {}", context));
    }

    message.push_str("\n\nRespond ONLY with the commit message, no explanation.");
    message
}
