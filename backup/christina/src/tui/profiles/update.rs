use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{ModalType, ProfileApp};
use crate::tui::form::FormMode;
use christina_core::ProviderProfile;

/// Actions that result from input handling
#[derive(Debug)]
pub enum ProfileAction {
    None,
    Create(ProviderProfile),
    Update {
        index: usize,
        profile: ProviderProfile,
    },
    Delete(usize),
    Switch(usize),
}

/// Handle keyboard input
pub fn handle_key(app: &mut ProfileApp, key: KeyEvent) -> ProfileAction {
    // Handle modal input separately
    if app.modal.is_some() {
        return handle_modal_key(app, key);
    }

    // Normal mode (list view)
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_up();
            app.clear_status();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_down();
            app.clear_status();
        }
        KeyCode::Char('n') | KeyCode::Char('a') => {
            // Create new profile
            app.open_create_modal();
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            // Edit selected profile
            app.open_edit_modal();
        }
        KeyCode::Char('c') => {
            // Duplicate selected profile
            app.open_duplicate_modal();
        }
        KeyCode::Char('d') => {
            // Delete selected profile
            app.open_delete_modal();
        }
        KeyCode::Char('s') | KeyCode::Char(' ') => {
            // Switch to selected profile
            if let Some(idx) = app.selected() {
                return ProfileAction::Switch(idx);
            }
        }
        _ => {}
    }

    ProfileAction::None
}

/// Handle keyboard input in modal mode
fn handle_modal_key(app: &mut ProfileApp, key: KeyEvent) -> ProfileAction {
    match app.modal {
        Some(ModalType::DeleteConfirm) => handle_delete_confirm_key(app, key),
        Some(ModalType::CreateProfile)
        | Some(ModalType::EditProfile)
        | Some(ModalType::DuplicateProfile) => handle_form_key(app, key),
        None => ProfileAction::None,
    }
}

fn handle_delete_confirm_key(app: &mut ProfileApp, key: KeyEvent) -> ProfileAction {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            if let Some(idx) = app.target_profile_idx {
                return ProfileAction::Delete(idx);
            }
            app.close_modal();
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.close_modal();
        }
        _ => {}
    }
    ProfileAction::None
}

fn handle_form_key(app: &mut ProfileApp, key: KeyEvent) -> ProfileAction {
    let Some(ref mut form) = app.form_state else {
        return ProfileAction::None;
    };

    let Some(ref mut profile) = app.edit_profile else {
        return ProfileAction::None;
    };

    // Handle edit mode separately
    if form.mode == FormMode::Editing {
        match key.code {
            KeyCode::Esc => {
                form.cancel_edit();
            }
            KeyCode::Enter => {
                let committed = form.commit_edit(profile);
                let error_msg = form.error.clone();

                if committed {
                    app.set_status("Field updated");
                } else if let Some(error) = error_msg {
                    app.set_status(&format!("Error: {}", error));
                }
            }
            KeyCode::Backspace => {
                form.delete_char();
            }
            KeyCode::Left => {
                form.move_cursor_left();
            }
            KeyCode::Right => {
                form.move_cursor_right();
            }
            KeyCode::Home => {
                form.move_to_start();
            }
            KeyCode::End => {
                form.move_to_end();
            }
            KeyCode::Char(c) => {
                form.insert_char(c);
            }
            _ => {}
        }
    } else {
        // Navigation mode
        match key.code {
            KeyCode::Esc => {
                app.close_modal();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                form.prev_field();
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab
                if !key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                form.next_field();
            }
            KeyCode::BackTab => {
                form.prev_field();
            }
            KeyCode::Enter => {
                form.start_editing(profile);
            }
            KeyCode::Char(' ') => {
                // Toggle boolean fields
                form.toggle_boolean(profile);
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+S: Submit form
                return submit_form(app);
            }
            _ => {}
        }
    }

    ProfileAction::None
}

fn submit_form(app: &mut ProfileApp) -> ProfileAction {
    let Some(ref profile) = app.edit_profile else {
        return ProfileAction::None;
    };

    // Validate the profile
    if let Err(e) = profile.validate() {
        app.set_status(&format!("Validation error: {}", e));
        return ProfileAction::None;
    }

    match app.modal {
        Some(ModalType::CreateProfile) | Some(ModalType::DuplicateProfile) => {
            ProfileAction::Create(profile.clone())
        }
        Some(ModalType::EditProfile) => {
            if let Some(idx) = app.target_profile_idx {
                ProfileAction::Update {
                    index: idx,
                    profile: profile.clone(),
                }
            } else {
                ProfileAction::None
            }
        }
        _ => ProfileAction::None,
    }
}
