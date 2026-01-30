use ratatui::{
    crossterm::event::KeyEvent,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use tui_textarea::{Input, Key, TextArea};

use crate::app::edit_history::{EditHistory, HistoryEntry};
use crate::tui::{elm::*, theme::*};
use christina_core::types::CommitMessage;

const HISTORY_CHAR_THRESHOLD: usize = 10;

/// Validate that a message follows Conventional Commits format
fn validate_conventional_commit(message: &str) -> bool {
    christina_core::validation::validate_commit_message(message, None).is_ok()
}

#[derive(Clone, Debug)]
pub struct EditingState {
    pub message: String,
    pub history: EditHistory,
    pub cursor: (usize, usize),
}

impl EditingState {
    pub fn new(message: String) -> Self {
        let mut history = EditHistory::new();
        history.initialize(&message);
        Self {
            message,
            history,
            cursor: (0, 0),
        }
    }

    pub fn update_message(&mut self, new_message: String, new_cursor: (usize, usize)) {
        self.message = new_message;
        self.cursor = new_cursor;
    }

    pub fn save_snapshot(&mut self, description: &str) {
        self.history
            .push(HistoryEntry::new(&self.message, self.cursor, description));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditingMsg {
    Undo,
    Redo,
    Save,
    Cancel,
}

impl Component for EditingState {
    type Msg = EditingMsg;

    fn update(&mut self, msg: Self::Msg) -> Vec<AppMsg> {
        match msg {
            EditingMsg::Undo => {
                if let Some(prev) = self.history.undo() {
                    self.message = prev.message.to_string();
                    self.cursor = prev.cursor;
                    vec![AppMsg::RestoreTextArea(self.message.clone(), self.cursor)]
                } else {
                    vec![]
                }
            }
            EditingMsg::Redo => {
                if let Some(next) = self.history.redo() {
                    self.message = next.message.to_string();
                    self.cursor = next.cursor;
                    vec![AppMsg::RestoreTextArea(self.message.clone(), self.cursor)]
                } else {
                    vec![]
                }
            }
            EditingMsg::Save => {
                let trimmed = self.message.trim();
                if trimmed.is_empty() {
                    vec![AppMsg::ShowToast(
                        "Cannot save empty message".to_string(),
                        ToastLevel::Warning,
                    )]
                } else if !validate_conventional_commit(trimmed) {
                    vec![AppMsg::ShowToast(
                        "Message does not follow Conventional Commits format (type(scope): description)".to_string(),
                        ToastLevel::Warning
                    )]
                } else {
                    self.save_snapshot("save");
                    match CommitMessage::try_from(self.message.clone()) {
                        Ok(commit_message) => vec![AppMsg::SaveEditedMessage(commit_message)],
                        Err(e) => vec![AppMsg::ShowToast(
                            format!("Invalid commit message: {}", e),
                            ToastLevel::Warning,
                        )],
                    }
                }
            }
            EditingMsg::Cancel => {
                vec![AppMsg::CancelEdit]
            }
        }
    }
}

pub fn key_to_message(key: KeyEvent, _textarea: &TextArea) -> Option<EditingMsg> {
    let input = Input::from(key);

    match (&input.key, input.ctrl, input.alt) {
        (Key::Char('z'), true, false) => Some(EditingMsg::Undo),
        (Key::Char('y'), true, false) | (Key::Char('Z'), true, false) => Some(EditingMsg::Redo),
        (Key::Char('s'), true, false) => Some(EditingMsg::Save),
        (Key::Enter, true, false) => Some(EditingMsg::Save),
        (Key::Esc, _, _) => Some(EditingMsg::Cancel),
        _ => None,
    }
}

pub fn should_save_snapshot(
    input: &Input,
    before: &str,
    after: &str,
    state: &EditingState,
) -> bool {
    match input.key {
        Key::Enter => true,
        Key::Char(' ') => true,
        Key::Backspace | Key::Delete => {
            before.len().saturating_sub(after.len()) >= HISTORY_CHAR_THRESHOLD
        }
        Key::Char(_) => {
            if let Some(current) = state.history.current() {
                let chars_since_snapshot = after.len().abs_diff(current.message.len());
                chars_since_snapshot >= HISTORY_CHAR_THRESHOLD
            } else {
                after.len() >= HISTORY_CHAR_THRESHOLD
            }
        }
        _ => false,
    }
}

pub fn render_editing(frame: &mut Frame, area: Rect, textarea: &TextArea) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(7),
            Constraint::Length(2),
        ])
        .split(area);

    render_header(frame, chunks[0]);
    frame.render_widget(textarea, chunks[1]);
    render_footer(frame, chunks[2]);
}

