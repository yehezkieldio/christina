use std::io;

use thiserror::Error;

use christina_core::error::CompletionError;

/// Errors that can occur during LLM orchestration.
#[derive(Debug, Error)]
pub enum OrchestratorError {
    /// No diff chunks to process.
    #[error("No diff chunks to process")]
    EmptyChunks,

    /// All chunks failed to process.
    #[error("All {0} chunks failed to process. Files affected: {1}")]
    AllChunksFailed(usize, String),

    /// Partial failure rate exceeded threshold.
    #[error(
        "Partial failure rate too high: {failed}/{total} chunks failed ({rate:.0}%). \
         This exceeds the {threshold:.0}% threshold for acceptable degradation. \
         Files affected: {files}"
    )]
    FailureRateExceeded {
        failed: usize,
        total: usize,
        rate: f64,
        threshold: f64,
        files: String,
    },

    /// User declined to proceed with partial failures.
    #[error(
        "User declined to proceed with partial failures. \
         {failed}/{total} chunks failed ({rate:.0}%). Files affected: {files}"
    )]
    UserDeclinedPartial {
        failed: usize,
        total: usize,
        rate: f64,
        files: String,
    },

    /// Systemic provider failure that aborts the pipeline.
    #[error(
        "Systemic provider failure detected - aborting pipeline: {0}. \
         Files affected: {1}. This typically indicates authentication issues, \
         rate limit exhaustion, or invalid API keys."
    )]
    SystemicFailure(String, String),

    /// Completion operation failed.
    #[error("Completion failed: {0}")]
    Completion(#[from] CompletionError),

    /// Direct generation failed.
    #[error("Direct generation failed")]
    DirectGenerationFailed,

    /// Intent extraction failed after retries.
    #[error("Intent extraction failed after retries")]
    IntentExtractionFailed,

    /// Sub-theme extraction failed.
    #[error("Sub-theme extraction failed")]
    SubThemeExtractionFailed,

    /// Reduce phase synthesis failed.
    #[error("Reduce phase synthesis failed")]
    ReducePhaseFailed,

    /// Failed to parse themes from LLM response.
    #[error("Failed to parse themes JSON: {0}")]
    ThemeParseFailed(String),

    /// Generated message does not follow Conventional Commits format.
    #[error("Generated message does not follow Conventional Commits format: {0}")]
    InvalidCommitFormat(String),

    /// IO error during user interaction.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// Result type for orchestrator operations.
pub type Result<T> = std::result::Result<T, OrchestratorError>;
