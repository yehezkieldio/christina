use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::super::theme::*;
use super::app::ConfigApp;
use crate::tui::form::FormWidget;

pub fn render(frame: &mut Frame, app: &mut ConfigApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Form
            Constraint::Length(3), // Footer
        ])
        .split(frame.area());

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

    // Form widget
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
            "Use ↑↓ to navigate, Enter to edit, Space for booleans",
            Style::default().fg(OVERLAY0).italic(),
        )])
    };

    let keybindings = Line::from(vec![
        Span::styled("↑↓", Style::default().fg(SUBTEXT0)),
        Span::styled(" navigate ", Style::default().fg(OVERLAY0)),
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
