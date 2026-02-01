use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};

use christina_core::GitFile;
use christina_core::types::FilePath;

use crate::tui::{elm::*, theme::*};

#[derive(Clone, Debug)]
pub struct StagingState {
    pub unstaged_files: Vec<GitFile>,
    pub selected_indices: Vec<usize>,
    pub list_state: ListState,
    /// Last synced data version (to avoid per-frame cloning)
    pub synced_version: u64,
}

impl StagingState {
    pub fn new(unstaged_files: Vec<GitFile>) -> Self {
        let mut list_state = ListState::default();
        if !unstaged_files.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            unstaged_files,
            selected_indices: vec![],
            list_state,
            synced_version: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StagingMsg {
    NavigateUp,
    NavigateDown,
    ToggleSelection,
    SelectAll,
    SelectNone,
    StageFiles,
    Quit,
}

impl Component for StagingState {
    type Msg = StagingMsg;

    fn update(&mut self, msg: Self::Msg) -> Vec<AppMsg> {
        match msg {
            StagingMsg::NavigateUp => {
                if let Some(selected) = self.list_state.selected()
                    && selected > 0
                {
                    self.list_state.select(Some(selected - 1));
                }
                vec![]
            }
            StagingMsg::NavigateDown => {
                if let Some(selected) = self.list_state.selected()
                    && selected < self.unstaged_files.len().saturating_sub(1)
                {
                    self.list_state.select(Some(selected + 1));
                }
                vec![]
            }
            StagingMsg::ToggleSelection => {
                if let Some(selected) = self.list_state.selected() {
                    if let Some(pos) = self.selected_indices.iter().position(|&i| i == selected) {
                        self.selected_indices.remove(pos);
                    } else {
                        self.selected_indices.push(selected);
                    }
                }
                vec![]
            }
            StagingMsg::SelectAll => {
                self.selected_indices = (0..self.unstaged_files.len()).collect();
                vec![]
            }
            StagingMsg::SelectNone => {
                self.selected_indices.clear();
                vec![]
            }
            StagingMsg::StageFiles => {
                if self.selected_indices.is_empty() {
                    vec![AppMsg::Navigate(christina_core::AppState::Dashboard)]
                } else {
                    let files_to_stage: Vec<FilePath> = self
                        .selected_indices
                        .iter()
                        .filter_map(|&idx| self.unstaged_files.get(idx))
                        .map(|f| f.path.clone())
                        .collect();
                    vec![AppMsg::StageFiles(files_to_stage)]
                }
            }
            StagingMsg::Quit => {
                vec![AppMsg::Quit]
            }
        }
    }
}

pub fn key_to_message(key: KeyEvent) -> Option<StagingMsg> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Some(StagingMsg::NavigateUp),
        KeyCode::Down | KeyCode::Char('j') => Some(StagingMsg::NavigateDown),
        KeyCode::Char(' ') => Some(StagingMsg::ToggleSelection),
        KeyCode::Char('a') => Some(StagingMsg::SelectAll),
        KeyCode::Char('A') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            Some(StagingMsg::SelectAll)
        }
        KeyCode::Char('n') => Some(StagingMsg::SelectNone),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(StagingMsg::Quit)
        }
        KeyCode::Enter => Some(StagingMsg::StageFiles),
        KeyCode::Char('q') | KeyCode::Esc => Some(StagingMsg::Quit),
        _ => None,
    }
}

pub fn render_staging(frame: &mut Frame, state: &mut StagingState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, chunks[0]);
    render_file_list(frame, state, chunks[1]);
    render_footer(frame, chunks[2]);
}

fn render_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled("christina", Style::default().fg(TEXT).bold()),
        Span::styled(" / ", Style::default().fg(SURFACE1)),
        Span::styled("stage", Style::default().fg(SUBTEXT0)),
    ])])
    .alignment(Alignment::Left);

    frame.render_widget(header, area);
}

fn render_file_list(frame: &mut Frame, state: &mut StagingState, area: Rect) {
    let items: Vec<ListItem> = state
        .unstaged_files
        .iter()
        .enumerate()
        .map(|(idx, file)| {
            let is_selected = state.selected_indices.contains(&idx);
            let (checkbox_str, checkbox_color) = if is_selected {
                (CHECKBOX_SELECTED, ROSEWATER)
            } else {
                (CHECKBOX_UNSELECTED, SURFACE1)
            };

            let (status_str, status_color) = match file.status.as_str() {
                "M" => (STATUS_MODIFIED, BLUE),
                "A" => (STATUS_ADDED, GREEN),
                "D" => (STATUS_DELETED, RED),
                _ => (STATUS_UNKNOWN, SUBTEXT0),
            };

            let path_color = if idx % 2 == 0 { TEXT } else { SUBTEXT1 };

            let line = Line::from(vec![
                Span::styled(checkbox_str, Style::default().fg(checkbox_color)),
                Span::styled(status_str, Style::default().fg(status_color)),
                Span::styled(file.path.as_str(), Style::default().fg(path_color)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let selected_count = state.selected_indices.len();
    let total_count = state.unstaged_files.len();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(SURFACE1))
                .title(format!(" Files ({selected_count}/{total_count}) "))
                .title_style(Style::default().fg(SUBTEXT0)),
        )
        .highlight_style(Style::default().bg(SURFACE0));

    frame.render_stateful_widget(list, area, &mut state.list_state);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("space", Style::default().fg(SUBTEXT0)),
        Span::styled(" select ", Style::default().fg(OVERLAY0)),
        Span::styled("a", Style::default().fg(SUBTEXT0)),
        Span::styled(" all ", Style::default().fg(OVERLAY0)),
        Span::styled("enter", Style::default().fg(ROSEWATER)),
        Span::styled(" continue ", Style::default().fg(OVERLAY0)),
        Span::styled("q", Style::default().fg(SUBTEXT0)),
        Span::styled(" quit", Style::default().fg(OVERLAY0)),
    ]))
    .alignment(Alignment::Center);

    frame.render_widget(footer, area);
}

