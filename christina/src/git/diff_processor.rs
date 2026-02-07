//! High-level diff processing that bridges git output to core chunking.
//!
//! WHY here: combines IO-derived diff text with core chunking rules and adds
//! user-facing notices for truncation and binary files.

use std::sync::Arc;

use tracing::info;

use christina_core::git::stage::BINARY_EXTENSIONS;
use christina_core::{
    Tokenizer,
    processing::chunking,
    types::tokens::TokenCount,
    types::{DiffChunk, MAX_DIFF_SIZE},
};
use memchr::memchr;

use crate::git::parsing::{self, safe_truncate};

/// Processor for splitting large diffs into manageable chunks.
pub struct DiffProcessor {
    tokenizer: Arc<dyn Tokenizer>,
    token_limit: TokenCount,
    ignore_files: Vec<String>,
    max_diff_size: usize,
    lockfile_token_limit: TokenCount,
}

impl DiffProcessor {
    pub fn new(tokenizer: Arc<dyn Tokenizer>, token_limit: TokenCount) -> Self {
        Self {
            tokenizer,
            token_limit,
            ignore_files: Vec::new(),
            max_diff_size: MAX_DIFF_SIZE,
            lockfile_token_limit: TokenCount::new_at_least_one(chunking::LOCKFILE_TOKEN_LIMIT),
        }
    }

    pub fn with_ignore_files(mut self, ignore_files: Vec<String>) -> Self {
        self.ignore_files = ignore_files;
        self
    }

    pub fn with_lockfile_token_limit(mut self, limit: TokenCount) -> Self {
        self.lockfile_token_limit = limit;
        self
    }

    /// Detects binary content in diff output.
    ///
    /// Detection strategy (applied in order):
    /// 1. Check for git's binary markers ("Binary files" or "GIT binary patch")
    /// 2. For small files (<8KB): scan all bytes for NUL
    /// 3. For larger files: scan content for NUL bytes using a fast memchr search
    /// 4. Check file extension against known binary types
    ///
    /// **NUL byte detection**: Small files are fully scanned for accuracy.
    /// Larger files use a fast byte search to avoid false negatives from sampling.
    fn is_binary_content(&self, content: &str) -> bool {
        if content.is_empty() {
            return false;
        }

        if content.contains("Binary files") || content.contains("GIT binary patch") {
            return true;
        }

        // For small files, do a full scan for accuracy
        // For larger files, use fast memchr scanning
        let content_bytes = content.as_bytes();
        let use_full_scan = content_bytes.len() < 8192;

        if use_full_scan {
            if content_bytes.contains(&0) {
                return true;
            }
        } else if has_nul_bytes(content_bytes) {
            return true;
        }

        self.has_binary_extension(content)
    }

    fn has_binary_extension(&self, content: &str) -> bool {
        let Some(first_line) = content.lines().next() else {
            return false;
        };

        if !first_line.starts_with("diff --git") {
            return false;
        }

        for word in first_line.split_whitespace() {
            if word.starts_with("a/") || word.starts_with("b/") {
                let path = &word[2..];
                if BINARY_EXTENSIONS
                    .iter()
                    .any(|ext| path.to_lowercase().ends_with(ext))
                {
                    return true;
                }
            }
        }

        false
    }

    /// Process a diff string into chunks that fit within the token budget.
    pub fn process(&self, diff: &str) -> Vec<DiffChunk> {
        if diff.is_empty() {
            return Vec::new();
        }

        if parsing::is_all_file_deletions(diff) || parsing::is_deletion_only(diff) {
            let limit = if diff.len() >= 500 * 1024 { 100 } else { 50 };
            return self.process_owned(parsing::truncate_deletion_diff(diff, limit));
        }

        self.process_borrowed(diff)
    }

