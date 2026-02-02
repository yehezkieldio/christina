//! Map-Reduce orchestrator for multi-chunk diff commit message generation.
//!
//! WHY map-reduce: Large diffs (100+ files) exceed LLM context windows. Map phase
//! summarizes each chunk independently (parallel, no sequential dependencies). Reduce
//! phase synthesizes summaries into coherent commit message. Alternative (sequential
//! processing) would be O(n) API calls with blocking; map-reduce is O(log n) with parallelism.
//!
//! WHY intent extraction: With 10+ chunk summaries, reduce phase prompt becomes
//! incoherent ("feat: X, fix: Y, refactor: Z..."). Intent extraction groups summaries
//! by theme/scope, producing structured context. Improves message quality and prevents
//! LLM confusion from contradictory summaries.
//!
//! WHY partial failure tolerance: Map phase can fail on individual chunks (rate limits,
//! malformed diffs). With 100 chunks, 5% failure is acceptable if we can still generate
//! meaningful message from 95 successes. Alternative (fail-fast) would abort entire
//! workflow on single chunk failure, wasting completed work.
//!
//! WHY systemic vs partial failure: Systemic errors (auth, invalid API key) affect ALL
//! requests—no point continuing. Partial errors (single chunk parsing) are isolated.
//! Detecting systemic early (first failure in map phase) prevents wasting API calls.
//!
//! WHY direct generation fast path: Single-chunk diffs don't need map-reduce overhead.
//! Direct generation saves 2 API calls (map + intent) and reduces latency from ~5s to ~1s.
//! Trade-off: slightly different prompt structure, but acceptable for simple diffs.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use serde::Deserialize;
use tracing::{debug, warn};

use crate::io::llm::concurrency::RequestLimiter;
use crate::io::llm::provider::Provider;
use crate::io::llm::retry::RetryPolicy;

use christina_core::error::CompletionError;
use christina_core::git::DiffChunk;
use christina_core::llm::{ChatMessage, Role};
use christina_core::prompt::{PromptBuilder, Theme};
use christina_core::types::{CommitMessage, FilePath, commit_message::ValidationMode};

// WHY 5 concurrent: Balance between throughput and rate limits. Lower = slower; higher
// risks rate limit violations. Most providers allow 10-50 req/s; 5 concurrent @ 1s/req = 5 req/s.
const MAX_CONCURRENT_REQUESTS: usize = 5;

// WHY progressive timeouts: Initial requests prime cache (cold start = slower). Retries
// benefit from warm cache. Final timeout (120s) prevents infinite hangs on provider issues.
const LLM_INITIAL_TIMEOUT_SECONDS: u64 = 30;
const LLM_RETRY_TIMEOUT_SECONDS: u64 = 60;
const LLM_TIMEOUT_SECONDS: u64 = 120;

// WHY 20 summaries: Intent extraction prompt has ~500 token overhead + 50 tokens/summary.
// 20 summaries = 1500 tokens, leaves ~2500 for context in 4K window. Higher = truncation risk.
const MAX_SUMMARIES_PER_INTENT_BATCH: usize = 20;

#[derive(Debug)]
enum MapError {
    Completion(CompletionError),
}

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapError::Completion(e) => write!(f, "{}", e),
        }
    }
}

impl MapError {
    fn is_systemic(&self) -> bool {
        match self {
            MapError::Completion(e) => e.is_provider_error(),
        }
    }
}

