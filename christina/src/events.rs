use christina_core::types::TokenCount;

#[derive(Debug)]
pub enum Event {
    GenerationProgress { stage: String },
    TokenCountUpdate { token_count: TokenCount },
}
