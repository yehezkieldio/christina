use crate::app::cmd::{Cmd, ToastSeverity};
use crate::app::model::{GenerationStatus, Model, Route};
use crate::app::msg::Msg;

/// Pure state transition function following Elm Architecture.
///
/// Takes current model and a message, returns commands to execute.
/// This function must be pure: no I/O, no side effects.
pub fn update(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::GitStatusRefreshed {
            files,
            staged,
            unstaged,
            branch,
        } => {
            model.git.files = files;
            model.git.staged = staged;
            model.git.unstaged = unstaged;
            model.git.branch = branch;
            vec![]
        }

        Msg::FilesStaged { paths } => {
            for path in paths {
                let path_str = path.as_str();
                if !model.git.staged.iter().any(|p| p == path_str) {
                    model.git.staged.push(path_str.to_string());
                }
                model.git.unstaged.retain(|p| p != path_str);
            }
            vec![]
        }

        Msg::FilesUnstaged { paths } => {
            for path in paths {
                let path_str = path.as_str();
                if !model.git.unstaged.iter().any(|p| p == path_str) {
                    model.git.unstaged.push(path_str.to_string());
                }
                model.git.staged.retain(|p| p != path_str);
            }
            vec![]
        }

        Msg::GenerationStarted { id } => {
            model.generation = GenerationStatus::Running { id };
            model.route = Route::Generating;
            vec![]
        }

        Msg::GenerationCompleted { id, message } => {
            if let GenerationStatus::Running { id: current_id } = model.generation
                && current_id == id {
                    model.generation = GenerationStatus::Completed { id, message };
                    model.route = Route::Review;
                }
            vec![]
        }

        Msg::GenerationFailed { id, error } => {
            if let GenerationStatus::Running { id: current_id } = model.generation
                && current_id == id {
                    model.generation = GenerationStatus::Failed {
                        id,
                        error: error.clone(),
                    };
                    model.route = Route::Error;
                    return vec![Cmd::ShowToast {
                        message: error,
                        severity: ToastSeverity::Error,
                    }];
                }
            vec![]
        }

        Msg::GenerationCancelled { id } => {
            if let GenerationStatus::Running { id: current_id } = model.generation
                && current_id == id {
                    model.generation = GenerationStatus::Idle;
                    model.route = Route::Dashboard;
                }
            vec![]
        }

        Msg::UserContextSet { context } => {
            model.user_context = context;
            vec![]
        }

        Msg::NavigateTo { state } => {
            let route = match state {
                crate::AppState::StagingSelection => Route::StagingSelection,
                crate::AppState::Dashboard => Route::Dashboard,
                crate::AppState::Generating => Route::Generating,
                crate::AppState::Review => Route::Review,
                crate::AppState::Editing => Route::Editing,
                crate::AppState::Error => Route::Error,
            };
            model.route = route;
            vec![]
        }

        Msg::SelectFiles { paths } => {
            vec![Cmd::StageFiles { paths }]
        }

        Msg::ToggleMultiSelect => {
            model.screens.staging.multi_select_mode = !model.screens.staging.multi_select_mode;
            vec![]
        }

        Msg::EditMessage { message } => {
            model.screens.editing.content = message.to_string();
            model.screens.editing.cursor_line = 0;
            model.screens.editing.cursor_column = 0;
            model.route = Route::Editing;
            vec![]
        }

        Msg::SaveMessage { message } => {
            if let GenerationStatus::Completed { id, .. } = model.generation {
                model.generation = GenerationStatus::Completed {
                    id,
                    message: message.clone(),
                };
                model.route = Route::Review;
            }
            vec![]
        }

        Msg::CancelEdit => {
            model.route = Route::Review;
            vec![]
        }

        Msg::Commit { message } => {
            vec![Cmd::CommitMessage { message }]
        }

        Msg::Tick => {
            let now = std::time::Instant::now();
            model
                .toasts
                .retain(|t| now.duration_since(t.created_at).as_secs() < 5);
            vec![]
        }

        Msg::Quit => vec![Cmd::Quit],

        Msg::None => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::{GitState, Screens};
    use crate::app::screens::{
        DashboardState, EditingState, ErrorState, GeneratingState, ReviewState, StagingState,
    };
    use crate::ids::GenerationId;
    use crate::types::CommitMessage;
    use std::path::PathBuf;

    fn test_model() -> Model {
        Model {
            route: Route::Dashboard,
            screens: Screens {
                staging: StagingState {
                    selected_indices: vec![],
                    multi_select_mode: false,
                    search_query: None,
                },
                dashboard: DashboardState::default(),
                review: ReviewState::default(),
                editing: EditingState::default(),
                generating: GeneratingState::default(),
                error: ErrorState::default(),
            },
            git: GitState {
                files: vec![],
                staged: vec![],
                unstaged: vec![],
                branch: "main".to_string(),
                repo_root: PathBuf::from("/tmp"),
            },
            generation: GenerationStatus::Idle,
            user_context: None,
            toasts: vec![],
        }
    }

    #[test]
    fn git_status_refreshed_updates_git_state() {
        let mut model = test_model();
        let files = vec![crate::git::GitFile::new(
            "test.rs".to_string(),
            "M".to_string(),
            "".to_string(),
        )];

        update(
            &mut model,
            Msg::GitStatusRefreshed {
                files: files.clone(),
                staged: vec!["staged.rs".to_string()],
                unstaged: vec!["unstaged.rs".to_string()],
                branch: "dev".to_string(),
            },
        );

        assert_eq!(model.git.files.len(), 1);
        assert_eq!(model.git.staged, vec!["staged.rs"]);
        assert_eq!(model.git.unstaged, vec!["unstaged.rs"]);
        assert_eq!(model.git.branch, "dev");
    }

    #[test]
    fn files_staged_moves_files_to_staged() {
        let mut model = test_model();
        model.git.unstaged = vec!["test.rs".to_string()];

        update(
            &mut model,
            Msg::FilesStaged {
                paths: vec![crate::types::FilePath::from("test.rs")],
            },
        );

        assert!(model.git.staged.contains(&"test.rs".to_string()));
        assert!(!model.git.unstaged.contains(&"test.rs".to_string()));
    }

    #[test]
    fn generation_started_sets_running_status() {
        let mut model = test_model();
        let id = GenerationId::new(42);

        update(&mut model, Msg::GenerationStarted { id });

        assert!(matches!(
            model.generation,
            GenerationStatus::Running { id: _ }
        ));
        assert_eq!(model.route, Route::Generating);
    }

    #[test]
    fn generation_completed_navigates_to_review() {
        let mut model = test_model();
        let id = GenerationId::new(42);
        model.generation = GenerationStatus::Running { id };

        let message = CommitMessage::try_from("feat: test".to_string()).unwrap();
        update(
            &mut model,
            Msg::GenerationCompleted {
                id,
                message: message.clone(),
            },
        );

        assert!(matches!(
            model.generation,
            GenerationStatus::Completed { .. }
        ));
        assert_eq!(model.route, Route::Review);
    }

    #[test]
    fn quit_returns_quit_command() {
        let mut model = test_model();
        let cmds = update(&mut model, Msg::Quit);

        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], Cmd::Quit));
    }
}
