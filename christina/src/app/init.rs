use compact_str::CompactString;
use ratatui::{
    style::Style,
    widgets::{Block, BorderType, Borders},
};
use tui_textarea::TextArea;

use crate::app::edit_history::EditHistory;
use crate::config::Config;
use crate::tui::{BASE, SUBTEXT0, SURFACE1, TEXT, ToastManager};
use christina_core::types::TokenCount;
use christina_core::{AppState, ReviewAction, StateMachine};
use christina_git::GitRepository;

use super::context::AppContextData;
use super::state::{TuiSessionData, TuiUiState};
use crate::tui::{DataState, UiState};

pub struct InitResult {
    pub context: AppContextData,
    pub ui: TuiUiState,
    pub data: TuiSessionData,
    pub initial_state: AppState,
    pub warnings: Vec<String>,
}

pub fn init_context() -> (AppContextData, Vec<String>) {
    let mut warnings = Vec::new();

    // Load config, surface errors instead of silently using defaults
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            warnings.push(format!("Failed to load config: {}. Using defaults.", e));
            Config::default()
        }
    };

    // Discover git repository and validate accessibility
    let (repo, branch_name) = match GitRepository::discover() {
        Ok(repo) => {
            // Validate repository is accessible
            let branch = repo.inner().head().ok().and_then(|h| {
                let name = h.shorthand()?;
                Some(CompactString::new(name))
            }); // OK: detached HEAD yields None
            (Some(repo), branch)
        }
        Err(e) => {
            warnings.push(format!(
                "Not in a git repository or repository is inaccessible: {}. \
                 Commit functionality will be disabled. Navigate to a git repository to enable commits.",
                e
            ));
            (None, None)
        }
    };

    (
        AppContextData {
            repo,
            config,
            branch_name,
        },
        warnings,
    )
}

pub fn load_file_lists(
    repo: Option<&GitRepository>,
) -> (
    Vec<christina_core::GitFile>,
    Vec<christina_core::GitFile>,
    Vec<String>,
) {
    let mut warnings = Vec::new();

    let Some(repo) = repo else {
        return (vec![], vec![], warnings);
    };

    let staged = match repo.get_staged_files_as_model() {
        Ok(files) => files,
        Err(e) => {
            warnings.push(format!(
                "Repository found but failed to load staged files: {}. \
                 Repository may be corrupted or inaccessible.",
                e
            ));
            vec![]
        }
    };

    let unstaged = match repo.get_unstaged_files_as_model() {
        Ok(files) => files,
        Err(e) => {
            warnings.push(format!(
                "Repository found but failed to load unstaged files: {}. \
                 Repository may be corrupted or inaccessible.",
                e
            ));
            vec![]
        }
    };

    // If both file loads failed, warn about potential repository issues
    if staged.is_empty() && unstaged.is_empty() && repo.inner().head().is_ok() {
        warnings.push(
            "Warning: Repository is accessible but contains no tracked changes. \
             This is normal for a clean working directory."
                .to_string(),
        );
    }

    (staged, unstaged, warnings)
}

pub fn determine_initial_state(
    staged_files: &[christina_core::GitFile],
    unstaged_files: &[christina_core::GitFile],
) -> (AppState, Vec<String>) {
    let mut warnings = Vec::new();

    // Check for processable staged files (not binary, not empty)
    let has_processable_staged_files = !staged_files.is_empty()
        && staged_files
            .iter()
            .any(|f| !f.is_binary && !f.diff_content.is_empty());

    // Only go to Dashboard if there are staged files AND no unstaged files
    let state = if has_processable_staged_files && unstaged_files.is_empty() {
        AppState::Dashboard
    } else if !staged_files.is_empty() && unstaged_files.is_empty() && !has_processable_staged_files
    {
        // All staged files are binary or empty - warn user
        warnings.push(
            "All staged files are binary or have no diff content. You may need to stage text files."
                .to_string(),
        );
        AppState::StagingSelection
    } else {
        AppState::StagingSelection
    };

    (state, warnings)
}

pub fn init_ui_state() -> TuiUiState {
    let mut textarea = TextArea::default();
    textarea.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SURFACE1))
            .title(" Edit Message ")
            .title_style(Style::default().fg(SUBTEXT0)),
    );
    textarea.set_cursor_line_style(Style::default());
    textarea.set_style(Style::default().fg(TEXT).bg(BASE));

    TuiUiState {
        base: UiState {
            textarea,
            spinner_idx: 0,
        },
        frame_count: 0,
        should_redraw: true,
    }
}

pub fn init_session_data(
    staged_files: Vec<christina_core::GitFile>,
    unstaged_files: Vec<christina_core::GitFile>,
) -> TuiSessionData {
    TuiSessionData {
        base: DataState {
            staged_files,
            unstaged_files,
            selected_indices: Vec::new(),
            multi_select_mode: false,
            generated_message: CompactString::default(),
            error_message: None,
            toasts: ToastManager::new(),
            token_count: TokenCount::new_saturating(1),
            user_context: None,
            review_action: ReviewAction::Accept,
            edit_history: EditHistory::default(),
            data_version: 0,
        },
        state_machine: StateMachine::new(),
        dashboard_state: None,
        error_state: None,
        review_state: None,
        staging_state: None,
        editing_state: None,
        generating_state_ui: None,
    }
}

pub fn initialize_app() -> InitResult {
    // Load context (config, repository)
    let (context, mut warnings) = init_context();

    // Load file lists
    let (staged_files, unstaged_files, file_warnings) = load_file_lists(context.repo.as_ref());
    warnings.extend(file_warnings);

    // Determine initial state
    let (initial_state, state_warnings) = determine_initial_state(&staged_files, &unstaged_files);
    warnings.extend(state_warnings);

    // Initialize UI and session data
    let ui = init_ui_state();
    let data = init_session_data(staged_files, unstaged_files);

    InitResult {
        context,
        ui,
        data,
        initial_state,
        warnings,
    }
}
