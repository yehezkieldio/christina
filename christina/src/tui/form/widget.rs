use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget, Wrap},
};

use crate::tui::form::{Editable, FieldType};

use super::state::{FormMode, FormState, SECTIONS};
use crate::tui::theme::{BLUE, OVERLAY0, RED, SUBTEXT0, TEXT};

/// Form widget for rendering editable forms with sections
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
        // Split area into tabs and content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Section tabs
                Constraint::Min(8),    // Field list
                Constraint::Length(3), // Help/error area
            ])
            .split(area);

        // Render section tabs
        self.render_section_tabs(chunks[0], buf);

        // Render field list (scrollable)
        self.render_fields(chunks[1], buf);

        // Render help or error
        if let Some(error) = &self.state.error {
            self.render_error(error, chunks[2], buf);
        } else if let Some(field) = self.state.current_field() {
            self.render_help(&field.help, chunks[2], buf);
        }
    }
}

impl<T: Editable> FormWidget<'_, T> {
    fn render_section_tabs(&self, area: Rect, buf: &mut Buffer) {
        let current_section_idx = self.state.current_section_index();
        let _current_section_key = self.state.current_section_key();

        // Calculate width for each tab
        let total_width = area.width as usize;
        let tab_count = SECTIONS.len();
        let tab_width = total_width / tab_count;

        for (i, (key, name)) in SECTIONS.iter().enumerate() {
            let is_active = i == current_section_idx;
            let x = area.x + (i as u16 * tab_width as u16);
            let tab_area = Rect {
                x,
                y: area.y,
                width: tab_width as u16,
                height: area.height,
            };

            // Clear the tab area
            Clear.render(tab_area, buf);

            let style = if is_active {
                Style::default().fg(BLUE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(OVERLAY0)
            };

            let border_style = if is_active {
                Style::default().fg(BLUE)
            } else {
                Style::default().fg(OVERLAY0)
            };

            // Count fields in this section
            let field_count = self
                .state
                .all_fields()
                .iter()
                .filter(|f| f.section == Some(*key))
                .count();

            let display_name = format!("{} {}", name, field_count);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(format!(
                    " {} [{}] ",
                    if is_active { "▸" } else { " " },
                    i + 1
                ))
                .title_style(style);

            let inner = block.inner(tab_area);
            block.render(tab_area, buf);

            // Center the text in the tab
            let text = Paragraph::new(display_name)
                .style(style)
                .alignment(ratatui::layout::Alignment::Center);
            text.render(inner, buf);
        }
    }

    fn render_fields(&self, area: Rect, buf: &mut Buffer) {
        let section_fields = self.state.current_section_fields();
        let scroll_offset = self.state.scroll_offset();
        let visible_rows = self.state.visible_rows().min(area.height as usize);

        // Get visible slice of fields
        let visible_fields: Vec<_> = section_fields
            .iter()
            .skip(scroll_offset)
            .take(visible_rows)
            .enumerate()
            .collect();

        let items: Vec<ListItem> = visible_fields
            .iter()
            .map(|(relative_idx, field)| {
                let absolute_idx = scroll_offset + relative_idx;
                let is_current = absolute_idx == self.state.cursor;
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
                    if is_editing {
                        "▶ "
                    } else {
                        "› "
                    }
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

        // Build title with section name and scroll indicators
        let section_name = self.state.current_section_name();
        let total_fields = section_fields.len();
        let start_idx = scroll_offset + 1;
        let end_idx = (scroll_offset + visible_rows).min(total_fields);

        let scroll_indicator = if total_fields > visible_rows {
            format!(" [{}-{} of {}]", start_idx, end_idx, total_fields)
        } else {
            String::new()
        };

        let title = format!("{} {}{}", self.title, section_name, scroll_indicator);

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(OVERLAY0))
                .title(title)
                .title_style(Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
        );

        Widget::render(list, area, buf);

        // Render scroll indicators if needed
        if total_fields > visible_rows {
            if scroll_offset > 0 {
                // Up arrow at top
                let up_span = Span::styled("▲", Style::default().fg(OVERLAY0));
                let up_line = Line::from(vec![up_span]);
                buf.set_line(area.right() - 2, area.y + 1, &up_line, area.width);
            }
            if end_idx < total_fields {
                // Down arrow at bottom
                let down_span = Span::styled("▼", Style::default().fg(OVERLAY0));
                let down_line = Line::from(vec![down_span]);
                buf.set_line(area.right() - 2, area.bottom() - 2, &down_line, area.width);
            }
        }
    }

    fn render_help(&self, help: &str, area: Rect, buf: &mut Buffer) {
        let help_text = if self.state.mode == FormMode::Editing {
            format!("{}  [Enter] save • [Esc] cancel", help)
        } else {
            format!(
                "{}  [Enter] edit • [↑↓] navigate • [Tab] sections • [Space] toggle boolean",
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
