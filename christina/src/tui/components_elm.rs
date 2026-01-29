use christina_core::AppState;
use christina_core::types::CommitMessage;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};

use crate::app::App;
use crate::tui::elm::Component;
use crate::tui::screens::*;

/// Render the current screen based on app state.
pub fn render(frame: &mut Frame, app: &mut App) {
    match app.state {
        AppState::StagingSelection => {
            if app.data.staging_state.is_none() {
                app.data.staging_state =
                    Some(StagingState::new(app.data.base.unstaged_files.clone()));
            }
            if let Some(ref mut state) = app.data.staging_state {
                // Only sync if data version changed (avoid per-frame cloning)
                if state.synced_version != app.data.base.data_version {
                    state.unstaged_files = app.data.base.unstaged_files.clone();
                    state.synced_version = app.data.base.data_version;

                    // Validate list_state after file list changes
                    if let Some(selected) = state.list_state.selected()
                        && selected >= state.unstaged_files.len()
                    {
                        if state.unstaged_files.is_empty() {
                            state.list_state.select(None);
                        } else {
                            state
                                .list_state
                                .select(Some(state.unstaged_files.len() - 1));
                        }
                    } else if !state.unstaged_files.is_empty()
                        && state.list_state.selected().is_none()
                    {
                        state.list_state.select(Some(0));
                    }
                }
                render_staging(frame, state, frame.area());
            }
        }
        AppState::Dashboard => {
            if app.data.dashboard_state.is_none() {
                app.data.dashboard_state =
                    Some(DashboardState::new(app.data.base.staged_files.clone()));
            }
            if let Some(ref mut state) = app.data.dashboard_state {
                state.staged_files = app.data.base.staged_files.clone();
                state.multi_select_mode = app.data.base.multi_select_mode;
                state.selected_indices = app.data.base.selected_indices.clone();

                if let Some(ref context) = app.data.base.user_context {
                    state.user_context_input = context.clone();
                }

                if let Some(selected) = state.list_state.selected()
                    && selected >= state.staged_files.len()
                {
                    if state.staged_files.is_empty() {
                        state.list_state.select(None);
                    } else {
                        state.list_state.select(Some(state.staged_files.len() - 1));
                    }
                } else if !state.staged_files.is_empty() && state.list_state.selected().is_none() {
                    state.list_state.select(Some(0));
                }
                let terminal_width = frame.area().width;
                let show_diff_preview = app.app_context.config.diff.show_preview;
                render_dashboard(
                    frame,
                    state,
                    frame.area(),
                    terminal_width,
                    show_diff_preview,
                );
            }
        }
        AppState::Generating => {
            if app.data.dashboard_state.is_none() {
                app.data.dashboard_state =
                    Some(DashboardState::new(app.data.base.staged_files.clone()));
            }
            if let Some(ref mut state) = app.data.dashboard_state {
                state.staged_files = app.data.base.staged_files.clone();

                if let Some(selected) = state.list_state.selected()
                    && selected >= state.staged_files.len()
                {
                    if state.staged_files.is_empty() {
                        state.list_state.select(None);
                    } else {
                        state.list_state.select(Some(state.staged_files.len() - 1));
                    }
                }
                let terminal_width = frame.area().width;
                let show_diff_preview = app.app_context.config.diff.show_preview;
                render_dashboard(
                    frame,
                    state,
                    frame.area(),
                    terminal_width,
                    show_diff_preview,
                );
            }

            if let Some(ref state) = app.data.generating_state_ui {
                render_generating(frame, state, frame.area());
            }
        }
        AppState::Review => {
            if app.data.review_state.is_none() {
                if let Ok(message) =
                    CommitMessage::try_from(app.data.base.generated_message.to_string())
                {
                    app.data.review_state = Some(ReviewState::new(message));
                } else {
                    app.data
                        .base
                        .toasts
                        .error("Generated message is invalid".to_string());
                    return;
                }
            }
            if let Some(ref mut state) = app.data.review_state {
                // Sync state with app data
                if let Ok(message) =
                    CommitMessage::try_from(app.data.base.generated_message.to_string())
                {
                    state.generated_message = message;
                } else {
                    app.data
                        .base
                        .toasts
                        .error("Generated message is invalid".to_string());
                    return;
                }
                state.review_action = app.data.base.review_action;
                render_review(frame, state, frame.area());
            }
        }
        AppState::Editing => {
            if app.data.editing_state.is_some() {
                render_editing(frame, frame.area(), &app.ui.base.textarea);
            }
        }
        AppState::Error => {
            if app.data.error_state.is_none() {
                let error_message = app
                    .data
                    .base
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "Unknown error".to_string());

                // Parse candidate if present (from orchestrator error format)
                let candidate_message = error_message
                    .find("Candidate: ")
                    .map(|pos| error_message[pos + 11..].to_string());

                let has_staged_files = !app.data.base.staged_files.is_empty();
                app.data.error_state = Some(ErrorState::new(
                    error_message,
                    has_staged_files,
                    candidate_message,
                ));
            }
            if let Some(ref state) = app.data.error_state {
                render_error(frame, state, frame.area());
            }
        }
    }

    // Render overlays (status bar, toasts)
    crate::tui::layout::render_overlays(frame, app);
}