fn render_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled("christina", Style::default().fg(TEXT).bold()),
        Span::styled(" / ", Style::default().fg(SURFACE1)),
        Span::styled("edit", Style::default().fg(SUBTEXT0)),
    ])])
    .alignment(Alignment::Left);

    frame.render_widget(header, area);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("ctrl+z", Style::default().fg(SUBTEXT0)),
        Span::styled(" undo ", Style::default().fg(OVERLAY0)),
        Span::styled("ctrl+y", Style::default().fg(SUBTEXT0)),
        Span::styled(" redo ", Style::default().fg(OVERLAY0)),
        Span::styled("enter", Style::default().fg(ROSEWATER)),
        Span::styled(" save ", Style::default().fg(OVERLAY0)),
        Span::styled("esc", Style::default().fg(SUBTEXT0)),
        Span::styled(" cancel", Style::default().fg(OVERLAY0)),
    ]))
    .alignment(Alignment::Center);

    frame.render_widget(footer, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_message() {
        let mut state = EditingState::new("Initial".to_string());
        assert_eq!(state.message, "Initial");

        state.update_message("Updated".to_string(), (0, 7));
        assert_eq!(state.message, "Updated");
        assert_eq!(state.cursor, (0, 7));
    }

    #[test]
    fn test_undo_redo() {
        let mut state = EditingState::new("v1".to_string());

        state.update_message("v2".to_string(), (0, 2));
        state.save_snapshot("typing");

        state.update_message("v3".to_string(), (0, 2));
        state.save_snapshot("more typing");

        let msgs = state.update(EditingMsg::Undo);
        assert_eq!(state.message, "v2");
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], AppMsg::RestoreTextArea(_, _)));

        let msgs = state.update(EditingMsg::Redo);
        assert_eq!(state.message, "v3");
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_save() {
        let mut state = EditingState::new("feat: add new feature".to_string());

        let msgs = state.update(EditingMsg::Save);
        assert_eq!(msgs.len(), 1);
        assert!(
            matches!(msgs[0], AppMsg::SaveEditedMessage(ref msg) if msg.as_ref() == "feat: add new feature")
        );
    }

    #[test]
    fn test_save_empty() {
        let mut state = EditingState::new("   ".to_string());

        let msgs = state.update(EditingMsg::Save);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], AppMsg::ShowToast(_, ToastLevel::Warning)));
    }

    #[test]
    fn test_cancel() {
        let mut state = EditingState::new("Test".to_string());

        let msgs = state.update(EditingMsg::Cancel);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], AppMsg::CancelEdit));
    }

    #[test]
    fn test_snapshot_on_significant_change() {
        let state = EditingState::new("initial".to_string());

        let input = Input::from(ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::Char(' '),
            ratatui::crossterm::event::KeyModifiers::NONE,
        ));

        assert!(should_save_snapshot(&input, "before", "after", &state));
    }

    #[test]
    fn test_snapshot_on_enter() {
        let state = EditingState::new("initial".to_string());

        let input = Input::from(ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::Enter,
            ratatui::crossterm::event::KeyModifiers::NONE,
        ));

        assert!(should_save_snapshot(&input, "before", "after", &state));
    }
}
