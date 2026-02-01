use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap},
};

use christina_core::ReviewAction;
use christina_core::types::CommitMessage;

use crate::tui::{elm::*, theme::*};

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewState {
    pub generated_message: CommitMessage,
    pub review_action: ReviewAction,
}

impl ReviewState {
    pub fn new(generated_message: CommitMessage) -> Self {
        Self {
            generated_message,
            review_action: ReviewAction::Accept,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReviewMsg {
    NavigateLeft,
    NavigateRight,
    SelectAction(ReviewAction),
    Accept,
    Edit,
    Regenerate,
    Cancel,
    Quit,
}

impl Component for ReviewState {
    type Msg = ReviewMsg;

    fn update(&mut self, msg: Self::Msg) -> Vec<AppMsg> {
        match msg {
            ReviewMsg::NavigateLeft => {
                self.review_action = match self.review_action {
                    ReviewAction::Accept => ReviewAction::Cancel,
                    ReviewAction::Edit => ReviewAction::Accept,
                    ReviewAction::Regenerate => ReviewAction::Edit,
                    ReviewAction::Cancel => ReviewAction::Regenerate,
                };
                vec![]
            }
            ReviewMsg::NavigateRight => {
                self.review_action = match self.review_action {
                    ReviewAction::Accept => ReviewAction::Edit,
                    ReviewAction::Edit => ReviewAction::Regenerate,
                    ReviewAction::Regenerate => ReviewAction::Cancel,
                    ReviewAction::Cancel => ReviewAction::Accept,
                };
                vec![]
            }
            ReviewMsg::SelectAction(action) => {
                self.review_action = action;
                vec![]
            }
            ReviewMsg::Accept => {
                vec![AppMsg::CommitMessage(self.generated_message.clone())]
            }
            ReviewMsg::Edit => {
                vec![AppMsg::EditMessage(self.generated_message.clone())]
            }
            ReviewMsg::Regenerate => {
                vec![AppMsg::RegenerateMessage]
            }
            ReviewMsg::Cancel | ReviewMsg::Quit => {
                vec![AppMsg::Quit]
            }
        }
    }
}

pub fn key_to_message(key: KeyEvent) -> Option<ReviewMsg> {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => Some(ReviewMsg::NavigateLeft),
        KeyCode::Right | KeyCode::Char('l') => Some(ReviewMsg::NavigateRight),
        KeyCode::Char('1') => Some(ReviewMsg::SelectAction(ReviewAction::Accept)),
        KeyCode::Char('2') => Some(ReviewMsg::SelectAction(ReviewAction::Edit)),
        KeyCode::Char('3') => Some(ReviewMsg::SelectAction(ReviewAction::Regenerate)),
        KeyCode::Char('4') => Some(ReviewMsg::SelectAction(ReviewAction::Cancel)),
        KeyCode::Char('y') => Some(ReviewMsg::Accept),
        KeyCode::Char('e') => Some(ReviewMsg::Edit),
        KeyCode::Char('r') => Some(ReviewMsg::Regenerate),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(ReviewMsg::Quit)
        }
        KeyCode::Enter => {
            // Enter will be dispatched based on current action
            None
        }
        KeyCode::Esc | KeyCode::Char('q') => Some(ReviewMsg::Quit),
        _ => None,
    }
}

pub fn handle_enter(state: &ReviewState) -> ReviewMsg {
    match state.review_action {
        ReviewAction::Accept => ReviewMsg::Accept,
        ReviewAction::Edit => ReviewMsg::Edit,
        ReviewAction::Regenerate => ReviewMsg::Regenerate,
        ReviewAction::Cancel => ReviewMsg::Cancel,
    }
}

pub fn render_review(frame: &mut Frame, state: &ReviewState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, chunks[0]);
    render_content(frame, state, chunks[1]);
    render_footer(frame, chunks[2]);
}

fn render_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled("christina", Style::default().fg(TEXT).bold()),
        Span::styled(" / ", Style::default().fg(SURFACE1)),
        Span::styled("review", Style::default().fg(SUBTEXT0)),
    ])])
    .alignment(Alignment::Left);

    frame.render_widget(header, area);
}

fn render_content(frame: &mut Frame, state: &ReviewState, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(5)])
        .split(area);

    render_message(frame, state.generated_message.as_ref(), layout[0]);
    render_buttons(frame, state.review_action, layout[1]);
}

fn render_message(frame: &mut Frame, message: &str, area: Rect) {
    let message_lines: Vec<Line> = message
        .lines()
        .enumerate()
        .map(|(idx, line)| {
            let color = if idx == 0 { TEXT } else { SUBTEXT0 };
            Line::from(Span::styled(line, Style::default().fg(color)))
        })
        .collect();

    let message_widget = Paragraph::new(message_lines)
        .style(Style::default().fg(TEXT))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Span::styled(" Message ", Style::default().fg(SUBTEXT1)))
                .style(Style::default().fg(SURFACE1))
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(message_widget, area);
}

