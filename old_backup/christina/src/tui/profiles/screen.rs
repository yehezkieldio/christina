use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph},
};

use super::super::theme::*;
use super::app::{ModalType, ProfileApp};
use crate::tui::form::FormWidget;

/// Render the profile TUI
pub fn render(frame: &mut Frame, app: &mut ProfileApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Profile list
            Constraint::Length(4), // Footer
        ])
        .split(frame.area());

    // Header
    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled("christina", Style::default().fg(TEXT).bold()),
        Span::styled(" / ", Style::default().fg(SURFACE1)),
        Span::styled("profiles", Style::default().fg(SUBTEXT0)),
    ])])
    .alignment(Alignment::Left);

    frame.render_widget(header, chunks[0]);

    // Profile list
    render_profile_list(frame, app, chunks[1]);

    // Footer
    render_footer(frame, app, chunks[2]);

    // Render modals
    if app.modal.is_some() {
        render_modal(frame, app);
    }
}

/// Render the profile list
fn render_profile_list(frame: &mut Frame, app: &mut ProfileApp, area: Rect) {
    let items: Vec<ListItem> = app
        .profiles
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let is_selected = app.selected() == Some(idx);

            // Selection indicator
            let indicator = if is_selected { " › " } else { "   " };
            let indicator_style = Style::default().fg(ROSEWATER);

            // Active marker
            let active_marker = if item.is_active { "✓ " } else { "  " };
            let active_style = if item.is_active {
                Style::default().fg(GREEN)
            } else {
                Style::default().fg(OVERLAY0)
            };

            // Profile name
            let name_style = if is_selected {
                Style::default().fg(ROSEWATER).bold()
            } else {
                Style::default().fg(TEXT)
            };

            // Provider and model
            let info_style = Style::default().fg(SUBTEXT0);

            let line = Line::from(vec![
                Span::styled(indicator, indicator_style),
                Span::styled(active_marker, active_style),
                Span::styled(format!("{:<20}", item.profile.name), name_style),
                Span::styled(" | ", Style::default().fg(SURFACE1)),
                Span::styled(item.profile.provider.to_string(), info_style),
                Span::styled(" / ", Style::default().fg(SURFACE1)),
                Span::styled(item.profile.model.to_string(), info_style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let title = if app.profiles.is_empty() {
        " No Profiles "
    } else {
        " Profiles "
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SURFACE1))
            .title(title)
            .title_style(Style::default().fg(SUBTEXT0)),
    );

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

/// Render footer with keybindings and status
fn render_footer(frame: &mut Frame, app: &ProfileApp, area: Rect) {
    let status_line = if let Some(ref msg) = app.status_message {
        Line::from(vec![Span::styled(msg.as_str(), Style::default().fg(GREEN))])
    } else if let Some(idx) = app.selected()
        && idx < app.profiles.len()
    {
        let profile = &app.profiles[idx].profile;
        Line::from(vec![
            Span::styled("Tokens: ", Style::default().fg(OVERLAY0)),
            Span::styled(
                format!(
                    "{}↔{}",
                    profile.max_input_tokens.get(),
                    profile.max_output_tokens.get()
                ),
                Style::default().fg(SUBTEXT0),
            ),
            Span::styled(" | ", Style::default().fg(SURFACE1)),
            Span::styled("URL: ", Style::default().fg(OVERLAY0)),
            Span::styled(
                profile
                    .api_url
                    .as_ref()
                    .map(|url| url.as_str())
                    .unwrap_or("(default)"),
                Style::default().fg(SUBTEXT0).italic(),
            ),
        ])
    } else {
        Line::default()
    };

    let keybindings = Line::from(vec![
        Span::styled("↑↓", Style::default().fg(SUBTEXT0)),
        Span::styled(" navigate ", Style::default().fg(OVERLAY0)),
        Span::styled("n", Style::default().fg(BLUE)),
        Span::styled(" new ", Style::default().fg(OVERLAY0)),
        Span::styled("e", Style::default().fg(ROSEWATER)),
        Span::styled(" edit ", Style::default().fg(OVERLAY0)),
        Span::styled("c", Style::default().fg(BLUE)),
        Span::styled(" copy ", Style::default().fg(OVERLAY0)),
        Span::styled("d", Style::default().fg(RED)),
        Span::styled(" delete ", Style::default().fg(OVERLAY0)),
        Span::styled("s", Style::default().fg(GREEN)),
        Span::styled(" switch ", Style::default().fg(OVERLAY0)),
        Span::styled("q", Style::default().fg(SUBTEXT0)),
        Span::styled(" quit", Style::default().fg(OVERLAY0)),
    ]);

    let footer = Paragraph::new(vec![status_line, Line::default(), keybindings])
        .alignment(Alignment::Center);

    frame.render_widget(footer, area);
}

/// Render modal overlay
fn render_modal(frame: &mut Frame, app: &ProfileApp) {
    match app.modal {
        Some(ModalType::DeleteConfirm) => render_delete_confirm(frame, app),
        Some(ModalType::CreateProfile)
        | Some(ModalType::EditProfile)
        | Some(ModalType::DuplicateProfile) => render_form_modal(frame, app),
        None => {}
    }
}

/// Render delete confirmation dialog
fn render_delete_confirm(frame: &mut Frame, app: &ProfileApp) {
    let area = frame.area();
    let modal_width = 50.min(area.width.saturating_sub(4));
    let modal_height = 7;
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);
    frame.render_widget(Clear, modal_area);

    let profile_name = app
        .target_profile_idx
        .and_then(|idx| app.profiles.get(idx))
        .map(|p| p.profile.name.as_str())
        .unwrap_or("(unknown)");

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(RED))
        .title(" Confirm Delete ")
        .title_style(Style::default().fg(RED).bold())
        .style(Style::default().bg(BASE));

    frame.render_widget(block, modal_area);

    let inner = modal_area.inner(ratatui::layout::Margin::new(2, 1));
    let text = vec![
        Line::from(vec![
            Span::styled("Delete profile ", Style::default().fg(TEXT)),
            Span::styled(profile_name, Style::default().fg(ROSEWATER).bold()),
            Span::styled("?", Style::default().fg(TEXT)),
        ]),
        Line::default(),
        Line::from(vec![
            Span::styled("y", Style::default().fg(RED).bold()),
            Span::styled(" yes  ", Style::default().fg(OVERLAY0)),
            Span::styled("n", Style::default().fg(SUBTEXT0).bold()),
            Span::styled(" no", Style::default().fg(OVERLAY0)),
        ]),
    ];

    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    frame.render_widget(paragraph, inner);
}

