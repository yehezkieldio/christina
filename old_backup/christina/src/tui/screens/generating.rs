use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::{elm::*, layout::centered_rect, theme::*};

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratingState {
    pub spinner_idx: usize,
    pub stage: String,
}

impl GeneratingState {
    pub fn new() -> Self {
        Self {
            spinner_idx: 0,
            stage: "Preparing...".to_string(),
        }
    }

    pub fn tick(&mut self) {
        const SPINNER_CHARS_COUNT: usize = 10;
        self.spinner_idx = (self.spinner_idx + 1) % SPINNER_CHARS_COUNT;
    }

    pub fn set_stage(&mut self, stage: String) {
        self.stage = stage;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GeneratingMsg {
    Cancel,
}

impl Component for GeneratingState {
    type Msg = GeneratingMsg;

    fn update(&mut self, msg: Self::Msg) -> Vec<AppMsg> {
        match msg {
            GeneratingMsg::Cancel => {
                vec![AppMsg::CancelGeneration]
            }
        }
    }
}

pub fn key_to_message(key: KeyEvent) -> Option<GeneratingMsg> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Some(GeneratingMsg::Cancel),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(GeneratingMsg::Cancel)
        }
        _ => None,
    }
}

pub fn render_generating(frame: &mut Frame, state: &GeneratingState, area: Rect) {
    let popup_area = centered_rect(50, 20, area);

    frame.render_widget(Clear, popup_area);

    const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let spinner = SPINNER_CHARS[state.spinner_idx];

    let popup_text = vec![
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{spinner} "), Style::default().fg(ROSEWATER)),
            Span::styled(&state.stage, Style::default().fg(TEXT)),
        ]),
        Line::from(""),
    ];

    let popup = Paragraph::new(popup_text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(SURFACE1)),
        );

    frame.render_widget(popup, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_tick() {
        let mut state = GeneratingState::new();
        assert_eq!(state.spinner_idx, 0);

        state.tick();
        assert_eq!(state.spinner_idx, 1);

        for _ in 0..9 {
            state.tick();
        }
        assert_eq!(state.spinner_idx, 0);
    }

    #[test]
    fn test_cancel() {
        let mut state = GeneratingState::new();

        let msgs = state.update(GeneratingMsg::Cancel);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], AppMsg::CancelGeneration));
    }

    #[test]
    fn test_key_to_message() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(key_to_message(key), Some(GeneratingMsg::Cancel));

        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(key_to_message(key), Some(GeneratingMsg::Cancel));

        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key_to_message(key), Some(GeneratingMsg::Cancel));

        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(key_to_message(key), None);
    }
}
