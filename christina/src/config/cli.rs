use anyhow::Result;

use crate::config::Config;
use crate::tui::{
    ConfigTuiOptions, ConfigTuiResult, ProfileTuiOptions, run_config_tui, run_profile_tui,
};

/// Handle config command - opens the config TUI.
pub fn handle_config_command() -> Result<()> {
    // Load config once before entering the loop
    let mut config = Config::load().unwrap_or_default();

    loop {
        let has_api_key = config.api_key.is_some();
        let api_key_source = Some("config/env");

        let options = ConfigTuiOptions {
            config: config.clone(),
            has_api_key,
            api_key_source,
            on_save: Box::new(move |cfg| cfg.clone().save_to_global()),
        };

        match run_config_tui(options)? {
            ConfigTuiResult::Quit => break,
            ConfigTuiResult::OpenProfiles => {
                // Open profiles with current config
                manage_profiles(&config)?;
                // Reload config after profile manager exits
                config = Config::load().unwrap_or_default();
            }
        }
    }

    Ok(())
}

fn manage_profiles(config: &Config) -> Result<()> {
    let options = ProfileTuiOptions {
        profiles: config.profiles.clone(),
        active_profile: config.profiles.active.clone(),
        on_save: Box::new(|profiles_manager| {
            let mut cfg = Config::load().unwrap_or_default();
            cfg.profiles = profiles_manager.clone();

            // Apply active profile if set
            if let Some(active) = profiles_manager.get_active() {
                cfg.apply_profile(active);
            }

            cfg.save_to_global()
        }),
    };

    run_profile_tui(options)
}
