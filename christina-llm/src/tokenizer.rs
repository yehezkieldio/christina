use std::{num::NonZeroUsize, sync::OnceLock};

use christina_core::{Tokenizer, types::TokenCount};
use parking_lot::Mutex;
use thiserror::Error;
use tiktoken_rs::CoreBPE;

#[derive(Debug, Error)]
pub enum TokenizerError {
    #[error("Tokenizer error: {0}")]
    Tokenizer(String),
}

pub type Result<T> = std::result::Result<T, TokenizerError>;

static TOKENIZER: OnceLock<TokenizerService> = OnceLock::new();

/// Get the global tokenizer service instance.
///
/// This initializes the tokenizer on first call and returns a reference
/// to the same instance on subsequent calls.
pub fn get_tokenizer() -> Result<&'static TokenizerService> {
    match TOKENIZER.get() {
        Some(t) => Ok(t),
        None => {
            let tokenizer = TokenizerService::new()?;
            match TOKENIZER.set(tokenizer) {
                // After successful set(), get() is guaranteed to return Some.
                // After failed set() (Err case), another thread succeeded, so get() returns Some.
                #[expect(clippy::unwrap_used, reason = "OnceLock guarantees value after set")]
                Ok(_) => Ok(TOKENIZER.get().unwrap()),
                #[expect(
                    clippy::unwrap_used,
                    reason = "Another thread set it, so get() is Some"
                )]
                Err(_) => Ok(TOKENIZER.get().unwrap()),
            }
        }
    }
}

const TOKEN_CACHE_CAPACITY: usize = 10_000;

pub struct TokenizerService {
    bpe: CoreBPE,
    /// LRU cache for token counts, keyed by content string.
    /// Stores content directly to eliminate hash collision risk.
    token_cache: Mutex<lru::LruCache<String, TokenCount>>,
}

impl TokenizerService {
    pub fn new() -> Result<Self> {
        let bpe = tiktoken_rs::o200k_base()
            .map_err(|e| TokenizerError::Tokenizer(format!("Failed to load o200k_base: {}", e)))?;
        #[expect(
            clippy::unwrap_used,
            reason = "TOKEN_CACHE_CAPACITY is non-zero constant"
        )]
        let cap = NonZeroUsize::new(TOKEN_CACHE_CAPACITY).unwrap();
        Ok(Self {
            bpe,
            token_cache: Mutex::new(lru::LruCache::new(cap)),
        })
    }

    #[inline]
    pub fn count_tokens(&self, text: &str) -> TokenCount {
        // Skip cache for very short strings (cache overhead > tokenization cost)
        if text.len() < 50 {
            let count = self.bpe.encode_ordinary(text).len();
            return TokenCount::new_saturating(count as u32);
        }

        // Single lock scope for cache check and potential insert
        let mut cache = self.token_cache.lock();

        // Try cache first
        if let Some(&count) = cache.get(text) {
            return count;
        }

        // Cache miss, compute token count
        // Drop lock temporarily for expensive computation
        drop(cache);
        let count = self.bpe.encode_ordinary(text).len();
        let token_count = TokenCount::new_saturating(count as u32);

        // Re-acquire lock and store in cache
        let mut cache = self.token_cache.lock();
        cache.put(text.to_string(), token_count);

        token_count
    }

    pub fn encode(&self, text: &str) -> Vec<u32> {
        self.bpe.encode_ordinary(text)
    }

    pub fn decode(&self, tokens: &[u32]) -> Result<String> {
        self.bpe
            .decode(tokens.to_vec())
            .map_err(|e| TokenizerError::Tokenizer(format!("Failed to decode tokens: {}", e)))
    }

    /// Slice a string to fit within a token limit.
    ///
    /// This method ensures that the returned slice:
    /// 1. Does not exceed the specified token limit
    /// 2. Ends at a valid UTF-8 boundary
    /// 3. Preferably ends at a line boundary for readability
    pub fn slice_to_token_limit<'a>(&self, text: &'a str, limit: TokenCount) -> &'a str {
        let tokens = self.bpe.encode_ordinary(text);
        if tokens.len() <= limit.get() as usize {
            return text;
        }

        // Binary search for the right slice point
        let mut low = 0;
        let mut high = text.len();
        let mut best = 0;

        while low < high {
            let mid = (low + high).div_ceil(2);

            // Find a valid UTF-8 boundary at or before mid
            let boundary = text
                .char_indices()
                .take_while(|(i, _)| *i <= mid)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0);

            let slice = &text[..boundary];
            let token_count = self.bpe.encode_ordinary(slice).len();

            if token_count <= limit.get() as usize {
                best = boundary;
                low = mid;
            } else {
                high = mid - 1;
            }
        }

        // Try to end at a line boundary for cleaner output
        let result = &text[..best];
        if let Some(last_newline) = result.rfind('\n') {
            // Only use line boundary if it doesn't lose too much content
            let line_slice = &result[..=last_newline];
            let line_tokens = self.bpe.encode_ordinary(line_slice).len();
            // Keep line boundary if we retain at least 80% of tokens
            if line_tokens >= (limit.get() as usize * 4) / 5 {
                return line_slice;
            }
        }

        result
    }
}

