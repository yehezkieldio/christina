use std::sync::Arc;

use christina_core::{
    Tokenizer,
    git::{DiffChunk, MAX_DIFF_SIZE, file::BINARY_EXTENSIONS},
    types::{FilePath, TokenCount},
};

use crate::{
    chunking,
    parsing::{self, safe_truncate},
};

/// Processor for splitting large diffs into manageable chunks.
pub struct DiffProcessor<'a> {
    tokenizer: &'a dyn Tokenizer,
    token_limit: TokenCount,
    ignore_files: Vec<String>,
    max_diff_size: usize,
}

impl<'a> DiffProcessor<'a> {
    pub fn new(tokenizer: &'a dyn Tokenizer, token_limit: TokenCount) -> Self {
        Self {
            tokenizer,
            token_limit,
            ignore_files: Vec::new(),
            max_diff_size: MAX_DIFF_SIZE,
        }
    }

    pub fn with_ignore_files(mut self, ignore_files: Vec<String>) -> Self {
        self.ignore_files = ignore_files;
        self
    }

    /// Maximum content size to scan for binary detection.
    /// Files larger than this use extension-only detection to avoid memory pressure.
    const MAX_BINARY_SCAN_SIZE: usize = 1024 * 1024; // 1MB

    /// Sampling interval for NUL byte detection in large files.
    /// Checks every Nth byte to reduce CPU usage while maintaining accuracy.
    const NUL_BYTE_SAMPLING_INTERVAL: usize = 16;