#[cfg(test)]
#[expect(
    clippy::panic,
    reason = "test assertions use panic for failure reporting"
)]
mod tests {
    use super::*;
    use christina_core::GitFileStatus;
    use compact_str::CompactString;

    fn create_test_file(path: &str, status: &str) -> GitFile {
        GitFile {
            path: FilePath::from(path),
            status: CompactString::new(status),
            status_enum: GitFileStatus::Modified,
            diff_content: String::new(),
            is_binary: false,
        }
    }

    #[test]
    fn test_navigation() {
        let files = vec![
            create_test_file("file1.txt", "M"),
            create_test_file("file2.txt", "A"),
            create_test_file("file3.txt", "D"),
        ];
        let mut state = StagingState::new(files);
        assert_eq!(state.list_state.selected(), Some(0));

        state.update(StagingMsg::NavigateDown);
        assert_eq!(state.list_state.selected(), Some(1));

        state.update(StagingMsg::NavigateDown);
        assert_eq!(state.list_state.selected(), Some(2));

        state.update(StagingMsg::NavigateDown);
        assert_eq!(state.list_state.selected(), Some(2));

        state.update(StagingMsg::NavigateUp);
        assert_eq!(state.list_state.selected(), Some(1));

        state.update(StagingMsg::NavigateUp);
        assert_eq!(state.list_state.selected(), Some(0));

        state.update(StagingMsg::NavigateUp);
        assert_eq!(state.list_state.selected(), Some(0));
    }

    #[test]
    fn test_toggle_selection() {
        let files = vec![
            create_test_file("file1.txt", "M"),
            create_test_file("file2.txt", "A"),
        ];
        let mut state = StagingState::new(files);
        state.list_state.select(Some(0));

        assert!(state.selected_indices.is_empty());

        state.update(StagingMsg::ToggleSelection);
        assert_eq!(state.selected_indices, vec![0]);

        state.update(StagingMsg::ToggleSelection);
        assert!(state.selected_indices.is_empty());

        state.list_state.select(Some(1));
        state.update(StagingMsg::ToggleSelection);
        assert_eq!(state.selected_indices, vec![1]);
    }

    #[test]
    fn test_select_all_none() {
        let files = vec![
            create_test_file("file1.txt", "M"),
            create_test_file("file2.txt", "A"),
            create_test_file("file3.txt", "D"),
        ];
        let mut state = StagingState::new(files);

        state.update(StagingMsg::SelectAll);
        assert_eq!(state.selected_indices, vec![0, 1, 2]);

        state.update(StagingMsg::SelectNone);
        assert!(state.selected_indices.is_empty());
    }

    #[test]
    fn test_stage_files() {
        let files = vec![
            create_test_file("file1.txt", "M"),
            create_test_file("file2.txt", "A"),
            create_test_file("file3.txt", "D"),
        ];
        let mut state = StagingState::new(files);

        state.selected_indices = vec![0, 2];
        let msgs = state.update(StagingMsg::StageFiles);

        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            AppMsg::StageFiles(files) => {
                assert_eq!(files.len(), 2);
                assert_eq!(files[0], FilePath::from("file1.txt"));
                assert_eq!(files[1], FilePath::from("file3.txt"));
            }
            other => panic!("Expected StageFiles message, got {:?}", other),
        }
    }

    #[test]
    fn test_stage_files_empty() {
        let files = vec![create_test_file("file1.txt", "M")];
        let mut state = StagingState::new(files);

        let msgs = state.update(StagingMsg::StageFiles);
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            AppMsg::Navigate(s) => assert_eq!(*s, christina_core::AppState::Dashboard),
            other => panic!("Expected Navigate to Dashboard, got {:?}", other),
        }
    }

    #[test]
    fn test_quit() {
        let files = vec![create_test_file("file1.txt", "M")];
        let mut state = StagingState::new(files);

        let msgs = state.update(StagingMsg::Quit);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], AppMsg::Quit));
    }
}
