use std::{num::NonZeroU32, str::FromStr};

use serde::{Deserialize, Serialize};

// WHY these specific limits: MAX_INPUT reflects typical LLM context windows (256K tokens
// is common for models like GPT-4 and Claude 3+). MAX_OUTPUT is conservative to prevent
// runaway generation costs - commit messages should be concise, not essays.
pub const MAX_INPUT: u32 = 256_000;
pub const MAX_OUTPUT: u32 = 4_096;

/// Count of tokens in text, guaranteed to be non-zero.
///
/// WHY NonZeroU32: Empty text is never valid input for token-counting operations.
/// By using NonZeroU32, we:
/// 1. Eliminate defensive checks for zero throughout the codebase
/// 2. Enable compiler optimizations (Option<TokenCount> is the same size as u32)
/// 3. Make the domain constraint explicit in the type system
///
/// WHY saturating arithmetic: When token counts are computed from potentially malicious
/// or corrupted input, clamping to NonZeroU32::MIN (1) is safer than panicking or
/// wrapping. This ensures the system remains operational even with bad data, trading
/// precision for resilience.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(try_from = "u32", into = "u32")]
pub struct TokenCount(NonZeroU32);

impl TokenCount {
    pub fn new(count: u32) -> Option<Self> {
        NonZeroU32::new(count).map(Self)
    }

    /// Creates a TokenCount from a u32, clamping zero to 1.
    ///
    /// This method ensures a non-zero result by converting 0 to 1. Use this when
    /// you need a guaranteed non-zero value and treating empty input as "at least
    /// one token" is acceptable (e.g., for limits where zero would be nonsensical).
    ///
    /// For exact counts that may be zero (e.g., empty strings), use
    /// [`Self::zero_or_at_least_one`] or the tokenizer's `count_tokens_exact()`
    /// method to preserve the zero value before converting to a TokenCount.
    ///
    /// For cases where zero should remain distinct (e.g., counting actual tokens
    /// in empty text), use [`Self::new`] which returns `Option<TokenCount>`.
    pub fn new_at_least_one(value: u32) -> Self {
        NonZeroU32::new(value)
            .map(Self)
            .unwrap_or(Self(NonZeroU32::MIN))
    }

    /// Creates a TokenCount that is zero-aware.
    ///
    /// Returns `None` when the input is 0, allowing callers to preserve the
    /// distinction between empty content and non-zero token counts.
    pub fn zero_or_at_least_one(value: u32) -> Option<Self> {
        Self::new(value)
    }

    pub fn try_from_usize(value: usize) -> Result<Self, String> {
        u32::try_from(value)
            .map_err(|_| "Value out of range for u32".to_string())
            .and_then(Self::try_from)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl TryFrom<u32> for TokenCount {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value).ok_or_else(|| "Token count must be greater than zero".to_string())
    }
}

impl From<TokenCount> for u32 {
    fn from(value: TokenCount) -> Self {
        value.get()
    }
}

impl From<TokenCount> for usize {
    fn from(value: TokenCount) -> Self {
        value.get() as usize
    }
}

impl FromStr for TokenCount {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value: u32 = s.parse().map_err(|_| "Invalid token count".to_string())?;
        Self::try_from(value)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn token_count_new_valid() {
        assert!(TokenCount::new(1).is_some());
        assert!(TokenCount::new(100).is_some());
        assert!(TokenCount::new(MAX_INPUT).is_some());
    }

    #[test]
    fn token_count_new_zero_fails() {
        assert!(TokenCount::new(0).is_none())
    }

    #[test]
    fn token_count_new_at_least_one() {
        assert_eq!(TokenCount::new_at_least_one(0).get(), 1);
        assert_eq!(TokenCount::new_at_least_one(50).get(), 50);
    }

    #[test]
    fn token_count_zero_or_at_least_one() {
        assert!(TokenCount::zero_or_at_least_one(0).is_none());
        assert_eq!(TokenCount::zero_or_at_least_one(5).unwrap().get(), 5);
    }

    #[test]
    fn token_count_try_from_usize() {
        assert!(TokenCount::try_from_usize(1).is_ok());
        assert!(TokenCount::try_from_usize(0).is_err());
    }

    #[test]
    fn token_count_max_values() {
        assert_eq!(MAX_INPUT, 256_000);
        assert_eq!(MAX_OUTPUT, 4_096);
    }

    #[test]
    fn token_count_conversions() {
        match TokenCount::new(42) {
            Some(tc) => {
                let as_u32: u32 = tc.into();
                let as_usize: usize = tc.into();
                assert_eq!(as_u32, 42);
                assert_eq!(as_usize, 42);
            }
            None => panic!("Failed to create TokenCount"),
        }
    }

    #[test]
    fn test_token_count_try_from() {
        assert!(TokenCount::try_from(1u32).is_ok());
        assert!(TokenCount::try_from(0u32).is_err());
    }

    #[test]
    fn test_token_count_from_str() {
        assert!("12".parse::<TokenCount>().is_ok());
        assert!("0".parse::<TokenCount>().is_err());
        assert!("abc".parse::<TokenCount>().is_err());
    }
}
