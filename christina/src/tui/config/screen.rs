use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::super::theme::*;
use super::app::ConfigApp;
use crate::tui::form::FormWidget;

pub fn render(frame: &mut Frame, app: &mut ConfigApp) {
    let area = frame.area();

    // Calculate available space for form (accounting for header and footer)
    let header_height = 3u16;
    let footer_height = 3u16;
    let form_height = area
        .height
        .saturating_sub(header_height + footer_height + 4); // 4 for margins

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(header_height), // Header
            Constraint::Length(form_height),   // Form (scrollable)
            Constraint::Length(footer_height), // Footer
        ])
        .split(area);

    // Update visible rows in form state based on available space
    // Account for: borders (2), title (1), help area (3) = ~6 lines overhead
    let visible_field_rows = form_height.saturating_sub(10) as usize;
    app.form_state.set_visible_rows(visible_field_rows.max(5));

    // Header / breadcrumb
    let active_profile = app.config.profiles.active.as_deref().unwrap_or("none");
    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled("christina", Style::default().fg(TEXT).bold()),
        Span::styled(" / ", Style::default().fg(SURFACE1)),
        Span::styled("config", Style::default().fg(SUBTEXT0)),
        Span::styled(
            format!(" (Active Profile: {})", active_profile),
            Style::default().fg(OVERLAY0).italic(),
        ),
    ])])
    .alignment(Alignment::Left);

    frame.render_widget(header, chunks[0]);

    // Form widget with sections
    let title = if app.has_changes {
        " Settings [modified] "
    } else {
        " Settings "
    };

    let form = FormWidget::new(&app.form_state, &app.config, title);
    frame.render_widget(form, chunks[1]);

    // Footer with keybindings and status
    let status_line = if let Some(ref msg) = app.status_message {
        Line::from(vec![Span::styled(msg.as_str(), Style::default().fg(GREEN))])
    } else {
        Line::from(vec![Span::styled(
            "Use ↑↓ to navigate fields, Tab for sections",
            Style::default().fg(OVERLAY0).italic(),
        )])
    };

    let keybindings = Line::from(vec![
        Span::styled("↑↓", Style::default().fg(SUBTEXT0)),
        Span::styled(" navigate ", Style::default().fg(OVERLAY0)),
        Span::styled("tab", Style::default().fg(SUBTEXT0)),
        Span::styled(" sections ", Style::default().fg(OVERLAY0)),
        Span::styled("enter", Style::default().fg(ROSEWATER)),
        Span::styled(" edit ", Style::default().fg(OVERLAY0)),
        Span::styled("ctrl+s", Style::default().fg(GREEN)),
        Span::styled(" save ", Style::default().fg(OVERLAY0)),
        Span::styled("ctrl+p", Style::default().fg(BLUE)),
        Span::styled(" profiles ", Style::default().fg(OVERLAY0)),
        Span::styled("q", Style::default().fg(SUBTEXT0)),
        Span::styled(" quit", Style::default().fg(OVERLAY0)),
    ]);

    let footer = Paragraph::new(vec![status_line, keybindings]).alignment(Alignment::Center);

    frame.render_widget(footer, chunks[2]);
}
