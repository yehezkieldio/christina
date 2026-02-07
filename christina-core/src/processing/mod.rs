//! Data transformation pipeline for diff processing.
//!
//! This module contains the hot-path logic for chunking diffs,
//! counting tokens, and merging user-provided context.

pub mod chunking;
pub mod context;
pub mod tokenizer;

pub use chunking::{
    LOCKFILE_TOKEN_LIMIT, should_limit_file, split_by_hunks, split_by_lines, split_recursive,
    truncate_to_token_limit,
};
pub use context::{fit_history_to_budget, fit_user_context_to_budget, normalize_user_context};
pub use tokenizer::{ByteTokenizer, TokenBudget, TokenizerService, get_tokenizer};
