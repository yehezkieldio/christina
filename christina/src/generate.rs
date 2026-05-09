//! Commit message generation pipeline with progress reporting.
//!
//! WHY lives in CLI crate: orchestration depends on IO (git, LLM provider) and
//! user-facing progress events, which are intentionally outside `christina-core`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::config::RuntimeConfig;
use crate::engines::Provider;
use crate::git::{diff_processor::DiffProcessor, parsing};
use crate::orchestrator::{AIOrchestrator, GenerationResult};
use crate::ui;
use crate::ui::events::Event;
use christina_core::processing::{TokenBudget, get_tokenizer, should_limit_file};
use christina_core::processing::{
    fit_history_to_budget, fit_user_context_to_budget, normalize_user_context,
};
use christina_core::prompt::{DIRECT_COMMIT_PROMPT, SYSTEM_PROMPT};
use christina_core::types::{DiffChunk, TokenCount};

/// Trait for accessing Git repository commit history.
/// Allows for testing without real repository access.
pub trait CommitHistoryProvider: Send + Sync {
    fn get_commit_history(&self, repo_path: &Path, limit: usize) -> Result<Vec<CommitInfo>>;
}

/// Real implementation using git2.
pub struct GitCommitHistoryProvider;

impl CommitHistoryProvider for GitCommitHistoryProvider {
    fn get_commit_history(&self, repo_path: &Path, limit: usize) -> Result<Vec<CommitInfo>> {
        get_commit_history_impl(repo_path, limit)
    }
}

