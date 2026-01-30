use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::ConfigApp;
use super::runner::SaveCallback;
use crate::tui::form::FormMode;

pub fn handle_key(app: &mut ConfigApp, key: KeyEvent, on_save: &mut SaveCallback) {
    // Handle edit mode separately
    if app.form_state.mode == FormMode::Editing {
        handle_edit_key(app, key);
        return;
    }

    // Normal mode (navigation)
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl+C - quit
            if app.has_changes {
                app.set_status("Unsaved changes! Press Ctrl+S to save or Shift+Q to discard");
            } else {
                app.should_quit = true;
            }
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            if app.has_changes {
                app.set_status("Unsaved changes! Press Ctrl+S to save or Shift+Q to discard");
            } else {
                app.should_quit = true;
            }
        }
        KeyCode::Char('Q') => {
            // Shift+Q - Force quit without saving
            app.should_quit = true;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.form_state.prev_field();
            app.status_message = None;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.form_state.next_field();
            app.status_message = None;
        }
        KeyCode::PageUp => {
            app.form_state.scroll_up();
            app.status_message = None;
        }
        KeyCode::PageDown => {
            app.form_state.scroll_down();
            app.status_message = None;
        }
        KeyCode::Tab | KeyCode::Right => {
            app.form_state.next_section();
            app.status_message = None;
        }
        KeyCode::Left => {
            app.form_state.prev_section();
            app.status_message = None;
        }
        KeyCode::Char('1') => {
            app.form_state.set_section(0);
            app.status_message = None;
        }
        KeyCode::Char('2') => {
            app.form_state.set_section(1);
            app.status_message = None;
        }
        KeyCode::Char('3') => {
            app.form_state.set_section(2);
            app.status_message = None;
        }
        KeyCode::Enter => {
            app.form_state.start_editing(&app.config);
        }
        KeyCode::Char(' ') => {
            // Toggle boolean fields
            app.form_state.toggle_boolean(&mut app.config);
            app.has_changes = true;
            app.set_status("Value updated (press Ctrl+S to save)");
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl+S - save
            if let Err(e) = on_save(app.config()) {
                app.set_status(&format!("Save failed: {}", e));
            } else {
                app.has_changes = false;
                app.set_status("Configuration saved!");
            }
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl+P - Open profile manager
            app.open_profiles = true;
            app.should_quit = true;
        }

        _ => {}
    }
}

/// Handle keyboard input in edit mode
fn handle_edit_key(app: &mut ConfigApp, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.form_state.cancel_edit();
        }
        KeyCode::Enter => {
            if app.form_state.commit_edit(&mut app.config) {
                app.has_changes = true;
                app.set_status("Value updated (press 's' to save)");
            } else if let Some(ref error) = app.form_state.error {
                app.set_status(&format!("Invalid value: {}", error));
            }
        }
        KeyCode::Backspace => {
            app.form_state.delete_char();
        }
        KeyCode::Left => {
            app.form_state.move_cursor_left();
        }
        KeyCode::Right => {
            app.form_state.move_cursor_right();
        }
        KeyCode::Home => {
            app.form_state.move_to_start();
        }
        KeyCode::End => {
            app.form_state.move_to_end();
        }
        KeyCode::Char(c) => {
            app.form_state.insert_char(c);
        }
        _ => {}
    }
}
