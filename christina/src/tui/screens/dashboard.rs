use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
};

use crate::config::DiffTool;
use crate::tui::diff_renderer::DiffRenderer;
use crate::tui::elm::{AppMsg, Component};
use crate::tui::theme::*;
use christina_core::types::FilePath;
use christina_core::{AppState, GitFile};

// =============================================================================
// Model (State)
// =============================================================================

/// Focus state for dashboard panels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardFocus {
    /// GitFile list panel is focused
    FileList,
    /// Diff preview panel is focused
    DiffPreview,
}

/// Dashboard state - pure data structure with no behavior
pub struct DashboardState {
    /// Currently staged files
    pub staged_files: Vec<GitFile>,
    /// List selection state
    pub list_state: ListState,
    /// Multi-select mode enabled
    pub multi_select_mode: bool,
    /// Selected file indices (for multi-select)
    pub selected_indices: Vec<usize>,
    /// Diff preview vertical scroll position
    pub diff_scroll: u16,
    /// Diff preview horizontal scroll position
    pub diff_scroll_horizontal: u16,
    /// Current focus (file list or diff preview)
    pub focus: DashboardFocus,
    /// Diff renderer with caching
    diff_renderer: DiffRenderer,
    /// Show user context input modal
    pub show_user_context_input: bool,
    /// User context input buffer
    pub user_context_input: String,
}

impl DashboardState {
    pub fn new(staged_files: Vec<GitFile>) -> Self {
        let mut list_state = ListState::default();
        if !staged_files.is_empty() {
            list_state.select(Some(0));
        }

        let diff_renderer = DiffRenderer::new(DiffTool::Auto);

        Self {
            staged_files,
            list_state,
            multi_select_mode: false,
            selected_indices: Vec::new(),
            diff_scroll: 0,
            diff_scroll_horizontal: 0,
            focus: DashboardFocus::FileList,
            diff_renderer,
            show_user_context_input: false,
            user_context_input: String::new(),
        }
    }

    /// Get the currently selected file index
    pub fn selected(&self) -> Option<usize> {
        self.list_state.selected()
    }
}

// =============================================================================
// Messages
// =============================================================================

/// Messages for dashboard interactions
#[derive(Debug, Clone)]
pub enum DashboardMsg {
    /// Navigate up (file list or diff depending on focus)
    NavigateUp,
    /// Navigate down (file list or diff depending on focus)
    NavigateDown,
    /// Navigate left (horizontal scroll in diff preview)
    NavigateLeft,
    /// Navigate right (horizontal scroll in diff preview)
    NavigateRight,
    /// Toggle focus between file list and diff preview
    ToggleFocus,
    /// Unstage the selected file
    UnstageSelected,
    /// Unstage all files
    UnstageAll,
    /// Toggle selection (multi-select mode)
    ToggleSelection,
    /// Toggle multi-select mode
    ToggleMultiSelectMode,
    /// Toggle user context input modal
    ToggleUserContextInput,
    /// Update user context input buffer
    UpdateUserContextInput(char),
    /// Delete character from user context input
    DeleteUserContextChar,
    /// Save user context
    SaveUserContext,
    /// Proceed to generate commit message
    Generate,
    /// Go back to staging selection
    BackToStaging,
    /// Quit application
    Quit,
}

// =============================================================================
// Update (Pure State Transitions)
// =============================================================================

impl Component for DashboardState {
    type Msg = DashboardMsg;

