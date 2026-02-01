use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::Config;
use crate::event_loop::Event;
use christina_core::ProviderProfile;
use christina_core::types::TokenCount;
use christina_git::{DiffProcessor, repository::GitRepository};
use christina_llm::Provider;
use christina_llm::{AIOrchestrator, GenerationResult};
use christina_llm::{TokenBudget, get_tokenizer};

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
    // Get API key
    let _ = progress_tx
        .send(Event::GenerationProgress {
            stage: "Retrieving API key...".to_string(),
            generation_id,
        })
        .await;
    let api_key = match config.api_key {
        Some(ref key) => key.clone(),
        None => {
            anyhow::bail!("API key not found in configuration");
        }
    };

    // Create provider
    let _ = progress_tx
        .send(Event::GenerationProgress {
            stage: "Connecting to AI provider...".to_string(),
            generation_id,
        })
        .await;

    let provider = Provider::from_profile(&config_to_profile(&config), &api_key)?;
    let provider = Arc::new(provider);

    // Process diff into chunks with safe binary detection
    let _ = progress_tx
        .send(Event::GenerationProgress {
            stage: "Processing diff content...".to_string(),
            generation_id,
        })
        .await;

    let tokenizer = get_tokenizer()?;

    // Dynamic token budgeting: measure prompt overhead
    let system_prompt_tokens = tokenizer.count_tokens(christina_core::prompt::SYSTEM_PROMPT);
    let direct_prompt_tokens = tokenizer.count_tokens(christina_core::prompt::DIRECT_COMMIT_PROMPT);
    // Reserve space for the larger of the two prompts, plus overhead for message formatting
    let reserved_for_prompt = system_prompt_tokens.max(direct_prompt_tokens);
    let reserved_for_messages = TokenCount::new_saturating(500);

    let budget = TokenBudget::new(
        config.max_input_tokens,
        config.max_output_tokens,
        reserved_for_prompt,
        reserved_for_messages,
    );
    let token_limit = budget
        .remaining_for_diff()
        .map_err(|e| anyhow::anyhow!("Invalid token budget configuration: {}", e))?;

    let processor =
        DiffProcessor::new(tokenizer, token_limit).with_ignore_files(config.ignore_files.clone());

    // Use process_safe to detect binary content and prevent garbage in AI prompts
    let chunks = processor
        .process_safe(&diff)
        .map_err(|e| anyhow::anyhow!("Diff processing error: {}", e))?;

    if chunks.is_empty() {
        anyhow::bail!("No processable diff content found");
    }

    // Calculate total token count from all chunks
    let total_tokens = chunks
        .iter()
        .map(|chunk| chunk.token_count.get())
        .sum::<u32>();
    let total_tokens = TokenCount::new_saturating(total_tokens);

    // Send token count update
    let _ = progress_tx
        .send(Event::TokenCountUpdate {
            token_count: total_tokens,
            generation_id,
        })
        .await;

    let _ = progress_tx
        .send(Event::GenerationProgress {
            stage: format!(
                "Analyzing {} chunk{}...",
                chunks.len(),
                if chunks.len() == 1 { "" } else { "s" }
            ),
            generation_id,
        })
        .await;

    // Generate commit message
    let orchestrator = AIOrchestrator::new(Arc::clone(&provider));

    let history_context = if config.use_commit_history {
        match GitRepository::discover() {
            Ok(repo) => match repo.get_commit_history(config.commit_history_depth) {
                Ok(mut commits) => {
                    if commits.is_empty() {
                        None
                    } else {
                        let budget_limit =
                            orchestrator.calculate_history_budget(config.max_input_tokens.get());
                        let original_count = commits.len();
                        commits.truncate(budget_limit);

                        if commits.len() < original_count {
                            info!(
                                "Truncated commit history from {} to {} commits to fit token budget",
                                original_count,
                                commits.len()
                            );
                        }

                        let formatted = commits
                            .iter()
                            .map(|c| format!("- {}: {}", c.sha, c.subject))
                            .collect::<Vec<_>>()
                            .join("\n");
                        Some(format!("Recent commits:\n{}", formatted))
                    }
                }
                Err(e) => {
                    warn!("Failed to retrieve commit history: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("Failed to discover git repository: {}", e);
                None
            }
        }
    } else {
        None
    };

    let _ = progress_tx
        .send(Event::GenerationProgress {
            stage: "Generating commit message...".to_string(),
            generation_id,
        })
        .await;

    let result = orchestrator
        .generate_commit_message(
            chunks,
            user_context.as_deref(),
            config.commit_message_validation_mode,
            config.commit_message_max_length,
            history_context,
        )
        .await?;

    let _ = progress_tx
        .send(Event::GenerationProgress {
            stage: "Finalizing...".to_string(),
            generation_id,
        })
        .await;

    Ok(result)
}
