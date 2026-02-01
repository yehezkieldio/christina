#[derive(Debug, Clone, Default)]
pub struct EditingState {
    pub content: String,
    pub cursor_line: usize,
    pub cursor_column: usize,
}
