//! Recursive diff chunking algorithm for LLM context management.
//!
//! WHY hierarchical splitting: Git diffs have natural semantic boundaries (files > hunks > lines).
//! Respecting these boundaries produces coherent chunks that LLMs can reason about. Naive
//! splitting (e.g., fixed-size slices) would fragment hunks mid-change, destroying semantic context.
//!
//! WHY lockfile truncation: Package lockfiles (Cargo.lock, package-lock.json) can be 10K+ lines
//! but contribute minimal semantic value. Truncating to 100 tokens preserves "lock file was modified"
//! signal without wasting context budget on noise.
//!
//! WHY greedy First-Fit: Packs files into chunks maximally, minimizing total chunk count.
//! Alternative (bin packing) would be overkill—we're optimizing for throughput (fewer API calls),
//! not perfect packing. First-Fit is O(n) and works well in practice.
//!
//! WHY UTF-8 boundary enforcement: Binary search on byte indices can land mid-codepoint. Without
//! boundary checks, we'd create invalid UTF-8 slices, causing panics. Performance cost is negligible
//! (few boundary adjustments per chunk).

use std::cell::RefCell;
use std::sync::Arc;

use crate::{
    tokenizer::Tokenizer,
    types::{
        diff::{DiffChunk, FileDiff},
        path::FilePath,
        tokens::TokenCount,
    },
};

// ---------------------------------------------------------------------------
// Inlined buffer pool (private to this module)
// ---------------------------------------------------------------------------

struct ChunkBuffer {
    content: String,
    file_paths: Vec<FilePath>,
}

impl ChunkBuffer {
    fn new() -> Self {
        Self {
            content: String::with_capacity(4096),
            file_paths: Vec::with_capacity(4),
        }
    }

    fn clear(&mut self) {
        self.content.clear();
        self.file_paths.clear();
    }

    fn content_mut(&mut self) -> &mut String {
        &mut self.content
    }

    fn file_paths_mut(&mut self) -> &mut Vec<FilePath> {
        &mut self.file_paths
    }

    fn take_content(&mut self) -> String {
        std::mem::replace(&mut self.content, String::with_capacity(4096))
    }

    fn take_file_paths(&mut self) -> Vec<FilePath> {
        std::mem::replace(&mut self.file_paths, Vec::with_capacity(4))
    }

    fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

thread_local! {
    static BUFFER_POOL: RefCell<Vec<ChunkBuffer>> = const { RefCell::new(Vec::new()) };
}

fn acquire_buffer() -> ChunkBuffer {
    BUFFER_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        match pool.pop() {
            Some(mut buffer) => {
                buffer.clear();
                buffer
            }
            None => ChunkBuffer::new(),
        }
    })
}

fn release_buffer(buffer: ChunkBuffer) {
    BUFFER_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < 16 {
            pool.push(buffer);
        }
    });
}

// ---------------------------------------------------------------------------
// Public chunking API
// ---------------------------------------------------------------------------

// WHY 100 tokens: Preserves "lockfile changed" signal without wasting context.
// Lockfiles are noise (auto-generated, low semantic value). 100 tokens ≈ 5-10 package entries,
// enough to show type of change (added/removed deps) without overwhelming the prompt.
pub const LOCKFILE_TOKEN_LIMIT: u32 = 100;

