use christina_core::types::TokenCount;

/// Events emitted during commit message generation.
///
/// These events provide progress updates and status information to consumers
/// (CLI output, TUI, etc.). The event system allows decoupling of generation
/// logic from UI concerns.
#[derive(Debug, Clone)]
pub enum Event {
    /// Generation has progressed to a new stage.
    GenerationProgress {
        /// Description of the current stage (e.g., "Processing diff", "Generating message")
        stage: String,
    },

    /// Token count for the current context has been computed.
    TokenCountUpdate {
        /// Total token count for the processed content
        token_count: TokenCount,
    },

    /// A file chunk has been processed.
    ///
    /// Emitted after successfully processing a diff chunk, useful for
    /// progress tracking in multi-file diffs.
    ChunkProcessed {
        /// Number of chunks processed so far
        chunks_processed: usize,
        /// Total number of chunks to process
        total_chunks: usize,
    },

    /// An LLM request retry is being attempted.
    ///
    /// Emitted when a transient error occurs and the request will be retried.
    /// Useful for showing retry progress and backoff delays to the user.
    RetryAttempt {
        /// Current attempt number (1-indexed)
        attempt: u32,
        /// Maximum number of retries configured
        max_retries: u32,
        /// Reason for the retry (e.g., "rate limit", "timeout")
        reason: String,
    },

    /// A commit has been successfully created.
    ///
    /// Emitted after the commit is written to the repository.
    CommitCreated {
        /// Git commit hash (short form)
        commit_hash: String,
    },

    /// Diff processing has completed.
    ///
    /// Emitted after the git diff has been parsed and chunked.
    DiffProcessed {
        /// Number of files in the diff
        file_count: usize,
        /// Total lines changed (additions + deletions)
        lines_changed: usize,
    },

    /// Connecting to the LLM provider.
    ///
    /// Emitted before making the first API request to the provider.
    ProviderConnecting {
        /// Provider name (e.g., "OpenAI", "Azure", "Groq")
        provider: String,
    },
}
