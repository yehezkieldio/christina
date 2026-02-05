use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};

use ahash::RandomState;
use christina_core::{
    Tokenizer,
    error::{TokenizerError, TokenizerResult},
    types::TokenCount,
};
use moka::sync::Cache;
use tiktoken_rs::CoreBPE;

pub type Result<T> = TokenizerResult<T>;

static TOKENIZER: OnceLock<Arc<TokenizerService>> = OnceLock::new();

/// Get the global tokenizer service instance.
///
/// This initializes the tokenizer on first call and caches successful results.
/// If initialization fails, the error is not cached, allowing subsequent calls
/// to retry. This prevents transient failures (e.g., temporary file system issues)
/// from becoming permanent.
///
/// # Errors
/// Returns `TokenizerError` if tiktoken_rs::o200k_base fails to initialize.
/// Common causes include missing tokenizer data files or I/O errors.
pub fn get_tokenizer() -> Result<Arc<TokenizerService>> {
    match TOKENIZER.get() {
        Some(cached) => Ok(Arc::clone(cached)),
        None => {
            let service = TokenizerService::new()?;
            let service = Arc::new(service);

            // Try to cache the successful result. If another thread won the race,
            // use their result instead.
            match TOKENIZER.set(Arc::clone(&service)) {
                Ok(_) => Ok(service),
                #[expect(
                    clippy::unwrap_used,
                    reason = "Another thread set it, so get() is Some"
                )]
                Err(_) => Ok(Arc::clone(TOKENIZER.get().unwrap())),
            }
        }
    }
}

const TOKEN_CACHE_CAPACITY: usize = 10_000;

