pub mod context;
pub mod edit_history;
pub mod handlers;
pub mod init;
pub mod state;

use std::path::Path;

use christina_core::AppState;
use context::AppContextData;
use state::{GenerationState, TuiSessionData, TuiUiState};

pub struct App {
    pub app_context: AppContextData,
    pub ui: TuiUiState,
    pub data: TuiSessionData,
    pub state: AppState,
    pub should_quit: bool,
    pub exit_message: Option<String>,
    pub generation_state: GenerationState,
}

impl App {
    pub fn new() -> Self {
        let init_result = init::initialize_app();

        let toasts = &init_result.data.base.toasts;
        for warning in init_result.warnings {
            toasts.warning(&warning);
        }

        Self {
            app_context: init_result.context,
            ui: init_result.ui,
            data: init_result.data,
            state: init_result.initial_state,
            should_quit: false,
            exit_message: None,
            generation_state: GenerationState::Idle,
        }
    }

    pub fn refresh_git_files(&mut self) {
        if let Some(ref repo) = self.app_context.repo {
            // Call git adapter to get staged and unstaged files
            match crate::io::git::adapter::get_staged_files(repo) {
                Ok(staged_git_files) => {
                    self.data.base.staged_files = staged_git_files;
                }
                Err(e) => {
                    self.data
                        .base
                        .toasts
                        .warning(format!("Failed to get staged files: {}", e));
                    self.data.base.staged_files.clear();
                }
            }

            match crate::io::git::adapter::get_unstaged_files(repo) {
                Ok(unstaged_git_files) => {
                    self.data.base.unstaged_files = unstaged_git_files;
                }
                Err(e) => {
                    self.data
                        .base
                        .toasts
                        .warning(format!("Failed to get unstaged files: {}", e));
                    self.data.base.unstaged_files.clear();
                }
            }

            self.data.base.data_version = self.data.base.data_version.wrapping_add(1);
            self.app_context.refresh_branch();
            self.data.base.selected_indices.clear();
        } else {
            // No repository available - attempt to discover one
            self.data.base.toasts.warning(
                "No git repository available. Attempting to discover repository...".to_string(),
            );
            self.validate_repo_state();
        }
    }

    /// Validate repository state and attempt recovery if repository is inaccessible.
    ///
    /// This addresses the risk that a repository might become inaccessible mid-session
    /// (e.g., network mount unmounted, permissions changed, .git directory moved).
    /// Called automatically when operations fail, but can also be triggered manually.
    pub fn validate_repo_state(&mut self) -> bool {
        // If we have a repo, try to validate it's still accessible
        if let Some(ref repo) = self.app_context.repo {
            // Try to access the repository head as a liveness check
            match repo.head() {
                Ok(_) => {
                    // Repository is accessible
                    self.data.base.staged_files.clear();
                    self.data.base.data_version = self.data.base.data_version.wrapping_add(1);
                    return true;
                }
                Err(e) => {
                    self.data.base.toasts.warning(format!(
                        "Repository is no longer accessible: {}. Attempting re-discovery...",
                        e
                    ));
                    // Fall through to re-discovery
                }
            }
        }

        // Attempt to discover a repository (either we never had one, or the previous one is gone)
        use compact_str::CompactString;

        match git2::Repository::discover(".") {
            Ok(new_repo) => {
                let branch = new_repo.head().ok().and_then(|h| {
                    let name = h.shorthand()?;
                    Some(CompactString::new(name))
                }); // OK: detached HEAD yields None

                // Load file lists using the initialized repository
                let (staged, unstaged, file_warnings) = init::load_file_lists(Some(&new_repo));

                // Surface any warnings from file loading
                for warning in file_warnings {
                    self.data.base.toasts.warning(warning);
                }

                self.app_context.repo = Some(new_repo);
                self.app_context.branch_name = branch;
                self.data.base.staged_files = staged;
                self.data.base.unstaged_files = unstaged;
                self.data.base.data_version = self.data.base.data_version.wrapping_add(1);
                self.data
                    .base
                    .toasts
                    .info("Repository discovered and loaded".to_string());
                true
            }
            Err(e) => {
                self.data.base.toasts.warning(format!(
                    "Could not discover git repository: {}. Commit functionality disabled.",
                    e
                ));
                self.app_context.repo = None;
                self.app_context.branch_name = None;
                self.data.base.staged_files.clear();
                self.data.base.unstaged_files.clear();
                self.data.base.data_version = self.data.base.data_version.wrapping_add(1);
                false
            }
        }
    }

