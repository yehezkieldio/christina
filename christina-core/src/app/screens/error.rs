#[derive(Debug, Clone, Default)]
pub struct ErrorState {
    pub message: Option<String>,
    pub can_retry: bool,
}
