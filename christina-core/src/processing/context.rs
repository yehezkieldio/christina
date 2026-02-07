//! User-provided context merging and budget fitting.
//!
//! Functions for normalizing, truncating, and fitting user context
//! and commit history into the available token budget.

use crate::prompt::{USER_CONTEXT_MAX_LEN, USER_CONTEXT_TEMPLATE};
use crate::tokenizer::Tokenizer;
use crate::types::tokens::TokenCount;

const HISTORY_CONTEXT_PREFIX: &str = "\n\nRecent commit history for style reference:\n";

/// Normalize raw user context: trim, enforce max length, respect UTF-8 boundaries.
pub fn normalize_user_context(raw: Option<String>) -> Option<String> {
    let ctx = raw?;
    let trimmed = ctx.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.len() <= USER_CONTEXT_MAX_LEN {
        return Some(trimmed.to_string());
    }

    let mut end = USER_CONTEXT_MAX_LEN;
    while end > 0 && !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    Some(trimmed[..end].to_string())
}

fn user_context_template_parts() -> (&'static str, &'static str) {
    // Split once so we can count overhead tokens without allocating a new string.
    if let Some(pos) = USER_CONTEXT_TEMPLATE.find("{context}") {
        (
            &USER_CONTEXT_TEMPLATE[..pos],
            &USER_CONTEXT_TEMPLATE[pos + 9..],
        )
    } else {
        ("", "")
    }
}

/// Fit user context into the available token budget.
///
/// Returns (effective_context, tokens_used, was_truncated).
pub fn fit_user_context_to_budget(
    tokenizer: &dyn Tokenizer,
    context: Option<String>,
    budget_tokens: u32,
) -> (Option<String>, u32, bool) {
    let Some(context) = context else {
        return (None, 0, false);
    };

    if budget_tokens == 0 {
        return (None, 0, true);
    }

    let (prefix, suffix) = user_context_template_parts();
    let prefix_tokens = tokenizer.count_tokens_exact(prefix);
    let suffix_tokens = tokenizer.count_tokens_exact(suffix);
    // Template overhead must be reserved before any user-provided text.
    let overhead = prefix_tokens.saturating_add(suffix_tokens);

    if budget_tokens <= overhead {
        return (None, 0, true);
    }

    let allowed_context_tokens = budget_tokens - overhead;
    let context_tokens = tokenizer.count_tokens_exact(&context);

    if context_tokens <= allowed_context_tokens {
        let used = overhead.saturating_add(context_tokens);
        return (Some(context), used, false);
    }

    let allowed = TokenCount::new_at_least_one(allowed_context_tokens);
    let truncated = tokenizer.slice_to_token_limit(&context, allowed).trim_end();
    if truncated.is_empty() {
        return (None, 0, true);
    }
    let truncated_tokens = tokenizer.count_tokens_exact(truncated);
    let used = overhead.saturating_add(truncated_tokens);
    (Some(truncated.to_string()), used, true)
}

/// Fit commit history into the available token budget.
///
/// Returns (effective_history, tokens_used, was_truncated).
pub fn fit_history_to_budget(
    tokenizer: &dyn Tokenizer,
    history: Option<String>,
    budget_tokens: u32,
) -> (Option<String>, u32, bool) {
    let Some(history) = history else {
        return (None, 0, false);
    };

    if budget_tokens == 0 {
        return (None, 0, true);
    }

    let prefix_tokens = tokenizer.count_tokens_exact(HISTORY_CONTEXT_PREFIX);
    if budget_tokens <= prefix_tokens {
        return (None, 0, true);
    }

    let allowed_history_tokens = budget_tokens - prefix_tokens;
    let history_tokens = tokenizer.count_tokens_exact(&history);

    if history_tokens <= allowed_history_tokens {
        let used = prefix_tokens.saturating_add(history_tokens);
        return (Some(history), used, false);
    }

    let allowed = TokenCount::new_at_least_one(allowed_history_tokens);
    let truncated = tokenizer.slice_to_token_limit(&history, allowed);
    let truncated = truncated
        .rfind('\n')
        .map(|idx| &truncated[..idx])
        .unwrap_or(truncated)
        .trim_end();

    if truncated.is_empty() {
        return (None, 0, true);
    }

    let truncated_tokens = tokenizer.count_tokens_exact(truncated);
    let used = prefix_tokens.saturating_add(truncated_tokens);
    (Some(truncated.to_string()), used, true)
}
