use crate::tui::screens::*;
use crate::tui::{DataState, UiState};
use christina_core::StateMachine;

/// Prevents orphaned background tasks from continuing to run after state transitions.
pub struct AbortOnDrop(pub tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Bundles task handle and generation ID to prevent invalid states.
pub enum GenerationState {
    Idle,
    Running {
        task: AbortOnDrop,
        generation_id: u64,
    },
}

pub struct TuiUiState {
    pub base: UiState,
    pub frame_count: usize,
    pub should_redraw: bool,
}

pub struct TuiSessionData {
    pub base: DataState,
    pub state_machine: StateMachine,
    pub dashboard_state: Option<DashboardState>,
    pub error_state: Option<ErrorState>,
    pub review_state: Option<ReviewState>,
    pub staging_state: Option<StagingState>,
    pub editing_state: Option<EditingState>,
    pub generating_state_ui: Option<GeneratingState>,
}
