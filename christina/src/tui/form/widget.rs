use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget, Wrap},
};

use crate::tui::form::{Editable, FieldType};

use super::state::{FormMode, FormState};
use crate::tui::theme::{BLUE, OVERLAY0, RED, SUBTEXT0, TEXT};

/// Form widget for rendering editable forms
pub struct FormWidget<'a, T: Editable> {
    state: &'a FormState,
    editable: &'a T,
    title: &'a str,
}

impl<'a, T: Editable> FormWidget<'a, T> {
    pub fn new(state: &'a FormState, editable: &'a T, title: &'a str) -> Self {
        Self {
            state,
            editable,
            title,
        }
    }
}

impl<T: Editable> Widget for FormWidget<'_, T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Split into field list and help/error area
        let chunks = Layout::vertical([Constraint::Min(10), Constraint::Length(3)]).split(area);

        // Render field list
        self.render_fields(chunks[0], buf);

        // Render help or error
        if let Some(error) = &self.state.error {
            self.render_error(error, chunks[1], buf);
        } else if let Some(field) = self.state.current_field() {
            self.render_help(&field.help, chunks[1], buf);
        }
    }
}

impl<T: Editable> FormWidget<'_, T> {
    fn render_fields(&self, area: Rect, buf: &mut Buffer) {
        let fields = self.state.fields();
        let items: Vec<ListItem> = fields
            .iter()
            .enumerate()
            .map(|(i, field)| {
                let is_current = i == self.state.cursor;
                let is_editing = is_current && self.state.mode == FormMode::Editing;

                let value = if is_editing {
                    &self.state.edit_buffer
                } else {
                    &self.editable.get_field(&field.key).unwrap_or_default()
                };

                let value_display = match &field.field_type {
                    FieldType::Secret if !is_editing => "••••••••".to_string(),
                    FieldType::Boolean => {
                        if value == "true" { "✓ Yes" } else { "✗ No" }.to_string()
                    }
                    _ => {
                        if value.is_empty() {
                            "<empty>".to_string()
                        } else {
                            value.to_string()
                        }
                    }
                };

                let (label_style, value_style) = if field.read_only {
                    (Style::default().fg(OVERLAY0), Style::default().fg(OVERLAY0))
                } else if is_editing {
                    (
                        Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                        Style::default().fg(BLUE).add_modifier(Modifier::UNDERLINED),
                    )
                } else if is_current {
                    (
                        Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                        Style::default().fg(BLUE),
                    )
                } else {
                    (Style::default().fg(TEXT), Style::default().fg(TEXT))
                };

                let marker = if is_current {
                    if is_editing { "▶ " } else { "› " }
                } else {
                    "  "
                };

                let line = Line::from(vec![
                    Span::styled(marker, Style::default().fg(BLUE)),
                    Span::styled(format!("{:<25}", field.label), label_style),
                    Span::raw(" "),
                    Span::styled(value_display, value_style),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(OVERLAY0))
                .title(self.title)
                .title_style(Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
        );

        Widget::render(list, area, buf);
    }

    fn render_help(&self, help: &str, area: Rect, buf: &mut Buffer) {
        let help_text = if self.state.mode == FormMode::Editing {
            format!("{}  [Enter] save • [Esc] cancel", help)
        } else {
            format!(
                "{}  [Enter] edit • [↑↓] navigate • [Space] toggle boolean",
                help
            )
        };

        let paragraph = Paragraph::new(help_text)
            .style(Style::default().fg(SUBTEXT0))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(OVERLAY0))
                    .title("Help"),
            );

        Widget::render(paragraph, area, buf);
    }

    fn render_error(&self, error: &str, area: Rect, buf: &mut Buffer) {
        let paragraph = Paragraph::new(error)
            .style(Style::default().fg(RED))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(RED))
                    .title("Error")
                    .title_style(Style::default().fg(RED).add_modifier(Modifier::BOLD)),
            );

        Widget::render(paragraph, area, buf);
    }
}