    fn update(&mut self, msg: Self::Msg) -> Vec<AppMsg> {
        match msg {
            DashboardMsg::NavigateUp => {
                match self.focus {
                    DashboardFocus::FileList => {
                        if let Some(selected) = self.list_state.selected()
                            && selected > 0
                        {
                            self.list_state.select(Some(selected - 1));
                            self.diff_scroll = 0;
                            self.diff_scroll_horizontal = 0;
                        }
                    }
                    DashboardFocus::DiffPreview => {
                        self.diff_scroll = self.diff_scroll.saturating_sub(1);
                    }
                }
                vec![]
            }

            DashboardMsg::NavigateDown => {
                match self.focus {
                    DashboardFocus::FileList => {
                        if let Some(selected) = self.list_state.selected() {
                            let max_idx = self.staged_files.len().saturating_sub(1);
                            if selected < max_idx {
                                self.list_state.select(Some(selected + 1));
                                self.diff_scroll = 0;
                                self.diff_scroll_horizontal = 0;
                            }
                        }
                    }
                    DashboardFocus::DiffPreview => {
                        self.diff_scroll = self.diff_scroll.saturating_add(1);
                    }
                }
                vec![]
            }

            DashboardMsg::NavigateLeft => {
                if self.focus == DashboardFocus::DiffPreview {
                    self.diff_scroll_horizontal = self.diff_scroll_horizontal.saturating_sub(4);
                }
                vec![]
            }

            DashboardMsg::NavigateRight => {
                if self.focus == DashboardFocus::DiffPreview {
                    self.diff_scroll_horizontal = self.diff_scroll_horizontal.saturating_add(4);
                }
                vec![]
            }

            DashboardMsg::ToggleFocus => {
                self.focus = match self.focus {
                    DashboardFocus::FileList => DashboardFocus::DiffPreview,
                    DashboardFocus::DiffPreview => DashboardFocus::FileList,
                };
                vec![]
            }

            DashboardMsg::UnstageSelected => {
                if let Some(selected) = self.selected()
                    && selected < self.staged_files.len()
                {
                    if self.multi_select_mode && !self.selected_indices.is_empty() {
                        // Unstage multiple files
                        let files: Vec<FilePath> = self
                            .selected_indices
                            .iter()
                            .filter_map(|&idx| self.staged_files.get(idx))
                            .map(|f| f.path.clone())
                            .collect();

                        self.selected_indices.clear();

                        let msgs: Vec<AppMsg> =
                            files.into_iter().map(AppMsg::UnstageFile).collect();
                        return msgs;
                    } else {
                        // Unstage single file
                        let file_path = self.staged_files[selected].path.clone();
                        return vec![AppMsg::UnstageFile(file_path)];
                    }
                }
                vec![]
            }

            DashboardMsg::UnstageAll => {
                let files: Vec<FilePath> =
                    self.staged_files.iter().map(|f| f.path.clone()).collect();

                let mut msgs: Vec<AppMsg> = files.into_iter().map(AppMsg::UnstageFile).collect();

                msgs.push(AppMsg::Navigate(AppState::StagingSelection));
                msgs
            }

            DashboardMsg::ToggleSelection => {
                if self.multi_select_mode
                    && let Some(selected) = self.selected()
                {
                    if let Some(pos) = self.selected_indices.iter().position(|&x| x == selected) {
                        self.selected_indices.remove(pos);
                    } else {
                        self.selected_indices.push(selected);
                    }
                }
                vec![]
            }

            DashboardMsg::ToggleMultiSelectMode => {
                self.multi_select_mode = !self.multi_select_mode;
                if !self.multi_select_mode {
                    self.selected_indices.clear();
                }

                let msg = if self.multi_select_mode {
                    "Multi-select mode enabled"
                } else {
                    "Multi-select mode disabled"
                };

                vec![AppMsg::ShowToast(
                    msg.to_string(),
                    crate::tui::elm::ToastLevel::Info,
                )]
            }

            DashboardMsg::ToggleUserContextInput => {
                self.show_user_context_input = !self.show_user_context_input;
                vec![]
            }

            DashboardMsg::UpdateUserContextInput(ch) => {
                if self.show_user_context_input {
                    self.user_context_input.push(ch);
                }
                vec![]
            }

            DashboardMsg::DeleteUserContextChar => {
                if self.show_user_context_input {
                    self.user_context_input.pop();
                }
                vec![]
            }

            DashboardMsg::SaveUserContext => {
                if !self.show_user_context_input {
                    vec![]
                } else {
                    self.show_user_context_input = false;
                    let context = if self.user_context_input.is_empty() {
                        None
                    } else {
                        Some(self.user_context_input.clone())
                    };
                    vec![AppMsg::SetUserContext(context)]
                }
            }

            DashboardMsg::Generate => {
                vec![AppMsg::Navigate(AppState::Generating)]
            }

            DashboardMsg::BackToStaging => {
                vec![AppMsg::Navigate(AppState::StagingSelection)]
            }

            DashboardMsg::Quit => {
                vec![AppMsg::Quit]
            }
        }
    }
}