/// Token budget management for AI model context windows.
/// Tracks the allocation of tokens across prompt, diff content, and response.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Maximum tokens the model can accept as input.
    pub max_input: TokenCount,
    /// Maximum tokens the model can generate as output.
    pub max_output: TokenCount,
    /// Tokens reserved for the system prompt and instructions.
    pub reserved_for_prompt: TokenCount,
    /// Tokens reserved for user messages and formatting.
    pub reserved_for_messages: TokenCount,
}

impl TokenBudget {
    pub fn new(
        max_input: TokenCount,
        max_output: TokenCount,
        reserved_for_prompt: TokenCount,
        reserved_for_messages: TokenCount,
    ) -> Self {
        Self {
            max_input,
            max_output,
            reserved_for_prompt,
            reserved_for_messages,
        }
    }

    pub fn small() -> Self {
        Self {
            max_input: TokenCount::new_saturating(32_000),
            max_output: TokenCount::new_saturating(4_096),
            reserved_for_prompt: TokenCount::new_saturating(1_000),
            reserved_for_messages: TokenCount::new_saturating(500),
        }
    }

    pub fn medium() -> Self {
        Self {
            max_input: TokenCount::new_saturating(128_000),
            max_output: TokenCount::new_saturating(4_096),
            reserved_for_prompt: TokenCount::new_saturating(1_000),
            reserved_for_messages: TokenCount::new_saturating(500),
        }
    }

    pub fn large() -> Self {
        Self {
            max_input: TokenCount::new_saturating(256_000),
            max_output: TokenCount::new_saturating(4_096),
            reserved_for_prompt: TokenCount::new_saturating(1_000),
            reserved_for_messages: TokenCount::new_saturating(500),
        }
    }

    pub fn massive() -> Self {
        Self {
            max_input: TokenCount::new_saturating(1_000_000),
            max_output: TokenCount::new_saturating(4_096),
            reserved_for_prompt: TokenCount::new_saturating(1_000),
            reserved_for_messages: TokenCount::new_saturating(500),
        }
    }

    /// Calculate the remaining budget available for diff content.
    #[inline]
    pub fn remaining_for_diff(&self) -> std::result::Result<TokenCount, String> {
        let reserved = self
            .max_output
            .get()
            .checked_add(self.reserved_for_prompt.get())
            .and_then(|sum| sum.checked_add(self.reserved_for_messages.get()))
            .ok_or_else(|| "Token budget overflow during calculation".to_string())?;

        if reserved > self.max_input.get() {
            return Err(format!(
                "Invalid token budget: max_output ({}) + reserved_for_prompt ({}) + reserved_for_messages ({}) = {} exceeds max_input ({})",
                self.max_output.get(),
                self.reserved_for_prompt.get(),
                self.reserved_for_messages.get(),
                reserved,
                self.max_input.get()
            ));
        }

        TokenCount::new(self.max_input.get() - reserved)
            .ok_or_else(|| "Token budget underflow during calculation".to_string())
    }
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self::medium()
    }
}

