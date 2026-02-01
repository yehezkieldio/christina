use std::io::stdout;

use anyhow::Result;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    crossterm::{
        ExecutableCommand,
        event::{self, Event, KeyEventKind},
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
};

use christina_core::{Profiles, ProviderProfile};

use super::app::ProfileApp;
use super::screen;
use super::update;

/// Type alias for profile operation callbacks
pub type ProfileCallback = Box<dyn FnMut(&Profiles) -> Result<()>>;

/// Configuration for running the profile TUI
pub struct ProfileTuiOptions {
    /// Current profiles
    pub profiles: Profiles,
    /// Active profile name
    pub active_profile: Option<String>,
    /// Callback to save profile changes
    pub on_save: ProfileCallback,
}

/// Run the profile management TUI
pub fn run_profile_tui(mut options: ProfileTuiOptions) -> Result<()> {
    // Set up terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let result = run_app(&mut terminal, &mut options);

    // Clean up terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

/// Main application loop
fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    options: &mut ProfileTuiOptions,
) -> Result<()> {
    // Helper to get sorted profiles
    let get_sorted_profiles = |profiles: &Profiles| -> Vec<ProviderProfile> {
        let mut list = profiles.definitions.values().cloned().collect::<Vec<_>>();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    };

    let initial_list = get_sorted_profiles(&options.profiles);
    let mut app = ProfileApp::new(initial_list, options.active_profile.clone());

    // Build profiles manager for callbacks
    let mut profiles_manager = options.profiles.clone();
    profiles_manager.active = options.active_profile.clone();

    loop {
        terminal.draw(|frame: &mut Frame| screen::render(frame, &mut app))?;

        if app.should_quit {
            break;
        }

        // Poll for events
        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            let action = update::handle_key(&mut app, key);

            // Handle profile operations
            match action {
                update::ProfileAction::None => {}
                update::ProfileAction::Create(profile) => {
                    if let Err(e) = profiles_manager.add(profile) {
                        app.set_status(&format!("Error: {}", e));
                    } else {
                        if let Err(e) = (options.on_save)(&profiles_manager) {
                            app.set_status(&format!("Save failed: {}", e));
                        } else {
                            app.set_status("Profile created!");
                            // Refresh app state
                            let sorted = get_sorted_profiles(&profiles_manager);
                            app.profiles = sorted
                                .into_iter()
                                .map(|p| super::app::ProfileListItem {
                                    is_active: Some(&p.name) == profiles_manager.active.as_ref(),
                                    profile: p,
                                })
                                .collect();
                            app.close_modal();
                        }
                    }
                }
                update::ProfileAction::Update { index, profile } => {
                    // We need to map index to name.
                    // app.profiles is sorted, so we can get the name from there.
                    if index < app.profiles.len() {
                        let old_name = app.profiles[index].profile.name.clone();
                        // If we are renaming, remove old and add new?
                        // Or use update logic. Profiles::update takes name.
                        // If profile.name differs from old_name, we need to handle rename.

                        let rename = old_name != profile.name;

                        let update_result = if rename {
                            if profiles_manager.exists(&profile.name) {
                                Err(anyhow::anyhow!("Profile '{}' already exists", profile.name))
                            } else {
                                // Preserve active state before remove
                                let was_active =
                                    profiles_manager.active.as_deref() == Some(&old_name);
                                profiles_manager.remove(&old_name)?;
                                let result = profiles_manager.add(profile.clone());
                                // Restore active state to new name if it was previously active
                                if was_active && result.is_ok() {
                                    let _ = profiles_manager.set_active(&profile.name);
                                }
                                result
                            }
                        } else {
                            profiles_manager.update(&old_name, profile)
                        };

                        if let Err(e) = update_result {
                            app.set_status(&format!("Error: {}", e));
                        } else {
                            if let Err(e) = (options.on_save)(&profiles_manager) {
                                app.set_status(&format!("Save failed: {}", e));
                            } else {
                                app.set_status("Profile updated!");
                                // Refresh app state
                                let sorted = get_sorted_profiles(&profiles_manager);
                                app.profiles = sorted
                                    .into_iter()
                                    .map(|p| super::app::ProfileListItem {
                                        is_active: Some(&p.name)
                                            == profiles_manager.active.as_ref(),
                                        profile: p,
                                    })
                                    .collect();
                                app.close_modal();
                            }
                        }
                    }
                }
                update::ProfileAction::Delete(index) => {
                    if index < app.profiles.len() {
                        let name = app.profiles[index].profile.name.clone();
                        if let Err(e) = profiles_manager.remove(&name) {
                            app.set_status(&format!("Error: {}", e));
                        } else {
                            if let Err(e) = (options.on_save)(&profiles_manager) {
                                app.set_status(&format!("Save failed: {}", e));
                            } else {
                                app.set_status("Profile deleted!");
                                // Refresh app state
                                let sorted = get_sorted_profiles(&profiles_manager);
                                app.profiles = sorted
                                    .into_iter()
                                    .map(|p| super::app::ProfileListItem {
                                        is_active: Some(&p.name)
                                            == profiles_manager.active.as_ref(),
                                        profile: p,
                                    })
                                    .collect();
                                app.close_modal();

                                // Adjust selection
                                if app.profiles.is_empty() {
                                    app.list_state.select(None);
                                } else if let Some(sel) = app.selected()
                                    && sel >= app.profiles.len()
                                {
                                    app.list_state.select(Some(app.profiles.len() - 1));
                                }
                            }
                        }
                    }
                }
                update::ProfileAction::Switch(index) => {
                    if index < app.profiles.len() {
                        let name = app.profiles[index].profile.name.clone();
                        if let Err(e) = profiles_manager.set_active(&name) {
                            app.set_status(&format!("Error: {}", e));
                        } else {
                            if let Err(e) = (options.on_save)(&profiles_manager) {
                                app.set_status(&format!("Save failed: {}", e));
                            } else {
                                app.set_status(&format!("Switched to profile '{}'", name));
                                // Refresh app state
                                let sorted = get_sorted_profiles(&profiles_manager);
                                app.profiles = sorted
                                    .into_iter()
                                    .map(|p| super::app::ProfileListItem {
                                        is_active: Some(&p.name)
                                            == profiles_manager.active.as_ref(),
                                        profile: p,
                                    })
                                    .collect();
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