// =============================================================================
// View (Pure Rendering)
// =============================================================================

/// Render the dashboard view
pub fn render_dashboard(
    frame: &mut Frame,
    state: &mut DashboardState,
    area: Rect,
    terminal_width: u16,
    show_diff_preview: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main content (file list + diff preview)
            Constraint::Length(2), // Footer
        ])
        .split(area);

    render_header(frame, chunks[0]);

    if show_diff_preview {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // GitFile list
                Constraint::Percentage(60), // Diff preview
            ])
            .split(chunks[1]);

        render_file_list(frame, state, content_chunks[0]);
        render_diff_preview(frame, state, content_chunks[1], terminal_width);
    } else {
        render_file_list(frame, state, chunks[1]);
    }

    render_footer(frame, chunks[2], state);

    if state.show_user_context_input {
        render_user_context_modal(frame, state, area);
    }
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

fn render_file_list(frame: &mut Frame, state: &mut DashboardState, area: Rect) {
    let items: Vec<ListItem> = state
        .staged_files
        .iter()
        .enumerate()
        .map(|(idx, file)| {
            let (status_str, status_color) = match file.status_enum.as_char() {
                'M' => (STATUS_MODIFIED, BLUE),
                'A' => (STATUS_ADDED, GREEN),
                'D' => (STATUS_DELETED, RED),
                _ => (STATUS_UNKNOWN, SUBTEXT0),
            };

            let mut line_spans = vec![
                Span::styled(status_str, Style::default().fg(status_color)),
                Span::styled(file.path.as_str(), Style::default().fg(TEXT)),
            ];

            // Show selection marker in multi-select mode
            if state.multi_select_mode && state.selected_indices.contains(&idx) {
                line_spans.insert(0, Span::styled("✓ ", Style::default().fg(GREEN)));
            }

            ListItem::new(Line::from(line_spans))
        })
        .collect();

    let file_count = state.staged_files.len();
    let border_color = if state.focus == DashboardFocus::FileList {
        ROSEWATER
    } else {
        SURFACE1
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color))
                .title(format!(" Staged ({file_count}) "))
                .title_style(Style::default().fg(SUBTEXT0)),
        )
        .highlight_style(Style::default().bg(SURFACE0));

    frame.render_stateful_widget(list, area, &mut state.list_state);
}