impl Tokenizer for TokenizerService {
    fn count_tokens(&self, text: &str) -> TokenCount {
        self.count_tokens(text)
    }

    fn encoding_name(&self) -> &str {
        "o200k_base"
    }

    fn encode(&self, text: &str) -> Vec<u32> {
        self.encode(text)
    }

    fn decode(&self, tokens: &[u32]) -> Option<String> {
        TokenizerService::decode(self, tokens).ok()
    }

    fn slice_to_token_limit<'a>(&self, text: &'a str, limit: TokenCount) -> &'a str {
        self.slice_to_token_limit(text, limit)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_count_tokens() {
        let tokenizer = TokenizerService::new().expect("tokenizer creation should succeed");

        // Simple text should have tokens
        let count = tokenizer.count_tokens("Hello, world!");
        assert!(count.get() > 0);
        assert!(count.get() < 10); // Reasonable upper bound
    }

    #[test]
    fn tokenizer_slice_to_limit() {
        let tokenizer = TokenizerService::new().expect("tokenizer creation should succeed");
        let text = "Hello, world! This is a longer text that has many tokens.";

        // Slicing to 3 tokens should return less text
        let sliced = tokenizer.slice_to_token_limit(text, TokenCount::new_saturating(3));
        assert!(sliced.len() < text.len());
        assert!(tokenizer.count_tokens(sliced).get() > 0);

        // Slicing with large limit should return full text
        let full = tokenizer.slice_to_token_limit(text, TokenCount::new_saturating(1000));
        assert_eq!(full, text);
    }

    #[test]
    fn tokenizer_slice_empty() {
        let tokenizer = TokenizerService::new().expect("tokenizer creation should succeed");

        // Zero limit should return empty string
        let slice = tokenizer.slice_to_token_limit("Hello", TokenCount::new_saturating(1));
        assert!(!slice.is_empty());

        // Empty input should return empty string
        assert_eq!(
            tokenizer.slice_to_token_limit("", TokenCount::new_saturating(1)),
            ""
        );
    }

    #[test]
    fn token_budget_remaining() {
        let budget = TokenBudget::new(
            TokenCount::new_saturating(100_000),
            TokenCount::new_saturating(4_000),
            TokenCount::new_saturating(1_000),
            TokenCount::new_saturating(500),
        );
        let remaining = budget.remaining_for_diff().expect("valid budget");

        // Should be max_input - max_output - reserved_for_prompt - reserved_for_messages
        assert_eq!(remaining.get(), 100_000 - 4_000 - 1_000 - 500);
    }

    #[test]
    fn token_budget_invalid_returns_error() {
        // max_output + reserved exceeds max_input
        let budget = TokenBudget::new(
            TokenCount::new_saturating(4_096),
            TokenCount::new_saturating(3_000),
            TokenCount::new_saturating(1_000),
            TokenCount::new_saturating(500),
        );
        let result = budget.remaining_for_diff();

        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.contains("exceeds max_input"));
        }
    }

    #[test]
    fn token_budget_presets() {
        let small = TokenBudget::small();
        assert_eq!(small.max_input.get(), 32_000);

        let medium = TokenBudget::medium();
        assert_eq!(medium.max_input.get(), 128_000);

        let large = TokenBudget::large();
        assert_eq!(large.max_input.get(), 256_000);

        let massive = TokenBudget::massive();
        assert_eq!(massive.max_input.get(), 1_000_000);
    }
}