impl From<MapError> for anyhow::Error {
    fn from(err: MapError) -> Self {
        match err {
            MapError::Completion(e) => anyhow::Error::new(e),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChunkSummary {
    pub summary: String,
    pub files: Vec<FilePath>,
}

#[derive(Debug, Clone)]
pub struct GenerationResult {
    pub message: CommitMessage,
    pub truncated: bool,
    pub salvaged: bool,
    pub failed_chunks: usize,
    pub failed_files: Vec<FilePath>,
    pub total_chunks: usize,
    pub intent_fallback_used: bool,
    pub validation_warnings: Vec<String>,
}

impl GenerationResult {
    pub fn has_warnings(&self) -> bool {
        self.truncated
            || self.salvaged
            || self.failed_chunks > 0
            || self.intent_fallback_used
            || !self.validation_warnings.is_empty()
    }

    pub fn warning_summary(&self) -> Option<String> {
        if !self.has_warnings() {
            return None;
        }

        let mut warnings = Vec::new();
        if self.truncated {
            warnings.push("Message was truncated to fit length limit".to_string());
        }
        if self.salvaged {
            warnings.push("Message was extracted from malformed LLM output".to_string());
        }
        if self.failed_chunks > 0 {
            if self.failed_files.is_empty() {
                warnings.push(format!(
                    "{} of {} chunks failed to process",
                    self.failed_chunks, self.total_chunks
                ));
            } else {
                let failed = self
                    .failed_files
                    .iter()
                    .map(|path| path.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                warnings.push(format!(
                    "{} of {} chunks failed: {}",
                    self.failed_chunks, self.total_chunks, failed
                ));
            }
        }
        if self.intent_fallback_used {
            warnings.push("Used fallback themes without intent extraction".to_string());
        }
        warnings.extend(self.validation_warnings.clone());

        Some(warnings.join("; "))
    }
}

#[derive(Debug, Deserialize)]
struct ThemeResponse {
    themes: Vec<ThemeItem>,
}

#[derive(Debug, Deserialize)]
struct ThemeItem {
    title: String,
    description: String,
    #[serde(rename = "fileCount")]
    file_count: usize,
    scope: String,
}

#[derive(Debug, Clone)]
struct SubTheme {
    title: String,
    description: String,
    file_count: usize,
    scope: String,
}

pub struct AIOrchestrator {
    provider: Arc<Provider>,
    limiter: RequestLimiter,
    retry_policy: RetryPolicy,
    concurrency_limit: usize,
    max_partial_failure_rate: f64,
}

impl AIOrchestrator {
    #[cfg(test)]
    pub fn new(provider: Arc<Provider>) -> Self {
        Self::with_config(provider, MAX_CONCURRENT_REQUESTS, 0.10)
    }

    pub fn with_config(
        provider: Arc<Provider>,
        concurrency_limit: usize,
        max_partial_failure_rate: f64,
    ) -> Self {
        let requests_per_second = 5.0;
        let concurrency_limit = concurrency_limit.clamp(1, 20);

        Self {
            provider,
            limiter: RequestLimiter::new(concurrency_limit, requests_per_second),
            retry_policy: RetryPolicy::default(),
            concurrency_limit,
            max_partial_failure_rate: max_partial_failure_rate.clamp(0.0, 1.0),
        }
    }

    /// Calculate budget for commit history context in prompt.
    ///
    /// WHY 15% allocation: Empirically derived balance. Less = insufficient style reference;
    /// more = reduces space for actual diff content. 150 tokens/commit is average for
    /// conventional commit format (type + scope + description).
    ///
    /// WHY minimum 3 commits: Below 3, style inference is unreliable (insufficient samples).
    /// Even tiny context budgets get at least 3 commits for pattern recognition.
    pub fn calculate_history_budget(&self, max_input_tokens: u32) -> usize {
        let budget_tokens = max_input_tokens as f64 * 0.15;
        let commits_available = (budget_tokens / 150.0).floor() as usize;
        commits_available.max(3)
    }

    /// Generate commit message using map-reduce or direct generation.
    ///
    /// WHY single-chunk fast path: Direct generation for 1 chunk saves 2 API calls
    /// (map summary + intent extraction). Reduces latency from ~5s to ~1s for simple diffs.
    ///
    /// WHY 3-phase pipeline: Map (parallel chunk summarization) → Intent (extract themes)
    /// → Reduce (synthesize message). Each phase is independently testable and optimizable.
    ///
    /// WHY detect_contradictions on ≤2 summaries: Intent extraction requires ≥3 summaries
    /// for meaningful clustering. With 2, fallback to heuristic theme generation. Contradiction
    /// detection warns about conflicting changes (feat+revert) that need manual review.
    pub async fn generate_commit_message(
        &self,
        chunks: Vec<DiffChunk>,
        user_context: Option<&str>,
        validation_mode: ValidationMode,
        max_length: Option<usize>,
        history_context: Option<String>,
    ) -> Result<GenerationResult> {
        if chunks.is_empty() {
            anyhow::bail!("No diff chunks to process");
        }

        let debug_enabled = debug_enabled();
        let total_chunks = chunks.len();

        if chunks.len() == 1 {
            let direct_start = if debug_enabled {
                Some(Instant::now())
            } else {
                None
            };
            let (message, truncated, salvaged, validation_warnings) = self
                .direct_generation(
                    &chunks[0],
                    user_context,
                    validation_mode,
                    max_length,
                    history_context.clone(),
                )
                .await?;
            if let Some(start) = direct_start {
                debug!("direct generation completed in {:?}", start.elapsed());
            }
            return Ok(GenerationResult {
                message,
                truncated,
                salvaged,
                failed_chunks: 0,
                failed_files: Vec::new(),
                total_chunks: 1,
                intent_fallback_used: false,
                validation_warnings,
            });
        }

        let map_start = if debug_enabled {
            Some(Instant::now())
        } else {
            None
        };
        let (summaries, failed_chunks, failed_files) = self.map_phase(&chunks).await?;
        if let Some(start) = map_start {
            debug!("map phase completed in {:?}", start.elapsed());
        }

        let intent_start = if debug_enabled {
            Some(Instant::now())
        } else {
            None
        };
        let (themes, intent_fallback_used) = if summaries.len() <= 2 {
            self.detect_contradictions(&summaries);
            (self.fallback_themes_from_summaries(&summaries), true)
        } else {
            self.extract_intent(&summaries).await?
        };
        if let Some(start) = intent_start {
            debug!("intent phase completed in {:?}", start.elapsed());
        }

        let reduce_start = if debug_enabled {
            Some(Instant::now())
        } else {
            None
        };
        let (message, truncated, salvaged, validation_warnings) = self
            .reduce_phase(
                &themes,
                user_context,
                validation_mode,
                max_length,
                history_context.clone(),
            )
            .await?;
        if let Some(start) = reduce_start {
            debug!("reduce phase completed in {:?}", start.elapsed());
        }

        Ok(GenerationResult {
            message,
            truncated,
            salvaged,
            failed_chunks,
            failed_files,
            total_chunks,
            intent_fallback_used,
            validation_warnings,
        })
    }

    async fn direct_generation(
        &self,
        chunk: &DiffChunk,
        user_context: Option<&str>,
        validation_mode: ValidationMode,
        max_length: Option<usize>,
        history_context: Option<String>,
    ) -> Result<(CommitMessage, bool, bool, Vec<String>)> {
        let builder = PromptBuilder::new().with_diff(&chunk.content);

        let builder = if let Some(ctx) = user_context {
            builder.with_user_context(ctx)
        } else {
            builder
        };

        let mut prompt = builder.build_direct_prompt();
        if let Some(hist) = history_context {
            prompt.push_str("\n\nRecent commit history for style reference:\n");
            prompt.push_str(&hist);
        }

        let messages = vec![
            ChatMessage {
                role: Role::System,
                content: builder.build_system_prompt(),
            },
            ChatMessage {
                role: Role::User,
                content: prompt,
            },
        ];

        let response = generate_with_retry(self.provider.as_ref(), &messages, &self.retry_policy)
            .await
            .context("Direct generation failed")?;

        let cleaned = self.clean_response(&response);

        let validation_future = async {
            validate_commit_message(
                &cleaned,
                validation_mode,
                max_length,
                |msg, mode, max_len| self.try_extract_valid_commit(msg, mode, max_len),
            )
        };
        let debug_future = async {
            debug!("validation input length: {}", cleaned.len());
        };
        let (validation_result, _) = tokio::join!(validation_future, debug_future);
        validation_result
    }

    /// Map phase: Summarize each diff chunk independently (parallel).
    ///
    /// WHY buffer_unordered: Processes chunks as soon as capacity available, regardless
    /// of input order. Maximizes throughput—no waiting for slow chunks to complete.
    /// Alternative (ordered) would block on slowest chunk, serializing work.
    ///
    /// WHY systemic failure detection: Auth errors, invalid API keys affect ALL chunks.
    /// Failing fast on first systemic error prevents wasting API quota on doomed requests.
    /// Partial failures (malformed diff, rate limit) are isolated—allow pipeline to continue.
    ///
    /// WHY partial failure tolerance: With 100 chunks, 5% failure (5 chunks) is acceptable
    /// if we can generate meaningful message from 95 successes. Failing fast would waste
    /// completed work. Only abort if failure rate exceeds max_partial_failure_rate threshold.
    async fn map_phase(
        &self,
        chunks: &[DiffChunk],
    ) -> Result<(Vec<ChunkSummary>, usize, Vec<FilePath>)> {
        let map_concurrency = self.map_concurrency(chunks.len());
        let retry_policy = self.retry_policy.clone();
        let mut futures = stream::iter(chunks.iter().cloned().map(move |chunk| {
            let provider = Arc::clone(&self.provider);
            let limiter = self.limiter.clone();
            let content = chunk.content;
            let files = chunk.files;
            let retry_policy = retry_policy.clone();

            async move {
                let files_for_error = files.clone();
                let result = async {
                    let _permit = limiter.acquire().await;

                    let builder = PromptBuilder::new().with_diff(&content);
                    let messages = vec![
                        ChatMessage {
                            role: Role::System,
                            content: builder.build_system_prompt(),
                        },
                        ChatMessage {
                            role: Role::User,
                            content: builder.build_summary_prompt(),
                        },
                    ];

                    let summary = generate_with_retry(provider.as_ref(), &messages, &retry_policy)
                        .await
                        .map_err(MapError::Completion)?;

                    Ok::<ChunkSummary, MapError>(ChunkSummary {
                        summary: summary.trim().to_string(),
                        files,
                    })
                }
                .await;
                result.map_err(|e| (e, files_for_error))
            }
        }))
        .buffer_unordered(map_concurrency);

        let mut successes = Vec::with_capacity(chunks.len());
        let mut failed_count = 0usize;
        let mut failed_files: Vec<FilePath> = Vec::with_capacity(chunks.len());

        while let Some(result) = futures.next().await {
            match result {
                Ok(summary) => successes.push(summary),
                Err((e, files)) => {
                    if e.is_systemic() {
                        anyhow::bail!(
                            "Systemic provider failure detected - aborting pipeline: {}. \
                             Files affected: {}. This typically indicates authentication issues, \
                             rate limit exhaustion, or invalid API keys.",
                            e,
                            files
                                .iter()
                                .map(|path| path.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }

                    failed_count += 1;
                    failed_files.extend(files);
                }
            }
        }

        if successes.is_empty() {
            anyhow::bail!(
                "All {} chunks failed to process. Files affected: {}",
                chunks.len(),
                failed_files
                    .iter()
                    .map(|path| path.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        let total_chunks = successes.len() + failed_count;
        let failure_rate = failed_count as f64 / total_chunks as f64;
        let max_failure_rate = self.max_failure_rate();

        if failed_count > 1 && failure_rate > max_failure_rate {
            anyhow::bail!(
                "Partial failure rate too high: {}/{} chunks failed ({:.0}%). \
                 This exceeds the {:.0}% threshold for acceptable degradation. \
                 Files affected: {}",
                failed_count,
                total_chunks,
                failure_rate * 100.0,
                max_failure_rate * 100.0,
                failed_files
                    .iter()
                    .map(|path| path.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        if failed_count > 0 {
            warn!(
                "{}/{} chunks failed ({:.0}%). Generated message may not reflect all changes. Files with failed analysis: {}",
                failed_count,
                total_chunks,
                failure_rate * 100.0,
                failed_files
                    .iter()
                    .map(|path| path.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        Ok((successes, failed_count, failed_files))
    }

    async fn extract_intent(&self, summaries: &[ChunkSummary]) -> Result<(Vec<Theme>, bool)> {
        self.detect_contradictions(summaries);

        if summaries.len() > MAX_SUMMARIES_PER_INTENT_BATCH {
            return self.extract_intent_hierarchical(summaries).await;
        }

        let summary_strings = self.format_summaries_for_prompt(summaries);
        let builder = PromptBuilder::new().with_summaries(&summary_strings);

        let messages = vec![
            ChatMessage {
                role: Role::System,
                content: builder.build_system_prompt(),
            },
            ChatMessage {
                role: Role::User,
                content: builder.build_intent_prompt(),
            },
        ];

        let _permit = self.limiter.acquire().await;

        let response =
            generate_with_retry(self.provider.as_ref(), &messages, &RetryPolicy::default())
                .await
                .context("Intent extraction failed after retries")?;

        match self.parse_themes(&response) {
            Ok(themes) => Ok((themes, false)),
            Err(e) => {
                debug!(
                    "Failed to parse themes JSON: {}. Using fallback theme generation.",
                    e
                );
                debug!(
                    "LLM response excerpt: {}",
                    &response.chars().take(200).collect::<String>()
                );
                Ok((self.fallback_themes_from_summaries(summaries), true))
            }
        }
    }

    async fn extract_intent_hierarchical(
        &self,
        summaries: &[ChunkSummary],
    ) -> Result<(Vec<Theme>, bool)> {
        debug!(
            "Using hierarchical theme extraction for {} summaries",
            summaries.len()
        );

        let batches: Vec<Vec<ChunkSummary>> = summaries
            .chunks(MAX_SUMMARIES_PER_INTENT_BATCH)
            .map(|chunk| chunk.to_vec())
            .collect();

        let batch_count = batches.len();
        debug!("Grouped into {} batches", batch_count);

        let sub_themes_results = stream::iter(batches.into_iter().enumerate().map(
            |(idx, batch)| async move {
                match self.extract_sub_themes(&batch).await {
                    Ok(themes) => {
                        debug!("Batch {}: extracted {} sub-themes", idx, themes.len());
                        Ok(themes)
                    }
                    Err(e) => {
                        debug!("Batch {}: failed to extract sub-themes: {}", idx, e);
                        Ok(self.fallback_sub_themes_from_summaries(&batch))
                    }
                }
            },
        ))
        .buffer_unordered(self.concurrency_limit.min(batch_count).max(1))
        .collect::<Vec<Result<Vec<SubTheme>>>>()
        .await;

        let mut all_sub_themes: Vec<SubTheme> = Vec::new();
        let mut any_fallback = false;

        for result in sub_themes_results {
            match result {
                Ok(themes) => all_sub_themes.extend(themes),
                Err(_) => {
                    any_fallback = true;
                }
            }
        }

        if all_sub_themes.is_empty() {
            return Ok((self.fallback_themes_from_summaries(summaries), true));
        }

        let final_themes = self.aggregate_sub_themes(&all_sub_themes).await;

        match final_themes {
            Ok(themes) => Ok((themes, any_fallback)),
            Err(e) => {
                debug!("Theme aggregation failed: {}. Using fallback.", e);
                Ok((self.fallback_themes_from_summaries(summaries), true))
            }
        }
    }

    async fn extract_sub_themes(&self, batch: &[ChunkSummary]) -> Result<Vec<SubTheme>> {
        let summary_strings = self.format_summaries_for_prompt(batch);
        let builder = PromptBuilder::new().with_summaries(&summary_strings);

        let messages = vec![
            ChatMessage {
                role: Role::System,
                content: builder.build_system_prompt(),
            },
            ChatMessage {
                role: Role::User,
                content: builder.build_intent_prompt(),
            },
        ];

        let _permit = self.limiter.acquire().await;

        let response =
            generate_with_retry(self.provider.as_ref(), &messages, &RetryPolicy::default())
                .await
                .context("Sub-theme extraction failed")?;

        self.parse_sub_themes(&response)
    }

    async fn aggregate_sub_themes(&self, sub_themes: &[SubTheme]) -> Result<Vec<Theme>> {
        let mut scope_groups: std::collections::HashMap<String, Vec<SubTheme>> =
            std::collections::HashMap::new();

        for theme in sub_themes {
            scope_groups
                .entry(theme.scope.clone())
                .or_default()
                .push(theme.clone());
        }

        let mut merged_themes: Vec<Theme> = Vec::new();

        for (scope, themes) in scope_groups {
            let total_files: usize = themes.iter().map(|t| t.file_count).sum();

            let title = if themes.len() == 1 {
                themes[0].title.clone()
            } else {
                let mut title_counts: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();
                for theme in &themes {
                    *title_counts.entry(theme.title.clone()).or_default() += 1;
                }
                title_counts
                    .into_iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(title, _)| title)
                    .unwrap_or_else(|| "Code changes".to_string())
            };

            let description = themes
                .iter()
                .map(|t| t.description.as_str())
                .collect::<Vec<_>>()
                .join("; ");

            merged_themes.push(Theme::new(title, description, total_files, scope));
        }

        merged_themes.sort_by(|a, b| b.file_count.cmp(&a.file_count));
        merged_themes.truncate(3);

        debug!(
            "Aggregated {} sub-themes into {} final themes",
            sub_themes.len(),
            merged_themes.len()
        );

        Ok(merged_themes)
    }

    fn format_summaries_for_prompt(&self, summaries: &[ChunkSummary]) -> Vec<String> {
        summaries
            .iter()
            .map(|summary| {
                let paths = summary
                    .files
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "[{} files: {}] {}",
                    summary.files.len(),
                    paths,
                    summary.summary
                )
            })
            .collect()
    }

    fn parse_sub_themes(&self, response: &str) -> Result<Vec<SubTheme>> {
        let json_str = self.extract_json(response);
        let theme_response: ThemeResponse =
            serde_json::from_str(&json_str).context("Failed to parse sub-themes JSON")?;

        Ok(theme_response
            .themes
            .into_iter()
            .map(|t| SubTheme {
                title: t.title,
                description: t.description,
                file_count: t.file_count,
                scope: t.scope,
            })
            .collect())
    }

    fn fallback_sub_themes_from_summaries(&self, batch: &[ChunkSummary]) -> Vec<SubTheme> {
        let total_files: usize = batch.iter().map(|s| s.files.len()).sum();
        let combined_description = batch
            .iter()
            .map(|s| s.summary.as_str())
            .collect::<Vec<_>>()
            .join("; ");

        vec![SubTheme {
            title: "Code changes".to_string(),
            description: combined_description,
            file_count: total_files,
            scope: "chore".to_string(),
        }]
    }

    fn detect_contradictions(&self, summaries: &[ChunkSummary]) {
        const CONTRADICTORY_PAIRS: &[(&str, &str)] = &[
            ("add", "remove"),
            ("create", "delete"),
            ("implement", "remove"),
            ("introduce", "delete"),
            ("new", "delete"),
        ];

        let mut summary_text = Vec::with_capacity(summaries.len());
        for summary in summaries {
            summary_text.push(summary.summary.to_lowercase());
        }

        for (action, counteraction) in CONTRADICTORY_PAIRS {
            let has_action = summary_text.iter().any(|s| s.contains(action));
            let has_counteraction = summary_text.iter().any(|s| s.contains(counteraction));

            if has_action && has_counteraction {
                warn!(
                    "Potential contradiction detected - summaries contain both '{}' and '{}' operations",
                    action, counteraction
                );
                return;
            }
        }
    }

    fn fallback_themes_from_summaries(&self, summaries: &[ChunkSummary]) -> Vec<Theme> {
        let total_files: usize = summaries.iter().map(|s| s.files.len()).sum();
        let combined_description = summaries
            .iter()
            .map(|s| s.summary.as_str())
            .collect::<Vec<_>>()
            .join("; ");

        vec![Theme::new(
            "Code changes".to_string(),
            combined_description,
            total_files,
            "chore".to_string(),
        )]
    }

    fn parse_themes(&self, response: &str) -> Result<Vec<Theme>> {
        let json_str = self.extract_json(response);
        let theme_response: ThemeResponse =
            serde_json::from_str(&json_str).context("Failed to parse themes JSON")?;

        Ok(theme_response
            .themes
            .into_iter()
            .map(|t| Theme::new(t.title, t.description, t.file_count, t.scope))
            .collect())
    }

    fn extract_json(&self, response: &str) -> String {
        if let Some(content) = Self::extract_from_markdown(response) {
            return content;
        }

        if let Some(json) = Self::extract_json_simplified(response) {
            return json;
        }

        if let Some(json) = Self::extract_balanced_json(response) {
            return json;
        }

        response.to_string()
    }

    fn extract_json_simplified(response: &str) -> Option<String> {
        let start = response.find('{')?;
        let end = response.rfind('}')?;

        if start >= end {
            return None;
        }

        let candidate = &response[start..=end];
        serde_json::from_str::<serde_json::Value>(candidate).ok()?;

        Some(candidate.to_string())
    }

    fn extract_from_markdown(response: &str) -> Option<String> {
        if let Some(start) = response.find("```json") {
            let after_marker = &response[start + 7..];
            if let Some(end) = after_marker.find("```") {
                return Some(after_marker[..end].trim().to_string());
            }
        }

        if let Some(start) = response.find("```") {
            let after_marker = &response[start + 3..];
            if let Some(end) = after_marker.find("```") {
                let content = after_marker[..end].trim();
                if content.starts_with('{') {
                    return Some(content.to_string());
                }
            }
        }

        None
    }

    fn extract_balanced_json(response: &str) -> Option<String> {
        let bytes = response.as_bytes();
        let mut start: Option<usize> = None;
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, &byte) in bytes.iter().enumerate() {
            if escape_next {
                escape_next = false;
                continue;
            }

            if byte == b'\\' && in_string {
                escape_next = true;
                continue;
            }

            if byte == b'"' {
                in_string = !in_string;
                continue;
            }

            if in_string {
                continue;
            }

            match byte {
                b'{' => {
                    if depth == 0 {
                        start = Some(i);
                    }
                    depth += 1;
                }
                b'}' => {
                    depth -= 1;
                    if depth == 0
                        && let Some(s) = start
                    {
                        return Some(response[s..=i].to_string());
                    }
                }
                _ => {}
            }
        }

        None
    }

    async fn reduce_phase(
        &self,
        themes: &[Theme],
        user_context: Option<&str>,
        validation_mode: ValidationMode,
        max_length: Option<usize>,
        history_context: Option<String>,
    ) -> Result<(CommitMessage, bool, bool, Vec<String>)> {
        let builder = PromptBuilder::new().with_themes(themes);

        let builder = if let Some(ctx) = user_context {
            builder.with_user_context(ctx)
        } else {
            builder
        };

        let mut prompt = builder.build_synthesis_prompt();
        if let Some(hist) = history_context {
            prompt.push_str("\n\nRecent commit history for style reference:\n");
            prompt.push_str(&hist);
        }

        let messages = vec![
            ChatMessage {
                role: Role::System,
                content: builder.build_system_prompt(),
            },
            ChatMessage {
                role: Role::User,
                content: prompt,
            },
        ];

        let response = generate_with_retry(self.provider.as_ref(), &messages, &self.retry_policy)
            .await
            .context("Reduce phase synthesis failed")?;

        let cleaned = self.clean_response(&response);

        let validation_future = async {
            validate_commit_message(
                &cleaned,
                validation_mode,
                max_length,
                |msg, mode, max_len| self.try_extract_valid_commit(msg, mode, max_len),
            )
        };
        let debug_future = async {
            debug!("validation input length: {}", cleaned.len());
        };
        let (validation_result, _) = tokio::join!(validation_future, debug_future);
        validation_result
    }

    fn clean_response(&self, response: &str) -> String {
        let mut message = response.trim().to_string();

        if message.starts_with("```")
            && let Some(end) = message[3..].find("```")
        {
            message = message[3..3 + end].trim().to_string();
        }

        let preambles = [
            "Here is the commit message:",
            "Here's the commit message:",
            "Commit message:",
            "The commit message is:",
        ];

        for preamble in &preambles {
            if let Some(pos) = message.to_lowercase().find(&preamble.to_lowercase()) {
                message = message[pos + preamble.len()..].trim().to_string();
            }
        }

        if let Some(newline) = message.find('\n') {
            message = message[..newline].trim().to_string();
        }

        message
    }

    fn map_concurrency(&self, chunk_count: usize) -> usize {
        let base = if chunk_count <= 3 {
            chunk_count.min(3)
        } else {
            MAX_CONCURRENT_REQUESTS
        };
        base.min(self.concurrency_limit).max(1)
    }

    fn try_extract_valid_commit(
        &self,
        message: &str,
        mode: ValidationMode,
        max_length: Option<usize>,
    ) -> Option<String> {
        try_extract_valid_commit(message, mode, max_length)
    }

    fn max_failure_rate(&self) -> f64 {
        self.max_partial_failure_rate
    }
}

fn debug_enabled() -> bool {
    std::env::var_os("CHRISTINA_DEBUG").is_some()
}

fn timeout_for_attempt(attempt: u32) -> Duration {
    match attempt {
        0 => Duration::from_secs(LLM_INITIAL_TIMEOUT_SECONDS),
        1 => Duration::from_secs(LLM_RETRY_TIMEOUT_SECONDS),
        _ => Duration::from_secs(LLM_TIMEOUT_SECONDS),
    }
}

async fn generate_with_retry(
    provider: &Provider,
    messages: &[ChatMessage],
    policy: &RetryPolicy,
) -> Result<String, CompletionError> {
    let mut last_error = None;

    for attempt in 0..=policy.max_retries {
        let timeout = timeout_for_attempt(attempt as u32);
        match tokio::time::timeout(timeout, provider.generate(messages)).await {
            Ok(Ok(result)) => return Ok(result),
            Ok(Err(err)) => {
                if !err.is_transient() {
                    return Err(err);
                }
                if attempt >= policy.max_retries {
                    return Err(err);
                }
                last_error = Some(err);
            }
            Err(_) => {
                let err = CompletionError::Timeout;
                if attempt >= policy.max_retries {
                    return Err(err);
                }
                last_error = Some(err);
            }
        }

        let delay = policy.calculate_delay(attempt as u32);
        tokio::time::sleep(delay).await;
    }

    // Fallback if all retries exhausted without recording an error (logic bug guard)
    Err(last_error.unwrap_or_else(|| {
        CompletionError::UnknownError(
            "All retry attempts exhausted without error details".to_string(),
        )
    }))
}

fn validate_commit_message(
    message: &str,
    mode: ValidationMode,
    max_length: Option<usize>,
    extract_valid: impl FnOnce(&str, ValidationMode, Option<usize>) -> Option<String>,
) -> Result<(CommitMessage, bool, bool, Vec<String>)> {
    let trimmed = message.trim();

    if let Ok((msg, warnings)) = CommitMessage::validate(trimmed.to_string(), mode, max_length) {
        return Ok((msg, false, false, warnings));
    }

    if let Some(valid_msg) = extract_valid(trimmed, mode, max_length) {
        let (message, warnings) = CommitMessage::validate(valid_msg, mode, max_length)
            .map_err(|e| anyhow::anyhow!("Invalid commit message: {}", e))?;
        return Ok((message, false, true, warnings));
    }

    anyhow::bail!(
        "Generated message does not follow Conventional Commits format: {}",
        trimmed
    );
}

fn try_extract_valid_commit(
    message: &str,
    mode: ValidationMode,
    max_length: Option<usize>,
) -> Option<String> {
    let mut earliest_match: Option<(usize, String)> = None;

    for (pos, _) in message.match_indices(':') {
        let start = pos.saturating_sub(50);
        let candidate = &message[start..];
        let candidate = candidate.trim_start();
        let end = candidate.find('\n').unwrap_or(candidate.len());
        let candidate = candidate[..end].trim();

        if CommitMessage::validate(candidate.to_string(), mode, max_length).is_ok() {
            let should_update = match &earliest_match {
                None => true,
                Some((earliest_pos, _)) => pos < *earliest_pos,
            };
            if should_update {
                earliest_match = Some((pos, candidate.to_string()));
            }
        }
    }

    earliest_match.map(|(_, msg)| msg)
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use christina_core::git::DiffChunk;
    use christina_core::types::TokenCount;

    fn sample_chunk() -> DiffChunk {
        DiffChunk::new(
            Arc::from("diff --git a/file.txt b/file.txt\n+new line\n"),
            vec![FilePath::from("file.txt")],
            TokenCount::new_saturating(10),
        )
    }

    #[test]
    fn extract_json_from_markdown() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"
```json
{"themes": [{"title": "Test", "description": "Test desc", "fileCount": 1, "scope": "feature"}]}
```
"#;

        let json = orchestrator.extract_json(response);
        assert!(json.starts_with('{'));
        assert!(json.contains("\"themes\""));
    }

    #[test]
    fn extract_json_raw() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"themes": [{"title": "Test", "description": "Test desc", "fileCount": 1, "scope": "feature"}]}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, response);
    }

    #[test]
    fn clean_response() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = "Here is the commit message:\nfeat(auth): add login flow\n\nSome extra text";
        let cleaned = orchestrator.clean_response(response);
        assert_eq!(cleaned, "feat(auth): add login flow");
    }

    #[test]
    fn clean_response_with_code_block() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = "```\nfeat(auth): add login flow\n```";
        let cleaned = orchestrator.clean_response(response);
        assert_eq!(cleaned, "feat(auth): add login flow");
    }

    #[tokio::test]
    async fn orchestrate_empty_chunks() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let result = orchestrator
            .generate_commit_message(Vec::new(), None, ValidationMode::default(), None, None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn orchestrate_single_chunk() {
        let provider = Arc::new(Provider::mock("feat(core): add pipeline"));
        let orchestrator = AIOrchestrator::new(provider);

        let result = orchestrator
            .generate_commit_message(
                vec![sample_chunk()],
                None,
                ValidationMode::default(),
                None,
                None,
            )
            .await
            .unwrap_or_else(|e| panic!("generation should succeed: {}", e));
        assert_eq!(result.message.as_ref(), "feat(core): add pipeline");
        assert_eq!(result.total_chunks, 1);
    }

    #[tokio::test]
    async fn orchestrate_many_chunks_batching() {
        let responses = vec![
            Ok("summary 1".to_string()),
            Ok("summary 2".to_string()),
            Ok("feat(core): add batching".to_string()),
        ];
        let provider = Arc::new(Provider::mock_sequence_with_delay(responses, 200));
        let orchestrator = AIOrchestrator::new(provider);

        let chunk = sample_chunk();
        let result = orchestrator
            .generate_commit_message(
                vec![chunk.clone(), chunk],
                None,
                ValidationMode::default(),
                None,
                None,
            )
            .await
            .unwrap_or_else(|e| panic!("generation should succeed: {}", e));
        assert_eq!(result.total_chunks, 2);
        assert_eq!(result.message.as_ref(), "feat(core): add batching");
    }

    #[tokio::test]
    async fn orchestrate_rate_limit_respected() {
        let responses = vec![
            Ok("summary 1".to_string()),
            Ok("summary 2".to_string()),
            Ok("feat(core): rate limit".to_string()),
        ];
        let provider = Arc::new(Provider::mock_sequence_with_delay(responses, 200));
        let orchestrator = AIOrchestrator::new(provider);

        let chunk = sample_chunk();
        let start = std::time::Instant::now();
        let result = orchestrator
            .generate_commit_message(
                vec![chunk.clone(), chunk],
                None,
                ValidationMode::default(),
                None,
                None,
            )
            .await
            .unwrap_or_else(|e| panic!("generation should succeed: {}", e));

        assert_eq!(result.total_chunks, 2);
        assert_eq!(result.message.as_ref(), "feat(core): rate limit");
        assert!(start.elapsed() >= std::time::Duration::from_millis(400));
    }

    #[tokio::test]
    async fn orchestrate_retry_on_failure() {
        tokio::time::pause();

        let responses = vec![
            Err(CompletionError::Timeout),
            Ok("feat(core): retry success".to_string()),
        ];
        let provider = Arc::new(Provider::mock_sequence(responses));
        let orchestrator = AIOrchestrator::new(provider);

        let result = orchestrator
            .generate_commit_message(
                vec![sample_chunk()],
                None,
                ValidationMode::default(),
                None,
                None,
            )
            .await
            .unwrap_or_else(|e| panic!("generation should succeed after retry: {}", e));
        assert_eq!(result.message.as_ref(), "feat(core): retry success");
    }

    #[tokio::test]
    async fn map_phase_systemic_failure_aborts_immediately() {
        let provider = Arc::new(Provider::mock_sequence(vec![Err(
            CompletionError::Unauthorized("Invalid API key".to_string()),
        )]));
        let orchestrator = AIOrchestrator::new(provider);

        let chunk = sample_chunk();
        let result = orchestrator
            .generate_commit_message(
                vec![chunk.clone(), chunk],
                None,
                ValidationMode::default(),
                None,
                None,
            )
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Systemic provider failure"));
        assert!(err_msg.contains("authentication issues"));
    }

    #[tokio::test(start_paused = true)]
    async fn map_phase_partial_failure_within_threshold() {
        let mut responses = Vec::new();

        responses.push(Err(CompletionError::Timeout));
        responses.push(Ok("summary 1".to_string()));

        for i in 2..=10 {
            responses.push(Ok(format!("summary {}", i)));
        }

        responses.push(Ok(
            r#"{"themes": [{"title": "Test", "description": "Test theme", "fileCount": 10, "scope": "feat"}]}"#
                .to_string(),
        ));
        responses.push(Ok("feat(core): partial success".to_string()));

        let provider = Arc::new(Provider::mock_sequence(responses));
        let orchestrator = AIOrchestrator::new(provider);

        let chunks: Vec<_> = (0..10).map(|_| sample_chunk()).collect();
        let result = orchestrator
            .generate_commit_message(chunks, None, ValidationMode::default(), None, None)
            .await
            .unwrap_or_else(|e| panic!("should succeed with transient failures: {}", e));

        assert_eq!(result.failed_chunks, 0);
        assert_eq!(result.total_chunks, 10);
        assert_eq!(result.message.as_ref(), "feat(core): partial success");
    }

    #[tokio::test(start_paused = true)]
    async fn map_phase_partial_failure_exceeds_threshold() {
        let mut responses = Vec::new();
        responses.push(Err(CompletionError::Timeout));
        responses.push(Err(CompletionError::NetworkError(
            "connection reset".to_string(),
        )));
        for i in 1..=8 {
            responses.push(Ok(format!("summary {}", i)));
        }

        let provider = Arc::new(Provider::mock_sequence(responses));
        let orchestrator = AIOrchestrator::new(provider);

        let chunks: Vec<_> = (0..10).map(|_| sample_chunk()).collect();
        let result = orchestrator
            .generate_commit_message(chunks, None, ValidationMode::default(), None, None)
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Partial failure rate too high"));
        assert!(err_msg.contains("20%"));
        assert!(err_msg.contains("10%"));
    }

    #[tokio::test(start_paused = true)]
    async fn map_phase_all_chunks_fail() {
        let responses = vec![Err(CompletionError::Timeout), Err(CompletionError::Timeout)];
        let provider = Arc::new(Provider::mock_sequence(responses));
        let orchestrator = AIOrchestrator::new(provider);

        let chunk = sample_chunk();
        let result = orchestrator
            .generate_commit_message(
                vec![chunk.clone(), chunk],
                None,
                ValidationMode::default(),
                None,
                None,
            )
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("All") && err_msg.contains("chunks failed"));
    }

    #[test]
    fn extract_json_malformed_json_inside_markers() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"
```json
{"themes": [{"title": "Test", "description": "Missing closing brace"
```
"#;

        let json = orchestrator.extract_json(response);
        assert!(json.contains("\"themes\""));
        assert!(json.contains("Missing closing brace"));
    }

    #[test]
    fn extract_json_missing_closing_marker() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"
```json
{"themes": [{"title": "Test"}]}
"#;

        let json = orchestrator.extract_json(response);
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
        assert!(json.contains("\"themes\""));
    }

    #[test]
    fn extract_json_multiple_blocks_first_used() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"
```json
{"themes": [{"title": "First", "description": "First block", "fileCount": 1, "scope": "feat"}]}
```

Some text in between

```json
{"themes": [{"title": "Second", "description": "Second block", "fileCount": 2, "scope": "fix"}]}
```
"#;

        let json = orchestrator.extract_json(response);
        assert!(json.contains("First"));
        assert!(json.contains("First block"));
        assert!(!json.contains("Second"));
    }

    #[test]
    fn extract_json_no_markers_raw_json() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"themes": [{"title": "Raw", "description": "No markers", "fileCount": 1, "scope": "test"}]}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, response);
    }

    #[test]
    fn extract_json_empty_content() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = "";

        let json = orchestrator.extract_json(response);
        assert_eq!(json, "");
    }

    #[test]
    fn extract_json_non_json_content_inside_markers() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"
```json
This is not JSON at all, just plain text
```
"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json.trim(), "This is not JSON at all, just plain text");
    }

    #[test]
    fn extract_json_generic_code_block_with_json() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"
```
{"themes": [{"title": "Generic", "description": "Generic block", "fileCount": 1, "scope": "feat"}]}
```
"#;

        let json = orchestrator.extract_json(response);
        assert!(json.starts_with('{'));
        assert!(json.contains("\"themes\""));
        assert!(json.contains("Generic"));
    }

    #[test]
    fn extract_json_json_in_text_no_braces() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = "Here is some text without any JSON content at all";

        let json = orchestrator.extract_json(response);
        assert_eq!(json, response);
    }

    #[test]
    fn extract_json_nested_braces() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"Some preamble {"outer": {"inner": {"deep": "value"}}} some suffix"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"outer": {"inner": {"deep": "value"}}}"#);
    }

    #[test]
    fn extract_json_multiple_objects_balanced() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"first": 1} {"second": 2}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"first": 1}"#);
    }

    #[test]
    fn extract_json_with_escaped_quotes() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"message": "He said \"hello\"", "count": 1}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"message": "He said \"hello\"", "count": 1}"#);
    }

    #[test]
    fn extract_json_with_escaped_backslash() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"path": "C:\\Users\\test", "valid": true}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"path": "C:\\Users\\test", "valid": true}"#);
    }

    #[test]
    fn extract_json_unbalanced_braces() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"unclosed": "brace" "#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, response);
    }

    #[test]
    fn extract_json_braces_in_string() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"code": "if (x) { return y; }", "lang": "js"}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"code": "if (x) { return y; }", "lang": "js"}"#);
    }

    #[test]
    fn extract_json_deeply_nested() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"a": {"b": {"c": {"d": {"e": "deep"}}}}}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"a": {"b": {"c": {"d": {"e": "deep"}}}}}"#);
    }

    #[test]
    fn extract_json_with_arrays() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"items": [{"id": 1}, {"id": 2}], "count": 2}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"items": [{"id": 1}, {"id": 2}], "count": 2}"#);
    }

    #[test]
    fn extract_json_no_json_content() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = "Just plain text without any braces";

        let json = orchestrator.extract_json(response);
        assert_eq!(json, response);
    }

    #[test]
    fn extract_json_partial_object_in_text() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let response = "Error: {code: 404} (not valid JSON)";

        let json = orchestrator.extract_json(response);
        assert_eq!(json, "{code: 404}");
    }

    #[test]
    fn calculate_history_budget() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let budget_4k = orchestrator.calculate_history_budget(4096);
        assert_eq!(budget_4k, 4);

        let budget_16k = orchestrator.calculate_history_budget(16384);
        assert_eq!(budget_16k, 16);

        let budget_1k = orchestrator.calculate_history_budget(1000);
        assert_eq!(budget_1k, 3);

        let budget_very_low = orchestrator.calculate_history_budget(100);
        assert_eq!(budget_very_low, 3);
    }

    #[test]
    fn format_summaries_for_prompt() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let summaries = vec![
            ChunkSummary {
                summary: "Added authentication".to_string(),
                files: vec![FilePath::from("src/auth.rs")],
            },
            ChunkSummary {
                summary: "Fixed login bug".to_string(),
                files: vec![
                    FilePath::from("src/login.rs"),
                    FilePath::from("src/user.rs"),
                ],
            },
        ];

        let formatted = orchestrator.format_summaries_for_prompt(&summaries);

        assert_eq!(formatted.len(), 2);
        assert!(formatted[0].contains("[1 files:"));
        assert!(formatted[0].contains("src/auth.rs"));
        assert!(formatted[0].contains("Added authentication"));
        assert!(formatted[1].contains("[2 files:"));
        assert!(formatted[1].contains("src/login.rs"));
        assert!(formatted[1].contains("Fixed login bug"));
    }

    #[test]
    fn fallback_sub_themes_from_summaries() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let summaries = vec![
            ChunkSummary {
                summary: "Added feature A".to_string(),
                files: vec![FilePath::from("src/a.rs")],
            },
            ChunkSummary {
                summary: "Added feature B".to_string(),
                files: vec![FilePath::from("src/b.rs"), FilePath::from("src/c.rs")],
            },
        ];

        let sub_themes = orchestrator.fallback_sub_themes_from_summaries(&summaries);

        assert_eq!(sub_themes.len(), 1);
        assert_eq!(sub_themes[0].title, "Code changes");
        assert_eq!(sub_themes[0].file_count, 3);
        assert_eq!(sub_themes[0].scope, "chore");
        assert!(sub_themes[0].description.contains("Added feature A"));
        assert!(sub_themes[0].description.contains("Added feature B"));
    }

    #[tokio::test]
    async fn aggregate_sub_themes_merges_by_scope() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let sub_themes = vec![
            SubTheme {
                title: "Auth feature 1".to_string(),
                description: "Added login".to_string(),
                file_count: 2,
                scope: "auth".to_string(),
            },
            SubTheme {
                title: "Auth feature 2".to_string(),
                description: "Added logout".to_string(),
                file_count: 3,
                scope: "auth".to_string(),
            },
            SubTheme {
                title: "API feature".to_string(),
                description: "Added endpoints".to_string(),
                file_count: 1,
                scope: "api".to_string(),
            },
        ];

        let themes = orchestrator
            .aggregate_sub_themes(&sub_themes)
            .await
            .expect("aggregation should succeed");

        assert_eq!(themes.len(), 2);

        let auth_theme = themes
            .iter()
            .find(|t| t.scope == "auth")
            .expect("auth theme present");
        assert_eq!(auth_theme.file_count, 5);

        let api_theme = themes
            .iter()
            .find(|t| t.scope == "api")
            .expect("api theme present");
        assert_eq!(api_theme.file_count, 1);
    }

    #[tokio::test]
    async fn aggregate_sub_themes_limits_to_top_three() {
        let provider = Arc::new(Provider::mock("unused"));
        let orchestrator = AIOrchestrator::new(provider);

        let sub_themes = vec![
            SubTheme {
                title: "Feature 1".to_string(),
                description: "Desc 1".to_string(),
                file_count: 10,
                scope: "scope1".to_string(),
            },
            SubTheme {
                title: "Feature 2".to_string(),
                description: "Desc 2".to_string(),
                file_count: 8,
                scope: "scope2".to_string(),
            },
            SubTheme {
                title: "Feature 3".to_string(),
                description: "Desc 3".to_string(),
                file_count: 6,
                scope: "scope3".to_string(),
            },
            SubTheme {
                title: "Feature 4".to_string(),
                description: "Desc 4".to_string(),
                file_count: 4,
                scope: "scope4".to_string(),
            },
            SubTheme {
                title: "Feature 5".to_string(),
                description: "Desc 5".to_string(),
                file_count: 2,
                scope: "scope5".to_string(),
            },
        ];

        let themes = orchestrator
            .aggregate_sub_themes(&sub_themes)
            .await
            .expect("aggregation should succeed");

        assert_eq!(themes.len(), 3);
        assert_eq!(themes[0].file_count, 10);
        assert_eq!(themes[1].file_count, 8);
        assert_eq!(themes[2].file_count, 6);
    }
}
