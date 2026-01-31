use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use serde::Deserialize;

use crate::concurrency::RequestLimiter;
use crate::provider::{ChatMessage, Provider};
use crate::retry::{IsTransient, RetryPolicy};
use christina_core::error::CompletionError;
use christina_core::prompt::{PromptBuilder, Theme};

use christina_core::git::DiffChunk;
use christina_core::types::{CommitMessage, FilePath, commit_message::ValidationMode};

const MAX_CONCURRENT_REQUESTS: usize = 5;
const LLM_INITIAL_TIMEOUT_SECONDS: u64 = 30;
const LLM_RETRY_TIMEOUT_SECONDS: u64 = 60;
const LLM_TIMEOUT_SECONDS: u64 = 120;

/// Maximum number of summaries to process in a single intent extraction batch.
/// When summaries exceed this threshold, hierarchical extraction is used:
/// 1. Summaries are grouped into batches of this size
/// 2. Sub-themes are extracted from each batch in parallel
/// 3. Sub-themes are aggregated into final themes
const MAX_SUMMARIES_PER_INTENT_BATCH: usize = 20;

impl IsTransient for CompletionError {
    fn is_transient(&self) -> bool {
        CompletionError::is_transient(self)
    }
}

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
    /// Check if this error is a systemic provider error that should abort the entire pipeline.
    /// Systemic errors: authentication failures, rate limits, invalid API keys
    /// These indicate the provider is unavailable or misconfigured.
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

/// A summary of a single diff chunk from the Map phase.
#[derive(Debug, Clone)]
pub struct ChunkSummary {
    /// The summary text describing the chunk's changes
    pub summary: String,
    /// Files affected by this chunk
    pub files: Vec<FilePath>,
}

/// Result of a generation attempt, including metadata.
#[derive(Debug, Clone)]
pub struct GenerationResult {
    /// The generated commit message
    pub message: CommitMessage,
    /// Whether the message was truncated
    pub truncated: bool,
    /// Whether the message was salvaged from invalid LLM output
    pub salvaged: bool,
    /// Number of chunks that failed during map phase (partial failure)
    pub failed_chunks: usize, // OK: summary count
    /// Files that failed during map phase (for user visibility)
    pub failed_files: Vec<FilePath>,
    /// Total chunks that were processed
    pub total_chunks: usize,
    /// Whether intent extraction fell back to generic themes
    pub intent_fallback_used: bool,
    /// Validation warnings (e.g., message exceeds recommended length in soft mode)
    pub validation_warnings: Vec<String>,
}

impl GenerationResult {
    /// Check if there were any partial failures or fallbacks during generation.
    pub fn has_warnings(&self) -> bool {
        self.truncated
            || self.salvaged
            || self.failed_chunks > 0
            || self.intent_fallback_used
            || !self.validation_warnings.is_empty()
    }

    /// Get a human-readable summary of any warnings.
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

/// Intermediate theme representation for hierarchical extraction.
/// Used when aggregating sub-themes from multiple batches.
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
}

impl AIOrchestrator {
    pub fn new(provider: Arc<Provider>) -> Self {
        // Clamp concurrency to sane range [1, 20] to prevent:
        //
        // - Deadlock if set to 0 (blocks forever)
        // - Resource exhaustion if set too high
        let concurrency_limit = std::env::var("CHRISTINA_CONCURRENCY_LIMIT")
            .ok() // missing env means default
            .and_then(|s| {
                let parsed: usize = s.parse().ok()?; // invalid value -> default
                Some(parsed.clamp(1, 20))
            })
            .unwrap_or(MAX_CONCURRENT_REQUESTS);

        // Rate limit: 5 requests per second to stay well under typical API limits
        // This prevents thundering herd by spacing out requests proactively
        let requests_per_second = 5.0;

        Self {
            provider,
            limiter: RequestLimiter::new(concurrency_limit, requests_per_second),
            retry_policy: RetryPolicy::default(),
            concurrency_limit,
        }
    }

