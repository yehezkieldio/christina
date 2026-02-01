use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::tui::elm::{AppMsg, Component};
use crate::tui::theme::*;
use christina_core::AppState;

// =============================================================================
// Model (State)
// =============================================================================

/// Error screen state - pure data structure
pub struct ErrorState {
    /// The error message to display
    pub error_message: String,
    /// Whether there are staged files (affects navigation)
    pub has_staged_files: bool,
    /// Candidate message (if error was due to validation failure)
    pub candidate_message: Option<String>,
}

impl ErrorState {
    pub fn new(
        error_message: String,
        has_staged_files: bool,
        candidate_message: Option<String>,
    ) -> Self {
        Self {
            error_message,
            has_staged_files,
            candidate_message,
        }
    }
}

// =============================================================================
// Messages
// =============================================================================

/// Messages for error screen interactions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorMsg {
    /// Dismiss error and return to previous screen
    Dismiss,
    /// Retry (same as dismiss, but explicit intent)
    Retry,
    /// Use the candidate message as is (edit mode)
    UseAsIs,
    /// Quit application
    Quit,
}

// =============================================================================
// Update (Pure State Transitions)
// =============================================================================

impl Component for ErrorState {
    type Msg = ErrorMsg;

    fn update(&mut self, msg: Self::Msg) -> Vec<AppMsg> {
        match msg {
            ErrorMsg::Dismiss | ErrorMsg::Retry => {
                // Navigate back to appropriate state based on context
                let next_state = if self.has_staged_files {
                    AppState::Dashboard
                } else {
                    AppState::StagingSelection
                };
                vec![AppMsg::Navigate(next_state)]
            }

            ErrorMsg::UseAsIs => {
                if let Some(ref candidate) = self.candidate_message {
                    vec![AppMsg::EditRawMessage(candidate.clone())]
                } else {
                    vec![]
                }
            }

            ErrorMsg::Quit => {
                vec![AppMsg::Quit]
            }
        }
    }
}

// =============================================================================
// View (Pure Rendering)
// =============================================================================

/// Render the error screen
pub fn render_error(frame: &mut Frame, state: &ErrorState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Content (modal)
            Constraint::Length(2), // Footer
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Header
    render_header(frame, chunks[0]);

    // Error modal
    render_error_modal(frame, state, chunks[1]);

    // Footer
    render_footer_with_state(frame, state, chunks[2]);
}

fn render_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled("christina", Style::default().fg(TEXT).bold()),
        Span::styled(" / ", Style::default().fg(SURFACE1)),
        Span::styled("error", Style::default().fg(RED)),
    ])])
    .alignment(Alignment::Left);

    frame.render_widget(header, area);
}

fn render_error_modal(frame: &mut Frame, state: &ErrorState, area: Rect) {
    // Center the modal
    let modal_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(area)[1];

    let modal_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(modal_area)[1];

    let mut instructions = "Press 'q' to quit, 'r' to retry".to_string();
    if state.candidate_message.is_some() {
        instructions.push_str(", 'u' to use as is");
    }
    instructions.push_str(", or any other key to dismiss");

    let error_content = vec![
        Line::from(""),
        Line::from(Span::styled("⚠ Error", Style::default().fg(RED).bold())),
        Line::from(""),
        Line::from(Span::styled(
            &state.error_message,
            Style::default().fg(TEXT),
        )),
        Line::from(""),
        Line::from(Span::styled(
            instructions,
            Style::default().fg(SUBTEXT0).italic(),
        )),
    ];

    let error_modal = Paragraph::new(error_content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(RED))
                .title(" Error ")
                .title_style(Style::default().fg(RED).bold()),
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });

    frame.render_widget(error_modal, modal_area);
}

fn render_footer_with_state(frame: &mut Frame, state: &ErrorState, area: Rect) {
    let mut spans = vec![
        Span::styled("q", Style::default().fg(ROSEWATER)),
        Span::styled(" quit ", Style::default().fg(OVERLAY0)),
        Span::styled("r", Style::default().fg(ROSEWATER)),
        Span::styled(" retry ", Style::default().fg(OVERLAY0)),
    ];

    if state.candidate_message.is_some() {
        spans.push(Span::styled("u", Style::default().fg(ROSEWATER)));
        spans.push(Span::styled(" use as is ", Style::default().fg(OVERLAY0)));
    }

    spans.push(Span::styled("other", Style::default().fg(SUBTEXT0)));
    spans.push(Span::styled(" dismiss", Style::default().fg(OVERLAY0)));

    let footer = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);

    frame.render_widget(footer, area);
}

// =============================================================================
// Key Event Mapping
// =============================================================================

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Map key events to error messages
pub fn key_to_message(key: KeyEvent) -> ErrorMsg {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => ErrorMsg::Quit,
        KeyCode::Char('q') | KeyCode::Esc => ErrorMsg::Quit,
        KeyCode::Char('r') => ErrorMsg::Retry,
        KeyCode::Char('u') => ErrorMsg::UseAsIs,
        _ => ErrorMsg::Dismiss, // Any other key dismisses
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[expect(
    clippy::panic,
    reason = "test assertions use panic for failure reporting"
)]
mod tests {
    use super::*;

    #[test]
    fn test_error_dismiss_with_staged_files() {
        let mut state = ErrorState::new("Test error".to_string(), true, None);

        let msgs = state.update(ErrorMsg::Dismiss);

        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            AppMsg::Navigate(AppState::Dashboard) => (),
            other => panic!("Expected Navigate to Dashboard, got {:?}", other),
        }
    }

    #[test]
    fn test_error_dismiss_without_staged_files() {
        let mut state = ErrorState::new("Test error".to_string(), false, None);

        let msgs = state.update(ErrorMsg::Dismiss);

        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            AppMsg::Navigate(AppState::StagingSelection) => (),
            other => panic!("Expected Navigate to StagingSelection, got {:?}", other),
        }
    }

    #[test]
    fn test_error_retry() {
        let mut state = ErrorState::new("Test error".to_string(), true, None);

        let msgs = state.update(ErrorMsg::Retry);

        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            AppMsg::Navigate(AppState::Dashboard) => (),
            other => panic!("Expected Navigate to Dashboard, got {:?}", other),
        }
    }

    #[test]
    fn test_error_quit() {
        let mut state = ErrorState::new("Test error".to_string(), false, None);

        let msgs = state.update(ErrorMsg::Quit);

        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], AppMsg::Quit));
    }

    #[test]
    fn test_error_use_as_is() {
        let mut state = ErrorState::new(
            "Invalid message".to_string(),
            true,
            Some("chore: invalid".to_string()),
        );

        let msgs = state.update(ErrorMsg::UseAsIs);

        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            AppMsg::EditRawMessage(msg) => assert_eq!(msg, "chore: invalid"),
            other => panic!("Expected EditRawMessage, got {:?}", other),
        }
    }

    #[test]
    fn test_key_to_message_quit() {
        let key = KeyEvent::from(KeyCode::Char('q'));
        assert_eq!(key_to_message(key), ErrorMsg::Quit);
    }

    #[test]
    fn test_key_to_message_retry() {
        let key = KeyEvent::from(KeyCode::Char('r'));
        assert_eq!(key_to_message(key), ErrorMsg::Retry);
    }

    #[test]
    fn test_key_to_message_dismiss() {
        let key = KeyEvent::from(KeyCode::Enter);
        assert_eq!(key_to_message(key), ErrorMsg::Dismiss);
    }
}
