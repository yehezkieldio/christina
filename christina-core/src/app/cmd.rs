use crate::AppState;
use crate::types::{CommitMessage, FilePath};

/// Commands represent side effect requests sent FROM the update function.
///
/// These are pure data describing what should happen, not the effects themselves.
/// The runtime layer interprets these and produces Msg results.
#[derive(Debug, Clone)]
pub enum Cmd {
    /// Request git status refresh
    RefreshGitStatus,

    /// Request staging of files
    StageFiles { paths: Vec<FilePath> },

    /// Request unstaging of files
    UnstageFiles { paths: Vec<FilePath> },

    /// Request commit with message
    CommitMessage { message: CommitMessage },

    /// Request generation of commit message
    StartGeneration,

    /// Request cancellation of ongoing generation
    CancelGeneration,

    /// Request showing a toast notification
    ShowToast {
        message: String,
        severity: ToastSeverity,
    },

    /// Request navigation to a new state
    NavigateTo { state: AppState },

    /// Request application quit
    Quit,
}

/// Toast notification severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSeverity {
    Info,
    Warning,
    Error,
}
