//! Token counting, ported from Christina's
//! `christina-core/src/processing/tokenizer.rs` and
//! `christina-core/src/types/tokens.rs`. Charlotte keeps the same
//! `o200k_base` encoding via `tiktoken-rs`, so counts match Christina's
//! exactly for the same input.
//!
//! Simplification: Christina layers a `moka` LRU cache over the raw BPE call
//! for hot-path repeat counts. That cache is a measured optimization over
//! there; here it would be an unmeasured one, so this module skips it and
//! calls the tokenizer directly. Add it back if a benchmark on a real
//! Charlotte workload shows it matters.

use std::num::NonZeroU32;
use std::sync::OnceLock;

use tiktoken_rs::CoreBPE;

/// Count of tokens in text, guaranteed non-zero. Mirrors Christina's
/// `TokenCount` newtype: `new_at_least_one` clamps 0 up to 1 so a caller
/// that needs "at least one token" never has to special-case empty input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenCount(NonZeroU32);

impl TokenCount {
    pub fn new(count: u32) -> Option<Self> {
        NonZeroU32::new(count).map(Self)
    }

    pub fn new_at_least_one(count: u32) -> Self {
        NonZeroU32::new(count).map(Self).unwrap_or(Self(NonZeroU32::MIN))
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl std::ops::Add for TokenCount {
    type Output = TokenCount;
    fn add(self, rhs: TokenCount) -> TokenCount {
        TokenCount::new_at_least_one(self.get().saturating_add(rhs.get()))
    }
}

fn bpe() -> &'static CoreBPE {
    static BPE: OnceLock<CoreBPE> = OnceLock::new();
    BPE.get_or_init(|| {
        // `o200k_base` is the encoding GPT-4o and newer OpenAI models use;
        // matching Christina's choice keeps token counts comparable.
        tiktoken_rs::o200k_base().unwrap_or_else(|_| {
            // Loading a bundled, version-pinned encoding table cannot fail
            // in practice; a failure here means the crate itself is broken.
            panic!("failed to load the o200k_base tiktoken encoding")
        })
    })
}

/// Plain token count for the FFI boundary (`02-native-core-and-ffi.md`
/// exposes `count_tokens(text) -> number`, not the `TokenCount` newtype).
pub fn count_tokens(text: &str) -> u32 {
    bpe().encode_ordinary(text).len() as u32
}

pub fn encode(text: &str) -> Vec<u32> {
    bpe().encode_ordinary(text)
}

pub fn decode(tokens: &[u32]) -> Option<String> {
    bpe().decode(tokens.to_vec()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_count_new_zero_fails() {
        assert!(TokenCount::new(0).is_none());
    }

    #[test]
    fn token_count_new_at_least_one_clamps_zero() {
        assert_eq!(TokenCount::new_at_least_one(0).get(), 1);
        assert_eq!(TokenCount::new_at_least_one(50).get(), 50);
    }

    #[test]
    fn count_tokens_nonempty_text_is_nonzero() {
        assert!(count_tokens("hello world") > 0);
    }

    #[test]
    fn count_tokens_empty_text_is_zero() {
        assert_eq!(count_tokens(""), 0);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let text = "fn main() { println!(\"hi\"); }";
        let tokens = encode(text);
        assert_eq!(decode(&tokens).as_deref(), Some(text));
    }
}
