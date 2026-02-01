use christina_core::types::{CommitMessage, TokenCount};
use ratatui::crossterm::event::KeyEvent;

/// Internal event type for the application event loop.
#[derive(Debug)]
pub enum Event {
    /// Keyboard input event
    Input(KeyEvent),
    /// Tick event for animations and state updates
    Tick,
    /// Terminal resize event
    Resize,
    /// Generation progress update
    GenerationProgress { stage: String, generation_id: u64 },
    /// Token count update after diff processing
    #[expect(
        dead_code,
        reason = "Will be constructed when token counting is implemented"
    )]
    TokenCountUpdate {
        token_count: TokenCount,
        generation_id: u64,
    },
    /// Generation completed successfully
    GenerationComplete {
        message: CommitMessage,
        warning_summary: Option<String>,
        generation_id: u64,
    },
    /// Generation failed with error
    GenerationError { error: String, generation_id: u64 },
}