    fn process_borrowed(&self, diff: &str) -> Vec<DiffChunk> {
        if diff.len() > self.max_diff_size {
            let file_diffs = parsing::split_by_files(diff, self.tokenizer.as_ref());

            if file_diffs.is_empty() {
                let truncated = safe_truncate(diff, self.max_diff_size);
                let files = parsing::extract_file_paths(truncated);
                info!(
                    "Diff truncated for size: {} bytes (max {})",
                    diff.len(),
                    self.max_diff_size
                );
                let content = format!("{}\n\n[diff truncated for size]", truncated);
                let token_count = self.tokenizer.count_tokens(&content);
                return vec![DiffChunk::new(Arc::from(content), files, token_count)];
            }

            let mut accumulated_size = 0usize;
            let mut included_files = Vec::new();
            let total_files = file_diffs.len();

            for file_diff in file_diffs {
                let file_size = file_diff.content.len();
                if accumulated_size + file_size <= self.max_diff_size {
                    accumulated_size += file_size;
                    included_files.push(file_diff);
                } else {
                    break;
                }
            }

            let mut chunks = Vec::new();
            let mut all_paths = Vec::new();

            for file_diff in included_files {
                all_paths.push(file_diff.path.clone());
                let token_count = self.tokenizer.count_tokens(&file_diff.content);
                chunks.push(DiffChunk::new(
                    Arc::clone(&file_diff.content),
                    vec![file_diff.path],
                    token_count,
                ));
            }

            if chunks.len() < total_files {
                info!(
                    "Diff truncated for size: {} of {} files included (max {} bytes)",
                    chunks.len(),
                    total_files,
                    self.max_diff_size
                );
                let notice = format!(
                    "\n[diff truncated: {} of {} files shown]",
                    chunks.len(),
                    total_files
                );
                chunks.push(DiffChunk::new(
                    Arc::from(notice.clone()),
                    all_paths,
                    self.tokenizer.count_tokens(&notice),
                ));
            }

            return chunks;
        }

        let total_tokens = self.tokenizer.count_tokens(diff);

        if total_tokens <= self.token_limit {
            let files = parsing::extract_file_paths(diff);
            return vec![DiffChunk::new(Arc::from(diff), files, total_tokens)];
        }

        let file_diffs = parsing::split_by_files(diff, self.tokenizer.as_ref());
        chunking::split_recursive(
            file_diffs,
            self.token_limit,
            &self.ignore_files,
            self.lockfile_token_limit,
            self.tokenizer.as_ref(),
        )
    }

    fn process_owned(&self, diff: String) -> Vec<DiffChunk> {
        if diff.len() > self.max_diff_size {
            let file_diffs = parsing::split_by_files(&diff, self.tokenizer.as_ref());
            let mut chunks = Vec::new();
            for file_diff in file_diffs {
                let token_count = self.tokenizer.count_tokens(&file_diff.content);
                chunks.push(DiffChunk::new(
                    Arc::clone(&file_diff.content),
                    vec![file_diff.path],
                    token_count,
                ));
            }
            return chunks;
        }

        let total_tokens = self.tokenizer.count_tokens(&diff);
        if total_tokens <= self.token_limit {
            let files = parsing::extract_file_paths(&diff);
            return vec![DiffChunk::new(Arc::from(diff), files, total_tokens)];
        }

        let file_diffs = parsing::split_by_files(&diff, self.tokenizer.as_ref());
        chunking::split_recursive(
            file_diffs,
            self.token_limit,
            &self.ignore_files,
            self.lockfile_token_limit,
            self.tokenizer.as_ref(),
        )
    }

