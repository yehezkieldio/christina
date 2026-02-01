use crate::state::ReviewAction;

#[derive(Debug, Clone)]
pub struct ReviewState {
    pub action: ReviewAction,
    pub show_diff: bool,
}

impl Default for ReviewState {
    fn default() -> Self {
        Self {
            action: ReviewAction::Accept,
            show_diff: false,
        }
    }
}
