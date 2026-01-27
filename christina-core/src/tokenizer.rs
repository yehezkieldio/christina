/// Trait for token counting and encoding operations.
pub trait Tokenizer: Send + Sync {
    /// Count the number of tokens in the given text.
    fn count_tokens(&self, text: &str) -> crate::types::TokenCount;

    /// Get the name of the encoding used by this tokenizer.
    fn encoding_name(&self) -> &str;

    /// Encode text into token IDs.
    fn encode(&self, text: &str) -> Vec<u32>;

    /// Decode token IDs back into text.
    fn decode(&self, tokens: &[u32]) -> Option<String>;

    /// Slice text to fit within a token limit.
    ///
    /// This method ensures that the returned slice:
    /// 1. Does not exceed the specified token limit
    /// 2. Ends at a valid UTF-8 boundary
    ///
    /// TODO: Currently we use a binary search approach to find the largest valid slice.
    ///       Research optimizations or alternative algorithms to improve performance.
    fn slice_to_token_limit<'a>(&self, text: &'a str, limit: crate::types::TokenCount) -> &'a str {
        if self.count_tokens(text) <= limit {
            return text;
        }

        let mut low = 0;
        let mut high = text.len();
        let mut best = 0;

        while low < high {
            let mid = (low + high).div_ceil(2);

            let boundary = text
                .char_indices()
                .take_while(|(i, _)| *i <= mid)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0);

            let slice = &text[..boundary];
            let token_count = self.count_tokens(slice);

            if token_count <= limit {
                best = boundary;
                low = mid + 1;
            } else {
                high = mid;
            }
        }

        &text[..best]
    }
}
