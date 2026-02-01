use compact_str::CompactString;

use crate::config::Config;
use crate::tui::form::FormState;

/// Config TUI application state
pub struct ConfigApp {
    /// Current configuration data
    pub config: Config,
    /// Form state for editing
    pub form_state: FormState,
    /// Whether to quit the TUI
    pub should_quit: bool,
    /// Whether to open profile manager
    pub open_profiles: bool,
    /// Whether changes have been made
    pub has_changes: bool,
    /// Status message to display
    pub status_message: Option<CompactString>,
}

impl ConfigApp {
    pub fn new(config: Config, _has_api_key: bool, _api_key_source: Option<&'static str>) -> Self {
        let form_state = FormState::new(&config);

        Self {
            config,
            form_state,
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
}