    /// Calculate the maximum number of commits that fit within the history budget.
    ///
    /// Allocates 15% of max_input_tokens for history context, estimating 150 tokens
    /// per commit subject line. Ensures a minimum of 3 commits for useful context.
    pub fn calculate_history_budget(&self, max_input_tokens: u32) -> usize {
        let budget_tokens = max_input_tokens as f64 * 0.15;
        let commits_available = (budget_tokens / 150.0).floor() as usize;
        // Ensure minimum of 3 commits for useful context
        commits_available.max(3)
    }

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

        // Single chunk that fits in context
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
                eprintln!("direct generation completed in {:?}", start.elapsed());
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

        // Map Phase: Generate summaries for each chunk (with partial failure handling)
        let map_start = if debug_enabled {
            Some(Instant::now())
        } else {
            None
        };
        let (summaries, failed_chunks, failed_files) = self.map_phase(&chunks).await?;
        if let Some(start) = map_start {
            eprintln!("map phase completed in {:?}", start.elapsed());
        }

        // Intent Extraction: Get high-level themes
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
            eprintln!("intent phase completed in {:?}", start.elapsed());
        }

        // Reduce Phase: Synthesize final commit message
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
            eprintln!("reduce phase completed in {:?}", start.elapsed());
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

    /// Direct generation for single-chunk diffs (bypasses Map-Reduce).
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
            ChatMessage::system(builder.build_system_prompt()),
            ChatMessage::user(prompt),
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
            if debug_enabled() {
                eprintln!("validation input length: {}", cleaned.len());
            }
        };
        let (validation_result, _) = tokio::join!(validation_future, debug_future);
        validation_result
    }

    /// Map Phase: Generate summaries for each chunk concurrently.
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
                        ChatMessage::system(builder.build_system_prompt()),
                        ChatMessage::user(builder.build_summary_prompt()),
                    ];

                    // Use retry with backoff for transient errors in map phase
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

        // Collect results with failure classification
        let mut successes = Vec::with_capacity(chunks.len());
        let mut failed_count = 0usize;
        let mut failed_files: Vec<FilePath> = Vec::with_capacity(chunks.len());

        while let Some(result) = futures.next().await {
            match result {
                Ok(summary) => successes.push(summary),
                Err((e, files)) => {
                    // SYSTEMIC FAILURES: Abort immediately
                    // These indicate provider misconfiguration or unavailability
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

                    // PARTIAL FAILURES: Track for threshold check
                    // These are transient errors that may resolve or affect only specific chunks
                    failed_count += 1;
                    failed_files.extend(files);
                }
            }
        }

        // Check if we have at least one successful chunk
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

        // Higher failure rates risk generating misleading messages that omit significant changes.
        let total_chunks = successes.len() + failed_count;
        let failure_rate = failed_count as f64 / total_chunks as f64;
        const MAX_PARTIAL_FAILURE_RATE: f64 = 0.10; // 10%

        if failure_rate > MAX_PARTIAL_FAILURE_RATE {
            anyhow::bail!(
                "Partial failure rate too high: {}/{} chunks failed ({:.0}%). \
                 This exceeds the {:.0}% threshold for acceptable degradation. \
                 Files affected: {}",
                failed_count,
                total_chunks,
                failure_rate * 100.0,
                MAX_PARTIAL_FAILURE_RATE * 100.0,
                failed_files
                    .iter()
                    .map(|path| path.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        // Log warning if there were any partial failures (even below threshold)
        if failed_count > 0 && debug_enabled() {
            eprintln!(
                "Warning: {}/{} chunks failed ({:.0}%). Generated message may not reflect all changes. \
                 Files with failed analysis: {}",
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

    /// Intent Extraction: Aggregate summaries and extract themes.
    /// Falls back to simple theme creation from summaries if JSON parsing fails.
    ///
    /// Uses hierarchical extraction when summaries exceed MAX_SUMMARIES_PER_INTENT_BATCH:
    /// 1. Groups summaries into batches
    /// 2. Extracts sub-themes from each batch in parallel
    /// 3. Aggregates sub-themes into final themes
    async fn extract_intent(&self, summaries: &[ChunkSummary]) -> Result<(Vec<Theme>, bool)> {
        // Check for potential contradictions in summaries
        self.detect_contradictions(summaries);

        // Use hierarchical extraction for large summary sets
        if summaries.len() > MAX_SUMMARIES_PER_INTENT_BATCH {
            return self.extract_intent_hierarchical(summaries).await;
        }

        // Format summaries with file paths for the prompt
        let summary_strings = self.format_summaries_for_prompt(summaries);
        let builder = PromptBuilder::new().with_summaries(&summary_strings);

        let messages = vec![
            ChatMessage::system(builder.build_system_prompt()),
            ChatMessage::user(builder.build_intent_prompt()),
        ];

        // Acquire permit for rate limiting
        let _permit = self.limiter.acquire().await;

        // Use retry with backoff for transient errors
        let response =
            generate_with_retry(self.provider.as_ref(), &messages, &RetryPolicy::default())
                .await
                .context("Intent extraction failed after retries")?;

        // Try to parse JSON response, fall back to simple themes if parsing fails
        match self.parse_themes(&response) {
            Ok(themes) => Ok((themes, false)),
            Err(e) => {
                if debug_enabled() {
                    eprintln!(
                        "Warning: Failed to parse themes JSON: {}. Using fallback theme generation.",
                        e
                    );
                    eprintln!(
                        "LLM response excerpt: {}",
                        &response.chars().take(200).collect::<String>()
                    );
                }
                Ok((self.fallback_themes_from_summaries(summaries), true))
            }
        }
    }

    /// Hierarchical theme extraction for large summary sets.
    ///
    /// Process:
    /// 1. Group summaries into batches of MAX_SUMMARIES_PER_INTENT_BATCH
    /// 2. Extract sub-themes from each batch in parallel
    /// 3. Aggregate sub-themes into final themes
    async fn extract_intent_hierarchical(
        &self,
        summaries: &[ChunkSummary],
    ) -> Result<(Vec<Theme>, bool)> {
        if debug_enabled() {
            eprintln!(
                "Using hierarchical theme extraction for {} summaries",
                summaries.len()
            );
        }

        // Step 1: Group summaries into batches
        let batches: Vec<Vec<ChunkSummary>> = summaries
            .chunks(MAX_SUMMARIES_PER_INTENT_BATCH)
            .map(|chunk| chunk.to_vec())
            .collect();

        let batch_count = batches.len();

        if debug_enabled() {
            eprintln!("Grouped into {} batches", batch_count);
        }

        // Step 2: Extract sub-themes from each batch in parallel
        let sub_themes_results = stream::iter(batches.into_iter().enumerate().map(
            |(idx, batch)| async move {
                match self.extract_sub_themes(&batch).await {
                    Ok(themes) => {
                        if debug_enabled() {
                            eprintln!("Batch {}: extracted {} sub-themes", idx, themes.len());
                        }
                        Ok(themes)
                    }
                    Err(e) => {
                        if debug_enabled() {
                            eprintln!("Batch {}: failed to extract sub-themes: {}", idx, e);
                        }
                        // Fall back to creating sub-themes from batch summaries
                        Ok(self.fallback_sub_themes_from_summaries(&batch))
                    }
                }
            },
        ))
        .buffer_unordered(self.concurrency_limit.min(batch_count).max(1))
        .collect::<Vec<Result<Vec<SubTheme>>>>()
        .await;

        // Collect all sub-themes, filtering out errors
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
            // All batches failed, use fallback
            return Ok((self.fallback_themes_from_summaries(summaries), true));
        }

        // Step 3: Aggregate sub-themes into final themes
        let final_themes = self.aggregate_sub_themes(&all_sub_themes).await;

        match final_themes {
            Ok(themes) => Ok((themes, any_fallback)),
            Err(e) => {
                if debug_enabled() {
                    eprintln!("Theme aggregation failed: {}. Using fallback.", e);
                }
                Ok((self.fallback_themes_from_summaries(summaries), true))
            }
        }
    }

    /// Extract sub-themes from a batch of summaries.
    /// Uses a simplified prompt optimized for intermediate theme extraction.
    async fn extract_sub_themes(&self, batch: &[ChunkSummary]) -> Result<Vec<SubTheme>> {
        let summary_strings = self.format_summaries_for_prompt(batch);
        let builder = PromptBuilder::new().with_summaries(&summary_strings);

        let messages = vec![
            ChatMessage::system(builder.build_system_prompt()),
            ChatMessage::user(builder.build_intent_prompt()),
        ];

        let _permit = self.limiter.acquire().await;

        let response =
            generate_with_retry(self.provider.as_ref(), &messages, &RetryPolicy::default())
                .await
                .context("Sub-theme extraction failed")?;

        self.parse_sub_themes(&response)
    }

    /// Aggregate sub-themes from multiple batches into final themes.
    ///
    /// Strategy:
    /// 1. Group sub-themes by similarity (title/scope matching)
    /// 2. Merge similar themes, summing file counts
    /// 3. Select top 1-3 themes by file count
    /// 4. If too many distinct themes, use LLM to synthesize
    async fn aggregate_sub_themes(&self, sub_themes: &[SubTheme]) -> Result<Vec<Theme>> {
        // Simple aggregation: group by exact scope match, keep top by file count
        let mut scope_groups: std::collections::HashMap<String, Vec<SubTheme>> =
            std::collections::HashMap::new();

        for theme in sub_themes {
            scope_groups
                .entry(theme.scope.clone())
                .or_default()
                .push(theme.clone());
        }

        // Merge themes within each scope group
        let mut merged_themes: Vec<Theme> = Vec::new();

        for (scope, themes) in scope_groups {
            let total_files: usize = themes.iter().map(|t| t.file_count).sum();

            // Use the most common title, or synthesize one
            let title = if themes.len() == 1 {
                themes[0].title.clone()
            } else {
                // Find the most representative title
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

            // Combine descriptions
            let description = themes
                .iter()
                .map(|t| t.description.as_str())
                .collect::<Vec<_>>()
                .join("; ");

            merged_themes.push(Theme::new(title, description, total_files, scope));
        }

        // Sort by file count descending and take top 3
        merged_themes.sort_by(|a, b| b.file_count.cmp(&a.file_count));
        merged_themes.truncate(3);

        if debug_enabled() {
            eprintln!(
                "Aggregated {} sub-themes into {} final themes",
                sub_themes.len(),
                merged_themes.len()
            );
        }

        Ok(merged_themes)
    }

    /// Format summaries for prompt inclusion.
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

    /// Parse sub-themes from LLM response.
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

    /// Create fallback sub-themes from a batch when LLM extraction fails.
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

    /// Detect potential contradictions in chunk summaries.
    /// Logs a warning if contradictory operations are detected.
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
                if debug_enabled() {
                    eprintln!(
                        "Warning: Potential contradiction detected - summaries contain both '{}' and '{}' operations",
                        action, counteraction
                    );
                    eprintln!("This may indicate conflicting changes or complex refactoring");
                }
                return;
            }
        }
    }

    /// Create fallback themes from summaries when JSON parsing fails.
    /// This ensures the pipeline can continue even with unstructured LLM output.
    fn fallback_themes_from_summaries(&self, summaries: &[ChunkSummary]) -> Vec<Theme> {
        // Group summaries into a single "changes" theme for simplicity
        let total_files: usize = summaries.iter().map(|s| s.files.len()).sum();
        let combined_description = summaries
            .iter()
            .map(|s| s.summary.as_str())
            .collect::<Vec<_>>()
            .join("; ");

        // Create a single encompassing theme
        vec![Theme::new(
            "Code changes".to_string(),
            combined_description,
            total_files,
            "chore".to_string(), // Safe fallback scope
        )]
    }

    fn parse_themes(&self, response: &str) -> Result<Vec<Theme>> {
        // Try to extract JSON from the response (LLM might wrap it in markdown)
        let json_str = self.extract_json(response);

        let theme_response: ThemeResponse =
            serde_json::from_str(&json_str).context("Failed to parse themes JSON")?;

        Ok(theme_response
            .themes
            .into_iter()
            .map(|t| Theme::new(t.title, t.description, t.file_count, t.scope))
            .collect())
    }

    /// Extract JSON from a response that might contain markdown formatting.
    ///
    /// Uses a brace-balancing algorithm to find the outermost valid JSON object
    /// when markdown markers are missing or incomplete.
    fn extract_json(&self, response: &str) -> String {
        // Try markdown code block extraction first
        if let Some(content) = Self::extract_from_markdown(response) {
            return content;
        }

        // Fall back to brace-balanced extraction
        if let Some(json) = Self::extract_balanced_json(response) {
            return json;
        }

        response.to_string()
    }

    /// Extract content from markdown code blocks.
    /// Returns None if no valid code block is found.
    fn extract_from_markdown(response: &str) -> Option<String> {
        // Try ```json first
        if let Some(start) = response.find("```json") {
            let after_marker = &response[start + 7..];
            if let Some(end) = after_marker.find("```") {
                return Some(after_marker[..end].trim().to_string());
            }
        }

        // Try generic ``` block containing JSON
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

    /// Extract the outermost balanced JSON object using brace counting.
    ///
    /// Handles nested objects correctly by tracking brace depth.
    /// Returns None if no balanced JSON object is found.
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
                    if depth == 0 && let Some(s) = start {
                        return Some(response[s..=i].to_string());
                    }
                }
                _ => {}
            }
        }

        None
    }

    /// Reduce Phase: Synthesize final commit message from themes.
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
            ChatMessage::system(builder.build_system_prompt()),
            ChatMessage::user(prompt),
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
            if debug_enabled() {
                eprintln!("validation input length: {}", cleaned.len());
            }
        };
        let (validation_result, _) = tokio::join!(validation_future, debug_future);
        validation_result
    }

    /// Clean up the LLM response to extract just the commit message.
    fn clean_response(&self, response: &str) -> String {
        let mut message = response.trim().to_string();

        // Remove markdown code blocks if present
        if message.starts_with("```")
            && let Some(end) = message[3..].find("```")
        {
            message = message[3..3 + end].trim().to_string();
        }

        // Remove common preamble phrases
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

        // Take only the first line (commit header)
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

    /// Attempt to extract a valid conventional commit from a malformed response.
    ///
    /// Finds the earliest valid conventional commit in the message.
    /// Prefers earliest match to avoid extracting from examples or history sections.
    fn try_extract_valid_commit(
        &self,
        message: &str,
        mode: ValidationMode,
        max_length: Option<usize>,
    ) -> Option<String> {
        try_extract_valid_commit(message, mode, max_length)
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
        let timeout = timeout_for_attempt(attempt);
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

        let delay = policy.calculate_delay(attempt);
        tokio::time::sleep(delay).await;
    }

    #[expect(
        clippy::expect_used,
        reason = "retry loop guarantees last_error is set before exhaustion"
    )]
    Err(last_error.expect("retry loop should have at least one error"))
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
#[allow(clippy::panic, clippy::unwrap_used)]
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
        let provider = Arc::new(Provider::default());
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
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"themes": [{"title": "Test", "description": "Test desc", "fileCount": 1, "scope": "feature"}]}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, response);
    }

    #[test]
    fn clean_response() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = "Here is the commit message:\nfeat(auth): add login flow\n\nSome extra text";
        let cleaned = orchestrator.clean_response(response);
        assert_eq!(cleaned, "feat(auth): add login flow");
    }

    #[test]
    fn clean_response_with_code_block() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = "```\nfeat(auth): add login flow\n```";
        let cleaned = orchestrator.clean_response(response);
        assert_eq!(cleaned, "feat(auth): add login flow");
    }

    #[tokio::test]
    async fn orchestrate_empty_chunks() {
        let provider = Arc::new(Provider::default());
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
        // Retry succeeded, timing may vary due to full jitter
    }

    #[tokio::test]
    async fn map_phase_systemic_failure_aborts_immediately() {
        // Systemic error (Unauthorized) should abort immediately without retrying
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
        // 1 failure out of 10 chunks = 10% failure rate (exactly at threshold)
        // With 3 retries, failed chunk needs 4 error responses
        let mut responses = Vec::new();

        // First chunk: fails initially, then succeeds on retry
        responses.push(Err(CompletionError::Timeout));
        responses.push(Ok("summary 1".to_string()));

        // Next 9 chunks succeed immediately
        for i in 2..=10 {
            responses.push(Ok(format!("summary {}", i)));
        }

        // Intent extraction phase (for >2 summaries)
        responses.push(Ok(r#"{"themes": [{"title": "Test", "description": "Test theme", "fileCount": 10, "scope": "feat"}]}"#.to_string()));

        // Final reduce phase
        responses.push(Ok("feat(core): partial success".to_string()));

        let provider = Arc::new(Provider::mock_sequence(responses));
        let orchestrator = AIOrchestrator::new(provider);

        let chunks: Vec<_> = (0..10).map(|_| sample_chunk()).collect();
        let result = orchestrator
            .generate_commit_message(chunks, None, ValidationMode::default(), None, None)
            .await
            .unwrap_or_else(|e| panic!("should succeed with transient failures: {}", e));

        // The retry succeeded, so no failed chunks reported
        assert_eq!(result.failed_chunks, 0);
        assert_eq!(result.total_chunks, 10);
        assert_eq!(result.message.as_ref(), "feat(core): partial success");
    }

    #[tokio::test(start_paused = true)]
    async fn map_phase_partial_failure_exceeds_threshold() {
        // 2 failures out of 10 chunks = 20% failure rate (exceeds 10% threshold)
        let mut responses = Vec::new();
        // First 2 chunks fail
        responses.push(Err(CompletionError::Timeout));
        responses.push(Err(CompletionError::NetworkError(
            "connection reset".to_string(),
        )));
        // Next 8 succeed
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
        // All chunks fail - should abort with clear message
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
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"
```json
{"themes": [{"title": "Test", "description": "Missing closing brace"
```
"#;

        let json = orchestrator.extract_json(response);
        // Should extract what's between markers even if malformed
        assert!(json.contains("\"themes\""));
        assert!(json.contains("Missing closing brace"));
    }

    #[test]
    fn extract_json_missing_closing_marker() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"
```json
{"themes": [{"title": "Test"}]}
"#;

        // Missing closing ```, should fall back to brace extraction
        let json = orchestrator.extract_json(response);
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
        assert!(json.contains("\"themes\""));
    }

    #[test]
    fn extract_json_multiple_blocks_first_used() {
        let provider = Arc::new(Provider::default());
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
        // Should use the FIRST block
        assert!(json.contains("First"));
        assert!(json.contains("First block"));
        assert!(!json.contains("Second"));
    }

    #[test]
    fn extract_json_no_markers_raw_json() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"themes": [{"title": "Raw", "description": "No markers", "fileCount": 1, "scope": "test"}]}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, response);
    }

    #[test]
    fn extract_json_empty_content() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = "";

        let json = orchestrator.extract_json(response);
        assert_eq!(json, "");
    }

    #[test]
    fn extract_json_non_json_content_inside_markers() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"