/// Split diff into chunks recursively by files, hunks, then lines.
///
/// WHY greedy First-Fit packing: Maximizes chunk density, minimizing API calls.
/// Bin packing would be optimal but O(n log n); First-Fit is O(n) and "good enough"
/// for typical workloads (10-100 files).
///
/// WHY hierarchical fallback: Try file-level packing first (preserves full context),
/// then hunk-level (preserves change locality), then line-level (last resort).
/// Each level maintains more semantic coherence than the next.
///
/// - All file_diffs must contain valid UTF-8 content
/// - tokenizer must be initialized and functional
pub fn split_recursive(
    file_diffs: Vec<FileDiff>,
    token_limit: TokenCount,
    ignore_patterns: &[String],
    lockfile_token_limit: TokenCount,
    tokenizer: &dyn Tokenizer,
) -> Vec<DiffChunk> {
    let mut chunks = Vec::new();
    let mut buffer = acquire_buffer();
    let mut current_tokens: Option<TokenCount> = None;

    for file_diff in file_diffs {
        let is_lockfile = should_limit_file(&file_diff.path, ignore_patterns);

        // If lockfile, strictly limit its tokens
        let effective_token_count = if is_lockfile {
            file_diff.token_count.min(lockfile_token_limit)
        } else {
            file_diff.token_count
        };

        // If this single file fits in our budget
        if effective_token_count <= token_limit {
            let combined_tokens = match current_tokens {
                Some(current) => current.get() + effective_token_count.get(),
                None => effective_token_count.get(),
            };

            if combined_tokens <= token_limit.get() {
                // Add to current chunk (truncate if lockfile)
                if is_lockfile && file_diff.token_count > lockfile_token_limit {
                    let truncated = truncate_to_token_limit(
                        &file_diff.content,
                        lockfile_token_limit,
                        tokenizer,
                    );
                    buffer.content_mut().push_str(&truncated);
                    buffer
                        .content_mut()
                        .push_str("\n[... truncated lockfile ...]\n");
                } else {
                    buffer.content_mut().push_str(&file_diff.content);
                }
                buffer.file_paths_mut().push(file_diff.path);
                current_tokens = TokenCount::new(combined_tokens);
            } else {
                // Flush current chunk and start new one
                if !buffer.is_empty() {
                    chunks.push(DiffChunk::new(
                        Arc::from(buffer.take_content()),
                        buffer.take_file_paths(),
                        current_tokens.unwrap_or_else(|| TokenCount::new_at_least_one(1)),
                    ));
                    buffer.clear();
                }

                // Add new file to fresh buffer
                if is_lockfile && file_diff.token_count > lockfile_token_limit {
                    let mut truncated = truncate_to_token_limit(
                        &file_diff.content,
                        lockfile_token_limit,
                        tokenizer,
                    );
                    truncated.push_str("\n[... truncated lockfile ...]\n");
                    buffer.content_mut().push_str(&truncated);
                } else {
                    buffer.content_mut().push_str(&file_diff.content);
                }
                buffer.file_paths_mut().push(file_diff.path);
                current_tokens = Some(effective_token_count);
            }
        } else if !is_lockfile {
            // File is too large, need to split by hunks
            if !buffer.is_empty() {
                chunks.push(DiffChunk::new(
                    Arc::from(buffer.take_content()),
                    buffer.take_file_paths(),
                    current_tokens.unwrap_or_else(|| TokenCount::new_at_least_one(1)),
                ));
                buffer.clear();
            }

            let hunk_chunks =
                split_by_hunks(&file_diff.path, &file_diff.content, token_limit, tokenizer);
            chunks.extend(hunk_chunks);
        }
    }

    // Flush the last chunk
    if !buffer.is_empty() {
        chunks.push(DiffChunk::new(
            Arc::from(buffer.take_content()),
            buffer.take_file_paths(),
            current_tokens.unwrap_or_else(|| TokenCount::new_at_least_one(1)),
        ));
    }

    // Return buffer to pool
    release_buffer(buffer);

    chunks
}

/// Check if a file should be limited based on ignore patterns.
///
/// WHY Path methods over string ends_with: Path::file_name() and Path::extension()
/// properly handle UTF-8 encoded paths, including Unicode-normalized filenames.
/// String ends_with is naive and can break with composed characters or multi-byte sequences.
/// Check if a file should have content limit applied based on ignore patterns.
///
/// Pattern matching rules:
/// - Exact filename: "Cargo.lock" matches only files named "Cargo.lock"
/// - Simple glob: "*.lock" matches any file ending with ".lock"
/// - Path suffix: "vendor/" matches any file under a vendor directory
///
/// WHY not true glob: Most ignore patterns are simple (lockfiles, vendor dirs).
/// Supporting `*` prefix and suffix patterns covers >95% of use cases without
/// adding glob crate dependency. Full glob (e.g., `**/*.lock`) can be added later.
pub fn should_limit_file(path: &FilePath, ignore_patterns: &[String]) -> bool {
    let path_ref: &std::path::Path = path.as_ref();
    let Some(path_str) = path_ref.to_str() else {
        return false;
    };

    ignore_patterns.iter().any(|pattern| {
        if let Some(suffix) = pattern.strip_prefix('*') {
            // Suffix wildcard: "*.lock" matches "Cargo.lock"
            path_str.ends_with(suffix)
        } else if pattern.ends_with('/') || pattern.ends_with('\\') {
            // Directory prefix: "vendor/" matches "vendor/pkg/file.go"
            path_str.contains(pattern)
        } else {
            // Exact filename match: "Cargo.lock" matches only "Cargo.lock", not "unlock.txt"
            path_ref
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|filename| filename == pattern)
        }
    })
}

