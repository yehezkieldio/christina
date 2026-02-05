use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::Config;
use crate::events::Event;
use crate::io::git::{chunking, diff_processor::DiffProcessor, parsing};
use crate::io::llm::provider::Provider;
use crate::io::llm::tokenizer::get_tokenizer;
use crate::io::llm::{AIOrchestrator, GenerationResult, TokenBudget};
use christina_core::ProviderProfile;
use christina_core::prompt::{
    DIRECT_COMMIT_PROMPT, SYSTEM_PROMPT, USER_CONTEXT_MAX_LEN, USER_CONTEXT_TEMPLATE,
};
use christina_core::types::{ProviderKind, TokenCount};
use christina_core::types::UsageTier;

const HISTORY_CONTEXT_PREFIX: &str = "\n\nRecent commit history for style reference:\n";

fn normalize_user_context(raw: Option<String>) -> Option<String> {
    let ctx = raw?;
    let trimmed = ctx.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.len() <= USER_CONTEXT_MAX_LEN {
        return Some(trimmed.to_string());
    }

    let mut end = USER_CONTEXT_MAX_LEN;
    while end > 0 && !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    Some(trimmed[..end].to_string())
}

fn user_context_template_parts() -> (&'static str, &'static str) {
    if let Some(pos) = USER_CONTEXT_TEMPLATE.find("{context}") {
        (
            &USER_CONTEXT_TEMPLATE[..pos],
            &USER_CONTEXT_TEMPLATE[pos + 9..],
        )
    } else {
        ("", "")
    }
}

fn fit_user_context_to_budget(
    tokenizer: &dyn christina_core::Tokenizer,
    context: Option<String>,
    budget_tokens: u32,
) -> (Option<String>, u32, bool) {
    let Some(context) = context else {
        return (None, 0, false);
    };

    if budget_tokens == 0 {
        return (None, 0, true);
    }

    let (prefix, suffix) = user_context_template_parts();
    let prefix_tokens = tokenizer.count_tokens_exact(prefix);
    let suffix_tokens = tokenizer.count_tokens_exact(suffix);
    let overhead = prefix_tokens.saturating_add(suffix_tokens);

    if budget_tokens <= overhead {
        return (None, 0, true);
    }

    let allowed_context_tokens = budget_tokens - overhead;
    let context_tokens = tokenizer.count_tokens_exact(&context);

    if context_tokens <= allowed_context_tokens {
        let used = overhead.saturating_add(context_tokens);
        return (Some(context), used, false);
    }

    let allowed = TokenCount::new_at_least_one(allowed_context_tokens);
    let truncated = tokenizer.slice_to_token_limit(&context, allowed).trim_end();
    if truncated.is_empty() {
        return (None, 0, true);
    }
    let truncated_tokens = tokenizer.count_tokens_exact(truncated);
    let used = overhead.saturating_add(truncated_tokens);
    (Some(truncated.to_string()), used, true)
}

fn fit_history_to_budget(
    tokenizer: &dyn christina_core::Tokenizer,
    history: Option<String>,
    budget_tokens: u32,
) -> (Option<String>, u32, bool) {
    let Some(history) = history else {
        return (None, 0, false);
    };

    if budget_tokens == 0 {
        return (None, 0, true);
    }

    let prefix_tokens = tokenizer.count_tokens_exact(HISTORY_CONTEXT_PREFIX);
    if budget_tokens <= prefix_tokens {
        return (None, 0, true);
    }

    let allowed_history_tokens = budget_tokens - prefix_tokens;
    let history_tokens = tokenizer.count_tokens_exact(&history);

    if history_tokens <= allowed_history_tokens {
        let used = prefix_tokens.saturating_add(history_tokens);
        return (Some(history), used, false);
    }

    let allowed = TokenCount::new_at_least_one(allowed_history_tokens);
    let truncated = tokenizer.slice_to_token_limit(&history, allowed);
    let truncated = truncated
        .rfind('\n')
        .map(|idx| &truncated[..idx])
        .unwrap_or(truncated)
        .trim_end();

    if truncated.is_empty() {
        return (None, 0, true);
    }

    let truncated_tokens = tokenizer.count_tokens_exact(truncated);
    let used = prefix_tokens.saturating_add(truncated_tokens);
    (Some(truncated.to_string()), used, true)
}

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

