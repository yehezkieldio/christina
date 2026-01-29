use christina_core::types::{CommitMessage, FilePath};
use compact_str::CompactString;
use ratatui::{
    style::Style,
    widgets::{Block, BorderType, Borders},
};
use std::path::PathBuf;
use tui_textarea::TextArea;

use crate::tui::elm::{AppMsg, ToastLevel};
use crate::tui::screens::EditingState;
use crate::tui::{BASE, SUBTEXT0, SURFACE1, TEXT};
use christina_core::AppState;

use super::App;
use super::state::GenerationState;

impl App {
    /// This is the central dispatcher for side effects requested by Elm components.
    pub fn handle_app_msg(&mut self, msg: AppMsg) {
        match msg {
            AppMsg::StageFile(path) => self.handle_stage_file(path),
            AppMsg::StageFiles(paths) => self.handle_stage_files(paths),
            AppMsg::UnstageFile(path) => self.handle_unstage_file(path),
            AppMsg::Navigate(state) => self.transition_to(state),
            AppMsg::CommitMessage(message) => self.handle_commit_message(message),
            AppMsg::EditMessage(message) => self.handle_edit_message(message),
            AppMsg::EditRawMessage(message) => self.handle_edit_raw_message(message),
            AppMsg::RegenerateMessage => self.transition_to(AppState::Generating),
            AppMsg::GenerateMessage => self.transition_to(AppState::Generating),
            AppMsg::CancelGeneration => self.handle_cancel_generation(),
            AppMsg::RefreshDiff => self.refresh_git_files(),
            AppMsg::ShowToast(message, level) => self.handle_show_toast(message, level),
            AppMsg::SaveEditedMessage(message) => self.handle_save_edited_message(message),
            AppMsg::CancelEdit => self.transition_to(AppState::Review),
            AppMsg::RestoreTextArea(text, cursor) => self.handle_restore_textarea(text, cursor),
            AppMsg::SetUserContext(context) => self.handle_set_user_context(context),
            AppMsg::Quit => self.should_quit = true,
            AppMsg::None => {}
        }
    }

    fn handle_stage_file(&mut self, path: FilePath) {
        let Some(ref repo) = self.app_context.repo else {
            self.data.base.toasts.error(
                "No git repository found. Run from inside a git-initialized directory.".to_string(),
            );
            return;
        };

        // Find the file status from unstaged_files
        if let Some(file) = self
            .data
            .base
            .unstaged_files
            .iter()
            .find(|f| f.path == path)
        {
            let file_to_stage = vec![(PathBuf::from(file.path.as_str()), file.status_enum)];
            if let Err(e) = repo.stage_files(&file_to_stage) {
                self.data
                    .base
                    .toasts
                    .error(format!("Failed to stage file: {}", e));
            } else {
                self.data.base.toasts.success("File staged".to_string());
                self.refresh_git_files();
            }
        } else {
            self.data
                .base
                .toasts
                .error("File not found in unstaged files".to_string());
        }
    }

    fn handle_stage_files(&mut self, paths: Vec<FilePath>) {
        let Some(ref repo) = self.app_context.repo else {
            self.data.base.toasts.error(
                "No git repository found. Run from inside a git-initialized directory.".to_string(),
            );
            return;
        };

        let files_to_stage: Vec<_> = paths
            .iter()
            .filter_map(|path_str| {
                self.data
                    .base
                    .unstaged_files
                    .iter()
                    .find(|f| f.path == *path_str)
                    .map(|f| (PathBuf::from(f.path.as_str()), f.status_enum))
            })
            .collect();

        if !files_to_stage.is_empty() {
            if let Err(e) = repo.stage_files(&files_to_stage) {
                self.data
                    .base
                    .toasts
                    .error(format!("Failed to stage files: {}", e));
            } else {
                self.data
                    .base
                    .toasts
                    .success(format!("Staged {} files", files_to_stage.len()));
                self.refresh_git_files();
                self.transition_to(AppState::Dashboard);
            }
        }
    }

    fn handle_unstage_file(&mut self, path: FilePath) {
        if let Err(e) = self.unstage_file(path.as_ref()) {
            self.data
                .base
                .toasts
                .error(format!("Failed to unstage file: {}", e));
        } else {
            self.data.base.toasts.success("File unstaged".to_string());
        }
    }