/// Truncate content to a maximum number of tokens, line by line.
///
/// WHY token-level truncation: Precise—respects exact token budget. Alternative
/// (character count) would be inaccurate due to BPE tokenization (1 char ≠ 1 token).
///
/// WHY line boundary alignment: Improves readability. If truncating mid-line would
/// waste <20% of budget, we align to last newline. Trade-off: slightly underutilizes
/// context but produces cleaner chunks for human review.
///
/// WHY fallback: Some tokenizers (e.g., sentencepiece) can't decode partial sequences.
/// Line-by-line fallback is less precise but guaranteed safe.
///
/// - content must be valid UTF-8
/// - tokenizer must be initialized
/// - Returns original content if already under limit
/// - Falls back to line-by-line truncation if decode fails
pub fn truncate_to_token_limit(
    content: &str,
    limit: TokenCount,
    tokenizer: &dyn Tokenizer,
) -> String {
    // Encode entire content once
    let tokens = tokenizer.encode(content);

    // If already within limit, return as-is
    if tokens.len() <= limit.get() as usize {
        return content.to_string();
    }

    // Take first `limit` tokens and decode
    let truncated_tokens = &tokens[..limit.get() as usize];
    match tokenizer.decode(truncated_tokens) {
        Some(mut result) => {
            // WHY 80% retention threshold: Balance between line-aligned readability
            // and token budget utilization. <80% retention = too much waste; align
            // to nearest line. >80% = acceptable loss; use line boundary for cleaner output.
            if let Some(last_newline) = result.rfind('\n') {
                // Only use line boundary if we don't lose too much content (>80% retention)
                let line_slice = &result[..=last_newline];
                let line_token_count = line_slice.len() * limit.get() as usize / result.len();
                if line_token_count >= (limit.get() as usize * 4) / 5 {
                    result.truncate(last_newline + 1);
                }
            }
            result
        }
        // Fallback: if decode fails, use line-by-line approach
        None => truncate_to_token_limit_fallback(content, limit, tokenizer),
    }
}

/// Fallback line-by-line truncation if token decode fails.
///
/// This is a safety net that uses incremental token counting per line.
///
/// **Token count variance**: Due to BPE tokenization boundary effects, counting tokens
/// line-by-line with newlines may produce slightly different counts than encoding the
/// entire text. This fallback errs on the conservative side by verifying the actual
/// token count of the accumulated result after each line addition, guaranteeing we
/// never exceed the limit.
///
/// **Why this approach**: If decode() fails, we can't use precise token-level truncation.
/// Line-by-line counting with verification is the safest alternative that maintains
/// readability while strictly respecting the token budget.
fn truncate_to_token_limit_fallback(
    content: &str,
    limit: TokenCount,
    tokenizer: &dyn Tokenizer,
) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut truncated = String::new();

    for line in lines {
        let mut candidate = truncated.clone();
        candidate.push_str(line);
        candidate.push('\n');

        // Verify actual token count to handle BPE boundary effects
        let candidate_tokens = tokenizer.count_tokens(&candidate);

        if candidate_tokens.get() > limit.get() {
            break;
        }

        truncated = candidate;
    }

    truncated
}

