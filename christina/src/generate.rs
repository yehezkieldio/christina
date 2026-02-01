use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::Config;
use crate::event_loop::Event;
use crate::io::git::diff_processor::DiffProcessor;
use crate::io::llm::provider::Provider;
use crate::io::llm::{AIOrchestrator, GenerationResult, TokenBudget, TokenizerService};
use christina_core::ProviderProfile;
use christina_core::prompt::{DIRECT_COMMIT_PROMPT, SYSTEM_PROMPT};
use christina_core::types::TokenCount;

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
        temperature: Some(config.model_temperature),
    }
}

pub async fn generate_commit_message_with_progress(
    config: Config,
    diff: String,
    repo_path: PathBuf,
    progress_tx: mpsc::Sender<Event>,
    generation_id: u64,
    user_context: Option<String>,
) -> Result<GenerationResult> {
    if progress_tx
        .send(Event::GenerationProgress {
            stage: "Retrieving API key...".to_string(),
            generation_id,
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    let api_key = match &config.api_key {
        Some(key) if !key.is_empty() => key.clone(),
        _ => anyhow::bail!("API key not found in configuration"),
    };

    if progress_tx
        .send(Event::GenerationProgress {
            stage: "Connecting to AI provider...".to_string(),
            generation_id,
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    let provider = Provider::from_profile(&config_to_profile(&config), &api_key)?;
    let provider = Arc::new(provider);

    if progress_tx
        .send(Event::GenerationProgress {
            stage: "Processing diff content...".to_string(),
            generation_id,
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    let tokenizer: Arc<dyn christina_core::Tokenizer> = Arc::new(TokenizerService::new()?);
    let system_prompt_tokens = tokenizer.count_tokens(SYSTEM_PROMPT);
    let direct_prompt_tokens = tokenizer.count_tokens(DIRECT_COMMIT_PROMPT);
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

    let processor = DiffProcessor::new(Arc::clone(&tokenizer), token_limit)
        .with_ignore_files(config.ignore_files.clone());

    let chunks = processor
        .process_safe(&diff)
        .map_err(|e| anyhow::anyhow!("Diff processing error: {}", e))?;

    if chunks.is_empty() {
        anyhow::bail!("No processable diff content found");
    }

    let total_tokens = chunks
        .iter()
        .map(|chunk| chunk.token_count.get())
        .sum::<u32>();
    let total_tokens = TokenCount::new_saturating(total_tokens);

    if progress_tx
        .send(Event::TokenCountUpdate {
            token_count: total_tokens,
            generation_id,
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    if progress_tx
        .send(Event::GenerationProgress {
            stage: format!(
                "Analyzing {} chunk{}...",
                chunks.len(),
                if chunks.len() == 1 { "" } else { "s" }
            ),
            generation_id,
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    let orchestrator = AIOrchestrator::with_config(
        Arc::clone(&provider),
        config.max_concurrent_requests,
        config.max_partial_failure_rate,
    );

    let history_context = if config.use_commit_history {
        match get_commit_history(&repo_path, config.commit_history_depth) {
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
        }
    } else {
        None
    };

    if progress_tx
        .send(Event::GenerationProgress {
            stage: "Generating commit message...".to_string(),
            generation_id,
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    let result = orchestrator
        .generate_commit_message(
            chunks,
            user_context.as_deref(),
            config.commit_message_validation_mode,
            config.commit_message_max_length,
            history_context,
        )
        .await?;

    if progress_tx
        .send(Event::GenerationProgress {
            stage: "Finalizing...".to_string(),
            generation_id,
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    Ok(result)
}

#[derive(Debug)]
struct CommitInfo {
    sha: String,
    subject: String,
}

fn get_commit_history(repo_path: &Path, limit: usize) -> Result<Vec<CommitInfo>> {
    let repo = git2::Repository::open(repo_path)?;

    if repo.head().is_err() {
        return Ok(vec![]);
    }

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    let mut commits = Vec::new();

    for oid_result in revwalk {
        if commits.len() >= limit {
            break;
        }

        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;

        if commit.parent_count() > 1 {
            continue;
        }

        let subject = commit
            .message()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .to_string();

        if subject.starts_with("fixup!")
            || subject.starts_with("squash!")
            || subject.starts_with("amend!")
        {
            continue;
        }

        let oid_str = format!("{}", oid);
        let sha = oid_str.get(..7).unwrap_or(oid_str.as_str()).to_string();

        commits.push(CommitInfo { sha, subject });
    }

    Ok(commits)
}