fn require_api_key(config: &Config) -> Result<&str> {
    match config.api_key.as_deref().filter(|key| !key.is_empty()) {
        Some(key) => Ok(key),
        None => anyhow::bail!(
            "API key not found in configuration. Add one with \
             `christina profile add <name> --provider <provider> --model <model> --api-key <key>` \
             or set `api_key` in your config file."
        ),
    }
}

fn config_to_profile(config: &Config, api_key: &str) -> ProviderProfile {
    ProviderProfile {
        name: "active".to_string(),
        provider: config.model_provider,
        model: config.model.clone(),
        api_url: config.model_api_url.clone(),
        api_key: christina_core::config::Secret::Value(api_key.to_string()),
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
    user_context: Option<String>,
) -> Result<GenerationResult> {
    generate_commit_message_with_progress_impl(
        config,
        diff,
        repo_path,
        progress_tx,
        user_context,
        &GitCommitHistoryProvider,
    )
    .await
}

async fn generate_commit_message_with_progress_impl(
    mut config: Config,
    diff: String,
    repo_path: PathBuf,
    progress_tx: mpsc::Sender<Event>,
    user_context: Option<String>,
    history_provider: &dyn CommitHistoryProvider,
) -> Result<GenerationResult> {
    if config.usage_tier == UsageTier::Free && config.model_provider == ProviderKind::Groq {
        let warnings = apply_free_tier_limits(&mut config);
        for warning in warnings {
            warn!("{}", warning);
            eprintln!("Warning: {}", warning);
        }
    } else if config.usage_tier == UsageTier::Free {
        warn!(
            "usage_tier=free is configured but provider is {}, free-tier limits not applied",
            config.model_provider
        );
    }
    // Validate configuration before starting progress events
    let api_key = require_api_key(&config)?;

    if progress_tx
        .send(Event::GenerationProgress {
            stage: "Connecting to AI provider...".to_string(),
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    let provider = Provider::from_profile(&config_to_profile(&config, api_key), api_key)?;
    let provider = Arc::new(provider);

    if progress_tx
        .send(Event::GenerationProgress {
            stage: "Processing diff content...".to_string(),
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    let tokenizer: Arc<dyn christina_core::Tokenizer> = get_tokenizer();
    let system_prompt_tokens = tokenizer.count_tokens(SYSTEM_PROMPT);
    let direct_prompt_tokens = tokenizer.count_tokens(DIRECT_COMMIT_PROMPT);
    let reserved_for_prompt = system_prompt_tokens.max(direct_prompt_tokens);

    let orchestrator = AIOrchestrator::with_config(
        Arc::clone(&provider),
        config.max_concurrent_requests,
        config.max_partial_failure_rate,
    );

    let history_context = if config.use_commit_history {
        match history_provider.get_commit_history(&repo_path, config.commit_history_depth) {
            Ok(mut commits) => {
                if commits.is_empty() {
                    None
                } else {
                    let budget_limit =
                        orchestrator.calculate_history_budget(config.max_input_tokens.get());
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

                    let formatted = commits
                        .iter()
                        .map(|c| format!("- {}: {}", c.sha, c.subject))
                        .collect::<Vec<_>>()
                        .join("\n");
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
                warn!("Failed to retrieve commit history: {}", e);
                None
            }
        }
    } else {
        None
    };

    let message_budget = config
        .max_input_tokens
        .get()
        .checked_sub(config.max_output_tokens.get())
        .and_then(|remaining| remaining.checked_sub(reserved_for_prompt.get()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid token budget: max_output ({}) + reserved_for_prompt ({}) exceeds max_input ({})",
                config.max_output_tokens.get(),
                reserved_for_prompt.get(),
                config.max_input_tokens.get()
            )
        })?;

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
        config.max_input_tokens,
        config.max_output_tokens,
        reserved_for_prompt,
        reserved_for_messages,
    )
    .map_err(|e| anyhow::anyhow!("Invalid token budget configuration: {}", e))?;

    let token_limit = budget
        .remaining_for_diff()
        .map_err(|e| anyhow::anyhow!("Failed to calculate token limit: {}", e))?;

    let processor = DiffProcessor::new(Arc::clone(&tokenizer), token_limit)
        .with_ignore_files(config.ignore_files.clone())
        .with_lockfile_token_limit(config.lockfile_token_limit);

    let chunks = processor.process_safe(&diff);

    if chunks.is_empty() {
        let file_paths = parsing::extract_file_paths(&diff);
        if !file_paths.is_empty()
            && file_paths
                .iter()
                .all(|path| chunking::should_limit_file(path, &config.ignore_files))
        {
            anyhow::bail!(
                "All staged files are in ignore_files list. Update ignore_files in your config or stage other files."
            );
        }
        anyhow::bail!("No processable diff content found");
    }

    let binary_only = chunks
        .iter()
        .all(|chunk| chunk.content.starts_with("[Binary file:"));

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

    if progress_tx
        .send(Event::GenerationProgress {
            stage: format!(
                "Analyzing {} chunk{}...",
                chunks.len(),
                if chunks.len() == 1 { "" } else { "s" }
            ),
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    let user_context = effective_user_context;

    if progress_tx
        .send(Event::GenerationProgress {
            stage: "Generating commit message...".to_string(),
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    let mut result = orchestrator
        .generate_commit_message(
            chunks,
            user_context.as_deref(),
            config.commit_message_validation_mode,
            config.commit_message_max_length,
            history_context,
        )
        .await?;

    if !budget_warnings.is_empty() {
        result.validation_warnings.extend(budget_warnings);
    }

    if binary_only {
        result.validation_warnings.push(
            "Only binary files detected; commit message may be generic.".to_string(),
        );
    }

    if progress_tx
        .send(Event::GenerationProgress {
            stage: "Finalizing...".to_string(),
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    Ok(result)
}

fn apply_free_tier_limits(config: &mut Config) -> Vec<String> {
    let mut warnings = Vec::new();
    let limits = &config.free_tier;

    if config.max_input_tokens > limits.max_input_tokens {
        warnings.push(format!(
            "Free-tier mode: max_input_tokens reduced from {} to {}",
            config.max_input_tokens.get(),
            limits.max_input_tokens.get()
        ));
        config.max_input_tokens = limits.max_input_tokens;
    }

    if config.max_output_tokens > limits.max_output_tokens {
        warnings.push(format!(
            "Free-tier mode: max_output_tokens reduced from {} to {}",
            config.max_output_tokens.get(),
            limits.max_output_tokens.get()
        ));
        config.max_output_tokens = limits.max_output_tokens;
    }

    if config.max_concurrent_requests > limits.max_concurrent_requests {
        warnings.push(format!(
            "Free-tier mode: max_concurrent_requests reduced from {} to {}",
            config.max_concurrent_requests, limits.max_concurrent_requests
        ));
        config.max_concurrent_requests = limits.max_concurrent_requests;
    }

    if config.commit_history_depth > limits.commit_history_depth {
        warnings.push(format!(
            "Free-tier mode: commit_history_depth reduced from {} to {}",
            config.commit_history_depth, limits.commit_history_depth
        ));
        config.commit_history_depth = limits.commit_history_depth;
    }

    warnings
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub sha: String,
    pub subject: String,
}

fn get_commit_history_impl(repo_path: &Path, limit: usize) -> Result<Vec<CommitInfo>> {
    let repo = git2::Repository::open(repo_path)?;

    if repo.is_shallow() {
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
    let effective_limit = if repo.is_shallow() {
        limit.min(3)
    } else {
        limit
    };

    for oid_result in revwalk {
        if commits.len() >= effective_limit {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;
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
    fn test_config_to_profile_openai() {
        let config = Config {
            model: "gpt-4".into(),
            model_provider: ProviderKind::OpenAI,
            api_key: Some("test-key".to_string()),
            max_input_tokens: TokenCount::new_at_least_one(4000),
            max_output_tokens: TokenCount::new_at_least_one(500),
            model_temperature: 0.7,
            ..Default::default()
        };

        let api_key = require_api_key(&config).unwrap();
        let profile = config_to_profile(&config, api_key);

        assert_eq!(profile.name, "active");
        assert_eq!(profile.provider, ProviderKind::OpenAI);
        assert_eq!(profile.model.as_str(), "gpt-4");
        assert_eq!(profile.max_input_tokens, TokenCount::new_at_least_one(4000));
        assert_eq!(profile.max_output_tokens, TokenCount::new_at_least_one(500));
        assert_eq!(profile.temperature, Some(0.7));
    }

    #[test]
    fn test_config_to_profile_azure() {
        let config = Config {
            model: "gpt-4".into(),
            model_provider: ProviderKind::Azure,
            api_key: Some("azure-key".to_string()),
            model_api_url: Some(url::Url::parse("https://test.openai.azure.com").unwrap()),
            azure_api_version: Some("2023-05-15".to_string()),
            azure_deployment_id: Some("gpt-4-deployment".to_string()),
            max_input_tokens: TokenCount::new_at_least_one(8000),
            max_output_tokens: TokenCount::new_at_least_one(1000),
            model_temperature: 0.5,
            ..Default::default()
        };

        let api_key = require_api_key(&config).unwrap();
        let profile = config_to_profile(&config, api_key);

        assert_eq!(profile.provider, ProviderKind::Azure);
        assert_eq!(profile.azure_api_version, Some("2023-05-15".to_string()));
        assert_eq!(
            profile.azure_deployment_id,
            Some("gpt-4-deployment".to_string())
        );
    }

    #[test]
    fn test_config_to_profile_groq() {
        let config = Config {
            model: "mixtral-8x7b".into(),
            model_provider: ProviderKind::Groq,
            api_key: Some("groq-key".to_string()),
            max_input_tokens: TokenCount::new_at_least_one(32000),
            max_output_tokens: TokenCount::new_at_least_one(2000),
            model_temperature: 0.3,
            ..Default::default()
        };

        let api_key = require_api_key(&config).unwrap();
        let profile = config_to_profile(&config, api_key);

        assert_eq!(profile.provider, ProviderKind::Groq);
        assert_eq!(profile.model.as_str(), "mixtral-8x7b");
    }

    #[test]
    fn test_config_to_profile_no_api_key() {
        let config = Config {
            model: "gpt-4".into(),
            model_provider: ProviderKind::OpenAI,
            api_key: None,
            ..Default::default()
        };

        let result = require_api_key(&config);
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

    #[tokio::test]
    async fn test_generate_with_missing_api_key() {
        let config = Config {
            model: "gpt-4".into(),
            model_provider: ProviderKind::OpenAI,
            api_key: None,
            ..Default::default()
        };

        let (tx, _rx) = mpsc::channel(10);
        let diff = "diff --git a/test.txt b/test.txt\n+new line\n".to_string();
        let repo_path = PathBuf::from("/fake/repo");

        let result = generate_commit_message_with_progress_impl(
            config,
            diff,
            repo_path,
            tx,
            None,
            &MockCommitHistoryProvider::empty(),
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("API key not found"));
    }

    #[tokio::test]
    async fn test_generate_with_empty_api_key() {
        let config = Config {
            model: "gpt-4".into(),
            model_provider: ProviderKind::OpenAI,
            api_key: Some(String::new()),
            ..Default::default()
        };

        let (tx, _rx) = mpsc::channel(10);
        let diff = "diff --git a/test.txt b/test.txt\n+new line\n".to_string();
        let repo_path = PathBuf::from("/fake/repo");

        let result = generate_commit_message_with_progress_impl(
            config,
            diff,
            repo_path,
            tx,
            None,
            &MockCommitHistoryProvider::empty(),
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("API key not found"));
    }

    #[tokio::test]
    async fn test_generate_with_empty_diff() {
        let config = Config {
            model: "gpt-4".into(),
            model_provider: ProviderKind::OpenAI,
            api_key: Some("test-key".to_string()),
            max_input_tokens: TokenCount::new_at_least_one(4000),
            max_output_tokens: TokenCount::new_at_least_one(500),
            ..Default::default()
        };

        let (tx, _rx) = mpsc::channel(10);
        let diff = String::new();
        let repo_path = PathBuf::from("/fake/repo");

        let result = generate_commit_message_with_progress_impl(
            config,
            diff,
            repo_path,
            tx,
            None,
            &MockCommitHistoryProvider::empty(),
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No processable diff content found")
        );
    }

    #[tokio::test]
    async fn test_generate_with_progress_receiver_dropped() {
        let config = Config {
            model: "gpt-4".into(),
            model_provider: ProviderKind::OpenAI,
            api_key: Some("test-key".to_string()),
            ..Default::default()
        };

        let (tx, rx) = mpsc::channel(1);
        drop(rx);

        let diff = "diff --git a/test.txt b/test.txt\n+new line\n".to_string();
        let repo_path = PathBuf::from("/fake/repo");

        let result = generate_commit_message_with_progress_impl(
            config,
            diff,
            repo_path,
            tx,
            None,
            &MockCommitHistoryProvider::empty(),
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Progress receiver dropped"));
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
