#[derive(Debug, Clone, Default)]
pub struct GeneratingState {
    pub progress_message: Option<String>,
    pub spinner_frame: usize,
}
