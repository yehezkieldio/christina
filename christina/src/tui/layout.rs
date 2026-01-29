use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

use super::widgets::{StatusBar, render_status_bar, render_toasts};
use crate::app::App;
use crate::app::state::GenerationState;

/// Render status bar and toasts on top of the current screen
pub fn render_overlays(frame: &mut Frame, app: &App) {
    let status_bar_area = Rect {
        x: 0,
        y: frame.area().height.saturating_sub(1),
        width: frame.area().width,
        height: 1,
    };

    let status_bar = StatusBar {
        branch: app
            .app_context
            .branch_name
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("(no branch)"),
        file_count: app.data.base.staged_files.len(),
        token_usage: app.data.base.token_count,
        token_limit: app.app_context.config.max_input_tokens,
        is_loading: matches!(app.generation_state, GenerationState::Running { .. }),
        active_profile: app.app_context.config.profiles.active.as_deref(),
    };

    render_status_bar(frame, status_bar_area, &status_bar);

    render_toasts(frame, &app.data.base.toasts);
}

/// Create a centered rectangle with the given percentage of width and height
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