```json
This is not JSON at all, just plain text
```
"#;

        let json = orchestrator.extract_json(response);
        // Should extract content even if it's not valid JSON
        assert_eq!(json.trim(), "This is not JSON at all, just plain text");
    }

    #[test]
    fn extract_json_generic_code_block_with_json() {
        let provider = Arc::new(Provider::default());
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
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = "Here is some text without any JSON content at all";

        let json = orchestrator.extract_json(response);
        // Should return the original text when no JSON structure is found
        assert_eq!(json, response);
    }

    #[test]
    fn extract_json_nested_braces() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"Some preamble {"outer": {"inner": {"deep": "value"}}} some suffix"#;

        let json = orchestrator.extract_json(response);
        // Should extract balanced JSON, not just first { to last }
        assert_eq!(json, r#"{"outer": {"inner": {"deep": "value"}}}"#);
    }

    #[test]
    fn extract_json_multiple_objects_balanced() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        // Multiple JSON objects - should extract first balanced one
        let response = r#"{"first": 1} {"second": 2}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"first": 1}"#);
    }

    #[test]
    fn extract_json_with_escaped_quotes() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"message": "He said \"hello\"", "count": 1}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"message": "He said \"hello\"", "count": 1}"#);
    }

    #[test]
    fn extract_json_with_escaped_backslash() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"path": "C:\\Users\\test", "valid": true}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"path": "C:\\Users\\test", "valid": true}"#);
    }

    #[test]
    fn extract_json_unbalanced_braces() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        // Unbalanced braces - should return original
        let response = r#"{"unclosed": "brace" "#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, response);
    }

    #[test]
    fn extract_json_braces_in_string() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        // Braces inside strings should not affect balancing
        let response = r#"{"code": "if (x) { return y; }", "lang": "js"}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"code": "if (x) { return y; }", "lang": "js"}"#);
    }

    #[test]
    fn extract_json_deeply_nested() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"a": {"b": {"c": {"d": {"e": "deep"}}}}}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"a": {"b": {"c": {"d": {"e": "deep"}}}}}"#);
    }

    #[test]
    fn extract_json_with_arrays() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = r#"{"items": [{"id": 1}, {"id": 2}], "count": 2}"#;

        let json = orchestrator.extract_json(response);
        assert_eq!(json, r#"{"items": [{"id": 1}, {"id": 2}], "count": 2}"#);
    }

    #[test]
    fn extract_json_no_json_content() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let response = "Just plain text without any braces";

        let json = orchestrator.extract_json(response);
        assert_eq!(json, response);
    }

    #[test]
    fn extract_json_partial_object_in_text() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        // Text with braces but not a JSON object
        let response = "Error: {code: 404} (not valid JSON)";

        let json = orchestrator.extract_json(response);
        // Should extract the balanced part
        assert_eq!(json, "{code: 404}");
    }

    #[test]
    fn calculate_history_budget() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        // Test: 4096 tokens → ~4 commits (4096 * 0.15 / 150 ≈ 4)
        let budget_4k = orchestrator.calculate_history_budget(4096);
        assert_eq!(budget_4k, 4);

        // Test: 16384 tokens → ~16 commits (16384 * 0.15 / 150 ≈ 16)
        let budget_16k = orchestrator.calculate_history_budget(16384);
        assert_eq!(budget_16k, 16);

        // Test: 1000 tokens → 3 commits (1000 * 0.15 / 150 ≈ 1, clamped to minimum 3)
        let budget_1k = orchestrator.calculate_history_budget(1000);
        assert_eq!(budget_1k, 3);

        // Test: Very low token count respects minimum
        let budget_very_low = orchestrator.calculate_history_budget(100);
        assert_eq!(budget_very_low, 3);
    }

    #[test]
    fn format_summaries_for_prompt() {
        let provider = Arc::new(Provider::default());
        let orchestrator = AIOrchestrator::new(provider);

        let summaries = vec![
            ChunkSummary {
                summary: "Added authentication".to_string(),
                files: vec![FilePath::from("src/auth.rs")],
            },
            ChunkSummary {
                summary: "Fixed login bug".to_string(),
                files: vec![FilePath::from("src/login.rs"), FilePath::from("src/user.rs")],
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
        let provider = Arc::new(Provider::default());
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
        let provider = Arc::new(Provider::default());
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

        let themes = orchestrator.aggregate_sub_themes(&sub_themes).await.unwrap();

        // Should have 2 themes (auth and api)
        assert_eq!(themes.len(), 2);

        // Auth theme should have merged file count
        let auth_theme = themes.iter().find(|t| t.scope == "auth").unwrap();
        assert_eq!(auth_theme.file_count, 5);

        // API theme should remain separate
        let api_theme = themes.iter().find(|t| t.scope == "api").unwrap();
        assert_eq!(api_theme.file_count, 1);
    }

    #[tokio::test]
    async fn aggregate_sub_themes_limits_to_top_three() {
        let provider = Arc::new(Provider::default());
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

        let themes = orchestrator.aggregate_sub_themes(&sub_themes).await.unwrap();

        // Should be limited to top 3 by file count
        assert_eq!(themes.len(), 3);
        assert_eq!(themes[0].file_count, 10);
        assert_eq!(themes[1].file_count, 8);
        assert_eq!(themes[2].file_count, 6);
    }
}
