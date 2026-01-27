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