fn render_diff_preview(
    frame: &mut Frame,
    state: &mut DashboardState,
    area: Rect,
    terminal_width: u16,
) {
    let selected_file = state.selected().and_then(|idx| state.staged_files.get(idx));

    let content = if let Some(file) = selected_file {
        if file.is_binary || file.diff_content.is_empty() {
            vec![Line::from(Span::styled(
                if file.is_binary {
                    "[Binary file - no preview available]"
                } else {
                    "[No diff content]"
                },
                Style::default().fg(SUBTEXT0).italic(),
            ))]
        } else {
            state.diff_renderer.render_diff(file, terminal_width)
        }
    } else {
        vec![Line::from(Span::styled(
            "No file selected",
            Style::default().fg(SUBTEXT0).italic(),
        ))]
    };

    // Show file path in title only when not using delta (delta already shows it)
    let title = if state.diff_renderer.is_using_delta() {
        " Diff Preview ".to_string()
    } else if let Some(file) = selected_file {
        format!(" {} ", file.path)
    } else {
        " Diff Preview ".to_string()
    };

    let border_color = if state.focus == DashboardFocus::DiffPreview {
        ROSEWATER
    } else {
        SURFACE1
    };

    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color))
                .title(title)
                .title_style(Style::default().fg(SUBTEXT0)),
        )
        .scroll((state.diff_scroll, state.diff_scroll_horizontal));

    frame.render_widget(paragraph, area);

    // Render vertical scrollbar if content is scrollable
    if let Some(file) = selected_file
        && !file.is_binary
        && !file.diff_content.is_empty()
    {
        let line_count = file.diff_content.lines().count();
        let visible_lines = area.height.saturating_sub(2) as usize; // Account for borders

        if line_count > visible_lines {
            let mut scrollbar_state = ScrollbarState::new(line_count)
                .position(state.diff_scroll as usize)
                .viewport_content_length(visible_lines);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
            frame.render_stateful_widget(
                scrollbar,
                area.inner(ratatui::layout::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        }
    }
}

fn render_footer(frame: &mut Frame, area: Rect, state: &DashboardState) {
    let (nav_keys, nav_hint) = match state.focus {
        DashboardFocus::FileList => ("↑↓", "files"),
        DashboardFocus::DiffPreview => ("↑↓←→", "scroll diff"),
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("tab", Style::default().fg(SUBTEXT0)),
        Span::styled(" switch focus ", Style::default().fg(OVERLAY0)),
        Span::styled(nav_keys, Style::default().fg(SUBTEXT0)),
        Span::styled(format!(" {} ", nav_hint), Style::default().fg(OVERLAY0)),
        Span::styled("i", Style::default().fg(SUBTEXT0)),
        Span::styled(" context ", Style::default().fg(OVERLAY0)),
        Span::styled("enter", Style::default().fg(ROSEWATER)),
        Span::styled(" generate ", Style::default().fg(OVERLAY0)),
        Span::styled("q", Style::default().fg(SUBTEXT0)),
        Span::styled(" quit", Style::default().fg(OVERLAY0)),
    ]))
    .alignment(Alignment::Center);

    frame.render_widget(footer, area);
}

