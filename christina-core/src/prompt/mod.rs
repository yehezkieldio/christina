//! Prompt template generation for LLM interactions.
//!
//! Contains embedded prompt assets and a builder for composing
//! prompts from diff content, summaries, and themes.

pub mod builder;
pub mod templates;

pub use builder::{PromptBuilder, Theme};
pub use templates::{
    DIRECT_COMMIT_PROMPT, INTENT_EXTRACTION_PROMPT, SUMMARY_PROMPT, SYSTEM_PROMPT,
    THEME_SYNTHESIS_PROMPT, USER_CONTEXT_MAX_LEN, USER_CONTEXT_TEMPLATE,
};
