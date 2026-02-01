use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Invalid transition from {from} to {to}")]
pub struct TransitionError {
    pub from: AppState,
    pub to: AppState,
}

/// Application state representing different screens/modes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppState {
    StagingSelection,
    Dashboard,
    Generating,
    Review,
    Editing,
    Error,
}

impl fmt::Display for AppState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Review action that user can take
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewAction {
    Accept,
    Edit,
    Regenerate,
    Cancel,
}

/// State machine for enforcing valid transitions
pub struct StateMachine {
    generation_id: u64,
}

impl StateMachine {
    pub fn new() -> Self {
        Self { generation_id: 0 }
    }

    /// Generate a new generation ID for tracking async operations.
    ///
    /// We use saturating addition to avoid wrap-around issues.
    /// At u64::MAX, the ID will stop incrementing.
    /// In practice, this is unreachable (would require 2^64 generations).
    pub fn next_generation_id(&mut self) -> u64 {
        self.generation_id = self.generation_id.saturating_add(1);
        self.generation_id
    }

    pub fn current_generation_id(&self) -> u64 {
        self.generation_id
    }

    /// Validate a state transition
    pub fn can_transition(&self, from: &AppState, to: &AppState) -> Result<(), TransitionError> {
        let valid = matches!(
            (from, to),
            // From StagingSelection
            (AppState::StagingSelection, AppState::Dashboard) // User confirms staging selection and proceeds to main dashboard
            | (AppState::StagingSelection, AppState::Error) // An error occurred during staging selection

            // From Dashboard
            | (AppState::Dashboard, AppState::StagingSelection) // User navigates back to staging selection
            | (AppState::Dashboard, AppState::Generating) // User initiates the generation process
            | (AppState::Dashboard, AppState::Error) // An error occurred on the dashboard

            // From Generating
            | (AppState::Generating, AppState::Review) // Generation completed successfully, presenting results for review
            | (AppState::Generating, AppState::Error) // Generation failed with an error
            | (AppState::Generating, AppState::Dashboard) // Generation was cancelled, returning to dashboard

            // From Review
            | (AppState::Review, AppState::Editing) // User chooses to edit the generated content
            | (AppState::Review, AppState::Dashboard) // User accepts the content and returns to dashboard
            | (AppState::Review, AppState::Generating) // User requests regeneration of content
            | (AppState::Review, AppState::Error) // An error occurred during review

            // From Editing
            | (AppState::Editing, AppState::Review) // User finished editing and returns to review
            | (AppState::Editing, AppState::Dashboard) // User cancels editing and accepts current state, back to dashboard
            | (AppState::Editing, AppState::Error) // An error occurred during editing

            // From Error
            | (AppState::Error, AppState::Dashboard) // Error resolved, returning to dashboard
            | (AppState::Error, AppState::StagingSelection) // Error requires reset to initial staging selection
        );

        if valid {
            Ok(())
        } else {
            Err(TransitionError {
                from: from.clone(),
                to: to.clone(),
            })
        }
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_staging_selection_to_dashboard() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::StagingSelection, &AppState::Dashboard)
            .is_ok());
    }

    #[test]
    fn transition_staging_selection_to_error() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::StagingSelection, &AppState::Error)
            .is_ok());
    }

    #[test]
    fn transition_dashboard_to_staging_selection() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Dashboard, &AppState::StagingSelection)
            .is_ok());
    }

    #[test]
    fn transition_dashboard_to_generating() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Dashboard, &AppState::Generating)
            .is_ok());
    }

    #[test]
    fn transition_dashboard_to_error() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Dashboard, &AppState::Error)
            .is_ok());
    }

    #[test]
    fn transition_generating_to_review() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Generating, &AppState::Review)
            .is_ok());
    }

    #[test]
    fn transition_generating_to_error() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Generating, &AppState::Error)
            .is_ok());
    }

    #[test]
    fn transition_generating_to_dashboard() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Generating, &AppState::Dashboard)
            .is_ok());
    }

    #[test]
    fn transition_review_to_editing() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Review, &AppState::Editing)
            .is_ok());
    }

    #[test]
    fn transition_review_to_dashboard() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Review, &AppState::Dashboard)
            .is_ok());
    }

    #[test]
    fn transition_review_to_generating() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Review, &AppState::Generating)
            .is_ok());
    }

    #[test]
    fn transition_review_to_error() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Review, &AppState::Error)
            .is_ok());
    }

    #[test]
    fn transition_editing_to_review() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Editing, &AppState::Review)
            .is_ok());
    }

    #[test]
    fn transition_editing_to_dashboard() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Editing, &AppState::Dashboard)
            .is_ok());
    }

    #[test]
    fn transition_editing_to_error() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Editing, &AppState::Error)
            .is_ok());
    }

    #[test]
    fn transition_error_to_dashboard() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Error, &AppState::Dashboard)
            .is_ok());
    }

    #[test]
    fn transition_error_to_staging_selection() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Error, &AppState::StagingSelection)
            .is_ok());
    }

    // ==================== Invalid Transition Tests (15 total) ====================

    #[test]
    fn invalid_transition_staging_selection_to_generating() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::StagingSelection, &AppState::Generating)
            .is_err());
    }

    #[test]
    fn invalid_transition_staging_selection_to_review() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::StagingSelection, &AppState::Review)
            .is_err());
    }

    #[test]
    fn invalid_transition_staging_selection_to_editing() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::StagingSelection, &AppState::Editing)
            .is_err());
    }

    #[test]
    fn invalid_transition_dashboard_to_review() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Dashboard, &AppState::Review)
            .is_err());
    }

    #[test]
    fn invalid_transition_dashboard_to_editing() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Dashboard, &AppState::Editing)
            .is_err());
    }

    #[test]
    fn invalid_transition_generating_to_staging_selection() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Generating, &AppState::StagingSelection)
            .is_err());
    }

    #[test]
    fn invalid_transition_generating_to_editing() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Generating, &AppState::Editing)
            .is_err());
    }

    #[test]
    fn invalid_transition_review_to_staging_selection() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Review, &AppState::StagingSelection)
            .is_err());
    }

    #[test]
    fn invalid_transition_editing_to_staging_selection() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Editing, &AppState::StagingSelection)
            .is_err());
    }

    #[test]
    fn invalid_transition_editing_to_generating() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Editing, &AppState::Generating)
            .is_err());
    }

    #[test]
    fn invalid_transition_error_to_generating() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Error, &AppState::Generating)
            .is_err());
    }

    #[test]
    fn invalid_transition_error_to_review() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Error, &AppState::Review)
            .is_err());
    }

    #[test]
    fn invalid_transition_error_to_editing() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Error, &AppState::Editing)
            .is_err());
    }

    #[test]
    fn invalid_transition_same_state() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Dashboard, &AppState::Dashboard)
            .is_err());
    }

    #[test]
    fn invalid_transition_generating_to_generating() {
        let sm = StateMachine::new();
        assert!(sm
            .can_transition(&AppState::Generating, &AppState::Generating)
            .is_err());
    }

    #[test]
    fn generation_id_starts_at_zero() {
        let sm = StateMachine::new();
        assert_eq!(sm.current_generation_id(), 0);
    }

    #[test]
    fn next_generation_id_increments() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.next_generation_id(), 1);
        assert_eq!(sm.next_generation_id(), 2);
        assert_eq!(sm.next_generation_id(), 3);
    }

    #[test]
    fn next_generation_id_updates_current() {
        let mut sm = StateMachine::new();
        sm.next_generation_id();
        assert_eq!(sm.current_generation_id(), 1);
    }

    #[test]
    fn generation_id_sequence() {
        let mut sm = StateMachine::new();
        for i in 1..=100 {
            assert_eq!(sm.next_generation_id(), i);
        }
        assert_eq!(sm.current_generation_id(), 100);
    }

    #[test]
    fn generation_id_saturating_add_at_max() {
        let mut sm = StateMachine {
            generation_id: u64::MAX,
        };
        let result = sm.next_generation_id();
        assert_eq!(result, u64::MAX);
        assert_eq!(sm.current_generation_id(), u64::MAX);
    }

    #[test]
    fn generation_id_stays_at_max() {
        let mut sm = StateMachine {
            generation_id: u64::MAX,
        };
        sm.next_generation_id();
        sm.next_generation_id();
        assert_eq!(sm.current_generation_id(), u64::MAX);
    }

    #[test]
    fn appstate_display_staging_selection() {
        let state = AppState::StagingSelection;
        assert_eq!(format!("{}", state), "StagingSelection");
    }

    #[test]
    fn appstate_display_dashboard() {
        let state = AppState::Dashboard;
        assert_eq!(format!("{}", state), "Dashboard");
    }

    #[test]
    fn appstate_display_generating() {
        let state = AppState::Generating;
        assert_eq!(format!("{}", state), "Generating");
    }

    #[test]
    fn appstate_display_review() {
        let state = AppState::Review;
        assert_eq!(format!("{}", state), "Review");
    }

    #[test]
    fn appstate_display_editing() {
        let state = AppState::Editing;
        assert_eq!(format!("{}", state), "Editing");
    }

    #[test]
    fn appstate_display_error() {
        let state = AppState::Error;
        assert_eq!(format!("{}", state), "Error");
    }

    #[test]
    fn transition_error_display() {
        let err = TransitionError {
            from: AppState::Dashboard,
            to: AppState::Review,
        };
        let msg = format!("{}", err);
        assert_eq!(msg, "Invalid transition from Dashboard to Review");
    }

    #[test]
    fn transition_error_debug_format() {
        let err = TransitionError {
            from: AppState::StagingSelection,
            to: AppState::Error,
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("TransitionError"));
        assert!(debug.contains("from"));
        assert!(debug.contains("to"));
    }

    #[test]
    fn transition_error_fields() {
        let err = TransitionError {
            from: AppState::Review,
            to: AppState::Dashboard,
        };
        assert_eq!(err.from, AppState::Review);
        assert_eq!(err.to, AppState::Dashboard);
    }

    #[test]
    fn state_machine_default() {
        let sm = StateMachine::default();
        assert_eq!(sm.current_generation_id(), 0);
    }

    #[test]
    fn appstate_clone() {
        let state = AppState::Dashboard;
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    #[test]
    fn review_action_enum() {
        let action = ReviewAction::Accept;
        assert_eq!(action, ReviewAction::Accept);
        assert_ne!(action, ReviewAction::Edit);
    }

    #[test]
    fn review_action_all_variants() {
        assert_eq!(ReviewAction::Accept, ReviewAction::Accept);
        assert_eq!(ReviewAction::Edit, ReviewAction::Edit);
        assert_eq!(ReviewAction::Regenerate, ReviewAction::Regenerate);
        assert_eq!(ReviewAction::Cancel, ReviewAction::Cancel);
    }
}
