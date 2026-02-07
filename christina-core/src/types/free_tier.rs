//! Usage-tier limits for conservative defaults.
//!
//! WHY in core: these constants shape prompt budgeting and concurrency, so they
//! must be shared across CLI config parsing and runtime enforcement.

use serde::{Deserialize, Serialize};
use super::TokenCount;

const FREE_TIER_MAX_INPUT_TOKENS: u32 = 16_000;
const FREE_TIER_MAX_OUTPUT_TOKENS: u32 = 512;
const FREE_TIER_MAX_CONCURRENT_REQUESTS: usize = 1;
const FREE_TIER_COMMIT_HISTORY_DEPTH: usize = 3;

/// Limits applied to the free usage tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct FreeTierLimits {
    pub max_input_tokens: TokenCount,
    pub max_output_tokens: TokenCount,
    pub max_concurrent_requests: usize,
    pub commit_history_depth: usize,
}

impl Default for FreeTierLimits {
    fn default() -> Self {
        Self {
            // Keep inputs modest to avoid saturating free-tier provider quotas.
            max_input_tokens: TokenCount::new_at_least_one(FREE_TIER_MAX_INPUT_TOKENS),
            max_output_tokens: TokenCount::new_at_least_one(FREE_TIER_MAX_OUTPUT_TOKENS),
            max_concurrent_requests: FREE_TIER_MAX_CONCURRENT_REQUESTS,
            commit_history_depth: FREE_TIER_COMMIT_HISTORY_DEPTH,
        }
    }
}
