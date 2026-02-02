use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use super::super::theme::*;
use super::app::ConfigApp;
use crate::config::ConfigTab;
use crate::tui::form::FormWidget;

pub fn render(frame: &mut Frame, app: &mut ConfigApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    render_header(frame, chunks[0], app);
    render_tabs(frame, chunks[1], app);
    render_form(frame, chunks[2], app);
    render_footer(frame, chunks[3], app);
}

fn render_header(frame: &mut Frame, area: Rect, app: &ConfigApp) {
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

    frame.render_widget(header, area);
}

fn render_tabs(frame: &mut Frame, area: Rect, app: &ConfigApp) {
    let tab_chunks = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(34),
        Constraint::Percentage(33),
    ])
    .split(area);

    for (i, tab) in ConfigTab::ALL.iter().enumerate() {
        let is_active = *tab == app.current_tab;
        let (style, title) = if is_active {
            (
                Style::default()
                    .fg(BLUE)
                    .bg(SURFACE0)
                    .add_modifier(Modifier::BOLD),
                format!(" {} {} ", i + 1, tab.name()),
            )
        } else {
            (
                Style::default().fg(OVERLAY0),
                format!(" {} {} ", i + 1, tab.name()),
            )
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(if is_active {
                Style::default().fg(BLUE)
            } else {
                Style::default().fg(SURFACE1)
            })
            .title(title)
            .title_style(style);

        frame.render_widget(block, tab_chunks[i]);
    }
}

fn render_form(frame: &mut Frame, area: Rect, app: &ConfigApp) {
    let modified_suffix = if app.has_changes { " [modified]" } else { "" };
    let title = format!(" {} Settings{} ", app.current_tab.name(), modified_suffix);

    if app.current_form_state().fields().is_empty() {
        let placeholder = Paragraph::new(Line::from(vec![Span::styled(
            "No experimental settings are available.",
            Style::default().fg(SUBTEXT0).italic(),
        )]))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(title)
                .title_style(Style::default().fg(SUBTEXT0)),
        );
        frame.render_widget(placeholder, area);
    } else {
        let form = FormWidget::new(app.current_form_state(), &app.config, &title);
        frame.render_widget(form, area);
    }
}

fn render_footer(frame: &mut Frame, area: Rect, app: &ConfigApp) {
    let help_text = app.current_tab.description();
    let status_line = if let Some(ref msg) = app.status_message {
        Line::from(vec![Span::styled(msg.as_str(), Style::default().fg(GREEN))])
    } else {
        Line::from(vec![Span::styled(
            help_text,
            Style::default().fg(SUBTEXT0).italic(),
        )])
    };

    let keybindings = Line::from(vec![
        Span::styled("1/2/3", Style::default().fg(SUBTEXT0)),
        Span::styled(" tabs ", Style::default().fg(OVERLAY0)),
        Span::styled("↑↓", Style::default().fg(SUBTEXT0)),
        Span::styled(" nav ", Style::default().fg(OVERLAY0)),
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
    frame.render_widget(footer, area);
}
