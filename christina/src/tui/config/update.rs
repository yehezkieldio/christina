use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::ConfigApp;
use super::runner::SaveCallback;
use crate::tui::form::FormMode;

pub fn handle_key(app: &mut ConfigApp, key: KeyEvent, on_save: &mut SaveCallback) {
    let is_editing = app.current_form_state().mode == FormMode::Editing;

    if is_editing {
        handle_edit_key(app, key);
        return;
    }

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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
            app.should_quit = true;
        }
        KeyCode::Char('1') => app.set_tab(1),
        KeyCode::Char('2') => app.set_tab(2),
        KeyCode::Char('3') => app.set_tab(3),
        KeyCode::Char('h') | KeyCode::Left => {
            app.prev_tab();
        }
        KeyCode::Char('l') | KeyCode::Right => {
            app.next_tab();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.current_form_state_mut().prev_field();
            app.status_message = None;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.current_form_state_mut().next_field();
            app.status_message = None;
        }
        KeyCode::Enter => {
            let idx = app.current_tab as usize;
            app.form_states[idx].start_editing(&app.config);
        }
        KeyCode::Char(' ') => {
            handle_toggle_boolean(app);
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            save_config(app, on_save);
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.open_profiles = true;
            app.should_quit = true;
        }
        _ => {}
    }
}

fn handle_toggle_boolean(app: &mut ConfigApp) {
    let idx = app.current_tab as usize;
    let cursor_before = app.form_states[idx].cursor;
    app.form_states[idx].toggle_boolean(&mut app.config);
    app.has_changes = true;

    if app.form_states[idx].cursor == cursor_before {
        app.set_status("Value updated (press Ctrl+S to save)");
    }
}

fn save_config(app: &mut ConfigApp, on_save: &mut SaveCallback) {
    match on_save(app.config()) {
        Ok(()) => {
            app.has_changes = false;
            app.set_status("Configuration saved!");
        }
        Err(e) => {
            app.set_status(&format!("Save failed: {}", e));
        }
    }
}

fn handle_edit_key(app: &mut ConfigApp, key: KeyEvent) {
    let idx = app.current_tab as usize;

    match key.code {
        KeyCode::Esc => {
            app.form_states[idx].cancel_edit();
        }
        KeyCode::Enter => {
            let old_provider = app.config.model_provider;
            let committed = app.form_states[idx].commit_edit(&mut app.config);

            if committed {
                app.has_changes = true;
                app.set_status("Value updated (press Ctrl+S to save)");

                if app.config.model_provider != old_provider {
                    app.refresh_fields();
                }
            } else if let Some(ref error) = app.form_states[idx].error {
                let error_msg = error.clone();
                app.set_status(&format!("Invalid value: {}", error_msg));
            }
        }
        KeyCode::Backspace => {
            app.form_states[idx].delete_char();
        }
        KeyCode::Left => {
            app.form_states[idx].move_cursor_left();
        }
        KeyCode::Right => {
            app.form_states[idx].move_cursor_right();
        }
        KeyCode::Home => {
            app.form_states[idx].move_to_start();
        }
        KeyCode::End => {
            app.form_states[idx].move_to_end();
        }
        KeyCode::Char(c) => {
            app.form_states[idx].insert_char(c);
        }
        _ => {}
    }
}
