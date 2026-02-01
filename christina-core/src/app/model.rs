use std::path::PathBuf;

use crate::git::GitFile;
use crate::ids::GenerationId;
use crate::types::CommitMessage;

pub use super::screens::{
    DashboardState, EditingState, ErrorState, GeneratingState, ReviewState, StagingState,
};

#[derive(Debug, Clone)]
pub struct Model {
    pub route: Route,
    pub screens: Screens,
    pub git: GitState,
    pub generation: GenerationStatus,
    pub user_context: Option<String>,
    pub toasts: Vec<Toast>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    StagingSelection,
    Dashboard,
    Generating,
    Review,
    Editing,
    Error,
}

#[derive(Debug, Clone)]
pub struct Screens {
    pub staging: StagingState,
    pub dashboard: DashboardState,
    pub review: ReviewState,
    pub editing: EditingState,
    pub generating: GeneratingState,
    pub error: ErrorState,
}

#[derive(Debug, Clone, Default)]
pub struct GitState {
    pub files: Vec<GitFile>,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub branch: String,
    pub repo_root: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub enum GenerationStatus {
    #[default]
    Idle,
    Running {
        id: GenerationId,
    },
    Completed {
        id: GenerationId,
        message: CommitMessage,
    },
    Failed {
        id: GenerationId,
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub severity: ToastSeverity,
    pub created_at: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSeverity {
    Info,
    Warning,
    Error,
}
