#![allow(clippy::derivable_impls)]

use christina_core::types::TokenCount;
use compact_str::CompactString;
use tui_textarea::TextArea;

use crate::app::edit_history::EditHistory;
use christina_core::{GitFile, ReviewAction};

use super::widgets::ToastManager;

pub struct UiState {
    pub textarea: TextArea<'static>,
    pub spinner_idx: usize,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            textarea: TextArea::default(),
            spinner_idx: 0,
        }
    }
}

/// Data state for the application.
/// Note: generation state (task handle + ID) is managed in App.generation_state,
/// not here, to keep christina-tui runtime-agnostic and prevent invalid states.
pub struct DataState {
    pub staged_files: Vec<GitFile>,
    pub unstaged_files: Vec<GitFile>,
    pub selected_indices: Vec<usize>,
    pub multi_select_mode: bool,
    pub generated_message: CompactString,
    pub error_message: Option<String>,
    pub toasts: ToastManager,
    pub token_count: TokenCount,
    pub user_context: Option<String>,
    pub review_action: ReviewAction,
    pub edit_history: EditHistory,
    /// Version counter incremented on data changes to avoid per-frame cloning
    pub data_version: u64,
}

impl Default for DataState {
    fn default() -> Self {
        Self {
            staged_files: Vec::new(),
            unstaged_files: Vec::new(),
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
        }
    }
}