/// Split a single file's diff by hunks (`@@`).
///
/// WHY header deduplication: Each chunk includes the file header (e.g., `diff --git a/file.txt`)
/// in the first chunk only. Subsequent chunks from the same file omit the header, saving tokens.
/// The LLM can infer the file context from:
/// - The `files` field in DiffChunk (contains file_path)
/// - The first hunk header `@@ -line,count +line,count @@` (provides line range context)
///
/// Duplicating the header in every chunk wastes ~20-50 tokens per chunk across large diffs.
/// LLMs process sequential hunk changes well without redundant file metadata.
///
/// - content must be valid UTF-8
pub fn split_by_hunks(
    file_path: &FilePath,
    content: &str,
    token_limit: TokenCount,
    tokenizer: &dyn Tokenizer,
) -> Vec<DiffChunk> {
    const HUNK_HEADER: &str = "\n@@";

    let mut chunks = Vec::new();

    // Find the file header end (up to first hunk or end)
    let header_end = content.find("\n@@").unwrap_or(content.len());
    let header = &content[..header_end];
    let header_tokens = tokenizer.count_tokens(header);
    let header_tokens_count = header_tokens.get();

    // If just the header exceeds limit, we need line-level splitting
    if header_tokens_count > token_limit.get() {
        return split_by_lines(file_path, content, token_limit, tokenizer);
    }

    // Acquire buffer from pool
    let mut buffer = acquire_buffer();
    buffer.content_mut().push_str(header);
    let mut current_tokens = header_tokens_count;

    // Find all hunk positions
    let hunk_positions: Vec<usize> = content
        .match_indices(HUNK_HEADER)
        .map(|(idx, _)| idx + 1)
        .collect();

    if hunk_positions.is_empty() {
        // No hunks found, treat as single chunk
        release_buffer(buffer);
        let token_count = tokenizer.count_tokens(content);
        if token_count <= token_limit {
            chunks.push(DiffChunk::new(
                Arc::from(content),
                vec![file_path.clone()],
                token_count,
            ));
        } else {
            chunks.extend(split_by_lines(file_path, content, token_limit, tokenizer));
        }
        return chunks;
    }

    // Process each hunk
    for i in 0..hunk_positions.len() {
        let hunk_start = hunk_positions[i];
        let hunk_end = hunk_positions.get(i + 1).copied().unwrap_or(content.len());
        let hunk = &content[hunk_start..hunk_end];
        let hunk_tokens = tokenizer.count_tokens(hunk);
        let hunk_tokens_count = hunk_tokens.get();

        // If this single hunk exceeds limit, split by lines
        if hunk_tokens_count > token_limit.get() {
            if !buffer.is_empty() {
                chunks.push(DiffChunk::new(
                    Arc::from(buffer.take_content()),
                    vec![file_path.clone()],
                    TokenCount::new_at_least_one(current_tokens),
                ));
                buffer.clear();
                current_tokens = 0;
            }
            chunks.extend(split_by_lines(file_path, hunk, token_limit, tokenizer));
            continue;
        }

        // Check if adding this hunk would exceed limit
        if current_tokens + hunk_tokens_count > token_limit.get() {
            if !buffer.is_empty() {
                chunks.push(DiffChunk::new(
                    Arc::from(buffer.take_content()),
                    vec![file_path.clone()],
                    TokenCount::new_at_least_one(current_tokens),
                ));
                buffer.clear();
            }
            // Start new chunk with hunk only (no header deduplication)
            buffer.content_mut().push('\n');
            buffer.content_mut().push_str(hunk);
            current_tokens = hunk_tokens_count + 1;
        } else {
            buffer.content_mut().push('\n');
            buffer.content_mut().push_str(hunk);
            current_tokens += hunk_tokens_count + 1;
        }
    }

    // Flush remaining content
    if !buffer.is_empty() {
        chunks.push(DiffChunk::new(
            Arc::from(buffer.take_content()),
            vec![file_path.clone()],
            TokenCount::new_at_least_one(current_tokens),
        ));
    }

    // Return buffer to pool
    release_buffer(buffer);

    chunks
}

/// Split content by lines (last resort for very large hunks).
///
/// - content must be valid UTF-8
pub fn split_by_lines(
    file_path: &FilePath,
    content: &str,
    token_limit: TokenCount,
    tokenizer: &dyn Tokenizer,
) -> Vec<DiffChunk> {
    let mut chunks = Vec::new();
    let mut buffer = acquire_buffer();
    let mut current_tokens = 0u32;

    for line in content.lines() {
        // Count tokens with line + newline
        // Temporarily build line in a separate buffer to count tokens
        let line_token_count = {
            let mut temp = String::with_capacity(line.len() + 1);
            temp.push_str(line);
            temp.push('\n');
            tokenizer.count_tokens(&temp).get()
        };

        // If a single line exceeds the limit, use smart slicing
        if line_token_count > token_limit.get() {
            if !buffer.is_empty() {
                chunks.push(DiffChunk::new(
                    Arc::from(buffer.take_content()),
                    vec![file_path.clone()],
                    TokenCount::new_at_least_one(current_tokens),
                ));
                buffer.clear();
                current_tokens = 0;
            }

            // Split this oversized line intelligently
            let line_chunks = split_oversized_line(file_path, line, token_limit, tokenizer);
            chunks.extend(line_chunks);
            continue;
        }

        // Check if adding this line would exceed limit
        if current_tokens + line_token_count > token_limit.get() && !buffer.is_empty() {
            chunks.push(DiffChunk::new(
                Arc::from(buffer.take_content()),
                vec![file_path.clone()],
                TokenCount::new_at_least_one(current_tokens),
            ));
            buffer.clear();
            current_tokens = 0;
        }

        buffer.content_mut().push_str(line);
        buffer.content_mut().push('\n');
        current_tokens += line_token_count;
    }

    // Flush remaining content
    if !buffer.is_empty() {
        chunks.push(DiffChunk::new(
            Arc::from(buffer.take_content()),
            vec![file_path.clone()],
            TokenCount::new_at_least_one(current_tokens),
        ));
    }

    // Return buffer to pool
    release_buffer(buffer);

    chunks
}