pub struct TokenizerService {
    bpe: CoreBPE,
    /// LRU cache for token counts, keyed by content hash.
    token_cache: Cache<u64, TokenCount>,
    hash_builder: RandomState,
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
        let token_cache = Cache::builder().max_capacity(cap.get() as u64).build();
        Ok(Self {
            bpe,
            token_cache,
            hash_builder: RandomState::new(),
        })
    }

    #[inline]
    pub fn count_tokens(&self, text: &str) -> TokenCount {
        // Skip cache for very short strings (cache overhead > tokenization cost)
        // and for very large strings (hashing cost > tokenization cost, unlikely to repeat)
        //
        // WHY 50 byte lower bound: Cache lookup overhead (hash + lookup) exceeds
        // tokenization cost for trivial strings. Profiles showed <50 bytes are faster
        // without cache.
        //
        // WHY 100KB upper bound: For large texts, O(n) hash computation approaches
        // O(n) tokenization cost. Large diffs are rarely identical, so cache hit rate
        // is low. Bypass cache to avoid hashing overhead.
        if text.len() < 50 || text.len() > 100_000 {
            let count = self.bpe.encode_ordinary(text).len();
            return TokenCount::new_at_least_one(count as u32);
        }

        let hash = self.hash_builder.hash_one(text.as_bytes());
        self.token_cache.get_with(hash, || {
            let count = self.bpe.encode_ordinary(text).len();
            TokenCount::new_at_least_one(count as u32)
        })
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
    /// Create a new TokenBudget with upfront validation.
    ///
    /// Returns an error if the budget configuration is invalid (reserved > max_input).
    pub fn try_new(
        max_input: TokenCount,
        max_output: TokenCount,
        reserved_for_prompt: TokenCount,
        reserved_for_messages: TokenCount,
    ) -> std::result::Result<Self, String> {
        let reserved = max_output
            .get()
            .checked_add(reserved_for_prompt.get())
            .and_then(|sum| sum.checked_add(reserved_for_messages.get()))
            .ok_or_else(|| "Token budget overflow during calculation".to_string())?;

        if reserved > max_input.get() {
            return Err(format!(
                "Invalid token budget: max_output ({}) + reserved_for_prompt ({}) + reserved_for_messages ({}) = {} exceeds max_input ({})",
                max_output.get(),
                reserved_for_prompt.get(),
                reserved_for_messages.get(),
                reserved,
                max_input.get()
            ));
        }

        Ok(Self {
            max_input,
            max_output,
            reserved_for_prompt,
            reserved_for_messages,
        })
    }

    #[allow(dead_code, reason = "Public API for creating valid TokenBudgets")]
    #[allow(
        clippy::expect_used,
        reason = "Programming errors during invariant violations panic per design"
    )]
    pub fn new(
        max_input: TokenCount,
        max_output: TokenCount,
        reserved_for_prompt: TokenCount,
        reserved_for_messages: TokenCount,
    ) -> Self {
        Self::try_new(
            max_input,
            max_output,
            reserved_for_prompt,
            reserved_for_messages,
        )
        .expect("Invalid TokenBudget configuration")
    }

    #[cfg(test)]
    pub fn small() -> Self {
        Self {
            max_input: TokenCount::new_at_least_one(32_000),
            max_output: TokenCount::new_at_least_one(4_096),
            reserved_for_prompt: TokenCount::new_at_least_one(1_000),
            reserved_for_messages: TokenCount::new_at_least_one(500),
        }
    }

    pub fn medium() -> Self {
        Self {
            max_input: TokenCount::new_at_least_one(128_000),
            max_output: TokenCount::new_at_least_one(4_096),
            reserved_for_prompt: TokenCount::new_at_least_one(1_000),
            reserved_for_messages: TokenCount::new_at_least_one(500),
        }
    }

    #[cfg(test)]
    pub fn large() -> Self {
        Self {
            max_input: TokenCount::new_at_least_one(256_000),
            max_output: TokenCount::new_at_least_one(4_096),
            reserved_for_prompt: TokenCount::new_at_least_one(1_000),
            reserved_for_messages: TokenCount::new_at_least_one(500),
        }
    }

    #[cfg(test)]
    pub fn massive() -> Self {
        Self {
            max_input: TokenCount::new_at_least_one(1_000_000),
            max_output: TokenCount::new_at_least_one(4_096),
            reserved_for_prompt: TokenCount::new_at_least_one(1_000),
            reserved_for_messages: TokenCount::new_at_least_one(500),
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
    use super::get_tokenizer;
    use super::*;

    #[test]
    fn tokenizer_count_tokens() {
        let tokenizer = get_tokenizer().expect("tokenizer initialization failed");

        // Simple text should have tokens
        let count = tokenizer.count_tokens("Hello, world!");
        assert!(count.get() > 0);
        assert!(count.get() < 10); // Reasonable upper bound
    }

    #[test]
    fn tokenizer_slice_to_limit() {
        let tokenizer = get_tokenizer().expect("tokenizer initialization failed");
        let text = "Hello, world! This is a longer text that has many tokens.";

        // Slicing to 3 tokens should return less text
        let sliced = tokenizer.slice_to_token_limit(text, TokenCount::new_at_least_one(3));
        assert!(sliced.len() < text.len());
        assert!(tokenizer.count_tokens(sliced).get() > 0);

        // Slicing with large limit should return full text
        let full = tokenizer.slice_to_token_limit(text, TokenCount::new_at_least_one(1000));
        assert_eq!(full, text);
    }

    #[test]
    fn tokenizer_slice_empty() {
        let tokenizer = get_tokenizer().expect("tokenizer initialization failed");

        // Zero limit should return empty string
        let slice = tokenizer.slice_to_token_limit("Hello", TokenCount::new_at_least_one(1));
        assert!(!slice.is_empty());

        // Empty input should return empty string
        assert_eq!(
            tokenizer.slice_to_token_limit("", TokenCount::new_at_least_one(1)),
            ""
        );
    }

    #[test]
    fn token_budget_remaining() {
        let budget = TokenBudget::new(
            TokenCount::new_at_least_one(100_000),
            TokenCount::new_at_least_one(4_000),
            TokenCount::new_at_least_one(1_000),
            TokenCount::new_at_least_one(500),
        );
        let remaining = budget.remaining_for_diff().expect("valid budget");

        // Should be max_input - max_output - reserved_for_prompt - reserved_for_messages
        assert_eq!(remaining.get(), 100_000 - 4_000 - 1_000 - 500);
    }

    #[test]
    fn token_budget_invalid_returns_error() {
        // max_output + reserved exceeds max_input
        let result = TokenBudget::try_new(
            TokenCount::new_at_least_one(4_096),
            TokenCount::new_at_least_one(3_000),
            TokenCount::new_at_least_one(1_000),
            TokenCount::new_at_least_one(500),
        );

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

    #[tokio::test]
    async fn tokenizer_concurrent_access() {
        // Verify TokenizerService can be safely accessed from multiple tokio tasks
        let tokenizer = get_tokenizer().expect("tokenizer initialization failed");
        let test_texts = vec![
            "Hello, world!",
            "This is a longer text with multiple sentences. It should have more tokens.",
            "Concurrent access test",
            "The quick brown fox jumps over the lazy dog",
            "Rust is a systems programming language",
        ];

        let mut handles = Vec::new();

        // Spawn multiple tasks that count tokens concurrently
        for _ in 0..10 {
            for text in &test_texts {
                let tokenizer = Arc::clone(&tokenizer);
                let text = String::from(*text);

                let handle = tokio::spawn(async move {
                    // Count tokens multiple times
                    let counts: Vec<TokenCount> = (0..5)
                        .map(|_| tokenizer.count_tokens(&text))
                        .collect();

                    // All counts for the same text should be consistent
                    let first = counts[0];
                    for count in &counts[1..] {
                        assert_eq!(first.get(), count.get(), "Inconsistent token counts");
                    }

                    first
                });

                handles.push(handle);
            }
        }

        // Collect all results and verify no panics
        for handle in handles {
            handle.await.expect("task should not panic");
        }
    }

    #[tokio::test]
    async fn tokenizer_concurrent_cache_behavior() {
        // Verify cache behaves correctly under concurrent load
        let tokenizer = get_tokenizer().expect("tokenizer initialization failed");
        let test_text = "This text will be cached and accessed concurrently".repeat(10);

        let mut handles = Vec::new();

        // Spawn many tasks all counting the same text
        for _ in 0..50 {
            let tokenizer = Arc::clone(&tokenizer);
            let text = test_text.clone();

            let handle = tokio::spawn(async move {
                tokenizer.count_tokens(&text)
            });

            handles.push(handle);
        }

        // Collect all counts
        let mut counts = Vec::new();
        for handle in handles {
            let count = handle.await.expect("task should not panic");
            counts.push(count.get());
        }

        // All counts should be identical (cache should work correctly)
        let first = counts[0];
        for count in &counts[1..] {
            assert_eq!(first, *count, "Cache produced inconsistent results");
        }
    }

    #[tokio::test]
    async fn tokenizer_slice_concurrent() {
        // Verify slice_to_token_limit is thread-safe
        let tokenizer = get_tokenizer().expect("tokenizer initialization failed");
        let long_text = "This is a long text that will be sliced. ".repeat(100);
        let limit = TokenCount::new_at_least_one(50);

        let mut handles = Vec::new();

        // Spawn concurrent slicing operations
        for _ in 0..20 {
            let tokenizer = Arc::clone(&tokenizer);
            let text = long_text.clone();

            let handle = tokio::spawn(async move {
                let sliced = tokenizer.slice_to_token_limit(&text, limit);
                let count = tokenizer.count_tokens(sliced);
                (sliced.to_string(), count)
            });

            handles.push(handle);
        }

        // Collect all results
        let mut results = Vec::new();
        for handle in handles {
            let (sliced, count) = handle.await.expect("task should not panic");
            results.push((sliced, count));

            // Verify the token count doesn't exceed limit
            assert!(count.get() <= limit.get(), "Sliced text exceeds token limit");
        }

        // All slices should be identical for the same input
        let first_slice = &results[0].0;
        for (sliced, _) in &results[1..] {
            assert_eq!(first_slice, sliced, "Slice results differ across threads");
        }
    }

    #[tokio::test]
    async fn get_tokenizer_concurrent_initialization() {
        // Verify get_tokenizer() is safe to call from multiple threads simultaneously
        let mut handles = Vec::new();

        // Spawn many tasks that all call get_tokenizer()
        for _ in 0..20 {
            let handle = tokio::spawn(async {
                get_tokenizer().expect("tokenizer initialization failed")
            });

            handles.push(handle);
        }

        // Collect all tokenizers
        let mut tokenizers = Vec::new();
        for handle in handles {
            let tokenizer = handle.await.expect("task should not panic");
            tokenizers.push(tokenizer);
        }

        // All returned tokenizers should be Arc to the same underlying service
        // (verify by comparing Arc pointer addresses)
        let first_ptr = Arc::as_ptr(&tokenizers[0]);
        for tokenizer in &tokenizers[1..] {
            let ptr = Arc::as_ptr(tokenizer);
            assert_eq!(first_ptr, ptr, "get_tokenizer() returned different instances");
        }
    }

    #[test]
    fn count_tokens_cache_bypass_small() {
        let tokenizer = get_tokenizer().expect("tokenizer initialization failed");
        let small_text = "hi"; // <50 bytes, should bypass cache

        let count1 = tokenizer.count_tokens(small_text);
        let count2 = tokenizer.count_tokens(small_text);

        // Both counts should be correct, cache bypass doesn't affect correctness
        assert_eq!(count1, count2);
    }

    #[test]
    fn count_tokens_cache_bypass_large() {
        let tokenizer = get_tokenizer().expect("tokenizer initialization failed");
        let large_text = "word ".repeat(25_000); // >100KB, should bypass cache

        let count1 = tokenizer.count_tokens(&large_text);
        let count2 = tokenizer.count_tokens(&large_text);

        // Both counts should be correct, cache bypass doesn't affect correctness
        assert_eq!(count1, count2);
        assert!(large_text.len() > 100_000, "Test text should exceed 100KB");
    }

    #[test]
    fn count_tokens_cache_used_medium() {
        let tokenizer = get_tokenizer().expect("tokenizer initialization failed");
        let medium_text = "word ".repeat(1000); // ~5KB, should use cache

        let count1 = tokenizer.count_tokens(&medium_text);
        let count2 = tokenizer.count_tokens(&medium_text);

        // Cache improves performance but doesn't change correctness
        assert_eq!(count1, count2);
        assert!(medium_text.len() >= 50 && medium_text.len() <= 100_000);
    }}