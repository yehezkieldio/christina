use crate::tui::screens::*;
use crate::tui::{DataState, UiState};
use christina_core::StateMachine;

/// Prevents orphaned background tasks from continuing to run after state transitions.
///
/// WHY RAII abort pattern: LLM generation tasks are async and long-running (5-30s).
/// If the user cancels or an error occurs, we must terminate the task immediately.
/// Without this, tasks would continue running and potentially mutate state after the
/// user has moved to a different screen. Drop-based abort guarantees cleanup on:
/// - Explicit transition to another state
/// - Error handling unwinding the stack
/// - User cancellation via input
///
/// This prevents race conditions where stale generation results arrive after we've
/// already started a new generation or returned to the dashboard.
pub struct AbortOnDrop(pub tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Bundles task handle and generation ID to prevent invalid states.
///
/// WHY bundle task + generation_id: Generation tasks run asynchronously and send
/// results via channels. Without version tracking, we can't distinguish between:
/// - Results from the current generation (should be displayed)
/// - Results from a cancelled/stale generation (should be ignored)
///
/// Bundling them together enforces the invariant: "if a task exists, its ID is known".
/// This prevents the impossible state of (task=Some, generation_id=None) which would
/// require error-prone runtime checks throughout the codebase.
///
/// The enum also makes the state machine explicit: you're either Idle (no task, no ID)
/// or Running (both task and ID present). This eliminates an entire class of bugs.
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

/// WHY Option for each screen state: Each TUI screen has unique ephemeral state
/// (cursor positions, scroll offsets, user input). Only one screen is active at a time.
///
/// Alternative considered: enum ScreenState { Dashboard(DashboardState), ... }
/// Rejected because transitions require moving data out of the enum (ownership issues)
/// and reconstructing the entire enum variant. With Options, we can:
/// - `.take()` old state without cloning
/// - Initialize new state independently
/// - Keep inactive state around for potential "back" navigation (future optimization)
///
/// Memory cost: ~200 bytes per Option when None (negligible for 6 screens).
/// Benefit: Zero-copy state transitions and simplified transition logic.
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_abort_on_drop_aborts_task() {
        let task_started = Arc::new(AtomicBool::new(false));
        let task_aborted = Arc::new(AtomicBool::new(false));

        let started = Arc::clone(&task_started);
        let aborted = Arc::clone(&task_aborted);

        let handle = tokio::spawn(async move {
            started.store(true, Ordering::SeqCst);
            
            tokio::time::sleep(Duration::from_secs(10)).await;
            
            aborted.store(false, Ordering::SeqCst);
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(task_started.load(Ordering::SeqCst));

        {
            let _abort_wrapper = AbortOnDrop(handle);
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
        
        assert!(!task_aborted.load(Ordering::SeqCst));
    }

    #[test]
    fn test_generation_state_idle() {
        let state = GenerationState::Idle;
        match state {
            GenerationState::Idle => {}
            GenerationState::Running { .. } => panic!("Expected Idle variant"),
        }
    }

    #[tokio::test]
    async fn test_generation_state_running() {
        let task = tokio::spawn(async {});
        let abort_wrapper = AbortOnDrop(task);
        let generation_id = 42u64;

        let state = GenerationState::Running {
            task: abort_wrapper,
            generation_id,
        };

        match state {
            GenerationState::Running { generation_id: id, .. } => {
                assert_eq!(id, 42);
            }
            GenerationState::Idle => panic!("Expected Running variant"),
        }
    }

    #[test]
    fn test_tui_ui_state_default() {
        let ui_state = TuiUiState {
            base: UiState::default(),
            frame_count: 0,
            should_redraw: false,
        };

        assert_eq!(ui_state.frame_count, 0);
        assert!(!ui_state.should_redraw);
    }

    #[test]
    fn test_tui_session_data_default() {
        let session_data = TuiSessionData {
            base: DataState::default(),
            state_machine: StateMachine::default(),
            dashboard_state: None,
            error_state: None,
            review_state: None,
            staging_state: None,
            editing_state: None,
            generating_state_ui: None,
        };

        assert!(session_data.dashboard_state.is_none());
        assert!(session_data.error_state.is_none());
        assert!(session_data.review_state.is_none());
        assert!(session_data.staging_state.is_none());
        assert!(session_data.editing_state.is_none());
        assert!(session_data.generating_state_ui.is_none());
    }
}
