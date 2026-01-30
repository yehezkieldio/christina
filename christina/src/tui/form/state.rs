use crate::tui::form::{Editable, FieldDef, FieldType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormMode {
    Navigation,
    Editing,
}

pub struct FormState {
    /// Current field index within the current section
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
    /// Current section index
    current_section: usize,
    /// Scroll offset for the current section's field list
    scroll_offset: usize,
    /// Number of visible rows for the field list
    visible_rows: usize,
}

/// Section definitions for configuration
pub const SECTIONS: &[(&str, &'static str)] = &[
    ("general", "General"),
    ("advanced", "Advanced"),
    ("experimental", "Experimental"),
];

impl FormState {
    pub fn new<T: Editable>(editable: &T, visible_rows: usize) -> Self {
        Self {
            cursor: 0,
            mode: FormMode::Navigation,
            edit_buffer: String::new(),
            edit_cursor: 0,
            error: None,
            fields: editable.fields(),
            current_section: 0,
            scroll_offset: 0,
            visible_rows,
        }
    }

    pub fn current_section_key(&self) -> &'static str {
        SECTIONS[self.current_section].0
    }

    pub fn current_section_name(&self) -> &'static str {
        SECTIONS[self.current_section].1
    }

    pub fn current_section_index(&self) -> usize {
        self.current_section
    }

    pub fn set_section(&mut self, index: usize) {
        if index < SECTIONS.len() {
            self.current_section = index;
            self.cursor = 0;
            self.scroll_offset = 0;
        }
    }

    pub fn next_section(&mut self) {
        self.current_section = (self.current_section + 1) % SECTIONS.len();
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    pub fn prev_section(&mut self) {
        self.current_section = self.current_section.saturating_sub(1);
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    pub fn visible_rows(&self) -> usize {
        self.visible_rows
    }

    pub fn set_visible_rows(&mut self, rows: usize) {
        self.visible_rows = rows;
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Get fields filtered to current section
    pub fn current_section_fields(&self) -> Vec<&FieldDef> {
        let current_key = self.current_section_key();
        self.fields
            .iter()
            .filter(|f| f.section == Some(current_key))
            .collect()
    }

    /// Get the currently selected field from the current section
    pub fn current_field(&self) -> Option<&FieldDef> {
        let section_fields = self.current_section_fields();
        section_fields.get(self.cursor).copied()
    }



    pub fn all_fields(&self) -> &[FieldDef] {
        &self.fields
    }

    /// Navigate to next field within current section
    pub fn next_field(&mut self) {
        let section_count = self.current_section_fields().len();
        if self.cursor < section_count.saturating_sub(1) {
            self.cursor += 1;
            // Adjust scroll if needed
            if self.cursor >= self.scroll_offset + self.visible_rows {
                self.scroll_offset = self.cursor.saturating_sub(self.visible_rows.saturating_sub(1));
            }
        }
    }

    /// Navigate to previous field within current section
    pub fn prev_field(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            // Adjust scroll if needed
            if self.cursor < self.scroll_offset {
                self.scroll_offset = self.cursor;
            }
        }
    }

    /// Scroll down in current section
    pub fn scroll_down(&mut self) {
        let section_count = self.current_section_fields().len();
        let max_offset = section_count.saturating_sub(self.visible_rows);
        if self.scroll_offset < max_offset {
            self.scroll_offset += 1;
        }
    }

    /// Scroll up in current section
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
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

                    // Refresh field definitions (in case they changed based on values)
                    self.fields = editable.fields();
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
                .nth(1)
                .map(|(i, _)| self.edit_cursor + i)
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

            // Refresh field definitions in case they changed
            self.fields = editable.fields();
        }
    }
}