fn render_user_context_modal(frame: &mut Frame, state: &DashboardState, area: Rect) {
    use ratatui::widgets::Clear;

    let modal_width = area.width.min(60);
    let modal_height = 7;
    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    };

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ROSEWATER))
        .title(" User Context ")
        .title_style(Style::default().fg(TEXT).bold());

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(inner);

    let label = Paragraph::new("Enter additional context for commit generation:")
        .style(Style::default().fg(SUBTEXT0));
    frame.render_widget(label, content_chunks[0]);

    let input_text = if state.user_context_input.is_empty() {
        Line::from(Span::styled(
            "Type here...",
            Style::default().fg(OVERLAY0).italic(),
        ))
    } else {
        Line::from(Span::styled(
            &state.user_context_input,
            Style::default().fg(TEXT),
        ))
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SURFACE1));

    let input = Paragraph::new(input_text).block(input_block);
    frame.render_widget(input, content_chunks[1]);

    let hint = Paragraph::new(Line::from(vec![
        Span::styled("enter", Style::default().fg(ROSEWATER)),
        Span::styled(" save ", Style::default().fg(OVERLAY0)),
        Span::styled("esc", Style::default().fg(SUBTEXT0)),
        Span::styled(" cancel", Style::default().fg(OVERLAY0)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(hint, content_chunks[2]);
}

// =============================================================================
// Key Event Mapping
// =============================================================================

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Map key events to dashboard messages
pub fn key_to_message(key: KeyEvent, show_context_modal: bool) -> Option<DashboardMsg> {
    if show_context_modal {
        return match key.code {
            KeyCode::Esc => Some(DashboardMsg::ToggleUserContextInput),
            KeyCode::Enter => Some(DashboardMsg::SaveUserContext),
            KeyCode::Backspace => Some(DashboardMsg::DeleteUserContextChar),
            KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(DashboardMsg::ToggleUserContextInput)
            }
            KeyCode::Char(c) => Some(DashboardMsg::UpdateUserContextInput(c)),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Some(DashboardMsg::NavigateUp),
        KeyCode::Down | KeyCode::Char('j') => Some(DashboardMsg::NavigateDown),
        KeyCode::Left | KeyCode::Char('h') => Some(DashboardMsg::NavigateLeft),
        KeyCode::Right | KeyCode::Char('l') => Some(DashboardMsg::NavigateRight),
        KeyCode::Tab => Some(DashboardMsg::ToggleFocus),
        KeyCode::Char('u') | KeyCode::Backspace => Some(DashboardMsg::UnstageSelected),
        KeyCode::Char('U') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            Some(DashboardMsg::UnstageAll)
        }
        KeyCode::Char(' ') => Some(DashboardMsg::ToggleSelection),
        KeyCode::Char('v') => Some(DashboardMsg::ToggleMultiSelectMode),
        KeyCode::Char('i') => Some(DashboardMsg::ToggleUserContextInput),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(DashboardMsg::Quit)
        }
        KeyCode::Char('c') | KeyCode::Enter => Some(DashboardMsg::Generate),
        KeyCode::Char('s') => Some(DashboardMsg::BackToStaging),
        KeyCode::Char('q') | KeyCode::Esc => Some(DashboardMsg::Quit),
        _ => None,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[expect(
    clippy::panic,
    reason = "test assertions use panic for failure reporting"
)]
mod tests {
    use super::*;
    use christina_core::GitFileStatus;
    use compact_str::CompactString;

    fn create_test_file(path: &str, status: char) -> GitFile {
        let status_str = status.to_string();
        GitFile {
            path: FilePath::from(path),
            status: CompactString::new(&status_str),
            status_enum: GitFileStatus::from_char(status),
            diff_content: String::new(),
            is_binary: false,
        }
    }

    #[test]
    fn test_dashboard_navigate_up() {
        let files = vec![
            create_test_file("file1.rs", 'M'),
            create_test_file("file2.rs", 'A'),
        ];
        let mut state = DashboardState::new(files);
        state.list_state.select(Some(1));

        let msgs = state.update(DashboardMsg::NavigateUp);

        assert_eq!(state.selected(), Some(0));
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_dashboard_navigate_down() {
        let files = vec![
            create_test_file("file1.rs", 'M'),
            create_test_file("file2.rs", 'A'),
        ];
        let mut state = DashboardState::new(files);
        state.list_state.select(Some(0));

        let msgs = state.update(DashboardMsg::NavigateDown);

        assert_eq!(state.selected(), Some(1));
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_dashboard_unstage_selected() {
        let files = vec![
            create_test_file("file1.rs", 'M'),
            create_test_file("file2.rs", 'A'),
        ];
        let mut state = DashboardState::new(files);
        state.list_state.select(Some(0));

        let msgs = state.update(DashboardMsg::UnstageSelected);

        assert_eq!(msgs.len(), 1);
        if let AppMsg::UnstageFile(path) = &msgs[0] {
            assert_eq!(path.as_str(), "file1.rs");
        } else {
            panic!("Expected UnstageFile message");
        }
    }

    #[test]
    fn test_dashboard_toggle_multi_select() {
        let files = vec![create_test_file("file1.rs", 'M')];
        let mut state = DashboardState::new(files);

        assert!(!state.multi_select_mode);

        let msgs = state.update(DashboardMsg::ToggleMultiSelectMode);

        assert!(state.multi_select_mode);
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_dashboard_multi_select_unstage() {
        let files = vec![
            create_test_file("file1.rs", 'M'),
            create_test_file("file2.rs", 'A'),
            create_test_file("file3.rs", 'D'),
        ];
        let mut state = DashboardState::new(files);
        state.multi_select_mode = true;
        state.selected_indices = vec![0, 2];
        state.list_state.select(Some(0));

        let msgs = state.update(DashboardMsg::UnstageSelected);

        assert_eq!(msgs.len(), 2);
        assert!(state.selected_indices.is_empty());
    }
}
