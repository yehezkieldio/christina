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
    /// Uses a fast path that tokenizes once and maps token counts to byte
    /// offsets when encode/decode align with token counts. Falls back to
    /// boundary-aware binary search for other implementations.
    fn slice_to_token_limit<'a>(&self, text: &'a str, limit: crate::types::TokenCount) -> &'a str {
        let total_tokens = self.count_tokens(text);
        if total_tokens <= limit {
            return text;
        }

        let limit_usize = usize::from(limit);
        let total_tokens_usize = usize::from(total_tokens);
        let tokens = self.encode(text);

        if tokens.len() == total_tokens_usize && limit_usize <= tokens.len() {
            let prefix_tokens = &tokens[..limit_usize];
            if let Some(decoded) = self.decode(prefix_tokens) {
                let end = decoded.len();
                if !decoded.is_empty() && text.starts_with(&decoded) && text.is_char_boundary(end) {
                    let prefix_count = self.count_tokens(&text[..end]);
                    if prefix_count == limit {
                        return &text[..end];
                    }
                }
            }
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
                low = mid;
            } else {
                high = mid - 1;
            }
        }

        &text[..best]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    struct MockTokenizer;

    impl Tokenizer for MockTokenizer {
        fn count_tokens(&self, text: &str) -> crate::types::TokenCount {
            if text.is_empty() {
                return crate::types::TokenCount::new_at_least_one(0);
            }
            let count = text.split_whitespace().count();
            crate::types::TokenCount::new_at_least_one(count as u32)
        }

        fn encoding_name(&self) -> &str {
            "mock-whitespace"
        }

        fn encode(&self, text: &str) -> Vec<u32> {
            text.chars().map(|c| c as u32).collect()
        }

        fn decode(&self, tokens: &[u32]) -> Option<String> {
            tokens
                .iter()
                .filter_map(|&token| char::from_u32(token))
                .collect::<String>()
                .into()
        }
    }

    #[test]
    fn mock_tokenizer_count_tokens_simple() {
        let tokenizer = MockTokenizer;
        assert_eq!(
            tokenizer.count_tokens("hello world"),
            crate::types::TokenCount::new(2).unwrap()
        );
    }

    #[test]
    fn mock_tokenizer_count_tokens_single_word() {
        let tokenizer = MockTokenizer;
        assert_eq!(
            tokenizer.count_tokens("hello"),
            crate::types::TokenCount::new(1).unwrap()
        );
    }

    #[test]
    fn mock_tokenizer_count_tokens_multiple_spaces() {
        let tokenizer = MockTokenizer;
        assert_eq!(
            tokenizer.count_tokens("hello   world   test"),
            crate::types::TokenCount::new(3).unwrap()
        );
    }

    #[test]
    fn mock_tokenizer_count_tokens_empty_string() {
        let tokenizer = MockTokenizer;
        assert_eq!(
            tokenizer.count_tokens(""),
            crate::types::TokenCount::new_at_least_one(0)
        );
    }

    #[test]
    fn mock_tokenizer_count_tokens_only_whitespace() {
        let tokenizer = MockTokenizer;
        assert_eq!(
            tokenizer.count_tokens("   \t  \n  "),
            crate::types::TokenCount::new_at_least_one(0)
        );
    }

    #[test]
    fn encoding_name() {
        let tokenizer = MockTokenizer;
        assert_eq!(tokenizer.encoding_name(), "mock-whitespace");
    }

    #[test]
    fn encode_simple() {
        let tokenizer = MockTokenizer;
        let encoded = tokenizer.encode("Hi");
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0], 'H' as u32);
        assert_eq!(encoded[1], 'i' as u32);
    }

    #[test]
    fn encode_with_spaces() {
        let tokenizer = MockTokenizer;
        let encoded = tokenizer.encode("a b");
        assert_eq!(encoded.len(), 3);
        assert_eq!(encoded[0], 'a' as u32);
        assert_eq!(encoded[1], ' ' as u32);
        assert_eq!(encoded[2], 'b' as u32);
    }

    #[test]
    fn decode_simple() {
        let tokenizer = MockTokenizer;
        let tokens = vec!['H' as u32, 'i' as u32];
        let decoded = tokenizer.decode(&tokens);
        assert_eq!(decoded, Some("Hi".to_string()));
    }

    #[test]
    fn decode_with_spaces() {
        let tokenizer = MockTokenizer;
        let tokens = vec!['a' as u32, ' ' as u32, 'b' as u32];
        let decoded = tokenizer.decode(&tokens);
        assert_eq!(decoded, Some("a b".to_string()));
    }

    #[test]
    fn encode_decode_roundtrip() {
        let tokenizer = MockTokenizer;
        let original = "Hello World";
        let encoded = tokenizer.encode(original);
        let decoded = tokenizer.decode(&encoded);
        assert_eq!(decoded, Some(original.to_string()));
    }

    #[test]
    fn encode_decode_roundtrip_special_chars() {
        let tokenizer = MockTokenizer;
        let original = "Test!@#$%^&*()";
        let encoded = tokenizer.encode(original);
        let decoded = tokenizer.decode(&encoded);
        assert_eq!(decoded, Some(original.to_string()));
    }

    #[test]
    fn slice_to_token_limit_text_shorter_than_limit() {
        let tokenizer = MockTokenizer;
        let text = "hello world";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(10).unwrap());
        assert_eq!(result, "hello world");
    }

    #[test]
    fn slice_to_token_limit_text_exactly_at_limit() {
        let tokenizer = MockTokenizer;
        let text = "hello world test";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(3).unwrap());
        assert_eq!(result, "hello world test");
    }

    #[test]
    fn slice_to_token_limit_text_longer_than_limit() {
        let tokenizer = MockTokenizer;
        let text = "hello world test sentence";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(2).unwrap());
        assert_eq!(result.trim(), "hello world");
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new(2).unwrap()
        );
    }

    #[test]
    fn slice_to_token_limit_limit_one() {
        let tokenizer = MockTokenizer;
        let text = "hello world test";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(1).unwrap());
        assert_eq!(result.trim(), "hello");
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new(1).unwrap()
        );
    }

    #[test]
    fn slice_to_token_limit_empty_string() {
        let tokenizer = MockTokenizer;
        let text = "";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(10).unwrap());
        assert_eq!(result, "");
    }

    #[test]
    fn slice_to_token_limit_empty_limit() {
        let tokenizer = MockTokenizer;
        let text = "hello world";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new_at_least_one(0));
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new_at_least_one(0)
        );
        assert_eq!(result.trim(), "hello");
    }

    #[test]
    fn slice_to_token_limit_emoji_no_split() {
        let tokenizer = MockTokenizer;
        let text = "Hello 👋";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(2).unwrap());
        assert_eq!(result, "Hello 👋");
    }

    #[test]
    fn slice_to_token_limit_emoji_with_limit() {
        let tokenizer = MockTokenizer;
        let text = "Hello 👋 World";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(1).unwrap());
        assert_eq!(result.trim(), "Hello");
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new(1).unwrap()
        );
    }

    #[test]
    fn slice_to_token_limit_emoji_boundary_respects_utf8() {
        let tokenizer = MockTokenizer;
        let text = "Hello👋World";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(1).unwrap());
        assert_eq!(result, "Hello👋World");
    }

    #[test]
    fn slice_to_token_limit_multi_emoji() {
        let tokenizer = MockTokenizer;
        let text = "👋 🌍 🚀";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(2).unwrap());
        assert_eq!(result.trim(), "👋 🌍");
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new(2).unwrap()
        );
    }

    #[test]
    fn slice_to_token_limit_cjk_characters() {
        let tokenizer = MockTokenizer;
        let text = "こんにちは世界";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(1).unwrap());
        assert_eq!(result, "こんにちは世界");
    }

    #[test]
    fn slice_to_token_limit_cjk_with_spaces() {
        let tokenizer = MockTokenizer;
        let text = "こんにちは 世界 テスト";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(2).unwrap());
        assert_eq!(result.trim(), "こんにちは 世界");
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new(2).unwrap()
        );
    }

    #[test]
    fn slice_to_token_limit_mixed_scripts() {
        let tokenizer = MockTokenizer;
        let text = "Hello 世界 World";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(2).unwrap());
        assert_eq!(result.trim(), "Hello 世界");
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new(2).unwrap()
        );
    }

    #[test]
    fn slice_to_token_limit_utf8_accented_characters() {
        let tokenizer = MockTokenizer;
        let text = "café naïve résumé";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(2).unwrap());
        assert_eq!(result.trim(), "café naïve");
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new(2).unwrap()
        );
    }

    #[test]
    fn slice_to_token_limit_very_long_text() {
        let tokenizer = MockTokenizer;
        let text = "word ".repeat(1000);
        let text = text.trim();
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(100).unwrap());
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new(100).unwrap()
        );
    }

    #[test]
    fn slice_to_token_limit_very_long_text_exact_boundary() {
        let tokenizer = MockTokenizer;
        let words: Vec<&str> = (0..1000)
            .map(|i| if i % 2 == 0 { "hello" } else { "world" })
            .collect();
        let text = words.join(" ");
        let result =
            tokenizer.slice_to_token_limit(&text, crate::types::TokenCount::new(500).unwrap());
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new(500).unwrap()
        );
        let trimmed = result.trim();
        assert!(trimmed.ends_with("hello") || trimmed.ends_with("world"));
    }

    #[test]
    fn slice_to_token_limit_newlines() {
        let tokenizer = MockTokenizer;
        let text = "hello\nworld\ntest";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(2).unwrap());
        assert_eq!(result.trim(), "hello\nworld");
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new(2).unwrap()
        );
    }

    #[test]
    fn slice_to_token_limit_tabs() {
        let tokenizer = MockTokenizer;
        let text = "hello\tworld\ttest";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(2).unwrap());
        assert_eq!(result.trim(), "hello\tworld");
        assert_eq!(
            tokenizer.count_tokens(result),
            crate::types::TokenCount::new(2).unwrap()
        );
    }

    #[test]
    fn decode_invalid_unicode() {
        let tokenizer = MockTokenizer;
        let tokens = vec![0xD800u32];
        let decoded = tokenizer.decode(&tokens);
        assert_eq!(decoded, Some("".to_string()));
    }

    #[test]
    fn slice_to_token_limit_preserves_slice_reference() {
        let tokenizer = MockTokenizer;
        let text = "hello world test";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(2).unwrap());
        assert!(std::ptr::eq(text.as_ptr(), result.as_ptr()));
    }

    #[test]
    fn count_tokens_with_unicode_variations() {
        let tokenizer = MockTokenizer;
        let text1 = "café naïve";
        let text2 = "cafe naive";
        assert_eq!(tokenizer.count_tokens(text1), tokenizer.count_tokens(text2));
    }

    #[test]
    fn slice_to_token_limit_single_long_word() {
        let tokenizer = MockTokenizer;
        let text = "supercalifragilisticexpialidocious";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(1).unwrap());
        assert_eq!(result, "supercalifragilisticexpialidocious");
    }

    #[test]
    fn slice_to_token_limit_with_zero_width_chars() {
        let tokenizer = MockTokenizer;
        let text = "hello\u{200B}world";
        let result =
            tokenizer.slice_to_token_limit(text, crate::types::TokenCount::new(1).unwrap());
        assert_eq!(result, "hello\u{200B}world");
    }
}
