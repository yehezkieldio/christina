use christina_core::types::tokens::TokenCount;

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

    /// Diff content has been chunked for processing.
    DiffChunked {
        /// Number of chunks generated from the diff
        chunk_count: usize,
        /// Whether the diff only contains binary files
        binary_only: bool,
    },
}