/// Split an oversized line into smaller chunks.
///
/// WHY binary search: Finds longest slice ≤ token_limit in O(log n) character scans.
/// Alternative (linear scan) would be O(n²) for large lines (e.g., minified JS).
///
/// WHY UTF-8 boundary adjustment: Binary search mid-point can land inside multi-byte
/// codepoint. Adjusting ensures valid slices. Without this, we'd panic on invalid UTF-8.
///
/// WHY guaranteed progress: If best == start (e.g., single emoji exceeds limit), force
/// +1 char to avoid infinite loop. Produces invalid chunk but prevents hang.
fn split_oversized_line(
    file_path: &FilePath,
    line: &str,
    token_limit: TokenCount,
    tokenizer: &dyn Tokenizer,
) -> Vec<DiffChunk> {
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < line.len() {
        // Binary search for the longest slice that fits
        let mut low = start + 1;
        let mut high = line.len();
        let mut best = start + 1;

        while low <= high {
            let mid = (low + high) / 2;

            // Ensure mid is at a UTF-8 character boundary
            let mut adjusted_mid = mid;
            while adjusted_mid > start && !line.is_char_boundary(adjusted_mid) {
                adjusted_mid -= 1;
            }

            // Guard against zero progress
            if adjusted_mid == start {
                // Take at least one character
                adjusted_mid = start + 1;
                while adjusted_mid < line.len() && !line.is_char_boundary(adjusted_mid) {
                    adjusted_mid += 1;
                }
            }

            let slice = &line[start..adjusted_mid];
            let tokens = tokenizer.count_tokens(slice);

            if tokens <= token_limit {
                best = adjusted_mid;
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }

        // Ensure best is at a UTF-8 character boundary
        while best > start && !line.is_char_boundary(best) {
            best -= 1;
        }

        // Ensure we make progress
        if best == start {
            // Take at least one character to avoid infinite loop
            best = start + 1;
            while best < line.len() && !line.is_char_boundary(best) {
                best += 1;
            }
        }

        let chunk_content = &line[start..best];
        let token_count = tokenizer.count_tokens(chunk_content);
        chunks.push(DiffChunk::new(
            Arc::from(chunk_content),
            vec![file_path.clone()],
            token_count,
        ));

        start = best;
    }

    chunks
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::test_helpers::DeterministicTokenizer;
    use proptest::prelude::*;

    fn file_diff(path: &str, content: &str, tokenizer: &DeterministicTokenizer) -> FileDiff {
        FileDiff {
            path: FilePath::from(path),
            content: Arc::from(content),
            token_count: tokenizer.count_tokens(content),
            truncated: false,
        }
    }

    fn tokenize_chunks(chunks: &[DiffChunk], tokenizer: &DeterministicTokenizer) -> Vec<u32> {
        chunks
            .iter()
            .map(|chunk| tokenizer.count_tokens(&chunk.content).get())
            .collect()
    }

    fn sample_header() -> &'static str {
        "diff --git a/file.txt b/file.txt\nindex 1111111..2222222 100644\n--- a/file.txt\n+++ b/file.txt"
    }

    fn sample_hunk(header: &str, lines: &[&str]) -> String {
        let mut content = String::new();
        content.push_str(header);
        content.push('\n');
        content.push_str("@@ -1,1 +1,1 @@\n");
        for line in lines {
            content.push_str(line);
            content.push('\n');
        }
        content
    }

    #[test]
    fn test_split_empty() {
        let tokenizer = DeterministicTokenizer;
        let chunks = split_recursive(
            Vec::new(),
            TokenCount::new_at_least_one(100),
            &[],
            TokenCount::new_at_least_one(LOCKFILE_TOKEN_LIMIT),
            &tokenizer,
        );
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_split_single_small_file() {
        let tokenizer = DeterministicTokenizer;
        let content = sample_hunk(sample_header(), &["+hello"]);
        let file = file_diff("file.txt", &content, &tokenizer);

        let chunks = split_recursive(
            vec![file],
            TokenCount::new_at_least_one(200),
            &[],
            TokenCount::new_at_least_one(LOCKFILE_TOKEN_LIMIT),
            &tokenizer,
        );

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].files, vec![FilePath::from("file.txt")]);
        assert_eq!(chunks[0].content.as_ref(), content);
    }

    #[test]
    fn test_split_multiple_files_one_chunk() {
        let tokenizer = DeterministicTokenizer;
        let content_a = sample_hunk(sample_header(), &["+alpha"]);
        let content_b = sample_hunk(sample_header(), &["+beta"]);
        let file_a = file_diff("a.txt", &content_a, &tokenizer);
        let file_b = file_diff("b.txt", &content_b, &tokenizer);

        let chunks = split_recursive(
            vec![file_a, file_b],
            TokenCount::new_at_least_one(500),
            &[],
            TokenCount::new_at_least_one(LOCKFILE_TOKEN_LIMIT),
            &tokenizer,
        );

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].files.len(), 2);
        assert!(chunks[0].content.contains(&content_a));
        assert!(chunks[0].content.contains(&content_b));
    }

    #[test]
    fn test_split_large_file_multiple_chunks() {
        let tokenizer = DeterministicTokenizer;
        let mut content = String::new();
        for _ in 0..200 {
            content.push_str("diff --git a/large.txt b/large.txt\n");
            content.push_str("@@ -1 +1 @@\n");
            content.push_str("+line one\n");
        }
        let file = file_diff("large.txt", &content, &tokenizer);

        let chunks = split_recursive(
            vec![file],
            TokenCount::new_at_least_one(50),
            &[],
            TokenCount::new_at_least_one(LOCKFILE_TOKEN_LIMIT),
            &tokenizer,
        );

        assert!(chunks.len() > 1);
        let counts = tokenize_chunks(&chunks, &tokenizer);
        assert!(counts.iter().all(|count| *count <= 50));
    }

    #[test]
    fn test_split_respects_token_limit() {
        let tokenizer = DeterministicTokenizer;
        let mut files = Vec::new();
        for i in 0..10 {
            let content = sample_hunk(sample_header(), &["+alpha", "+beta", "+gamma"]);
            files.push(file_diff(&format!("file_{i}.txt"), &content, &tokenizer));
        }

        let chunks = split_recursive(
            files,
            TokenCount::new_at_least_one(25),
            &[],
            TokenCount::new_at_least_one(LOCKFILE_TOKEN_LIMIT),
            &tokenizer,
        );

        for count in tokenize_chunks(&chunks, &tokenizer) {
            assert!(count <= 25);
        }
    }

    #[test]
    fn test_split_lockfile_truncation() {
        let tokenizer = DeterministicTokenizer;
        let mut content = String::new();
        for _ in 0..50 {
            content.push_str("diff --git a/Cargo.lock b/Cargo.lock\n");
            content.push_str("@@ -1 +1 @@\n");
            content.push_str("+package name version\n");
        }
        let file = file_diff("Cargo.lock", &content, &tokenizer);

        let chunks = split_recursive(
            vec![file],
            TokenCount::new_at_least_one(500),
            &["Cargo.lock".to_string()],
            TokenCount::new_at_least_one(LOCKFILE_TOKEN_LIMIT),
            &tokenizer,
        );

        assert_eq!(chunks.len(), 1);
        let chunk_content = &chunks[0].content;
        assert!(chunk_content.contains("[... truncated lockfile ...]"));
        let tokens = tokenizer.count_tokens(chunk_content);
        assert!(tokens.get() <= 500);
    }

    #[test]
    fn test_split_by_hunks_basic() {
        let tokenizer = DeterministicTokenizer;
        let content = format!(
            "{header}\n@@ -1 +1 @@\n+alpha\n@@ -2 +2 @@\n+beta\n@@ -3 +3 @@\n+gamma\n",
            header = sample_header()
        );

        let chunks = split_by_hunks(
            &FilePath::from("file.txt"),
            &content,
            TokenCount::new_at_least_one(20),
            &tokenizer,
        );

        assert!(chunks.len() >= 2);
        assert!(chunks[0].content.contains("@@ -1 +1 @@"));
        assert!(chunks[1].content.contains("@@ -2 +2 @@"));
    }

    #[test]
    fn test_split_by_hunks_header_only() {
        let tokenizer = DeterministicTokenizer;
        let content = sample_header().to_string();

        let chunks = split_by_hunks(
            &FilePath::from("file.txt"),
            &content,
            TokenCount::new_at_least_one(200),
            &tokenizer,
        );

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content.as_ref(), content);
    }

    #[test]
    fn test_split_by_hunks_fallback_to_lines() {
        let tokenizer = DeterministicTokenizer;
        let header = sample_header();
        let mut content = String::new();
        content.push_str(header);
        content.push('\n');
        content.push_str("@@ -1 +1 @@\n");
        for _ in 0..40 {
            content.push_str("+oversized hunk line content\n");
        }

        let chunks = split_by_hunks(
            &FilePath::from("file.txt"),
            &content,
            TokenCount::new_at_least_one(20),
            &tokenizer,
        );

        assert!(chunks.len() > 1);
        for count in tokenize_chunks(&chunks, &tokenizer) {
            assert!(count <= 20);
        }
    }

    #[test]
    fn test_split_by_lines_basic() {
        let tokenizer = DeterministicTokenizer;
        let content = "line one\nline two\nline three\n";

        let chunks = split_by_lines(
            &FilePath::from("file.txt"),
            content,
            TokenCount::new_at_least_one(3),
            &tokenizer,
        );

        assert!(chunks.len() >= 2);
        for count in tokenize_chunks(&chunks, &tokenizer) {
            assert!(count <= 3);
        }
    }

    #[test]
    fn test_split_oversized_line() {
        let tokenizer = DeterministicTokenizer;
        let content = "this line is far too long to fit\n";

        let chunks = split_by_lines(
            &FilePath::from("file.txt"),
            content,
            TokenCount::new_at_least_one(2),
            &tokenizer,
        );

        assert!(chunks.len() > 1);
        for count in tokenize_chunks(&chunks, &tokenizer) {
            assert!(count <= 2);
        }
    }

    #[test]
    fn test_truncate_respects_limit() {
        let tokenizer = DeterministicTokenizer;
        let content = "alpha beta gamma delta epsilon";
        let truncated =
            truncate_to_token_limit(content, TokenCount::new_at_least_one(3), &tokenizer);
        let tokens = tokenizer.count_tokens(&truncated);
        assert!(tokens.get() <= 3);
    }

    #[test]
    fn test_truncate_preserves_newline() {
        let tokenizer = DeterministicTokenizer;
        let content = "alpha beta\ngamma delta\nepsilon zeta";
        let truncated =
            truncate_to_token_limit(content, TokenCount::new_at_least_one(12), &tokenizer);
        assert!(truncated.ends_with('\n') || truncated == content);
        let tokens = tokenizer.count_tokens(&truncated);
        assert!(tokens.get() <= 12);
    }

    #[test]
    fn test_truncate_fallback_to_lines() {
        struct FailingDecodeTokenizer;

        impl Tokenizer for FailingDecodeTokenizer {
            fn count_tokens_exact(&self, text: &str) -> u32 {
                if text.is_empty() {
                    return 0;
                }
                text.split_whitespace().count() as u32
            }

            fn count_tokens(&self, text: &str) -> TokenCount {
                TokenCount::new_at_least_one(self.count_tokens_exact(text))
            }

            fn encoding_name(&self) -> &str {
                "failing-decode"
            }

            fn encode(&self, text: &str) -> Vec<u32> {
                text.chars().map(|c| c as u32).collect()
            }

            fn decode(&self, _tokens: &[u32]) -> Option<String> {
                None
            }
        }

        let tokenizer = FailingDecodeTokenizer;
        let content = "alpha beta\ngamma delta\nepsilon zeta";
        let truncated =
            truncate_to_token_limit(content, TokenCount::new_at_least_one(3), &tokenizer);
        assert!(truncated.contains('\n'));
        let tokens = tokenizer.count_tokens(&truncated);
        assert!(tokens.get() <= 3);
    }

    #[test]
    fn fallback_truncation_respects_token_limit() {
        // Test that fallback truncation never exceeds limits across various scenarios
        struct FailingDecodeTokenizer;

        impl Tokenizer for FailingDecodeTokenizer {
            fn count_tokens_exact(&self, text: &str) -> u32 {
                // Simulate realistic token counts (roughly 4 chars per token)
                (text.len() / 4) as u32
            }

            fn count_tokens(&self, text: &str) -> TokenCount {
                TokenCount::new_at_least_one(self.count_tokens_exact(text))
            }

            fn encoding_name(&self) -> &str {
                "test"
            }

            fn encode(&self, text: &str) -> Vec<u32> {
                text.chars().map(|c| c as u32).collect()
            }

            fn decode(&self, _tokens: &[u32]) -> Option<String> {
                None // Force fallback
            }
        }

        let tokenizer = FailingDecodeTokenizer;

        // Test 1: Multiple lines with varying lengths
        let content = "line 1\nthis is a longer line with more content\nshort\nanother medium length line here\nfinal";
        for limit in [5, 10, 20, 50] {
            let truncated =
                truncate_to_token_limit(content, TokenCount::new_at_least_one(limit), &tokenizer);
            let actual_tokens = tokenizer.count_tokens(&truncated);
            assert!(
                actual_tokens.get() <= limit,
                "Fallback exceeded limit: {} tokens with limit {}",
                actual_tokens.get(),
                limit
            );
        }

        // Test 2: Content with very long lines
        let long_line = "x".repeat(200);
        let content_long = format!("{}\n{}\n{}", long_line, long_line, long_line);
        let truncated =
            truncate_to_token_limit(&content_long, TokenCount::new_at_least_one(30), &tokenizer);
        let actual_tokens = tokenizer.count_tokens(&truncated);
        assert!(actual_tokens.get() <= 30);

        // Test 3: Edge case with limit = 1
        let content_small = "a\nb\nc\nd";
        let truncated =
            truncate_to_token_limit(content_small, TokenCount::new_at_least_one(1), &tokenizer);
        let actual_tokens = tokenizer.count_tokens(&truncated);
        assert!(actual_tokens.get() <= 1);
    }

    fn file_diff_strategy() -> impl Strategy<Value = FileDiff> {
        let content_strategy =
            proptest::string::string_regex("[a-zA-Z0-9 _\n@+-]{0,200}").expect("valid regex");
        ("[a-z]{1,8}\\.txt", content_strategy).prop_map(|(name, content)| {
            let tokenizer = DeterministicTokenizer;
            FileDiff {
                path: FilePath::from(name),
                content: Arc::from(content.as_str()),
                token_count: tokenizer.count_tokens(&content),
                truncated: false,
            }
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        #[test]
        fn chunks_never_exceed_token_limit(
            files in prop::collection::vec(file_diff_strategy(), 0..10),
            limit in 100usize..10000
        ) {
            let tokenizer = DeterministicTokenizer;
            let chunks = split_recursive(
                files,
                TokenCount::new_at_least_one(limit as u32),
                &[],
                TokenCount::new_at_least_one(LOCKFILE_TOKEN_LIMIT),
                &tokenizer,
            );
            for chunk in chunks {
                let tokens = tokenizer.count_tokens(&chunk.content);
                prop_assert!(tokens.get() <= limit as u32);
            }
        }

        #[test]
        fn utf8_boundaries_preserved(
            content in "\\PC{0,10000}"
        ) {
            let tokenizer = DeterministicTokenizer;
            let truncated = truncate_to_token_limit(&content, TokenCount::new_at_least_one(100), &tokenizer);
            prop_assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
        }
    }

    #[test]
    fn should_limit_file_exact_filename_match() {
        let patterns = vec!["Cargo.lock".to_string(), "package.json".to_string()];

        // Should match exact filename
        assert!(should_limit_file(&FilePath::from("Cargo.lock"), &patterns));
        assert!(should_limit_file(
            &FilePath::from("path/to/Cargo.lock"),
            &patterns
        ));
        assert!(should_limit_file(
            &FilePath::from("package.json"),
            &patterns
        ));

        // Should NOT match partial matches
        assert!(!should_limit_file(&FilePath::from("unlock.txt"), &patterns));
        assert!(!should_limit_file(
            &FilePath::from("Cargo.lock.bak"),
            &patterns
        ));
        assert!(!should_limit_file(
            &FilePath::from("my-package.json"),
            &patterns
        ));
    }

    #[test]
    fn should_limit_file_wildcard_pattern() {
        let patterns = vec!["*.lock".to_string(), "*.min.js".to_string()];

        // Should match wildcard suffix
        assert!(should_limit_file(&FilePath::from("Cargo.lock"), &patterns));
        assert!(should_limit_file(&FilePath::from("yarn.lock"), &patterns));
        assert!(should_limit_file(&FilePath::from("app.min.js"), &patterns));

        // Should NOT match if suffix doesn't match
        assert!(!should_limit_file(&FilePath::from("unlock.txt"), &patterns));
        assert!(!should_limit_file(&FilePath::from("app.js"), &patterns));
    }

    #[test]
    fn should_limit_file_directory_pattern() {
        let patterns = vec!["vendor/".to_string(), "node_modules/".to_string()];

        // Should match directory prefix
        assert!(should_limit_file(
            &FilePath::from("vendor/package/file.go"),
            &patterns
        ));
        assert!(should_limit_file(
            &FilePath::from("node_modules/react/index.js"),
            &patterns
        ));

        // Should NOT match if not in directory
        assert!(!should_limit_file(
            &FilePath::from("src/vendor.go"),
            &patterns
        ));
        assert!(!should_limit_file(
            &FilePath::from("my_node_modules.txt"),
            &patterns
        ));
    }

    #[test]
    fn should_limit_file_mixed_patterns() {
        let patterns = vec![
            "Cargo.lock".to_string(),
            "*.min.js".to_string(),
            "dist/".to_string(),
        ];

        assert!(should_limit_file(&FilePath::from("Cargo.lock"), &patterns));
        assert!(should_limit_file(&FilePath::from("app.min.js"), &patterns));
        assert!(should_limit_file(
            &FilePath::from("dist/bundle.js"),
            &patterns
        ));
        assert!(!should_limit_file(
            &FilePath::from("src/main.rs"),
            &patterns
        ));
    }

    #[test]
    fn should_limit_file_empty_patterns() {
        let patterns: Vec<String> = vec![];
        assert!(!should_limit_file(
            &FilePath::from("any/file.txt"),
            &patterns
        ));
    }
}