/// Render form modal (create/edit/duplicate)
fn render_form_modal(frame: &mut Frame, app: &ProfileApp) {
    let area = frame.area();

    let Some(ref form) = app.form_state else {
        return;
    };

    let Some(ref profile) = app.edit_profile else {
        return;
    };

    let modal_width = 70.min(area.width.saturating_sub(4));
    let modal_height = 25.min(area.height.saturating_sub(4));
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);
    frame.render_widget(Clear, modal_area);

    let title = match app.modal {
        Some(ModalType::CreateProfile) => " Create Profile ",
        Some(ModalType::EditProfile) => " Edit Profile ",
        Some(ModalType::DuplicateProfile) => " Duplicate Profile ",
        _ => " Profile ",
    };

    // Render form widget
    let form_widget = FormWidget::new(form, profile, title);
    frame.render_widget(form_widget, modal_area);

    // Footer help
    let footer_area = Rect::new(
        modal_area.x + 1,
        modal_area.y + modal_area.height.saturating_sub(2),
        modal_area.width.saturating_sub(2),
        1,
    );

    let help_text = Line::from(vec![
        Span::styled("ctrl+s", Style::default().fg(GREEN)),
        Span::styled(" save  ", Style::default().fg(OVERLAY0)),
        Span::styled("esc", Style::default().fg(RED)),
        Span::styled(" cancel", Style::default().fg(OVERLAY0)),
    ]);

    let help = Paragraph::new(help_text).alignment(Alignment::Center);
    frame.render_widget(help, footer_area);
}
