//! Centralized constants for the christina codebase.
//!
//! This module provides a single source of truth for all default values,
//! limits, and magic numbers used across the application. Organizing
//! constants by domain improves maintainability and prevents drift.

/// LLM-related constants.
pub mod llm {
    /// Default temperature for LLM sampling (0.0 = deterministic, 2.0 = very random).
    pub const DEFAULT_TEMPERATURE: f32 = 0.3;

    /// Maximum valid temperature value.
    pub const MAX_TEMPERATURE: f32 = 2.0;

    /// Minimum valid temperature value.
    pub const MIN_TEMPERATURE: f32 = 0.0;

    /// Maximum tokens allowed for LLM input context.
    pub const MAX_INPUT_TOKENS: u32 = 256_000;

    /// Maximum tokens allowed for LLM output generation.
    pub const MAX_OUTPUT_TOKENS: u32 = 4_096;

    /// Default input token budget for most models.
    pub const DEFAULT_INPUT_TOKENS: u32 = 128_000;

    /// Default output token budget.
    pub const DEFAULT_OUTPUT_TOKENS: u32 = 2_048;

    /// Minimum input token budget for constrained scenarios.
    pub const MIN_INPUT_TOKENS: u32 = 4_096;

    /// Minimum output token budget for simple responses.
    pub const MIN_OUTPUT_TOKENS: u32 = 500;

    /// Maximum number of concurrent LLM requests.
    pub const MAX_CONCURRENT_REQUESTS: usize = 5;

    /// Timeout for LLM API requests in seconds.
    pub const REQUEST_TIMEOUT_SECS: u64 = 60;

    /// Timeout for streaming responses in seconds.
    pub const STREAM_TIMEOUT_SECS: u64 = 180;

    /// Retry delay base in milliseconds (exponential backoff).
    pub const RETRY_DELAY_BASE_MS: u64 = 500;
}

/// Git-related constants.
pub mod git {
    /// Maximum length for a conventional commit message header.
    pub const MAX_COMMIT_MESSAGE_LENGTH: usize = 72;

    /// Minimum number of commits to include in history analysis.
    pub const MIN_COMMIT_HISTORY_DEPTH: usize = 5;

    /// Maximum number of commits to include in history analysis.
    pub const MAX_COMMIT_HISTORY_DEPTH: usize = 20;

    /// Default number of commits to analyze for style context.
    pub const DEFAULT_COMMIT_HISTORY_DEPTH: usize = 5;

    /// Whether to include commit history by default.
    pub const DEFAULT_USE_COMMIT_HISTORY: bool = true;
}

/// UI-related constants.
pub mod ui {
    /// Maximum diff size to preview in bytes (64KB).
    pub const MAX_DIFF_PREVIEW_BYTES: usize = 64 * 1024;

    /// Maximum number of suggestion entries to display.
    pub const MAX_SUGGESTIONS: usize = 10;
}

/// File patterns to ignore by default when generating commits.
pub const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "*.lock",
    "node_modules/",
    "target/",
    ".git/",
    "*.log",
];

/// Azure OpenAI default API version.
pub const DEFAULT_AZURE_API_VERSION: &str = "2024-12-01-preview";

/// Default model for new profiles.
pub const DEFAULT_MODEL: &str = "gpt-4o-mini";

/// Default provider for new profiles.
pub const DEFAULT_PROVIDER: &str = "openai";

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used, clippy::assertions_on_constants)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_range() {
        assert!(llm::MIN_TEMPERATURE >= 0.0);
        assert!(llm::MAX_TEMPERATURE <= 2.0);
        assert!((llm::MIN_TEMPERATURE..=llm::MAX_TEMPERATURE).contains(&llm::DEFAULT_TEMPERATURE));
    }

    #[test]
    fn test_token_limits() {
        assert!(llm::MIN_INPUT_TOKENS < llm::DEFAULT_INPUT_TOKENS);
        assert!(llm::DEFAULT_INPUT_TOKENS <= llm::MAX_INPUT_TOKENS);
        assert!(llm::MIN_OUTPUT_TOKENS < llm::DEFAULT_OUTPUT_TOKENS);
        assert!(llm::DEFAULT_OUTPUT_TOKENS <= llm::MAX_OUTPUT_TOKENS);
    }

    #[test]
    fn test_history_depth_range() {
        assert!(git::MIN_COMMIT_HISTORY_DEPTH < git::MAX_COMMIT_HISTORY_DEPTH);
        assert!(
            (git::MIN_COMMIT_HISTORY_DEPTH..=git::MAX_COMMIT_HISTORY_DEPTH)
                .contains(&git::DEFAULT_COMMIT_HISTORY_DEPTH)
        );
    }
}
