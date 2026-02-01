#[derive(Debug, Clone, Default)]
pub struct StagingState {
    pub selected_indices: Vec<usize>,
    pub multi_select_mode: bool,
    pub search_query: Option<String>,
}
