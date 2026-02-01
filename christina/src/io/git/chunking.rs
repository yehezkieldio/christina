#![allow(dead_code, reason = "Chunking stub until implementation lands")]

use christina_core::{
    git::{DiffChunk, FileDiff},
    types::TokenCount,
    Tokenizer,
};

pub(crate) fn split_recursive(
    files: Vec<FileDiff>,
    token_limit: TokenCount,
    ignore_patterns: &[String],
    tokenizer: &dyn Tokenizer,
) -> Vec<DiffChunk> {
    let _ = (files, token_limit, ignore_patterns, tokenizer);
    unimplemented!("Will be implemented in chunking.rs")
}
