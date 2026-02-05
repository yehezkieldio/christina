use christina_core::types::TokenCount;

#[derive(Debug)]
pub enum Event {
    GenerationProgress {
        stage: String,
        #[allow(dead_code)]
        generation_id: u64,
    },
    TokenCountUpdate {
        #[allow(dead_code)]
        token_count: TokenCount,
        #[allow(dead_code)]
        generation_id: u64,
    },
}