    pub fn process_safe(&self, diff: &str) -> Vec<DiffChunk> {
        let mut truncation_notice = None;
        let mut diff_content = diff;
        if diff.len() > self.max_diff_size {
            let truncated = parsing::safe_truncate(diff, self.max_diff_size);
            let omitted = diff.len().saturating_sub(truncated.len());
            truncation_notice = Some(format!(
                "[Diff truncated: {} bytes omitted; processed first {} bytes]",
                omitted,
                truncated.len()
            ));
            diff_content = truncated;
        }

        let file_diffs = parsing::split_by_files(diff_content, self.tokenizer.as_ref());

        if file_diffs.is_empty() {
            if let Some(notice) = truncation_notice {
                let token_count = self.tokenizer.count_tokens(&notice);
                return vec![DiffChunk::new(Arc::from(notice), Vec::new(), token_count)];
            }
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let mut has_text_content = false;

        if let Some(notice) = truncation_notice {
            let token_count = self.tokenizer.count_tokens(&notice);
            // Put truncation notice first so the model knows context is partial.
            chunks.push(DiffChunk::new(Arc::from(notice), Vec::new(), token_count));
        }

        for file_diff in file_diffs {
            if self.is_binary_content(&file_diff.content) {
                let binary_notice = format!("[Binary file:] {}", file_diff.path);
                let token_count = self.tokenizer.count_tokens(&binary_notice);
                chunks.push(DiffChunk::new(
                    Arc::from(binary_notice),
                    vec![file_diff.path],
                    token_count,
                ));
            } else {
                has_text_content = true;
                let file_chunks = self.process(&file_diff.content);
                chunks.extend(file_chunks);
            }
        }

        if !has_text_content {
            return chunks;
        }

        chunks
    }
}

fn has_nul_bytes(bytes: &[u8]) -> bool {
    memchr(0, bytes).is_some()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    struct SimpleTokenizer;

    impl Tokenizer for SimpleTokenizer {
        fn count_tokens_exact(&self, text: &str) -> u32 {
            (text.len() / 4) as u32
        }

        fn count_tokens(&self, text: &str) -> TokenCount {
            TokenCount::new_at_least_one(self.count_tokens_exact(text))
        }

        fn encoding_name(&self) -> &str {
            "mock-4chars"
        }

        fn encode(&self, text: &str) -> Vec<u32> {
            text.chars()
                .collect::<Vec<_>>()
                .chunks(4)
                .enumerate()
                .map(|(i, _)| i as u32)
                .collect()
        }

        fn decode(&self, tokens: &[u32]) -> Option<String> {
            Some("x".repeat(tokens.len() * 4))
        }
    }

    fn create_processor(token_limit: u32) -> DiffProcessor {
        let tokenizer: Arc<dyn Tokenizer> = Arc::new(SimpleTokenizer);
        DiffProcessor::new(tokenizer, TokenCount::new_at_least_one(token_limit))
    }

    #[test]
    fn empty_diff_processing() {
        let processor = create_processor(1000);
        let chunks = processor.process("");
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn small_diff_single_chunk() {
        let processor = create_processor(10_000);
        let diff = "diff --git a/test.txt b/test.txt\n+new line\n";
        let chunks = processor.process(diff);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn binary_detection_with_nul_byte_start() {
        let processor = create_processor(1000);
        let content = "diff --git a/file.bin b/file.bin\n\0binary content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_detection_with_nul_byte_middle() {
        let processor = create_processor(1000);
        let content = "diff --git a/file.bin b/file.bin\nsome text\0more binary stuff";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_detection_with_nul_byte_late() {
        let processor = create_processor(1000);
        let mut content = String::with_capacity(4000);
        content.push_str("diff --git a/file.bin b/file.bin\n");
        content.push_str(&"a".repeat(1500));
        content.push('\0');
        content.push_str(&"b".repeat(1500));
        assert!(processor.is_binary_content(&content));
    }

    #[test]
    fn text_file_without_nul_bytes() {
        let processor = create_processor(1000);
        let content = "diff --git a/file.txt b/file.txt\n+This is a normal text file\n+with multiple lines\n+of UTF-8 content";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn empty_file_is_not_binary() {
        let processor = create_processor(1000);
        let content = "";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn utf8_content_is_not_binary() {
        let processor = create_processor(1000);
        let content = "diff --git a/file.txt b/file.txt\n+Hello 世界 🌍 Привет\n+Γεια σας αΛΛΕΣ";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn mixed_content_mostly_text_with_nul() {
        let processor = create_processor(1000);
        let content = "diff --git a/file.mixed b/file.mixed\n+lots of text\n+and more text\n\0\n+but also binary";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_detection_git_binary_patch_marker() {
        let processor = create_processor(1000);
        let content = "diff --git a/image.png b/image.png\nGIT binary patch\nliteral 100";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_detection_binary_files_marker() {
        let processor = create_processor(1000);
        let content =
            "diff --git a/image.png b/image.png\nBinary files a/image.png and b/image.png differ";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_png() {
        let processor = create_processor(1000);
        let content = "diff --git a/image.png b/image.png\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_jpg() {
        let processor = create_processor(1000);
        let content = "diff --git a/photo.jpg b/photo.jpg\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_jpeg() {
        let processor = create_processor(1000);
        let content = "diff --git a/photo.jpeg b/photo.jpeg\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_gif() {
        let processor = create_processor(1000);
        let content = "diff --git a/animation.gif b/animation.gif\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_pdf() {
        let processor = create_processor(1000);
        let content = "diff --git a/document.pdf b/document.pdf\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_zip() {
        let processor = create_processor(1000);
        let content = "diff --git a/archive.zip b/archive.zip\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_tar_gz() {
        let processor = create_processor(1000);
        let content = "diff --git a/package.tar.gz b/package.tar.gz\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_woff() {
        let processor = create_processor(1000);
        let content = "diff --git a/font.woff b/font.woff\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_mp4() {
        let processor = create_processor(1000);
        let content = "diff --git a/video.mp4 b/video.mp4\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_in_path_a() {
        let processor = create_processor(1000);
        let content = "diff --git a/src/assets/image.png b/src/assets/image.png\n+content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_in_path_b() {
        let processor = create_processor(1000);
        let content = "diff --git a/src/file.txt b/src/assets/image.jpg\n+content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn non_binary_extension_txt() {
        let processor = create_processor(1000);
        let content = "diff --git a/readme.txt b/readme.txt\n+This is text\n+More text";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn non_binary_extension_rs() {
        let processor = create_processor(1000);
        let content = "diff --git a/main.rs b/main.rs\n+fn main() {\n+    println!(\"Hello\");\n+}";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn non_binary_extension_json() {
        let processor = create_processor(1000);
        let content = "diff --git a/config.json b/config.json\n+{\n+  \"key\": \"value\"\n+}";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn non_diff_header_no_binary_detection() {
        let processor = create_processor(1000);
        let content = "+some content about image.png";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn process_safe_empty_diff() {
        let processor = create_processor(1000);
        let chunks = processor.process_safe("");
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn process_safe_text_file_only() {
        let processor = create_processor(1000);
        let diff = "diff --git a/test.txt b/test.txt\nindex 1234567..abcdefg\n--- a/test.txt\n+++ b/test.txt\n@@ -0,0 +1 @@\n+new line\n";
        let chunks = processor.process_safe(diff);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn process_safe_binary_file_generates_notice() {
        let processor = create_processor(1000);
        let diff = "diff --git a/test.txt b/test.txt\nindex 1234567..abcdefg\n--- a/test.txt\n+++ b/test.txt\n@@ -0,0 +1 @@\n+text content\ndiff --git a/test.bin b/test.bin\nindex 1234567..abcdefg\n--- a/test.bin\n+++ b/test.bin\n@@ -0,0 +1 @@\n\0binary content";
        let chunks = processor.process_safe(diff);
        assert!(chunks.iter().any(|c| c.content.contains("[Binary file:]")));
    }

    #[test]
    fn process_safe_all_binary_files_returns_chunks() {
        let processor = create_processor(1000);
        let diff = "diff --git a/test.bin b/test.bin\n\0binary\n";
        let chunks = processor.process_safe(diff);
        assert!(!chunks.is_empty());
        assert!(chunks.iter().any(|c| c.content.contains("[Binary file:]")));
    }

    #[test]
    fn process_safe_mixed_binary_and_text() {
        let processor = create_processor(1000);
        let diff = "diff --git a/test.txt b/test.txt\nindex 1234567..abcdefg\n--- a/test.txt\n+++ b/test.txt\n@@ -0,0 +1 @@\n+text content\n";
        let chunks = processor.process_safe(diff);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn process_safe_respects_max_diff_size() {
        let processor = create_processor(1000);
        let huge_diff = "a".repeat(MAX_DIFF_SIZE + 1000);
        let chunks = processor.process_safe(&huge_diff);
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.content.contains("Diff truncated"))
        );
    }

    #[test]
    fn process_safe_binary_extension_image() {
        let processor = create_processor(1000);
        let diff = "diff --git a/readme.txt b/readme.txt\nindex 1234567..abcdefg\n--- a/readme.txt\n+++ b/readme.txt\n@@ -0,0 +1 @@\n+text\ndiff --git a/logo.png b/logo.png\nindex 1234567..abcdefg\n--- a/logo.png\n+++ b/logo.png\n@@ -0,0 +1 @@\n";
        let chunks = processor.process_safe(diff);
        assert!(chunks.iter().any(|c| c.content.contains("[Binary file:]")));
    }

    #[test]
    fn processor_new_initializes_defaults() {
        let processor = create_processor(5000);
        assert_eq!(processor.token_limit, TokenCount::new_at_least_one(5000));
        assert_eq!(processor.ignore_files.len(), 0);
        assert_eq!(processor.max_diff_size, MAX_DIFF_SIZE);
        assert_eq!(
            processor.lockfile_token_limit,
            TokenCount::new_at_least_one(chunking::LOCKFILE_TOKEN_LIMIT)
        );
    }

    #[test]
    fn processor_with_ignore_files() {
        let tokenizer: Arc<dyn Tokenizer> = Arc::new(SimpleTokenizer);
        let processor = DiffProcessor::new(tokenizer, TokenCount::new_at_least_one(5000))
            .with_ignore_files(vec!["*.lock".to_string(), "*.log".to_string()]);
        assert_eq!(processor.ignore_files.len(), 2);
        assert!(processor.ignore_files.contains(&"*.lock".to_string()));
        assert!(processor.ignore_files.contains(&"*.log".to_string()));
    }

    #[test]
    fn binary_detection_large_file_sampling() {
        let processor = create_processor(1000);
        let mut content = String::with_capacity(2_000_000);
        content.push_str("diff --git a/large.bin b/large.bin\n");
        content.push_str(&"a".repeat(16));
        content.push('\0');
        content.push_str(&"a".repeat(2_000_000 - content.len()));
        assert!(processor.is_binary_content(&content));
    }

    #[test]
    fn binary_detection_large_file_no_nul_bytes() {
        let processor = create_processor(1000);
        let mut content = String::with_capacity(2_000_000);
        content.push_str("diff --git a/large.txt b/large.txt\n");
        content.push_str(&"a".repeat(2_000_000 - content.len()));
        assert!(!processor.is_binary_content(&content));
    }

    #[test]
    fn binary_detection_nul_byte_beyond_8kb() {
        let processor = create_processor(1000);
        let mut content = String::with_capacity(10_000);
        content.push_str("diff --git a/file.bin b/file.bin\n");
        content.push_str(&"a".repeat(9000));
        content.push('\0');
        content.push_str("more content");
        // With sampling, NUL bytes beyond 8KB are now detected
        assert!(processor.is_binary_content(&content));
    }

    #[test]
    fn binary_detection_nul_at_8191_boundary() {
        let processor = create_processor(1000);
        let mut content = String::with_capacity(8200);
        content.push_str("diff --git a/file.bin b/file.bin\n");
        let header_len = content.len();
        content.push_str(&"a".repeat(8192 - header_len - 1));
        content.push('\0');
        assert!(processor.is_binary_content(&content));
    }

    #[test]
    fn binary_detection_nul_at_multiple_positions() {
        let processor = create_processor(1000);

        // Test NUL byte at various positions in a medium-sized file
        for nul_position in [100, 1000, 10_000, 50_000, 100_000] {
            let mut content = String::with_capacity(nul_position + 1000);
            content.push_str("diff --git a/file.bin b/file.bin\n");
            content.push_str(&"a".repeat(nul_position - content.len()));
            content.push('\0');
            content.push_str(&"b".repeat(500));

            assert!(
                processor.is_binary_content(&content),
                "Failed to detect NUL byte at position {}",
                nul_position
            );
        }
    }

    #[test]
    fn binary_detection_performance_large_text_file() {
        let processor = create_processor(1000);
        let mut content = String::with_capacity(5_000_000);
        content.push_str("diff --git a/bundle.js b/bundle.js\n");
        for _ in 0..100_000 {
            content.push_str("function test() { return 'hello world'; }\n");
        }
        let start = std::time::Instant::now();
        let result = processor.is_binary_content(&content);
        let elapsed = start.elapsed();
        assert!(!result);
        assert!(
            elapsed.as_millis() < 100,
            "Large file detection took too long: {:?}",
            elapsed
        );
    }

    #[test]
    fn process_borrowed_truncates_large_diff_with_notice() {
        let processor = create_processor(10_000);
        let mut diff = String::new();
        for i in 0..10 {
            diff.push_str(&format!(
                "diff --git a/file{}.txt b/file{}.txt\n+content\n",
                i, i
            ));
        }
        let mut processor = processor;
        processor.max_diff_size = 80;
        let chunks = processor.process(&diff);
        assert!(chunks.iter().any(|c| c.content.contains("diff truncated")));
    }

    #[test]
    fn deletion_only_truncation_uses_small_limit() {
        let processor = create_processor(10_000);
        let mut diff = String::from(
            "diff --git a/file.txt b/file.txt\n\
 deleted file mode 100644\n\
 --- a/file.txt\n\
 +++ /dev/null\n\
 @@ -1,120 +0,0 @@\n",
        );
        diff.push_str(&"-line\n".repeat(120));
        let chunks = processor.process(&diff);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("deleted content truncated"));
    }

    #[test]
    fn deletion_only_truncation_uses_large_limit_for_big_diff() {
        let processor = create_processor(10_000);
        let mut diff = String::from(
            "diff --git a/file.txt b/file.txt\n\
 deleted file mode 100644\n\
 --- a/file.txt\n\
 +++ /dev/null\n\
 @@ -1,8000 +0,0 @@\n",
        );
        diff.push_str(&"-line\n".repeat(200_000));
        assert!(diff.len() >= 500 * 1024);
        let chunks = processor.process(&diff);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("deleted content truncated"));
    }

    #[test]
    fn process_owned_respects_max_diff_size() {
        let processor = create_processor(10_000);
        let mut diff = String::new();
        for i in 0..5 {
            diff.push_str(&format!(
                "diff --git a/file{}.txt b/file{}.txt\n+content\n",
                i, i
            ));
        }
        let mut processor = processor;
        processor.max_diff_size = 20;
        let chunks = processor.process(diff.as_str());
        assert!(!chunks.is_empty());
    }
}
