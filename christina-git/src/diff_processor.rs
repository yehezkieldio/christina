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

    fn is_binary_content(&self, content: &str) -> bool {
        // Check for git binary markers
        if content.contains("Binary files") || content.contains("GIT binary patch") {
            return true;
        }

        // Check for NUL bytes throughout content (not just first 1024 bytes)
        // Use a reasonable limit to avoid scanning enormous files
        let scan_limit = content.len().min(8192);
        if content.bytes().take(scan_limit).any(|b| b == 0) {
            return true;
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
            // All files are being deleted - heavily truncate
            return self.process_owned(parsing::truncate_deletion_diff(diff, 3));
        } else if parsing::is_deletion_only(diff) {
            // Only deletions (no additions) - moderately truncate
            return self.process_owned(parsing::truncate_deletion_diff(diff, 10));
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
