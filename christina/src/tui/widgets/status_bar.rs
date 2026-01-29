use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::super::theme::*;
use christina_core::types::TokenCount;

/// Status bar data.
pub struct StatusBar<'a> {
    pub branch: &'a str,
    pub file_count: usize,
    pub token_usage: TokenCount,
    pub token_limit: TokenCount,
    pub is_loading: bool,
    pub active_profile: Option<&'a str>,
}

/// Render the status bar.
pub fn render_status_bar(frame: &mut Frame, area: Rect, status_bar: &StatusBar) {
    if area.height < 1 {
        return;
    }

    let mut spans = Vec::new();

    // Git branch
    spans.push(Span::styled(" ", Style::default().fg(ROSEWATER)));
    spans.push(Span::styled(
        status_bar.branch,
        Style::default().fg(ROSEWATER),
    ));
    spans.push(Span::styled(" │ ", Style::default().fg(SURFACE1)));

    // File counts
    spans.push(Span::styled("📄 ", Style::default().fg(BLUE)));
    spans.push(Span::styled(
        format!("{}", status_bar.file_count),
        Style::default().fg(TEXT),
    ));
    spans.push(Span::styled(" │ ", Style::default().fg(SURFACE1)));

    // Token usage
    let token_usage = status_bar.token_usage.get();
    let token_limit = status_bar.token_limit.get();
    let token_color = if token_usage > token_limit {
        RED
    } else if token_usage as f32 > token_limit as f32 * 0.8 {
        Color::Rgb(249, 226, 175) // Yellow
    } else {
        GREEN
    };

    spans.push(Span::styled("🔤 ", Style::default().fg(token_color)));
    spans.push(Span::styled(
        format!("{}/{}", token_usage, token_limit),
        Style::default().fg(TEXT),
    ));

    // Active profile
    if let Some(profile) = status_bar.active_profile {
        spans.push(Span::styled(" │ ", Style::default().fg(SURFACE1)));
        spans.push(Span::styled("⚙ ", Style::default().fg(BLUE)));
        spans.push(Span::styled(profile, Style::default().fg(TEXT)));
    }

    // Loading indicator
    if status_bar.is_loading {
        spans.push(Span::styled(" │ ", Style::default().fg(SURFACE1)));
        spans.push(Span::styled("⟳ ", Style::default().fg(ROSEWATER)));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Style::default().bg(SURFACE0).fg(TEXT));

    frame.render_widget(paragraph, area);
}
