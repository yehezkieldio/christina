use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use serde::Deserialize;

use crate::concurrency::RequestLimiter;
use crate::provider::{ChatMessage, CompletionError, Provider};
use crate::retry::{IsTransient, RetryPolicy};
use christina_core::prompt::{PromptBuilder, Theme};

use christina_core::git::DiffChunk;
use christina_core::types::{CommitMessage, FilePath, commit_message::ValidationMode};

const MAX_CONCURRENT_REQUESTS: usize = 5;
const LLM_INITIAL_TIMEOUT_SECONDS: u64 = 30;
const LLM_RETRY_TIMEOUT_SECONDS: u64 = 60;
const LLM_TIMEOUT_SECONDS: u64 = 120;

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

        Self {
            provider,
            limiter: RequestLimiter::new(concurrency_limit),
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
        let retry_policy = RetryPolicy::default();
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
    async fn extract_intent(&self, summaries: &[ChunkSummary]) -> Result<(Vec<Theme>, bool)> {
        // Check for potential contradictions in summaries
        self.detect_contradictions(summaries);

        // Format summaries with file paths for the prompt
        let mut summary_strings = Vec::with_capacity(summaries.len());
        for summary in summaries {
            let paths = summary
                .files
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            summary_strings.push(format!(
                "[{} files: {}] {}",
                summary.files.len(),
                paths,
                summary.summary
            ));
        }

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
                // Log detailed parsing error for debugging
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
    fn extract_json(&self, response: &str) -> String {
        if let Some(start) = response.find("```json")
            && let Some(end) = response[start + 7..].find("```")
        {
            return response[start + 7..start + 7 + end].trim().to_string();
        }

        if let Some(start) = response.find("```")
            && let Some(end) = response[start + 3..].find("```")
        {
            let content = response[start + 3..start + 3 + end].trim();
            if content.starts_with('{') {
                return content.to_string();
            }
        }

        if let Some(start) = response.find('{')
            && let Some(end) = response.rfind('}')
        {
            return response[start..=end].to_string();
        }

        response.to_string()
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
