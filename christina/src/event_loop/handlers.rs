use christina_core::error::CompletionError;
use christina_core::types::{CommitMessage, TokenCount};
use compact_str::CompactString;

use crate::app::App;
use crate::app::state::GenerationState;
use crate::tui::handle_key;
use christina_core::{AppState, ReviewAction};

pub fn handle_input(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    handle_key(app, key);
}

pub fn handle_tick(app: &mut App) {
    app.ui.frame_count = app.ui.frame_count.wrapping_add(1);
    app.data.base.toasts.update();

    if app.state == AppState::Generating {
        app.update_spinner();
        // Tick the generating UI state
        if let Some(ref mut state) = app.data.generating_state_ui {
            state.tick();
        }
    }

    // Always trigger redraw on tick for animations
    app.ui.should_redraw = true;
}

pub fn handle_generation_progress(app: &mut App, stage: String, generation_id: u64) {
    let matches = matches!(
        app.generation_state,
        GenerationState::Running { generation_id: id, .. } if id == generation_id
    );
    if matches {
        if let Some(ref mut state) = app.data.generating_state_ui {
            state.set_stage(stage);
        }
        app.ui.should_redraw = true;
    }
}

pub fn handle_token_count_update(app: &mut App, token_count: TokenCount, generation_id: u64) {
    let matches = matches!(
        app.generation_state,
        GenerationState::Running { generation_id: id, .. } if id == generation_id
    );
    if matches {
        app.data.base.token_count = token_count;
        app.ui.should_redraw = true;
    }
}

pub fn handle_generation_complete(
    app: &mut App,
    message: CommitMessage,
    warning_summary: Option<String>,
    generation_id: u64,
) {
    let matches = matches!(
        app.generation_state,
        GenerationState::Running { generation_id: id, .. } if id == generation_id
    );
    if matches {
        app.data.base.generated_message = CompactString::new(message.as_ref());
        app.data.base.edit_history.initialize(message.as_ref());
        app.transition_to(AppState::Review);
        app.data.base.review_action = ReviewAction::Accept;
        app.generation_state = GenerationState::Idle;

        // Display warning toast if there were any issues during generation
        if let Some(warning) = warning_summary {
            app.data.base.toasts.warning(warning);
        }
    }
}

pub fn handle_generation_error(app: &mut App, error: String, generation_id: u64) {
    let matches = matches!(
        app.generation_state,
        GenerationState::Running { generation_id: id, .. } if id == generation_id
    );
    if matches {
        app.data.base.error_message = Some(error);
        app.transition_to(AppState::Error);
        app.generation_state = GenerationState::Idle;
    }
}

pub fn format_error_message(err: &anyhow::Error) -> String {
    // Try to downcast to CompletionError for typed handling
    if let Some(completion_err) = err.downcast_ref::<CompletionError>() {
        return match completion_err {
            CompletionError::Unauthorized(_) => {
                "API key is invalid or expired. Run `christina config setup` to reconfigure."
                    .to_string()
            }
            CompletionError::RateLimited => {
                "API rate limit exceeded. Please wait a moment and try again.".to_string()
            }
            CompletionError::Timeout => {
                "Request timed out. Check your network connection and try again.".to_string()
            }
            CompletionError::ServerError(msg) => {
                format!("Server error: {}. Please try again later.", msg)
            }
            CompletionError::NetworkError(msg) => {
                format!("Network error: {}. Check your connection.", msg)
            }
            CompletionError::InvalidResponse(msg) => {
                format!("Invalid response from API: {}", msg)
            }
        };
    }

    // Fallback to string representation
    err.to_string()
}