/// Handle key input for the current screen based on app state.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    // Global key handler for Ctrl-c (Force Quit)
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.handle_app_msg(crate::tui::elm::AppMsg::Quit);
        return;
    }

    let app_msgs = match app.state {
        AppState::StagingSelection => {
            if let Some(ref mut state) = app.data.staging_state {
                if let Some(msg) = staging_key_to_message(key) {
                    let msgs = state.update(msg);
                    // Sync back to app
                    app.data.base.selected_indices = state.selected_indices.clone();
                    msgs
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        AppState::Dashboard => {
            if let Some(ref mut state) = app.data.dashboard_state {
                if let Some(msg) = dashboard_key_to_message(key, state.show_user_context_input) {
                    let msgs = state.update(msg);
                    app.data.base.multi_select_mode = state.multi_select_mode;
                    app.data.base.selected_indices = state.selected_indices.clone();
                    msgs
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        AppState::Generating => {
            if let Some(ref mut state) = app.data.generating_state_ui {
                if let Some(msg) = generating_key_to_message(key) {
                    state.update(msg)
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        AppState::Review => {
            // Defensive initialization - ensure state exists before handling keys
            if app.data.review_state.is_none() {
                if let Ok(message) =
                    CommitMessage::try_from(app.data.base.generated_message.to_string())
                {
                    app.data.review_state = Some(ReviewState::new(message));
                } else {
                    app.data
                        .base
                        .toasts
                        .error("Generated message is invalid".to_string());
                }
            }

            if let Some(ref mut state) = app.data.review_state {
                // Sync state with app data before processing key
                if let Ok(message) =
                    CommitMessage::try_from(app.data.base.generated_message.to_string())
                {
                    state.generated_message = message;
                } else {
                    app.data
                        .base
                        .toasts
                        .error("Generated message is invalid".to_string());
                    return;
                }
                state.review_action = app.data.base.review_action;

                let msg = if key.code == ratatui::crossterm::event::KeyCode::Enter {
                    Some(review_handle_enter(state))
                } else {
                    review_key_to_message(key)
                };

                if let Some(msg) = msg {
                    let msgs = state.update(msg);
                    // Sync back to app
                    app.data.base.review_action = state.review_action;
                    msgs
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        AppState::Editing => {
            if let Some(ref mut state) = app.data.editing_state {
                // Check for special keys first
                if let Some(msg) = editing_key_to_message(key, &app.ui.base.textarea) {
                    state.update(msg)
                } else {
                    // Handle regular text input
                    let before = app.ui.base.textarea.lines().join("\n");
                    let input = tui_textarea::Input::from(key);
                    app.ui.base.textarea.input(input.clone());
                    let after = app.ui.base.textarea.lines().join("\n");
                    let cursor = app.ui.base.textarea.cursor();

                    // Check if we should save a snapshot
                    if crate::tui::screens::editing::should_save_snapshot(
                        &input, &before, &after, state,
                    ) {
                        state.save_snapshot("edit");
                    }

                    // Update state
                    state.update_message(after, (cursor.0, cursor.1));
                    vec![]
                }
            } else {
                vec![]
            }
        }
        AppState::Error => {
            if let Some(ref mut state) = app.data.error_state {
                let msg = error_key_to_message(key);
                state.update(msg)
            } else {
                vec![]
            }
        }
    };

    // Process all app-level messages
    for msg in app_msgs {
        app.handle_app_msg(msg);
    }

    // Mark for redraw
    app.ui.should_redraw = true;
}