    fn handle_commit_message(&mut self, message: CommitMessage) {
        if let Err(e) = self.validate_for_commit() {
            self.data.base.error_message = Some(e);
            self.transition_to(AppState::Error);
            return;
        }

        match self.create_commit(&message) {
            Ok(oid) => {
                let short_oid = &oid[..7.min(oid.len())];
                let first_line = message.as_ref();
                self.exit_message = Some(format!("✓ {} ({})", first_line, short_oid));
                self.should_quit = true;
            }
            Err(e) => {
                self.exit_message = Some(format!("✗ Commit failed: {}", e));
                self.should_quit = true;
            }
        }
    }

    fn handle_edit_message(&mut self, message: CommitMessage) {
        self.handle_edit_raw_message(message.as_ref().to_string());
    }

    fn handle_edit_raw_message(&mut self, message_str: String) {
        self.ui.base.textarea = TextArea::new(vec![message_str.clone()]);
        self.ui.base.textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(SURFACE1))
                .title(" Edit Message ")
                .title_style(Style::default().fg(SUBTEXT0)),
        );
        self.ui
            .base
            .textarea
            .set_cursor_line_style(Style::default());
        self.ui
            .base
            .textarea
            .set_style(Style::default().fg(TEXT).bg(BASE));

        self.data.base.edit_history.initialize(&message_str);
        self.data.editing_state = Some(EditingState::new(message_str));
        self.transition_to(AppState::Editing);
    }

    fn handle_cancel_generation(&mut self) {
        if let GenerationState::Running { task, .. } =
            std::mem::replace(&mut self.generation_state, GenerationState::Idle)
        {
            task.0.abort();
        }
        self.data
            .base
            .toasts
            .warning("Generation cancelled".to_string());
        self.transition_to(AppState::Dashboard);
    }

    fn handle_show_toast(&mut self, message: String, level: ToastLevel) {
        match level {
            ToastLevel::Info => self.data.base.toasts.info(message),
            ToastLevel::Success => self.data.base.toasts.success(message),
            ToastLevel::Warning => self.data.base.toasts.warning(message),
            ToastLevel::Error => self.data.base.toasts.error(message),
        }
    }

    fn handle_save_edited_message(&mut self, message: CommitMessage) {
        self.data.base.generated_message = CompactString::new(message.as_ref());
        self.data.base.toasts.success("Message saved".to_string());
        self.transition_to(AppState::Review);
    }

    fn handle_restore_textarea(&mut self, text: String, cursor: (usize, usize)) {
        self.ui.base.textarea = TextArea::new(vec![text]);
        self.ui
            .base
            .textarea
            .move_cursor(tui_textarea::CursorMove::Jump(
                cursor.0 as u16,
                cursor.1 as u16,
            ));
    }

    fn handle_set_user_context(&mut self, context: Option<String>) {
        self.data.base.user_context = context.clone();
        if context.is_some() {
            self.data
                .base
                .toasts
                .success("User context set".to_string());
        } else {
            self.data
                .base
                .toasts
                .info("User context cleared".to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::context::AppContextData;
    use crate::app::state::{GenerationState, TuiSessionData, TuiUiState};
    use crate::config::Config;
    use crate::tui::{DataState, UiState};
    use christina_core::{AppState, StateMachine};

    fn create_test_app() -> App {
        App {
            app_context: AppContextData {
                repo: None,
                config: Config::default(),
                branch_name: None,
            },
            ui: TuiUiState {
                base: UiState::default(),
                frame_count: 0,
                should_redraw: false,
            },
            data: TuiSessionData {
                base: DataState::default(),
                state_machine: StateMachine::new(),
                dashboard_state: None,
                error_state: None,
                review_state: None,
                staging_state: None,
                editing_state: None,
                generating_state_ui: None,
            },
            state: AppState::Dashboard,
            should_quit: false,
            exit_message: None,
            generation_state: GenerationState::Idle,
        }
    }

    #[test]
    fn test_handle_show_toast_info() {
        let mut app = create_test_app();
        let initial_count = app.data.base.toasts.get_visible().len();

        app.handle_show_toast("Info message".to_string(), ToastLevel::Info);

        let visible = app.data.base.toasts.get_visible();
        assert_eq!(visible.len(), initial_count + 1);
        assert_eq!(visible.last().unwrap().message, "Info message");
    }

    #[test]
    fn test_handle_show_toast_success() {
        let mut app = create_test_app();
        let initial_count = app.data.base.toasts.get_visible().len();

        app.handle_show_toast("Success message".to_string(), ToastLevel::Success);

        let visible = app.data.base.toasts.get_visible();
        assert_eq!(visible.len(), initial_count + 1);
        assert_eq!(visible.last().unwrap().message, "Success message");
    }

    #[test]
    fn test_handle_show_toast_warning() {
        let mut app = create_test_app();
        let initial_count = app.data.base.toasts.get_visible().len();

        app.handle_show_toast("Warning message".to_string(), ToastLevel::Warning);

        let visible = app.data.base.toasts.get_visible();
        assert_eq!(visible.len(), initial_count + 1);
        assert_eq!(visible.last().unwrap().message, "Warning message");
    }

    #[test]
    fn test_handle_show_toast_error() {
        let mut app = create_test_app();
        let initial_count = app.data.base.toasts.get_visible().len();

        app.handle_show_toast("Error message".to_string(), ToastLevel::Error);

        let visible = app.data.base.toasts.get_visible();
        assert_eq!(visible.len(), initial_count + 1);
        assert_eq!(visible.last().unwrap().message, "Error message");
    }

    #[test]
    fn test_handle_set_user_context_set() {
        let mut app = create_test_app();
        assert!(app.data.base.user_context.is_none());

        app.handle_set_user_context(Some("Test context".to_string()));

        assert_eq!(app.data.base.user_context, Some("Test context".to_string()));
        let visible = app.data.base.toasts.get_visible();
        assert_eq!(visible.last().unwrap().message, "User context set");
    }

    #[test]
    fn test_handle_set_user_context_clear() {
        let mut app = create_test_app();
        app.data.base.user_context = Some("Existing context".to_string());

        app.handle_set_user_context(None);

        assert!(app.data.base.user_context.is_none());
        let visible = app.data.base.toasts.get_visible();
        assert_eq!(visible.last().unwrap().message, "User context cleared");
    }

    #[test]
    fn test_handle_save_edited_message() {
        let mut app = create_test_app();
        app.state = AppState::Editing;
        assert_eq!(app.data.base.generated_message, CompactString::default());

        let message = CommitMessage::try_from("feat: add new feature".to_string()).unwrap();
        app.handle_save_edited_message(message);

        assert_eq!(
            app.data.base.generated_message,
            CompactString::new("feat: add new feature")
        );
        assert_eq!(app.state, AppState::Review);
        let visible = app.data.base.toasts.get_visible();
        assert_eq!(visible.last().unwrap().message, "Message saved");
    }

    #[test]
    fn test_handle_cancel_generation_idle() {
        let mut app = create_test_app();
        app.state = AppState::Generating;
        app.generation_state = GenerationState::Idle;

        app.handle_cancel_generation();

        assert_eq!(app.state, AppState::Dashboard);
        let visible = app.data.base.toasts.get_visible();
        assert_eq!(visible.last().unwrap().message, "Generation cancelled");
    }

    #[test]
    #[cfg(not(miri))]
    fn test_handle_cancel_generation_running() {
        use crate::app::state::AbortOnDrop;

        let mut app = create_test_app();
        app.state = AppState::Generating;

        let rt = tokio::runtime::Runtime::new().unwrap();
        let handle = rt.spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        });

        app.generation_state = GenerationState::Running {
            task: AbortOnDrop(handle),
            generation_id: 42,
        };

        app.handle_cancel_generation();

        assert_eq!(app.state, AppState::Dashboard);
        assert!(matches!(app.generation_state, GenerationState::Idle));
        let visible = app.data.base.toasts.get_visible();
        assert_eq!(visible.last().unwrap().message, "Generation cancelled");
    }

    #[test]
    fn test_handle_app_msg_show_toast() {
        let mut app = create_test_app();

        app.handle_app_msg(AppMsg::ShowToast(
            "Test message".to_string(),
            ToastLevel::Info,
        ));

        let visible = app.data.base.toasts.get_visible();
        assert_eq!(visible.last().unwrap().message, "Test message");
    }

    #[test]
    fn test_handle_app_msg_set_user_context() {
        let mut app = create_test_app();

        app.handle_app_msg(AppMsg::SetUserContext(Some("Context via msg".to_string())));

        assert_eq!(
            app.data.base.user_context,
            Some("Context via msg".to_string())
        );
    }

    #[test]
    fn test_handle_app_msg_quit() {
        let mut app = create_test_app();
        assert!(!app.should_quit);

        app.handle_app_msg(AppMsg::Quit);

        assert!(app.should_quit);
    }

    #[test]
    fn test_handle_app_msg_none() {
        let mut app = create_test_app();
        let initial_state = app.state.clone();

        app.handle_app_msg(AppMsg::None);

        assert_eq!(app.state, initial_state);
    }
}
