use compact_str::CompactString;
use ratatui::widgets::ListState;

use crate::tui::form::FormState as CoreFormState;
use christina_core::{
    ProviderProfile,
    types::{ModelName, ProviderKind},
};

/// Profile list item for display
#[derive(Debug, Clone)]
pub struct ProfileListItem {
    pub profile: ProviderProfile,
    pub is_active: bool,
}

/// Modal/overlay type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalType {
    CreateProfile,
    EditProfile,
    DeleteConfirm,
    DuplicateProfile,
}

/// Profile management app state
pub struct ProfileApp {
    /// List of profiles
    pub profiles: Vec<ProfileListItem>,
    /// List selection state
    pub list_state: ListState,
    /// Current modal/overlay
    pub modal: Option<ModalType>,
    /// Form state for create/edit
    pub form_state: Option<CoreFormState>,
    /// Profile being edited (for form)
    pub edit_profile: Option<ProviderProfile>,
    /// Profile being edited/deleted (by index)
    pub target_profile_idx: Option<usize>,
    /// Whether to quit
    pub should_quit: bool,
    /// Status message
    pub status_message: Option<CompactString>,
}

impl ProfileApp {
    pub fn new(profiles: Vec<ProviderProfile>, active_profile: Option<String>) -> Self {
        let profiles: Vec<ProfileListItem> = profiles
            .into_iter()
            .map(|p| ProfileListItem {
                is_active: Some(&p.name) == active_profile.as_ref(),
                profile: p,
            })
            .collect();

        let mut list_state = ListState::default();
        if !profiles.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            profiles,
            list_state,
            modal: None,
            form_state: None,
            edit_profile: None,
            target_profile_idx: None,
            should_quit: false,
            status_message: None,
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn move_up(&mut self) {
        if let Some(selected) = self.selected()
            && selected > 0
        {
            self.list_state.select(Some(selected - 1));
        }
    }

    pub fn move_down(&mut self) {
        if let Some(selected) = self.selected()
            && selected + 1 < self.profiles.len()
        {
            self.list_state.select(Some(selected + 1));
        }
    }

    pub fn set_status(&mut self, msg: &str) {
        self.status_message = Some(CompactString::new(msg));
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    /// Default number of visible rows for the form
    const FORM_VISIBLE_ROWS: usize = 15;

    pub fn open_create_modal(&mut self) {
        let new_profile = ProviderProfile::new(
            String::new(),
            ProviderKind::OpenAI,
            ModelName::from("gpt-4"),
        );
        self.modal = Some(ModalType::CreateProfile);
        self.form_state = Some(CoreFormState::new(&new_profile, Self::FORM_VISIBLE_ROWS));
        self.edit_profile = Some(new_profile);
        self.target_profile_idx = None;
    }

    pub fn open_edit_modal(&mut self) {
        if let Some(idx) = self.selected()
            && idx < self.profiles.len()
        {
            let profile = self.profiles[idx].profile.clone();
            self.modal = Some(ModalType::EditProfile);
            self.form_state = Some(CoreFormState::new(&profile, Self::FORM_VISIBLE_ROWS));
            self.edit_profile = Some(profile);
            self.target_profile_idx = Some(idx);
        }
    }

    pub fn open_duplicate_modal(&mut self) {
        if let Some(idx) = self.selected()
            && idx < self.profiles.len()
        {
            let mut profile = self.profiles[idx].profile.clone();
            profile.name = format!("{} (copy)", profile.name);
            self.modal = Some(ModalType::DuplicateProfile);
            self.form_state = Some(CoreFormState::new(&profile, Self::FORM_VISIBLE_ROWS));
            self.edit_profile = Some(profile);
            self.target_profile_idx = None;
        }
    }

    pub fn open_delete_modal(&mut self) {
        if let Some(idx) = self.selected()
            && idx < self.profiles.len()
        {
            self.modal = Some(ModalType::DeleteConfirm);
            self.target_profile_idx = Some(idx);
        }
    }

    pub fn close_modal(&mut self) {
        self.modal = None;
        self.form_state = None;
        self.edit_profile = None;
        self.target_profile_idx = None;
    }
}
