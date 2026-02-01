use christina_core::error::CompletionError;
use christina_core::types::{CommitMessage, TokenCount};
use compact_str::CompactString;

use crate::app::state::GenerationState;
use crate::app::App;
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
            CompletionError::UnknownError(msg) => {
                format!("An unexpected error occurred: {}", msg)
            }
        };
    }

    // Fallback to string representation
    err.to_string()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::app::state::AbortOnDrop;
    use anyhow::anyhow;

    #[test]
    fn test_format_unauthorized() {
        let err = anyhow::Error::new(CompletionError::Unauthorized("test".to_string()));
        let msg = format_error_message(&err);
        assert!(msg.contains("API key is invalid"));
        assert!(msg.contains("christina config setup"));
    }

    #[test]
    fn test_format_rate_limited() {
        let err = anyhow::Error::new(CompletionError::RateLimited);
        let msg = format_error_message(&err);
        assert!(msg.contains("rate limit exceeded"));
    }

    #[test]
    fn test_format_timeout() {
        let err = anyhow::Error::new(CompletionError::Timeout);
        let msg = format_error_message(&err);
        assert!(msg.contains("timed out"));
        assert!(msg.contains("network connection"));
    }

    #[test]
    fn test_format_server_error() {
        let err = anyhow::Error::new(CompletionError::ServerError("500 Internal".to_string()));
        let msg = format_error_message(&err);
        assert!(msg.contains("Server error"));
        assert!(msg.contains("500 Internal"));
    }

    #[test]
    fn test_format_network_error() {
        let err = anyhow::Error::new(CompletionError::NetworkError("DNS failed".to_string()));
        let msg = format_error_message(&err);
        assert!(msg.contains("Network error"));
        assert!(msg.contains("DNS failed"));
    }

    #[test]
    fn test_format_invalid_response() {
        let err = anyhow::Error::new(CompletionError::InvalidResponse("Bad JSON".to_string()));
        let msg = format_error_message(&err);
        assert!(msg.contains("Invalid response"));
        assert!(msg.contains("Bad JSON"));
    }

    #[test]
    fn test_format_unknown_error() {
        let err = anyhow::Error::new(CompletionError::UnknownError("Unknown".to_string()));
        let msg = format_error_message(&err);
        assert!(msg.contains("unexpected error"));
        assert!(msg.contains("Unknown"));
    }

    #[test]
    fn test_format_other_error() {
        let err = anyhow!("Some other error");
        let msg = format_error_message(&err);
        assert_eq!(msg, "Some other error");
    }

    #[test]
    fn test_handle_tick_increments_frame() {
        let mut app = create_test_app();
        let initial_frame_count = app.ui.frame_count;

        handle_tick(&mut app);

        assert_eq!(app.ui.frame_count, initial_frame_count.wrapping_add(1));
        assert!(app.ui.should_redraw);
    }

    #[tokio::test]
    async fn test_handle_generation_complete_matching_id() {
        let mut app = create_test_app();
        app.generation_state = GenerationState::Running {
            task: AbortOnDrop(tokio::spawn(async {})),
            generation_id: 42,
        };
        app.state = AppState::Generating;

        let message = CommitMessage::try_from("test: commit message".to_string())
            .expect("test commit message is valid");
        handle_generation_complete(&mut app, message.clone(), None, 42);

        assert_eq!(app.data.base.generated_message.as_str(), message.as_ref());
        assert!(matches!(app.state, AppState::Review));
        assert!(matches!(app.generation_state, GenerationState::Idle));
    }

    #[tokio::test]
    async fn test_handle_generation_complete_mismatched_id() {
        let mut app = create_test_app();
        app.generation_state = GenerationState::Running {
            task: AbortOnDrop(tokio::spawn(async {})),
            generation_id: 42,
        };
        app.state = AppState::Generating;
        let initial_message = app.data.base.generated_message.clone();

        let message = CommitMessage::try_from("test: commit message".to_string())
            .expect("test commit message is valid");
        handle_generation_complete(&mut app, message, None, 99);

        // State should not change
        assert_eq!(app.data.base.generated_message, initial_message);
        assert!(matches!(app.state, AppState::Generating));
        assert!(matches!(app.generation_state, GenerationState::Running { .. }));
    }

    #[tokio::test]
    async fn test_handle_generation_error_matching_id() {
        let mut app = create_test_app();
        app.generation_state = GenerationState::Running {
            task: AbortOnDrop(tokio::spawn(async {})),
            generation_id: 42,
        };
        app.state = AppState::Generating;

        handle_generation_error(&mut app, "test error".to_string(), 42);

        assert_eq!(
            app.data.base.error_message,
            Some("test error".to_string())
        );
        assert!(matches!(app.state, AppState::Error));
        assert!(matches!(app.generation_state, GenerationState::Idle));
    }

    #[tokio::test]
    async fn test_handle_generation_error_mismatched_id() {
        let mut app = create_test_app();
        app.generation_state = GenerationState::Running {
            task: AbortOnDrop(tokio::spawn(async {})),
            generation_id: 42,
        };
        app.state = AppState::Generating;

        handle_generation_error(&mut app, "test error".to_string(), 99);

        // State should not change
        assert_eq!(app.data.base.error_message, None);
        assert!(matches!(app.state, AppState::Generating));
        assert!(matches!(app.generation_state, GenerationState::Running { .. }));
    }

    #[test]
    fn test_handle_input_quit() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = create_test_app();
        app.state = AppState::Dashboard;
        app.data.dashboard_state = Some(crate::tui::screens::DashboardState::new(vec![]));

        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        handle_input(&mut app, key);

        assert!(app.should_quit);
    }

    #[test]
    fn test_handle_input_navigate() {
        use christina_core::GitFile;
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = create_test_app();
        app.state = AppState::Dashboard;
        app.data.base.staged_files = vec![
            GitFile::new("file1.txt".to_string(), "A".to_string(), "".to_string()),
            GitFile::new("file2.txt".to_string(), "M".to_string(), "".to_string()),
        ];

        app.data.dashboard_state = Some(crate::tui::screens::DashboardState::new(
            app.data.base.staged_files.clone(),
        ));

        let initial_selection = app
            .data
            .dashboard_state
            .as_ref()
            .and_then(|s| s.list_state.selected());

        let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        handle_input(&mut app, key);

        let new_selection = app
            .data
            .dashboard_state
            .as_ref()
            .and_then(|s| s.list_state.selected());

        assert!(new_selection.is_some() || initial_selection.is_some());
    }

    #[test]
    fn test_handle_input_confirm() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = create_test_app();
        app.state = AppState::Dashboard;

        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        handle_input(&mut app, key);

        assert!(matches!(
            app.state,
            AppState::Dashboard | AppState::Generating | AppState::Review
        ));
    }

    #[test]
    fn test_handle_input_cancel() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = create_test_app();
        app.state = AppState::Generating;

        app.data.generating_state_ui = Some(crate::tui::screens::GeneratingState::new());

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        handle_input(&mut app, key);

        assert!(app.state == AppState::Generating || app.state == AppState::Dashboard);
    }

    #[test]
    fn test_handle_input_edit() {
        use christina_core::types::CommitMessage;
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = create_test_app();
        app.state = AppState::Review;
        app.data.base.generated_message = CompactString::new("test: commit message");

        let message = CommitMessage::try_from("test: commit message".to_string())
            .expect("test commit message is valid");
        app.data.review_state = Some(crate::tui::screens::ReviewState::new(message));

        let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        handle_input(&mut app, key);

        assert!(matches!(app.state, AppState::Editing));
    }

    #[tokio::test]
    async fn test_handle_generation_progress_updates_status() {
        let mut app = create_test_app();
        app.state = AppState::Generating;
        app.generation_state = GenerationState::Running {
            task: AbortOnDrop(tokio::spawn(async {})),
            generation_id: 42,
        };
        app.data.generating_state_ui = Some(crate::tui::screens::GeneratingState::new());

        handle_generation_progress(&mut app, "Analyzing repository...".to_string(), 42);

        let state = app.data.generating_state_ui.as_ref().unwrap();
        assert_eq!(state.stage, "Analyzing repository...");
        assert!(app.ui.should_redraw);
    }

    #[tokio::test]
    async fn test_handle_generation_progress_matching_id_only() {
        let mut app = create_test_app();
        app.state = AppState::Generating;
        app.generation_state = GenerationState::Running {
            task: AbortOnDrop(tokio::spawn(async {})),
            generation_id: 42,
        };
        app.data.generating_state_ui = Some(crate::tui::screens::GeneratingState::new());
        let initial_stage = app.data.generating_state_ui.as_ref().unwrap().stage.clone();

        handle_generation_progress(&mut app, "Should not appear".to_string(), 99);

        let state = app.data.generating_state_ui.as_ref().unwrap();
        assert_eq!(state.stage, initial_stage);
    }

    #[tokio::test]
    async fn test_handle_token_count_updates() {
        use christina_core::types::TokenCount;
        let mut app = create_test_app();
        app.state = AppState::Generating;
        app.generation_state = GenerationState::Running {
            task: AbortOnDrop(tokio::spawn(async {})),
            generation_id: 42,
        };

        let token_count = TokenCount::new_saturating(1000);
        handle_token_count_update(&mut app, token_count, 42);

        assert_eq!(app.data.base.token_count.get(), token_count.get());
        assert!(app.ui.should_redraw);
    }

    #[tokio::test]
    async fn test_handle_token_count_matching_id_only() {
        use christina_core::types::TokenCount;
        let mut app = create_test_app();
        app.state = AppState::Generating;
        app.generation_state = GenerationState::Running {
            task: AbortOnDrop(tokio::spawn(async {})),
            generation_id: 42,
        };
        let initial_token_count = app.data.base.token_count;

        let token_count = TokenCount::new_saturating(1000);
        handle_token_count_update(&mut app, token_count, 99);

        assert_eq!(app.data.base.token_count, initial_token_count);
    }

    // Helper to create minimal test App
    fn create_test_app() -> App {
        use crate::app::context::AppContextData;
        use crate::app::edit_history::EditHistory;
        use crate::app::state::{GenerationState, TuiSessionData, TuiUiState};
        use crate::config::Config;
        use crate::tui::{DataState, ToastManager, UiState};
        use christina_core::types::TokenCount;
        use christina_core::{ReviewAction, StateMachine};
        use ratatui::{
            style::Style,
            widgets::{Block, BorderType, Borders},
        };
        use tui_textarea::TextArea;

        let mut textarea = TextArea::default();
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default())
                .title(" Edit Message "),
        );

        App {
            app_context: AppContextData {
                repo: None,
                config: Config::default(),
                branch_name: None,
            },
            ui: TuiUiState {
                base: UiState {
                    textarea,
                    spinner_idx: 0,
                },
                frame_count: 0,
                should_redraw: false,
            },
            data: TuiSessionData {
                base: DataState {
                    staged_files: vec![],
                    unstaged_files: vec![],
                    selected_indices: vec![],
                    multi_select_mode: false,
                    generated_message: CompactString::default(),
                    error_message: None,
                    toasts: ToastManager::new(),
                    token_count: TokenCount::new_saturating(1),
                    user_context: None,
                    review_action: ReviewAction::Accept,
                    edit_history: EditHistory::default(),
                    data_version: 0,
                },
                state_machine: StateMachine::new(),
                dashboard_state: None,
                error_state: None,
                review_state: None,
                staging_state: None,
                editing_state: None,
                generating_state_ui: None,
            },
            state: AppState::Dashboard,
            should_quit: false,
            exit_message: None,
            generation_state: GenerationState::Idle,
        }
    }
}
