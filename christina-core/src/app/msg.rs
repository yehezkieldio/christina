use crate::git::GitFile;
use crate::ids::GenerationId;
use crate::types::{CommitMessage, FilePath};
use crate::AppState;

/// Messages represent I/O results or user events fed INTO the update function.
///
/// These are the outcomes of side effects, not requests for them.
/// The update function processes these to produce new state and Cmd requests.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Git status was refreshed with new file state
    GitStatusRefreshed {
        files: Vec<GitFile>,
        staged: Vec<String>,
        unstaged: Vec<String>,
        branch: String,
    },

    /// Files were successfully staged
    FilesStaged { paths: Vec<FilePath> },

    /// Files were successfully unstaged
    FilesUnstaged { paths: Vec<FilePath> },

    /// Generation process started with a new ID
    GenerationStarted { id: GenerationId },

    /// Generation completed successfully
    GenerationCompleted {
        id: GenerationId,
        message: CommitMessage,
    },

    /// Generation failed with an error
    GenerationFailed { id: GenerationId, error: String },

    /// Generation was cancelled by user
    GenerationCancelled { id: GenerationId },

    /// User context was set or cleared
    UserContextSet { context: Option<String> },

    /// Navigation to a new state was requested
    NavigateTo { state: AppState },

    /// User selected files (from staging screen)
    SelectFiles { paths: Vec<FilePath> },

    /// User toggled multi-select mode
    ToggleMultiSelect,

    /// User initiated message editing
    EditMessage { message: CommitMessage },

    /// User saved edited message
    SaveMessage { message: CommitMessage },

    /// User cancelled editing
    CancelEdit,

    /// User confirmed commit
    Commit { message: CommitMessage },

    /// Timer tick (for animations, timeouts, etc.)
    Tick,

    /// User requested quit
    Quit,

    /// No operation
    None,
}
