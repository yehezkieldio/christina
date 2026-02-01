use crate::tui::form::{Editable, FieldDef, FieldType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormMode {
    Navigation,
    Editing,
}

pub struct FormState {
    /// Current field index
    pub cursor: usize,
    /// Current mode
    pub mode: FormMode,
    /// Edit buffer for current field
    pub edit_buffer: String,
    /// Edit cursor position (char index in buffer)
    pub edit_cursor: usize,
    /// Validation error message (if any)
    pub error: Option<String>,
    /// Cached field definitions
    fields: Vec<FieldDef>,
}

impl FormState {
    pub fn new<T: Editable>(editable: &T) -> Self {
        Self::with_fields(editable.fields())
    }

    pub fn with_fields(fields: Vec<FieldDef>) -> Self {
        Self {
            cursor: 0,
            mode: FormMode::Navigation,
            edit_buffer: String::new(),
            edit_cursor: 0,
            error: None,
            fields,
        }
    }

    pub fn current_field(&self) -> Option<&FieldDef> {
        self.fields.get(self.cursor)
    }

    pub fn fields(&self) -> &[FieldDef] {
        &self.fields
    }

    pub fn next_field(&mut self) {
        if self.cursor < self.fields.len().saturating_sub(1) {
            self.cursor += 1;
        }
    }

    pub fn prev_field(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn start_editing<T: Editable>(&mut self, editable: &T) {
        if let Some(field) = self.current_field()
            && !field.read_only
        {
            self.edit_buffer = editable.get_field(&field.key).unwrap_or_default();
            self.edit_cursor = self.edit_buffer.len();
            self.mode = FormMode::Editing;
            self.error = None;
        }
    }

    pub fn commit_edit<T: Editable>(&mut self, editable: &mut T) -> bool {
        if self.mode != FormMode::Editing {
            return false;
        }

        if let Some(field) = self.current_field() {
            match editable.set_field(&field.key, &self.edit_buffer) {
                Ok(()) => {
                    self.mode = FormMode::Navigation;
                    self.error = None;

                    // Validate after change
                    if let Err(e) = editable.validate() {
                        self.error = Some(e.to_string());
                    }

                    true
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    false
                }
            }
        } else {
            false
        }
    }

    pub fn cancel_edit(&mut self) {
        self.mode = FormMode::Navigation;
        self.edit_buffer.clear();
        self.edit_cursor = 0;
    }

    pub fn insert_char(&mut self, c: char) {
        if self.mode == FormMode::Editing {
            self.edit_buffer.insert(self.edit_cursor, c);
            self.edit_cursor += c.len_utf8();
        }
    }

    pub fn delete_char(&mut self) {
        if self.mode == FormMode::Editing && self.edit_cursor > 0 {
            let prev_char_boundary = self.edit_buffer[..self.edit_cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.edit_buffer.remove(prev_char_boundary);
            self.edit_cursor = prev_char_boundary;
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.mode == FormMode::Editing && self.edit_cursor > 0 {
            self.edit_cursor = self.edit_buffer[..self.edit_cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.mode == FormMode::Editing && self.edit_cursor < self.edit_buffer.len() {
            self.edit_cursor = self.edit_buffer[self.edit_cursor..]
                .char_indices()
                .next()
                .map(|(i, c)| self.edit_cursor + i + c.len_utf8())
                .unwrap_or(self.edit_buffer.len());
        }
    }

    pub fn move_to_start(&mut self) {
        if self.mode == FormMode::Editing {
            self.edit_cursor = 0;
        }
    }

    pub fn move_to_end(&mut self) {
        if self.mode == FormMode::Editing {
            self.edit_cursor = self.edit_buffer.len();
        }
    }

    pub fn toggle_boolean<T: Editable>(&mut self, editable: &mut T) {
        if let Some(field) = self.current_field()
            && matches!(field.field_type, FieldType::Boolean)
        {
            let current = editable.get_field(&field.key).unwrap_or_default();
            let new_value = if current == "true" { "false" } else { "true" };
            let _ = editable.set_field(&field.key, new_value);

            // Validate after change
            if let Err(e) = editable.validate() {
                self.error = Some(e.to_string());
            } else {
                self.error = None;
            }
        }
    }
}
