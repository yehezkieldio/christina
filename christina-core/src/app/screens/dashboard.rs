use compact_str::CompactString;

#[derive(Debug, Clone, Default)]
pub struct DashboardState {
    pub generated_message: CompactString,
    pub edit_history: Vec<String>,
    pub cursor_position: usize,
}