    fn is_binary_content(&self, content: &str) -> bool {
        // Check for git binary markers (fast path)
        if content.contains("Binary files") || content.contains("GIT binary patch") {
            return true;
        }

        // For very large content, use sampling to avoid performance issues
        let content_len = content.len();
        let should_sample = content_len > Self::MAX_BINARY_SCAN_SIZE;

        // Check for NUL bytes with size-aware scanning
        if should_sample {
            // Sample at intervals for large files
            if self.has_nul_bytes_sampled(content) {
                return true;
            }
        } else {
            // Full scan for smaller files (up to 8KB as before)
            let scan_limit = content_len.min(8192);
            if content.bytes().take(scan_limit).any(|b| b == 0) {
                return true;
            }
        }

        // Check file extension heuristics for common binary types
        if let Some(first_line) = content.lines().next()
            && first_line.starts_with("diff --git")
        {
            // Extract file paths from diff header: "diff --git a/path b/path"
            // Check both a/ and b/ paths

            // Parse paths from header
            for word in first_line.split_whitespace() {
                if word.starts_with("a/") || word.starts_with("b/") {
                    let path = &word[2..]; // Remove "a/" or "b/" prefix
                    for ext in BINARY_EXTENSIONS {
                        if path.ends_with(ext) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Sample content for NUL bytes at regular intervals.
    /// Used for large files to avoid scanning every byte.
    fn has_nul_bytes_sampled(&self, content: &str) -> bool {
        let bytes = content.as_bytes();
        let sample_count = (bytes.len() / Self::NUL_BYTE_SAMPLING_INTERVAL).min(65536); // Max 64K samples

        for i in 0..sample_count {
            let idx = i * Self::NUL_BYTE_SAMPLING_INTERVAL;
            if idx < bytes.len() && bytes[idx] == 0 {
                return true;
            }
        }

        false
    }

    /// Process a diff string into chunks that fit within the token budget.
    ///
    /// This is the main entry point for diff processing.
    pub fn process(&self, diff: &str) -> Vec<DiffChunk> {
        if diff.is_empty() {
            return Vec::new();
        }

        // Truncate deletion-only diffs to save tokens
        // The LLM doesn't need to see all deleted content to generate "delete file" messages
        if parsing::is_all_file_deletions(diff) {
            return self.process_owned(parsing::truncate_deletion_diff(diff, 50));
        } else if parsing::is_deletion_only(diff) {
            return self.process_owned(parsing::truncate_deletion_diff(diff, 100));
        }

        // Normal processing path for non-deletion-only diffs
        self.process_borrowed(diff)
    }

    /// Process a borrowed diff
    fn process_borrowed(&self, diff: &str) -> Vec<DiffChunk> {
        // Reject extremely large diffs
        if diff.len() > self.max_diff_size {
            let tokenizer_fn = |text: &str| self.tokenizer.count_tokens(text);
            let file_diffs = parsing::split_by_files(diff, tokenizer_fn);

            if file_diffs.is_empty() {
                let truncated = safe_truncate(diff, self.max_diff_size);
                let files = parsing::extract_file_paths(truncated);
                let token_count = self.tokenizer.count_tokens(truncated);
                return vec![DiffChunk::new(
                    Arc::from(format!(
                        "{}\n\n[... diff truncated: exceeded {} byte limit ...]",
                        truncated, self.max_diff_size
                    )),
                    files,
                    token_count,
                )];
            }

            let mut accumulated_size = 0;
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
            let mut all_paths: Vec<FilePath> = Vec::new();

            for file_diff in included_files {
                all_paths.push(file_diff.path.clone());
                let token_count = self.tokenizer.count_tokens(&file_diff.content);
                chunks.push(DiffChunk::new(
                    Arc::from(file_diff.content),
                    vec![file_diff.path],
                    token_count,
                ));
            }

            if chunks.len() < total_files {
                let notice = format!(
                    "\n[... diff truncated: {} of {} files included, exceeded {} byte limit ...]",
                    chunks.len(),
                    total_files,
                    self.max_diff_size
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

        let tokenizer_fn = |text: &str| self.tokenizer.count_tokens(text);
        let file_diffs = parsing::split_by_files(diff, tokenizer_fn);
        chunking::split_recursive(
            file_diffs,
            self.token_limit,
            &self.ignore_files,
            self.tokenizer,
        )
    }

    /// Process an owned diff string
    fn process_owned(&self, diff: String) -> Vec<DiffChunk> {
        if diff.len() > self.max_diff_size {
            let tokenizer_fn = |text: &str| self.tokenizer.count_tokens(text);
            let file_diffs = parsing::split_by_files(&diff, tokenizer_fn);

            let mut chunks = Vec::new();
            for file_diff in file_diffs {
                let token_count = self.tokenizer.count_tokens(&file_diff.content);
                chunks.push(DiffChunk::new(
                    Arc::from(file_diff.content.to_owned()),
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

        let tokenizer_fn = |text: &str| self.tokenizer.count_tokens(text);
        let file_diffs = parsing::split_by_files(&diff, tokenizer_fn);

        // Convert all borrowed chunks to owned
        let mut chunks = Vec::new();
        for file_diff in file_diffs {
            if file_diff.token_count <= self.token_limit {
                chunks.push(DiffChunk::new(
                    Arc::from(file_diff.content.to_owned()),
                    vec![file_diff.path],
                    file_diff.token_count,
                ));
            } else {
                // Split by hunks and convert to owned
                let hunk_chunks = chunking::split_by_hunks(
                    &file_diff.path,
                    &file_diff.content,
                    self.token_limit,
                    self.tokenizer,
                );
                // Convert borrowed chunks to owned
                for chunk in hunk_chunks {
                    chunks.push(DiffChunk::new(
                        chunk.content,
                        chunk.files,
                        chunk.token_count,
                    ));
                }
            }
        }
        chunks
    }

    pub fn process_safe(&self, diff: &str) -> Result<Vec<DiffChunk>, String> {
        if diff.len() > self.max_diff_size {
            return Err(format!(
                "Diff size ({} bytes) exceeds maximum ({} bytes)",
                diff.len(),
                self.max_diff_size
            ));
        }

        let tokenizer_fn = |text: &str| self.tokenizer.count_tokens(text);
        let file_diffs = parsing::split_by_files(diff, tokenizer_fn);

        if file_diffs.is_empty() {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();
        let mut has_text_content = false;

        for file_diff in file_diffs {
            if self.is_binary_content(&file_diff.content) {
                let binary_notice = format!("[Binary file: {}]", file_diff.path);
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
            return Err("No processable diff content found".to_string());
        }

        Ok(chunks)
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    struct SimpleTokenizer;
    impl Tokenizer for SimpleTokenizer {
        fn count_tokens(&self, text: &str) -> TokenCount {
            TokenCount::new_saturating((text.len() / 4).max(1) as u32)
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

    fn create_tokenizer() -> SimpleTokenizer {
        SimpleTokenizer
    }

    #[test]
    fn empty_diff_processing() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let chunks = processor.process("");
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn small_diff_single_chunk() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(10000));
        let diff = "diff --git a/test.txt b/test.txt\n+new line\n";
        let chunks = processor.process(diff);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn binary_detection_with_nul_byte_start() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // NUL byte at the start
        let content = "diff --git a/file.bin b/file.bin\n\0binary content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_detection_with_nul_byte_middle() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // NUL byte in the middle of content
        let content = "diff --git a/file.bin b/file.bin\nsome text\0more binary stuff";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_detection_with_nul_byte_late() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // NUL byte after 1000 bytes but within 8192 limit
        let mut content = String::with_capacity(4000);
        content.push_str("diff --git a/file.bin b/file.bin\n");
        content.push_str(&"a".repeat(1500));
        content.push('\0');
        content.push_str(&"b".repeat(1500));
        assert!(processor.is_binary_content(&content));
    }

    #[test]
    fn text_file_without_nul_bytes() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/file.txt b/file.txt\n+This is a normal text file\n+with multiple lines\n+of UTF-8 content";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn empty_file_is_not_binary() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn utf8_content_is_not_binary() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // UTF-8 with various unicode characters
        let content = "diff --git a/file.txt b/file.txt\n+Hello 世界 🌍 Привет\n+Γεια σας αΛΛΕΣ";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn mixed_content_mostly_text_with_nul() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/file.mixed b/file.mixed\n+lots of text\n+and more text\n\0\n+but also binary";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_detection_git_binary_patch_marker() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/image.png b/image.png\nGIT binary patch\nliteral 100";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_detection_binary_files_marker() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content =
            "diff --git a/image.png b/image.png\nBinary files a/image.png and b/image.png differ";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_png() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/image.png b/image.png\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_jpg() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/photo.jpg b/photo.jpg\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_jpeg() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/photo.jpeg b/photo.jpeg\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_gif() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/animation.gif b/animation.gif\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_pdf() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/document.pdf b/document.pdf\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_zip() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/archive.zip b/archive.zip\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_tar_gz() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/package.tar.gz b/package.tar.gz\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_woff() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/font.woff b/font.woff\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_mp4() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/video.mp4 b/video.mp4\n+file content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_in_path_a() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // Test detection in a/ path
        let content = "diff --git a/src/assets/image.png b/src/assets/image.png\n+content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn binary_extension_in_path_b() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // Test detection in b/ path
        let content = "diff --git a/src/file.txt b/src/assets/image.jpg\n+content";
        assert!(processor.is_binary_content(content));
    }

    #[test]
    fn non_binary_extension_txt() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/readme.txt b/readme.txt\n+This is text\n+More text";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn non_binary_extension_rs() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/main.rs b/main.rs\n+fn main() {\n+    println!(\"Hello\");\n+}";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn non_binary_extension_json() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let content = "diff --git a/config.json b/config.json\n+{\n+  \"key\": \"value\"\n+}";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn non_diff_header_no_binary_detection() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // Content without proper diff header - extension detection should not apply
        let content = "+some content about image.png";
        assert!(!processor.is_binary_content(content));
    }

    #[test]
    fn process_safe_empty_diff() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let result = processor.process_safe("");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn process_safe_text_file_only() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let diff = "diff --git a/test.txt b/test.txt\nindex 1234567..abcdefg\n--- a/test.txt\n+++ b/test.txt\n@@ -0,0 +1 @@\n+new line\n";
        let result = processor.process_safe(diff);
        assert!(result.is_ok());
        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn process_safe_binary_file_generates_notice() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let diff = "diff --git a/test.txt b/test.txt\nindex 1234567..abcdefg\n--- a/test.txt\n+++ b/test.txt\n@@ -0,0 +1 @@\n+text content\ndiff --git a/test.bin b/test.bin\nindex 1234567..abcdefg\n--- a/test.bin\n+++ b/test.bin\n@@ -0,0 +1 @@\n\0binary content";
        let result = processor.process_safe(diff);
        assert!(result.is_ok());
        let chunks = result.unwrap();
        assert!(chunks.iter().any(|c| c.content.contains("[Binary file:")));
    }

    #[test]
    fn process_safe_all_binary_files_returns_error() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // Create a diff with only binary files and no text files
        let diff = "diff --git a/test.bin b/test.bin\n\0binary\n";
        let result = processor.process_safe(diff);
        // Should return error because there's no text content
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No processable diff content"));
    }

    #[test]
    fn process_safe_mixed_binary_and_text() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // Mix of binary and text files
        let diff = "diff --git a/test.txt b/test.txt\nindex 1234567..abcdefg\n--- a/test.txt\n+++ b/test.txt\n@@ -0,0 +1 @@\n+text content\n";
        let result = processor.process_safe(diff);
        assert!(result.is_ok());
        let chunks = result.unwrap();
        // Should have at least one chunk from text file
        assert!(!chunks.is_empty());
    }

    #[test]
    fn process_safe_respects_max_diff_size() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // Create a very large diff that exceeds MAX_DIFF_SIZE
        let huge_diff = "a".repeat(MAX_DIFF_SIZE + 1000);
        let result = processor.process_safe(&huge_diff);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds maximum"));
    }

    #[test]
    fn process_safe_binary_extension_image() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        let diff = "diff --git a/readme.txt b/readme.txt\nindex 1234567..abcdefg\n--- a/readme.txt\n+++ b/readme.txt\n@@ -0,0 +1 @@\n+text\ndiff --git a/logo.png b/logo.png\nindex 1234567..abcdefg\n--- a/logo.png\n+++ b/logo.png\n@@ -0,0 +1 @@\n";
        let result = processor.process_safe(diff);
        assert!(result.is_ok());
        let chunks = result.unwrap();
        assert!(chunks.iter().any(|c| c.content.contains("[Binary file:")));
    }

    #[test]
    fn processor_new_initializes_defaults() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(5000));
        // Verify that processor was created successfully
        assert_eq!(processor.token_limit, TokenCount::new_saturating(5000));
        assert_eq!(processor.ignore_files.len(), 0);
        assert_eq!(processor.max_diff_size, MAX_DIFF_SIZE);
    }

    #[test]
    fn processor_with_ignore_files() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(5000))
            .with_ignore_files(vec!["*.lock".to_string(), "*.log".to_string()]);
        assert_eq!(processor.ignore_files.len(), 2);
        assert!(processor.ignore_files.contains(&"*.lock".to_string()));
        assert!(processor.ignore_files.contains(&"*.log".to_string()));
    }

    #[test]
    fn binary_detection_large_file_sampling() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // Create content larger than MAX_BINARY_SCAN_SIZE (1MB) with NUL byte at sampled position
        let mut content = String::with_capacity(2_000_000);
        content.push_str("diff --git a/large.bin b/large.bin\n");
        // Fill with 'a's, but put a NUL byte at a sampled position
        content.push_str(&"a".repeat(DiffProcessor::NUL_BYTE_SAMPLING_INTERVAL));
        content.push('\0');
        content.push_str(&"a".repeat(2_000_000 - content.len()));
        assert!(processor.is_binary_content(&content));
    }

    #[test]
    fn binary_detection_large_file_no_nul_bytes() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // Create content larger than MAX_BINARY_SCAN_SIZE without NUL bytes
        let mut content = String::with_capacity(2_000_000);
        content.push_str("diff --git a/large.txt b/large.txt\n");
        content.push_str(&"a".repeat(2_000_000 - content.len()));
        // Should NOT be detected as binary since no NUL bytes at sampled positions
        // and no binary extension
        assert!(!processor.is_binary_content(&content));
    }

    #[test]
    fn binary_detection_nul_byte_beyond_8kb() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // NUL byte after 8192 bytes - with the 8192 byte scan limit, this won't be detected
        // Use .txt extension to avoid extension-based detection
        let mut content = String::with_capacity(10000);
        content.push_str("diff --git a/file.txt b/file.txt\n");
        // Add content to push NUL byte beyond 8192 bytes
        content.push_str(&"a".repeat(9000));
        content.push('\0');
        content.push_str("more content");
        // File is under 1MB, but scan is limited to 8192 bytes
        // NUL byte is at position 9000+, so it won't be detected
        assert!(!processor.is_binary_content(&content));
    }

    #[test]
    fn binary_detection_nul_at_8191_boundary() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // NUL byte exactly at the 8192 boundary (position 8191 after header)
        let mut content = String::with_capacity(8200);
        content.push_str("diff --git a/file.bin b/file.bin\n");
        let header_len = content.len();
        content.push_str(&"a".repeat(8192 - header_len - 1));
        content.push('\0');
        // NUL byte is within the first 8192 bytes, should be detected
        assert!(processor.is_binary_content(&content));
    }

    #[test]
    fn binary_detection_performance_large_text_file() {
        let tokenizer = create_tokenizer();
        let processor = DiffProcessor::new(&tokenizer, TokenCount::new_saturating(1000));
        // Simulate a very large text file (e.g., minified JS)
        // Should use sampling and complete quickly
        let mut content = String::with_capacity(5_000_000);
        content.push_str("diff --git a/bundle.js b/bundle.js\n");
        // Add lots of text content without NUL bytes
        for _ in 0..100000 {
            content.push_str("function test() { return 'hello world'; }\n");
        }
        // Should complete quickly due to sampling
        let start = std::time::Instant::now();
        let result = processor.is_binary_content(&content);
        let elapsed = start.elapsed();
        assert!(!result);
        // Should complete in reasonable time (sampling prevents O(n) scan)
        assert!(
            elapsed.as_millis() < 100,
            "Large file detection took too long: {:?}",
            elapsed
        );
    }
}