pub async fn generate_commit_message_with_progress_and_trace(
    config: RuntimeConfig,
    diff: Arc<str>,
    repo_path: PathBuf,
    progress_tx: mpsc::Sender<Event>,
    user_context: Option<String>,
    trace: bool,
    shutdown: CancellationToken,
) -> Result<GenerationResult> {
    generate_commit_message_with_progress_impl(
        config,
        diff,
        repo_path,
        progress_tx,
        user_context,
        &GitCommitHistoryProvider,
        trace,
        shutdown,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn generate_commit_message_with_progress_impl(
    config: RuntimeConfig,
    diff: Arc<str>,
    repo_path: PathBuf,
    progress_tx: mpsc::Sender<Event>,
    user_context: Option<String>,
    history_provider: &dyn CommitHistoryProvider,
    trace: bool,
    shutdown: CancellationToken,
) -> Result<GenerationResult> {
    ensure_not_cancelled(&shutdown)?;

    if trace {
        ui::print_trace("starting commit message generation");
    }
    send_generation_progress(&progress_tx, "connecting to provider").await?;

    let provider = Provider::from_profile(&config.provider_profile, config.api_key())?;
    let provider = Arc::new(provider);
    ensure_not_cancelled(&shutdown)?;

    if trace {
        ui::print_trace("created AI provider");
    }
    send_generation_progress(&progress_tx, "processing diff").await?;

    let tokenizer: Arc<dyn christina_core::Tokenizer> = get_tokenizer();
    if trace {
        ui::print_trace("initialized tokenizer");
    }
    let system_prompt_tokens = tokenizer.count_tokens(SYSTEM_PROMPT);
    let direct_prompt_tokens = tokenizer.count_tokens(DIRECT_COMMIT_PROMPT);
    // Reserve worst-case prompt size so later budgeting cannot undercount.
    let reserved_for_prompt = system_prompt_tokens.max(direct_prompt_tokens);

    let orchestrator = AIOrchestrator::with_config(
        Arc::clone(&provider),
        config.max_concurrent_requests,
        config.max_partial_failure_rate,
    );
    if trace {
        ui::print_trace("initialized orchestrator");
    }
    ensure_not_cancelled(&shutdown)?;

    let history_context = if config.use_commit_history {
        if trace {
            ui::print_trace("retrieving commit history");
        }
        match history_provider.get_commit_history(&repo_path, config.commit_history_depth) {
            Ok(mut commits) => {
                if trace {
                    ui::print_trace(&format!("retrieved {} commits from history", commits.len()));
                }
                if commits.is_empty() {
                    None
                } else {
                    let budget_limit =
                        orchestrator.calculate_history_budget(config.max_input_tokens().get());
                    let original_count = commits.len();
                    commits.truncate(budget_limit);

                    let omitted_count = original_count.saturating_sub(commits.len());
                    if omitted_count > 0 {
                        info!(
                            "Truncated commit history from {} to {} commits to fit token budget",
                            original_count,
                            commits.len()
                        );
                    }

                    let mut formatted = String::with_capacity(commits.len() * 80);
                    for (index, commit) in commits.iter().enumerate() {
                        if index > 0 {
                            formatted.push('\n');
                        }
                        formatted.push_str("- ");
                        formatted.push_str(&commit.sha);
                        formatted.push_str(": ");
                        formatted.push_str(&commit.subject);
                    }
                    let history = if omitted_count > 0 {
                        format!(
                            "Recent commits:\n{}\n[... {} older commits omitted ...]",
                            formatted, omitted_count
                        )
                    } else {
                        format!("Recent commits:\n{}", formatted)
                    };
                    Some(history)
                }
            }
            Err(e) => {
                if trace {
                    ui::print_trace("failed to retrieve commit history");
                }
                warn!("Failed to retrieve commit history: {}", e);
                None
            }
        }
    } else {
        if trace {
            ui::print_trace("skipping commit history (disabled in config)");
        }
        None
    };

    let message_budget = config
        .max_input_tokens()
        .get()
        .checked_sub(config.max_output_tokens().get())
        .and_then(|remaining| remaining.checked_sub(reserved_for_prompt.get()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid token budget: max_output ({}) + reserved_for_prompt ({}) exceeds max_input ({})",
                config.max_output_tokens().get(),
                reserved_for_prompt.get(),
                config.max_input_tokens().get()
            )
        })?;

    if trace {
        ui::print_trace(&format!(
            "calculated message budget: {} tokens",
            message_budget
        ));
    }
    let mut budget_warnings = Vec::new();
    let normalized_user_context = normalize_user_context(user_context);
    let had_user_context = normalized_user_context.is_some();
    let (effective_user_context, user_context_tokens, user_context_truncated) =
        fit_user_context_to_budget(tokenizer.as_ref(), normalized_user_context, message_budget);

    if user_context_truncated && had_user_context {
        if effective_user_context.is_some() {
            budget_warnings.push("User context truncated to fit token budget.".to_string());
        } else {
            budget_warnings.push("User context omitted to fit token budget.".to_string());
        }
    }

    let remaining_for_history = message_budget.saturating_sub(user_context_tokens);
    let had_history_context = history_context.is_some();
    let (history_context, history_tokens, history_truncated) =
        fit_history_to_budget(tokenizer.as_ref(), history_context, remaining_for_history);

    if history_truncated && had_history_context {
        if history_context.is_some() {
            budget_warnings.push("Commit history truncated to fit token budget.".to_string());
        } else {
            budget_warnings.push("Commit history omitted to fit token budget.".to_string());
        }
    }

    let reserved_for_messages =
        TokenCount::new_at_least_one(user_context_tokens.saturating_add(history_tokens));

    let budget = TokenBudget::try_new(
        config.max_input_tokens(),
        config.max_output_tokens(),
        reserved_for_prompt,
        reserved_for_messages,
    )
    .map_err(|e| anyhow::anyhow!("Invalid token budget configuration: {}", e))?;

    let token_limit = budget
        .remaining_for_diff()
        .map_err(|e| anyhow::anyhow!("Failed to calculate token limit: {}", e))?;

    if trace {
        ui::print_trace(&format!(
            "set token limit for diff processing: {} tokens",
            token_limit.get()
        ));
    }

    let processor = DiffProcessor::new(Arc::clone(&tokenizer), token_limit)
        .with_ignore_files(config.ignore_files.clone())
        .with_lockfile_token_limit(config.lockfile_token_limit);

    if trace {
        ui::print_trace("processing diff content");
    }
    let mut chunks = processor.process_safe(diff.as_ref());

    if trace {
        ui::print_trace(&format!("raw diff length: {} characters", diff.len()));
        ui::print_trace(&format!("processed {} diff chunks", chunks.len()));
        for (i, chunk) in chunks.iter().enumerate() {
            let mut file_list = String::with_capacity(chunk.files.len() * 32);
            for (index, file) in chunk.files.iter().enumerate() {
                if index > 0 {
                    file_list.push_str(", ");
                }
                file_list.push_str(file.as_ref());
            }
            ui::print_trace(&format!(
                "  [{}] {} tokens, {} files: {}",
                i,
                chunk.token_count.get(),
                chunk.files.len(),
                file_list
            ));
        }
    }

    if chunks.is_empty() {
        let file_paths = parsing::extract_file_paths(diff.as_ref());
        if !file_paths.is_empty()
            && file_paths
                .iter()
                .all(|path| should_limit_file(path, &config.ignore_files))
        {
            anyhow::bail!(
                "All staged files are in ignore_files list. Update ignore_files in your config or stage other files."
            );
        }
        anyhow::bail!("No processable diff content found");
    }

    let original_chunk_count = chunks.len();
    let total_tokens_before_merge = chunks
        .iter()
        .map(|chunk| chunk.token_count.get() as u64)
        .sum::<u64>();
    let total_tokens_before_merge =
        TokenCount::new_at_least_one(total_tokens_before_merge.try_into().unwrap_or(u32::MAX));

    if original_chunk_count > 1 && total_tokens_before_merge <= token_limit {
        let total_files = chunks.iter().map(|chunk| chunk.files.len()).sum::<usize>();
        let mut combined_files = Vec::with_capacity(total_files);
        let mut seen = HashSet::with_capacity(total_files);
        let combined_capacity = chunks
            .iter()
            .map(|chunk| chunk.content.len())
            .sum::<usize>()
            .saturating_add((original_chunk_count.saturating_sub(1)) * 2);
        let mut combined_content = String::with_capacity(combined_capacity);
        for (index, chunk) in chunks.iter().enumerate() {
            if index > 0 {
                combined_content.push_str("\n\n");
            }
            combined_content.push_str(chunk.content.as_ref());
        }

        for chunk in &chunks {
            for file in &chunk.files {
                if seen.insert(file.clone()) {
                    combined_files.push(file.clone());
                }
            }
        }

        let combined_token_count = tokenizer.count_tokens(&combined_content);
        chunks = vec![DiffChunk::new(
            Arc::from(combined_content),
            combined_files,
            combined_token_count,
        )];

        if trace {
            ui::print_trace(&format!(
                "fast path: merged {} chunks into single prompt",
                original_chunk_count
            ));
        }
    }
    let binary_only = chunks
        .iter()
        .all(|chunk| chunk.content.starts_with("[Binary file:"));

    if progress_tx
        .send(Event::DiffChunked {
            chunk_count: chunks.len(),
            binary_only,
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    let total_tokens = chunks
        .iter()
        .map(|chunk| chunk.token_count.get() as u64)
        .sum::<u64>();
    let total_tokens = TokenCount::new_at_least_one(total_tokens.try_into().unwrap_or(u32::MAX));

    if progress_tx
        .send(Event::TokenCountUpdate {
            token_count: total_tokens,
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    if trace {
        ui::print_trace(&format!("total tokens in chunks: {}", total_tokens.get()));
    }

    let chunk_label = if chunks.len() == 1 { "" } else { "s" };
    send_generation_progress(
        &progress_tx,
        format!("analyzing {} chunk{}", chunks.len(), chunk_label),
    )
    .await?;

    let user_context = effective_user_context;

    send_generation_progress(&progress_tx, "generating message").await?;

    if trace {
        ui::print_trace(&format!(
            "calling orchestrator to generate commit message with {} chunks",
            chunks.len()
        ));
        ui::print_trace(&format!(
            "user context: {}",
            user_context
                .as_deref()
                .map_or("none".to_string(), |ctx| format!(
                    "present ({} chars)",
                    ctx.len()
                ))
        ));
        ui::print_trace(&format!(
            "history context: {}",
            history_context
                .as_ref()
                .map_or("none".to_string(), |ctx| format!(
                    "present ({} chars)",
                    ctx.len()
                ))
        ));
    }
    let mut result = orchestrator
        .generate_commit_message_with_trace_and_cancellation(
            chunks,
            user_context.as_deref(),
            config.commit_message_validation_mode,
            config.commit_message_max_length,
            history_context,
            trace,
            shutdown,
        )
        .await?;

    if trace {
        ui::print_trace(&format!(
            "generation completed with {} failed chunks out of {}",
            result.failed_chunks, result.total_chunks
        ));
        ui::print_trace(&format!(
            "intent fallback used: {}",
            result.intent_fallback_used
        ));
    }

    if !budget_warnings.is_empty() {
        result.validation_warnings.extend(budget_warnings);
    }

    if result.total_chunks > 0 {
        let failure_rate = result.failed_chunks as f64 / result.total_chunks as f64;
        if failure_rate > config.prompt_failure_rate_threshold {
            result.validation_warnings.push(format!(
                "High chunk failure rate: {:.0}% exceeds threshold {:.0}%",
                failure_rate * 100.0,
                config.prompt_failure_rate_threshold * 100.0
            ));
        }
    }

    if binary_only {
        result
            .validation_warnings
            .push("Only binary files detected; commit message may be generic.".to_string());
    }

    if trace {
        ui::print_trace("finalizing generation result");
    }
    send_generation_progress(&progress_tx, "finalizing").await?;

    Ok(result)
}

async fn send_generation_progress(
    progress_tx: &mpsc::Sender<Event>,
    stage: impl Into<String>,
) -> Result<()> {
    progress_tx
        .send(Event::GenerationProgress {
            stage: stage.into(),
        })
        .await
        .map_err(|_| anyhow::anyhow!("Progress receiver dropped, aborting generation"))
}

fn ensure_not_cancelled(shutdown: &CancellationToken) -> Result<()> {
    if shutdown.is_cancelled() {
        anyhow::bail!("Generation cancelled");
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub sha: String,
    pub subject: String,
}

fn get_commit_history_impl(repo_path: &Path, limit: usize) -> Result<Vec<CommitInfo>> {
    let repo = git2::Repository::open(repo_path)?;

    let is_shallow = repo.is_shallow();
    if is_shallow {
        warn!("Running in shallow clone, commit history may be limited");
        // In shallow clones, limit history to what's available (typically 1 commit)
        // to avoid spending time on unavailable history
        info!("Adapting commit history depth for shallow clone");
    }

    if repo.head().is_err() {
        return Ok(vec![]);
    }

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    let mut commits = Vec::new();

    // For shallow clones, cap at a lower limit since history is limited anyway
    let effective_limit = if is_shallow { limit.min(3) } else { limit };

    for oid_result in revwalk {
        if commits.len() >= effective_limit {
            break;
        }

        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;

        // Skip merge commits to keep history focused on linear summaries.
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

        // Skip work-in-progress helper commits; they add noise to style inference.
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::Config;
    use christina_core::types::ProviderKind;

    struct MockCommitHistoryProvider {
        commits: Vec<CommitInfo>,
    }

    impl MockCommitHistoryProvider {
        fn new(commits: Vec<CommitInfo>) -> Self {
            Self { commits }
        }

        fn empty() -> Self {
            Self {
                commits: Vec::new(),
            }
        }
    }

    impl CommitHistoryProvider for MockCommitHistoryProvider {
        fn get_commit_history(&self, _repo_path: &Path, limit: usize) -> Result<Vec<CommitInfo>> {
            Ok(self.commits.iter().take(limit).cloned().collect())
        }
    }

    struct FailingCommitHistoryProvider;

    impl CommitHistoryProvider for FailingCommitHistoryProvider {
        fn get_commit_history(&self, _repo_path: &Path, _limit: usize) -> Result<Vec<CommitInfo>> {
            anyhow::bail!("Failed to retrieve commit history")
        }
    }

    #[test]
    fn test_config_resolves_runtime_profile() {
        let config = Config {
            model: "gpt-4o".into(),
            model_provider: ProviderKind::Azure,
            api_key: Some("test-key".to_string()),
            max_input_tokens: TokenCount::new_at_least_one(256000),
            max_output_tokens: TokenCount::new_at_least_one(4096),
            model_temperature: 0.7,
            azure_api_version: Some("2024-12-01-preview".to_string()),
            azure_deployment_id: Some("gpt-4o".to_string()),
            ..Default::default()
        };

        let runtime = config.validate_and_resolve().unwrap();
        let profile = &runtime.provider_profile;

        assert_eq!(profile.name, "active");
        assert_eq!(profile.provider, ProviderKind::Azure);
        assert_eq!(profile.model.as_str(), "gpt-4o");
        assert_eq!(
            profile.max_input_tokens,
            TokenCount::new_at_least_one(256000)
        );
        assert_eq!(
            profile.max_output_tokens,
            TokenCount::new_at_least_one(4096)
        );
        assert_eq!(profile.temperature, Some(0.7));
        assert_eq!(runtime.api_key(), "test-key");
    }

    #[test]
    fn test_runtime_config_requires_api_key() {
        let config = Config {
            model: "gpt-4o".into(),
            model_provider: ProviderKind::Azure,
            api_key: None,
            ..Default::default()
        };

        let result = config.validate_and_resolve();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("API key not found"));
    }

    #[test]
    fn test_mock_commit_history_provider_empty() {
        let provider = MockCommitHistoryProvider::empty();
        let result = provider
            .get_commit_history(Path::new("/fake/path"), 10)
            .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_mock_commit_history_provider_with_commits() {
        let commits = vec![
            CommitInfo {
                sha: "abc1234".to_string(),
                subject: "feat: add feature A".to_string(),
            },
            CommitInfo {
                sha: "def5678".to_string(),
                subject: "fix: resolve bug B".to_string(),
            },
        ];

        let provider = MockCommitHistoryProvider::new(commits);
        let result = provider
            .get_commit_history(Path::new("/fake/path"), 10)
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].sha, "abc1234");
        assert_eq!(result[0].subject, "feat: add feature A");
        assert_eq!(result[1].sha, "def5678");
        assert_eq!(result[1].subject, "fix: resolve bug B");
    }

    #[test]
    fn test_mock_commit_history_provider_respects_limit() {
        let commits = vec![
            CommitInfo {
                sha: "abc1234".to_string(),
                subject: "feat: add feature A".to_string(),
            },
            CommitInfo {
                sha: "def5678".to_string(),
                subject: "fix: resolve bug B".to_string(),
            },
            CommitInfo {
                sha: "ghi9012".to_string(),
                subject: "chore: update deps".to_string(),
            },
        ];

        let provider = MockCommitHistoryProvider::new(commits);
        let result = provider
            .get_commit_history(Path::new("/fake/path"), 2)
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].sha, "abc1234");
        assert_eq!(result[1].sha, "def5678");
    }

    #[test]
    fn test_failing_commit_history_provider() {
        let provider = FailingCommitHistoryProvider;
        let result = provider.get_commit_history(Path::new("/fake/path"), 10);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to retrieve commit history")
        );
    }

    #[test]
    fn test_runtime_config_rejects_empty_api_key() {
        let config = Config {
            model: "gpt-4o".into(),
            model_provider: ProviderKind::Azure,
            api_key: Some(String::new()),
            ..Default::default()
        };

        let result = config.validate_and_resolve();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("API key not found")
        );
    }

    #[tokio::test]
    async fn test_generate_with_empty_diff() {
        let config = Config {
            model: "gpt-4o".into(),
            model_provider: ProviderKind::Azure,
            api_key: Some("test-key".to_string()),
            max_input_tokens: TokenCount::new_at_least_one(256000),
            max_output_tokens: TokenCount::new_at_least_one(4096),
            model_api_url: Some(url::Url::parse("https://test.openai.azure.com/").unwrap()),
            azure_api_version: Some("2024-12-01-preview".to_string()),
            azure_deployment_id: Some("gpt-4o".to_string()),
            ..Default::default()
        }
        .validate_and_resolve()
        .unwrap();

        let (tx, _rx) = mpsc::channel(10);
        let diff = Arc::<str>::from("");
        let repo_path = PathBuf::from("/fake/repo");

        let result = generate_commit_message_with_progress_impl(
            config,
            diff,
            repo_path,
            tx,
            None,
            &MockCommitHistoryProvider::empty(),
            false,
            CancellationToken::new(),
        )
        .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        eprintln!("Actual error message: {}", error_msg); // Debug print
        assert!(error_msg.contains("No processable diff content found"));
    }

    #[test]
    fn test_commit_info_debug() {
        let commit = CommitInfo {
            sha: "abc1234".to_string(),
            subject: "feat: add feature".to_string(),
        };

        let debug_str = format!("{:?}", commit);
        assert!(debug_str.contains("abc1234"));
        assert!(debug_str.contains("feat: add feature"));
    }

    #[test]
    fn test_commit_info_clone() {
        let commit = CommitInfo {
            sha: "abc1234".to_string(),
            subject: "feat: add feature".to_string(),
        };

        let cloned = commit.clone();
        assert_eq!(commit.sha, cloned.sha);
        assert_eq!(commit.subject, cloned.subject);
    }

    #[test]
    fn test_git_commit_history_provider_non_existent_repo() {
        let provider = GitCommitHistoryProvider;
        let result = provider.get_commit_history(Path::new("/non/existent/path"), 10);
        assert!(result.is_err());
    }
}