    pub fn update_spinner(&mut self) {
        const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        self.ui.base.spinner_idx = (self.ui.base.spinner_idx + 1) % SPINNER_CHARS.len();
        self.ui.frame_count = self.ui.frame_count.wrapping_add(1);
        self.ui.should_redraw = true;
    }

    pub fn transition_to(&mut self, new_state: AppState) {
        use crate::tui::screens::GeneratingState;

        // Validate transition
        if let Err(e) = self
            .data
            .state_machine
            .can_transition(&self.state, &new_state)
        {
            self.data
                .base
                .toasts
                .warning(format!("Invalid state transition: {}", e));
            return;
        }

        // Clean up old state to prevent stale data
        match self.state {
            AppState::StagingSelection => {
                // Keep staging state as it might be revisited
            }
            AppState::Dashboard => {
                // Keep dashboard state as it might be revisited
            }
            AppState::Generating => {
                // Clear generating state when leaving - it's always fresh
                self.data.generating_state_ui = None;
            }
            AppState::Review => {
                // Clear review state when leaving to force fresh data on return
                self.data.review_state = None;
            }
            AppState::Editing => {
                // Clear editing state when leaving
                self.data.editing_state = None;
            }
            AppState::Error => {
                // Clear error state when leaving
                self.data.error_state = None;
            }
        }

        // Initialize new state
        if new_state == AppState::Generating {
            // Initialize generating UI state immediately before event loop starts generation
            // This ensures progress updates can be applied as soon as they arrive
            self.data.generating_state_ui = Some(GeneratingState::new());
        }

        self.state = new_state;
        self.ui.should_redraw = true;
    }

    pub fn create_commit(
        &mut self,
        message: &christina_core::types::CommitMessage,
    ) -> Result<String, String> {
        let Some(ref repo) = self.app_context.repo else {
            return Err("No git repository".to_string());
        };

        if let Err(e) = crate::io::git::adapter::validate_for_commit(repo) {
            return Err(format!("Commit validation failed: {}", e));
        }

        match crate::io::git::adapter::create_commit(repo, message.as_ref()) {
            Ok(oid) => {
                self.refresh_git_files();
                Ok(oid.to_string())
            }
            Err(e) => Err(format!("Failed to create commit: {}", e)),
        }
    }

    pub fn validate_for_commit(&self) -> Result<(), String> {
        let Some(ref repo) = self.app_context.repo else {
            return Err("No git repository".to_string());
        };

        // Basic validation
        if repo.state() != git2::RepositoryState::Clean {
            return Err(format!("Repository is in {:?} state", repo.state()));
        }
        Ok(())
    }

    pub fn unstage_file(&mut self, path: &Path) -> Result<(), String> {
        let Some(ref repo) = self.app_context.repo else {
            return Err("No git repository".to_string());
        };

        let path_str = path.to_string_lossy().to_string();
        match crate::io::git::adapter::unstage_files(repo, &[path_str]) {
            Ok(()) => {
                self.refresh_git_files();
                Ok(())
            }
            Err(e) => Err(format!("Failed to unstage file: {}", e)),
        }
    }

    pub fn validate_configuration(&self) -> Result<(), String> {
        let config = &self.app_context.config;

        if config
            .api_key
            .as_ref()
            .map(|k| k.is_empty())
            .unwrap_or(true)
        {
            return Err("API key is not configured. Set it via:\n\
                 - Config: christina config set api_key <key>\n\
                 - Environment: CHRISTINA_MODEL_API_KEY=<key>\n\
                 - Keyring: Set api_key to 'keyring:christina.api_key' and store via system keyring"
                .to_string());
        }

        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
