use compact_str::CompactString;

use crate::config::{Config, ConfigTab};
use crate::tui::form::FormState;

/// Config TUI application state with tabbed interface
pub struct ConfigApp {
    /// Current configuration data
    pub config: Config,
    /// Form states for each tab (indexed by ConfigTab ordinal)
    pub form_states: [FormState; 3],
    /// Current active tab
    pub current_tab: ConfigTab,
    /// Whether to quit the TUI
    pub should_quit: bool,
    /// Whether to open profile manager
    pub open_profiles: bool,
    /// Whether changes have been made (global across all tabs)
    pub has_changes: bool,
    /// Status message to display
    pub status_message: Option<CompactString>,
}

impl ConfigApp {
    pub fn new(config: Config, _has_api_key: bool, _api_key_source: Option<&'static str>) -> Self {
        // Create separate form states for each tab
        let general_state = FormState::with_fields(config.fields_for_tab(ConfigTab::General));
        let advanced_state = FormState::with_fields(config.fields_for_tab(ConfigTab::Advanced));
        let experimental_state =
            FormState::with_fields(config.fields_for_tab(ConfigTab::Experimental));

        Self {
            config,
            form_states: [general_state, advanced_state, experimental_state],
            current_tab: ConfigTab::General,
            should_quit: false,
            open_profiles: false,
            has_changes: false,
            status_message: None,
        }
    }

    /// Set status message
    pub fn set_status(&mut self, msg: &str) {
        self.status_message = Some(CompactString::new(msg));
    }

    /// Get a reference to the config data
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get the current form state for the active tab
    pub fn current_form_state(&self) -> &FormState {
        &self.form_states[self.current_tab as usize]
    }

    /// Get a mutable reference to the current form state
    pub fn current_form_state_mut(&mut self) -> &mut FormState {
        &mut self.form_states[self.current_tab as usize]
    }

    /// Switch to the next tab (wrapping around)
    pub fn next_tab(&mut self) {
        self.current_tab = self.current_tab.next();
        self.status_message = None;
    }

    /// Switch to the previous tab (wrapping around)
    pub fn prev_tab(&mut self) {
        self.current_tab = self.current_tab.prev();
        self.status_message = None;
    }

    /// Switch to a specific tab by index (1=General, 2=Advanced, 3=Experimental)
    pub fn set_tab(&mut self, index: usize) {
        if (1..=3).contains(&index) {
            self.current_tab = ConfigTab::ALL[index - 1];
            self.status_message = None;
        }
    }

    /// Refresh field definitions for all tabs (call after config changes that affect field visibility)
    pub fn refresh_fields(&mut self) {
        self.form_states[0] =
            FormState::with_fields(self.config.fields_for_tab(ConfigTab::General));
        self.form_states[1] =
            FormState::with_fields(self.config.fields_for_tab(ConfigTab::Advanced));
        self.form_states[2] =
            FormState::with_fields(self.config.fields_for_tab(ConfigTab::Experimental));
    }
}
