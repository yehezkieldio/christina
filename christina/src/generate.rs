use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::Config;
use crate::events::Event;
use crate::io::git::diff_processor::DiffProcessor;
use crate::io::llm::provider::Provider;
use crate::io::llm::tokenizer::get_tokenizer;
use crate::io::llm::{AIOrchestrator, GenerationResult, TokenBudget};
use christina_core::ProviderProfile;
use christina_core::prompt::{DIRECT_COMMIT_PROMPT, SYSTEM_PROMPT};
use christina_core::types::TokenCount;

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

fn config_to_profile(config: &Config) -> ProviderProfile {
    // This function should only be called after API key validation.
    // Empty API keys will cause authorization failures downstream.
    // Use regular assert (not debug_assert) to catch this in release builds.
    assert!(
        config.api_key.as_ref().is_some_and(|k| !k.is_empty()),
        "config_to_profile called with missing or empty API key - this is a programming error"
    );

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
    config: Config,
    diff: String,
    repo_path: PathBuf,
    progress_tx: mpsc::Sender<Event>,
    user_context: Option<String>,
    history_provider: &dyn CommitHistoryProvider,
) -> Result<GenerationResult> {
    // Validate configuration before starting progress events
    let api_key = match &config.api_key {
        Some(key) if !key.is_empty() => key.clone(),
        _ => anyhow::bail!("API key not found in configuration"),
    };

    if progress_tx
        .send(Event::GenerationProgress {
            stage: "Connecting to AI provider...".to_string(),
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
        })
        .await
        .is_err()
    {
        anyhow::bail!("Progress receiver dropped, aborting generation");
    }

    let tokenizer: Arc<dyn christina_core::Tokenizer> = Arc::clone(&get_tokenizer()?);
    let system_prompt_tokens = tokenizer.count_tokens(SYSTEM_PROMPT);
    let direct_prompt_tokens = tokenizer.count_tokens(DIRECT_COMMIT_PROMPT);
    let reserved_for_prompt = system_prompt_tokens.max(direct_prompt_tokens);
    let reserved_for_messages = TokenCount::new_at_least_one(500);

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
        .with_ignore_files(config.ignore_files.clone());

    let chunks = processor
        .process_safe(&diff)
        .map_err(|e| anyhow::anyhow!("Diff processing error: {}", e))?;

    if chunks.is_empty() {
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

        let profile = config_to_profile(&config);

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

        let profile = config_to_profile(&config);

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

        let profile = config_to_profile(&config);

        assert_eq!(profile.provider, ProviderKind::Groq);
        assert_eq!(profile.model.as_str(), "mixtral-8x7b");
    }

    #[test]
    #[should_panic(
        expected = "config_to_profile called with missing or empty API key - this is a programming error"
    )]
    fn test_config_to_profile_no_api_key() {
        let config = Config {
            model: "gpt-4".into(),
            model_provider: ProviderKind::OpenAI,
            api_key: None,
            ..Default::default()
        };

        // This should panic because config_to_profile requires a valid API key
        let _profile = config_to_profile(&config);
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