fn render_buttons(frame: &mut Frame, current_action: ReviewAction, area: Rect) {
    let actions = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    render_button(
        frame,
        "Accept",
        ReviewAction::Accept,
        current_action,
        actions[0],
    );
    render_button(
        frame,
        "Edit",
        ReviewAction::Edit,
        current_action,
        actions[1],
    );
    render_button(
        frame,
        "Regenerate",
        ReviewAction::Regenerate,
        current_action,
        actions[2],
    );
    render_button(
        frame,
        "Cancel",
        ReviewAction::Cancel,
        current_action,
        actions[3],
    );
}

fn render_button(
    frame: &mut Frame,
    label: &str,
    action: ReviewAction,
    current: ReviewAction,
    area: Rect,
) {
    let is_selected = action == current;

    let text_style = if is_selected {
        Style::default().fg(BASE).bg(ROSEWATER)
    } else {
        Style::default().fg(TEXT)
    };

    let border_style = if is_selected {
        Style::default().fg(ROSEWATER)
    } else {
        Style::default().fg(SURFACE0)
    };

    let button = Paragraph::new(Line::from(Span::styled(label, text_style)))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .style(border_style)
                .padding(Padding::vertical(1)),
        );

    frame.render_widget(button, area);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("←→", Style::default().fg(SUBTEXT0)),
        Span::styled(" navigate ", Style::default().fg(OVERLAY0)),
        Span::styled("enter", Style::default().fg(ROSEWATER)),
        Span::styled(" confirm ", Style::default().fg(OVERLAY0)),
        Span::styled("esc", Style::default().fg(SUBTEXT0)),
        Span::styled(" cancel", Style::default().fg(OVERLAY0)),
    ]))
    .alignment(Alignment::Center);

    frame.render_widget(footer, area);
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn commit_msg(s: &str) -> CommitMessage {
        CommitMessage::try_from(s.to_string())
            .unwrap_or_else(|e| panic!("test: invalid commit message: {}", e))
    }

    #[test]
    fn test_navigation() {
        let mut state = ReviewState::new(commit_msg("feat: test message"));
        assert_eq!(state.review_action, ReviewAction::Accept);

        state.update(ReviewMsg::NavigateRight);
        assert_eq!(state.review_action, ReviewAction::Edit);

        state.update(ReviewMsg::NavigateRight);
        assert_eq!(state.review_action, ReviewAction::Regenerate);

        state.update(ReviewMsg::NavigateRight);
        assert_eq!(state.review_action, ReviewAction::Cancel);

        state.update(ReviewMsg::NavigateRight);
        assert_eq!(state.review_action, ReviewAction::Accept);

        state.update(ReviewMsg::NavigateLeft);
        assert_eq!(state.review_action, ReviewAction::Cancel);
    }

    #[test]
    fn test_select_action() {
        let mut state = ReviewState::new(commit_msg("feat: test message"));

        state.update(ReviewMsg::SelectAction(ReviewAction::Regenerate));
        assert_eq!(state.review_action, ReviewAction::Regenerate);

        state.update(ReviewMsg::SelectAction(ReviewAction::Edit));
        assert_eq!(state.review_action, ReviewAction::Edit);
    }

    #[test]
    fn test_accept_message() {
        let mut state = ReviewState::new(commit_msg("feat: test commit message"));
        let msgs = state.update(ReviewMsg::Accept);

        assert_eq!(msgs.len(), 1);
        assert!(
            matches!(msgs[0], AppMsg::CommitMessage(ref msg) if msg == &commit_msg("feat: test commit message"))
        );
    }

    #[test]
    fn test_edit_message() {
        let mut state = ReviewState::new(commit_msg("feat: original message"));
        let msgs = state.update(ReviewMsg::Edit);

        assert_eq!(msgs.len(), 1);
        assert!(
            matches!(msgs[0], AppMsg::EditMessage(ref msg) if msg == &commit_msg("feat: original message"))
        );
    }

    #[test]
    fn test_regenerate() {
        let mut state = ReviewState::new(commit_msg("feat: test message"));
        let msgs = state.update(ReviewMsg::Regenerate);

        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], AppMsg::RegenerateMessage));
    }

    #[test]
    fn test_quit() {
        let mut state = ReviewState::new(commit_msg("feat: test message"));

        let msgs = state.update(ReviewMsg::Cancel);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], AppMsg::Quit));

        let msgs = state.update(ReviewMsg::Quit);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], AppMsg::Quit));
    }

    #[test]
    fn test_key_to_message() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let key = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(key_to_message(key), Some(ReviewMsg::NavigateLeft));

        let key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        assert_eq!(key_to_message(key), Some(ReviewMsg::Accept));

        let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
        assert_eq!(
            key_to_message(key),
            Some(ReviewMsg::SelectAction(ReviewAction::Accept))
        );
    }

    #[test]
    fn test_handle_enter() {
        let mut state = ReviewState::new(commit_msg("feat: test"));

        state.review_action = ReviewAction::Accept;
        assert_eq!(handle_enter(&state), ReviewMsg::Accept);

        state.review_action = ReviewAction::Edit;
        assert_eq!(handle_enter(&state), ReviewMsg::Edit);

        state.review_action = ReviewAction::Regenerate;
        assert_eq!(handle_enter(&state), ReviewMsg::Regenerate);

        state.review_action = ReviewAction::Cancel;
        assert_eq!(handle_enter(&state), ReviewMsg::Cancel);
    }
}
