use compact_str::CompactString;
use parking_lot::Mutex;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
    Frame,
};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::super::theme::*;

/// Toast notification level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    /// Informational message.
    Info,
    /// Warning message.
    Warning,
}

impl ToastLevel {
    pub fn color(&self) -> Color {
        match self {
            ToastLevel::Info => BLUE,
            ToastLevel::Warning => Color::Rgb(249, 226, 175), // Yellow
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ToastLevel::Info => "ℹ",
            ToastLevel::Warning => "⚠",
        }
    }
}

/// A single toast notification.
#[derive(Debug, Clone)]
pub struct Toast {
    /// The message to display.
    pub message: CompactString,
    /// The notification level.
    pub level: ToastLevel,
    /// When the toast was created.
    pub created_at: Instant,
    /// How long the toast should be visible.
    pub duration: Duration,
}

impl Toast {
    /// Create a new toast.
    pub fn new(message: impl Into<CompactString>, level: ToastLevel) -> Self {
        Self {
            message: message.into(),
            level,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        }
    }

    #[cfg(test)]
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn info(message: impl Into<CompactString>) -> Self {
        Self::new(message, ToastLevel::Info)
    }

    pub fn warning(message: impl Into<CompactString>) -> Self {
        Self::new(message, ToastLevel::Warning)
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.duration
    }

    /// Get the remaining time as a fraction (1.0 = full, 0.0 = expired).
    pub fn remaining_fraction(&self) -> f32 {
        let elapsed = self.created_at.elapsed().as_secs_f32();
        let total = self.duration.as_secs_f32();
        (1.0 - elapsed / total).max(0.0)
    }
}

/// Manager for toast notifications.
#[derive(Debug, Default)]
pub struct ToastManager {
    toasts: Mutex<VecDeque<Toast>>,
    history: Mutex<VecDeque<Toast>>,
    max_visible: usize,
    max_history: usize,
}

impl ToastManager {
    pub fn new() -> Self {
        Self {
            toasts: Mutex::new(VecDeque::new()),
            history: Mutex::new(VecDeque::new()),
            max_visible: 3,
            max_history: 100,
        }
    }

    pub fn push(&self, toast: Toast) {
        // Add to active toasts
        let mut toasts = self.toasts.lock();
        toasts.push_back(toast.clone());

        // Limit total active toasts
        while toasts.len() > 10 {
            toasts.pop_front();
        }
        drop(toasts);

        // Add to history
        let mut history = self.history.lock();
        history.push_back(toast);

        // Limit history size
        while history.len() > self.max_history {
            history.pop_front();
        }
    }

    pub fn info(&self, message: impl Into<CompactString>) {
        self.push(Toast::info(message));
    }

    pub fn warning(&self, message: impl Into<CompactString>) {
        self.push(Toast::warning(message));
    }

    /// Remove expired toasts and return visible ones.
    pub fn get_visible(&self) -> Vec<Toast> {
        let mut toasts = self.toasts.lock();

        // Remove expired
        toasts.retain(|t| !t.is_expired());

        // Return visible subset
        toasts.iter().take(self.max_visible).cloned().collect()
    }

    pub fn update(&self) {
        let mut toasts = self.toasts.lock();
        toasts.retain(|t| !t.is_expired());
    }

    #[cfg(test)]
    pub fn clear_history(&self) {
        let mut history = self.history.lock();
        history.clear();
    }
}

pub struct ToastWidget<'a> {
    toasts: &'a [Toast],
}

impl<'a> ToastWidget<'a> {
    pub fn new(toasts: &'a [Toast]) -> Self {
        Self { toasts }
    }
}

impl Widget for ToastWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.toasts.is_empty() || area.height < 3 {
            return;
        }

        // Render toasts from bottom-right
        // Calculate width dynamically based on message length
        // Min: 30, Max: 80% of available width or 70 chars
        let max_message_len = self
            .toasts
            .iter()
            .map(|t| t.message.len() + 4) // +4 for icon and padding
            .max()
            .unwrap_or(30);
        let toast_width = (max_message_len as u16)
            .max(30)
            .min(area.width * 4 / 5)
            .min(70);
        let mut y_offset = area.height.saturating_sub(1);

        for toast in self.toasts.iter().rev() {
            if y_offset < 3 {
                break;
            }

            let toast_height = 3;
            let toast_area = Rect::new(
                area.x + area.width.saturating_sub(toast_width + 1),
                area.y + y_offset.saturating_sub(toast_height),
                toast_width,
                toast_height,
            );

            // Clear background
            Clear.render(toast_area, buf);

            // Render toast
            let icon = toast.level.icon();
            let color = toast.level.color();
            let remaining = toast.remaining_fraction();

            let content = Line::from(vec![
                Span::styled(format!("{} ", icon), Style::default().fg(color)),
                Span::styled(toast.message.as_str(), Style::default().fg(TEXT)),
            ]);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(color));

            let paragraph = Paragraph::new(content)
                .block(block)
                .alignment(Alignment::Left);

            paragraph.render(toast_area, buf);

            // Progress bar at bottom
            if toast_area.height >= 3 {
                let progress_y = toast_area.y + toast_area.height - 1;
                let progress_width = ((toast_area.width - 2) as f32 * remaining) as u16;

                for x in toast_area.x + 1..toast_area.x + 1 + progress_width {
                    if let Some(cell) = buf.cell_mut((x, progress_y)) {
                        cell.set_char('─').set_fg(color);
                    }
                }
            }

            y_offset = y_offset.saturating_sub(toast_height + 1);
        }
    }
}

pub fn render_toasts(frame: &mut Frame, manager: &ToastManager) {
    let toasts = manager.get_visible();
    if toasts.is_empty() {
        return;
    }

    let widget = ToastWidget::new(&toasts);
    frame.render_widget(widget, frame.area());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toast_expiry() {
        let toast = Toast::new("test", ToastLevel::Info).with_duration(Duration::from_millis(50));

        assert!(!toast.is_expired());
        std::thread::sleep(Duration::from_millis(60));
        assert!(toast.is_expired());
    }

    #[test]
    fn test_toast_manager() {
        let manager = ToastManager::new();

        manager.info("info message");

        let visible = manager.get_visible();
        assert_eq!(visible.len(), 1);

        // Clear and verify history is managed internally
        manager.clear_history();
    }
}
