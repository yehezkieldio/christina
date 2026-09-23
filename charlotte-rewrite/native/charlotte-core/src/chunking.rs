//! Diff chunking, ported from Christina's
//! `christina-core/src/processing/chunking.rs` (`split_recursive` and the
//! hunk/line/token-span fallback chain) and `christina/src/git/parsing.rs`
//! (file-header splitting and deletion-only truncation). Per
//! `02-native-core-and-ffi.md`, this module's only externally relevant
//! entry point is [`chunk_diff`]; everything else is an internal step in
//! that pipeline.
//!
//! Simplification (open decision in `05-diff-processing-and-chunking.md`):
//! Christina pools `ChunkBuffer` allocations across recursive splits
//! (`acquire_buffer`/`release_buffer`) to cut allocation overhead. This
//! port allocates plain `String`/`Vec` values instead. The FFI boundary's
//! own JSON serialization cost likely dominates the runtime here anyway;
//! reintroduce pooling only if a benchmark on a representative large diff
//! shows it matters.
//!
//! Simplification: `02-native-core-and-ffi.md`'s `chunk_diff` signature
//! takes a single `lockfile_token_limit`, not Christina's caller-supplied
//! `ignore_patterns` list (`03-config-and-profiles.md`'s config-driven
//! patterns apply at a higher layer in Charlotte, not inside the native
//! core). This module instead recognizes a fixed set of common lockfile
//! basenames directly, matching the spirit of Christina's "lockfile"
//! special-casing without threading a pattern list across the FFI edge.

// Offset/length arithmetic throughout this module operates on `usize`
// lengths and indices of a diff already loaded into memory as a `String`
// (bounded well under `usize::MAX` on every real target, and each
// individual file's contribution is further capped by
// `MAX_FILE_DIFF_SIZE`). Converting every `+`/`+=`/`-=` here to
// `checked_add`/`saturating_sub` would triple the line count of the
// binary-search and hunk-splitting logic below for a wraparound that is
// not reachable from any real diff.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "loop counters and offset math here index into an in-memory diff string well under usize::MAX; see module doc comment"
)]

use crate::tokenizer::{self, TokenCount};

/// Matches Christina's `LOCKFILE_TOKEN_LIMIT` default: preserves the "a
/// lockfile changed" signal without spending prompt budget on
/// auto-generated noise. `chunk_diff`'s FFI signature takes this value from
/// the caller rather than defaulting it here — `@charlotte/config`'s schema
/// owns the real default — so this constant only exists for tests below.
#[cfg(test)]
const DEFAULT_LOCKFILE_TOKEN_LIMIT: u32 = 100;

const KNOWN_LOCKFILE_BASENAMES: &[&str] = &[
    "cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lock",
    "bun.lockb",
    "gemfile.lock",
    "poetry.lock",
    "composer.lock",
    "go.sum",
];

const FILE_HEADER: &str = "diff --git ";
/// Per-file cap keeps one pathological file diff from dominating memory,
/// matching Christina's `MAX_FILE_DIFF_SIZE`.
const MAX_FILE_DIFF_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub content: String,
    pub file_paths: Vec<String>,
}

#[derive(Debug, Clone)]
struct FileDiff {
    path: String,
    content: String,
    token_count: TokenCount,
}

fn is_lockfile(path: &str) -> bool {
    let basename = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    KNOWN_LOCKFILE_BASENAMES.contains(&basename.as_str())
}

fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Extracts the destination (`b/`) path from a `diff --git a/... b/...`
/// header line. Simplified from Christina's `parse_git_diff_header`: this
/// port handles the common unquoted-path case (the overwhelming majority of
/// real diffs) and skips the quoted/escaped-path edge case for paths
/// containing spaces or control characters.
fn parse_git_diff_header(line: &str) -> Option<String> {
    let after_git = line.trim().strip_prefix(FILE_HEADER)?;
    let mut parts = after_git.split_whitespace();
    let _path_a = parts.next()?;
    let path_b = parts.next()?;
    let stripped = path_b.strip_prefix("b/").or_else(|| path_b.strip_prefix("a/")).unwrap_or(path_b);
    Some(stripped.to_string())
}

fn split_by_files(diff: &str) -> Vec<FileDiff> {
    let mut positions = Vec::new();
    let mut offset = 0usize;
    for line in diff.split_inclusive('\n') {
        if line.starts_with(FILE_HEADER) {
            positions.push(offset);
        }
        offset += line.len();
    }

    if positions.is_empty() {
        return Vec::new();
    }

    let mut files = Vec::with_capacity(positions.len());
    for (i, &start) in positions.iter().enumerate() {
        let end = positions.get(i + 1).copied().unwrap_or(diff.len());
        let raw_content = &diff[start..end];
        let Some(header_line) = raw_content.lines().next() else {
            continue;
        };
        let Some(path) = parse_git_diff_header(header_line) else {
            continue;
        };

        let content = if raw_content.len() > MAX_FILE_DIFF_SIZE {
            safe_truncate(raw_content, MAX_FILE_DIFF_SIZE).to_string()
        } else {
            raw_content.to_string()
        };

        let token_count = TokenCount::new_at_least_one(tokenizer::count_tokens(&content));
        files.push(FileDiff { path, content, token_count });
    }

    files
}

fn is_deletion_only(content: &str) -> bool {
    let mut has_deletion = false;
    for line in content.lines() {
        let line = line.trim_start();
        if line.starts_with('+') && !line.starts_with("+++") {
            return false;
        }
        if line.starts_with('-') && !line.starts_with("---") {
            has_deletion = true;
        }
    }
    has_deletion
}

/// Truncates a deletion-only diff to `max_deletion_lines` deletion lines per
/// hunk, keeping every metadata line. Ported from Christina's
/// `truncate_deletion_diff`.
fn truncate_deletion_diff(content: &str, max_deletion_lines: usize) -> String {
    let mut result = String::new();
    let mut deletion_lines_shown = 0usize;
    let mut truncation_notice_emitted = false;
    let mut total_deletions_skipped = 0usize;

    for line in content.lines() {
        let trimmed = line.trim_start();
        let is_metadata = trimmed.starts_with(FILE_HEADER)
            || trimmed.starts_with("index ")
            || trimmed.starts_with("---")
            || trimmed.starts_with("+++")
            || trimmed.starts_with("@@")
            || trimmed.starts_with("deleted file mode")
            || trimmed.starts_with("new file mode")
            || trimmed.starts_with("similarity index")
            || trimmed.starts_with("rename from")
            || trimmed.starts_with("rename to")
            || trimmed.starts_with("copy from")
            || trimmed.starts_with("copy to");

        if is_metadata {
            result.push_str(line);
            result.push('\n');
            if trimmed.starts_with("@@") {
                deletion_lines_shown = 0;
                truncation_notice_emitted = false;
            }
            continue;
        }

        if trimmed.starts_with('-') && !trimmed.starts_with("---") {
            if deletion_lines_shown < max_deletion_lines {
                result.push_str(line);
                result.push('\n');
                deletion_lines_shown += 1;
            } else {
                total_deletions_skipped += 1;
                if !truncation_notice_emitted {
                    result.push_str("[... deleted content truncated to save tokens ...]\n");
                    truncation_notice_emitted = true;
                }
            }
            continue;
        }

        result.push_str(line);
        result.push('\n');
    }

    if total_deletions_skipped > 0 {
        result.push_str(&format!(
            "\n[Truncated {total_deletions_skipped} deletion lines - full deletion diff not needed for commit message generation]\n"
        ));
    }

    result
}

/// The two-tier deletion-line cap: a very large deletion-only diff (≥500 KB)
/// keeps more lines of context (100) than a smaller one (50), matching
/// Christina's `process`/`process_file_diff`.
fn deletion_line_limit(content_len: usize) -> usize {
    if content_len >= 500 * 1024 { 100 } else { 50 }
}

/// Truncates `content` to at most `limit` tokens, preferring to land on a
/// line boundary when that only costs a small amount of budget. Ported from
/// Christina's `truncate_to_token_limit`.
fn truncate_to_token_limit(content: &str, limit: TokenCount) -> String {
    let tokens = tokenizer::encode(content);
    if tokens.len() <= limit.get() as usize {
        return content.to_string();
    }

    let Some(truncated_tokens) = tokens.get(..limit.get() as usize) else {
        return content.to_string();
    };
    match tokenizer::decode(truncated_tokens) {
        Some(mut result) => {
            if let Some(last_newline) = result.rfind('\n') {
                let line_slice = &result[..=last_newline];
                let line_token_count = line_slice.len() * limit.get() as usize / result.len().max(1);
                if line_token_count >= (limit.get() as usize * 4) / 5 {
                    result.truncate(last_newline + 1);
                }
            }
            result
        }
        None => truncate_to_token_limit_fallback(content, limit),
    }
}

fn truncate_to_token_limit_fallback(content: &str, limit: TokenCount) -> String {
    let mut truncated = String::new();
    for line in content.lines() {
        let previous_len = truncated.len();
        truncated.push_str(line);
        truncated.push('\n');
        let candidate_tokens = TokenCount::new_at_least_one(tokenizer::count_tokens(&truncated));
        if candidate_tokens.get() > limit.get() {
            truncated.truncate(previous_len);
            break;
        }
    }
    truncated
}

/// Splits a single file's diff by hunk (`@@`), falling back to line-level
/// splitting when a single hunk (or the file header itself) exceeds the
/// token limit. Ported from Christina's `split_by_hunks`.
fn split_by_hunks(file_path: &str, content: &str, token_limit: TokenCount) -> Vec<Chunk> {
    const HUNK_HEADER: &str = "\n@@";

    let header_end = content.find("\n@@").unwrap_or(content.len());
    let header = &content[..header_end];
    let header_tokens = TokenCount::new_at_least_one(tokenizer::count_tokens(header));

    if header_tokens.get() > token_limit.get() {
        return split_by_lines(file_path, content, token_limit);
    }

    let hunk_positions: Vec<usize> = content.match_indices(HUNK_HEADER).map(|(idx, _)| idx + 1).collect();

    if hunk_positions.is_empty() {
        let token_count = TokenCount::new_at_least_one(tokenizer::count_tokens(content));
        if token_count.get() <= token_limit.get() {
            return vec![Chunk { content: content.to_string(), file_paths: vec![file_path.to_string()] }];
        }
        return split_by_lines(file_path, content, token_limit);
    }

    let mut chunks = Vec::new();
    let mut buffer = header.to_string();

    for (i, &hunk_start) in hunk_positions.iter().enumerate() {
        let hunk_end = hunk_positions.get(i + 1).copied().unwrap_or(content.len());
        let hunk = &content[hunk_start..hunk_end];

        let solo_tokens = TokenCount::new_at_least_one(tokenizer::count_tokens(&format!("\n{hunk}"))).get();
        if solo_tokens > token_limit.get() {
            if !buffer.is_empty() {
                chunks.push(Chunk { content: std::mem::take(&mut buffer), file_paths: vec![file_path.to_string()] });
            }
            chunks.extend(split_by_lines(file_path, hunk, token_limit));
            continue;
        }

        let candidate = format!("{buffer}\n{hunk}");
        let candidate_tokens = TokenCount::new_at_least_one(tokenizer::count_tokens(&candidate)).get();

        if candidate_tokens > token_limit.get() {
            if !buffer.is_empty() {
                chunks.push(Chunk { content: std::mem::take(&mut buffer), file_paths: vec![file_path.to_string()] });
            }
            buffer = format!("\n{hunk}");
        } else {
            buffer = candidate;
        }
    }

    if !buffer.is_empty() {
        chunks.push(Chunk { content: buffer, file_paths: vec![file_path.to_string()] });
    }

    chunks
}

/// Splits content line by line, the second-to-last resort before splitting
/// a single oversized line. Ported from Christina's `split_by_lines`.
fn split_by_lines(file_path: &str, content: &str, token_limit: TokenCount) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut buffer = String::new();
    let mut current_tokens = 0u32;

    for line in content.lines() {
        let line_with_newline = format!("{line}\n");
        let line_token_count = tokenizer::count_tokens(&line_with_newline);

        if line_token_count > token_limit.get() {
            if !buffer.is_empty() {
                chunks.push(Chunk { content: std::mem::take(&mut buffer), file_paths: vec![file_path.to_string()] });
                current_tokens = 0;
            }
            chunks.extend(split_oversized_line(file_path, line, token_limit));
            continue;
        }

        if current_tokens + line_token_count > token_limit.get() && !buffer.is_empty() {
            chunks.push(Chunk { content: std::mem::take(&mut buffer), file_paths: vec![file_path.to_string()] });
            current_tokens = 0;
        }

        buffer.push_str(&line_with_newline);
        current_tokens += line_token_count;
    }

    if !buffer.is_empty() {
        chunks.push(Chunk { content: buffer, file_paths: vec![file_path.to_string()] });
    }

    chunks
}

/// Splits a single oversized line: first by exact raw token span, falling
/// back to a binary-search byte-slice approach if token-level slicing can't
/// round-trip. Ported from Christina's `split_oversized_line`,
/// `split_oversized_line_by_tokens`, and `split_oversized_line_by_search`.
fn split_oversized_line(file_path: &str, line: &str, token_limit: TokenCount) -> Vec<Chunk> {
    if let Some(chunks) = split_oversized_line_by_tokens(file_path, line, token_limit) {
        return chunks;
    }
    split_oversized_line_by_search(file_path, line, token_limit)
}

fn split_oversized_line_by_tokens(file_path: &str, line: &str, token_limit: TokenCount) -> Option<Vec<Chunk>> {
    let tokens = tokenizer::encode(line);
    let limit = token_limit.get() as usize;
    if tokens.len() <= limit || limit == 0 {
        return None;
    }

    let mut chunks = Vec::with_capacity(tokens.len().div_ceil(limit));
    let mut byte_offset = 0usize;

    for token_chunk in tokens.chunks(limit) {
        let decoded = tokenizer::decode(token_chunk)?;
        if decoded.is_empty() || !line[byte_offset..].starts_with(&decoded) {
            return None;
        }
        let end = byte_offset + decoded.len();
        if !line.is_char_boundary(end) {
            return None;
        }
        chunks.push(Chunk { content: line[byte_offset..end].to_string(), file_paths: vec![file_path.to_string()] });
        byte_offset = end;
    }

    (byte_offset == line.len()).then_some(chunks)
}

fn split_oversized_line_by_search(file_path: &str, line: &str, token_limit: TokenCount) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < line.len() {
        let mut low = start + 1;
        let mut high = line.len();
        let mut best = start + 1;

        while low <= high {
            let mid = (low + high) / 2;
            let mut adjusted_mid = mid;
            while adjusted_mid > start && !line.is_char_boundary(adjusted_mid) {
                adjusted_mid -= 1;
            }
            if adjusted_mid == start {
                adjusted_mid = start + 1;
                while adjusted_mid < line.len() && !line.is_char_boundary(adjusted_mid) {
                    adjusted_mid += 1;
                }
            }

            let slice = &line[start..adjusted_mid];
            let tokens = TokenCount::new_at_least_one(tokenizer::count_tokens(slice));

            if tokens.get() <= token_limit.get() {
                best = adjusted_mid;
                low = mid + 1;
            } else if mid == 0 {
                break;
            } else {
                high = mid - 1;
            }
        }

        while best > start && !line.is_char_boundary(best) {
            best -= 1;
        }
        if best == start {
            best = start + 1;
            while best < line.len() && !line.is_char_boundary(best) {
                best += 1;
            }
        }

        let chunk_content = &line[start..best];
        let token_count = TokenCount::new_at_least_one(tokenizer::count_tokens(chunk_content));
        if token_count.get() > token_limit.get() {
            start = best;
            continue;
        }
        chunks.push(Chunk { content: chunk_content.to_string(), file_paths: vec![file_path.to_string()] });
        start = best;
    }

    chunks
}

/// Packs whole files into chunks by greedy first-fit, splitting a file that
/// alone exceeds `token_limit` by hunk (then line, then raw token span).
/// Ported from Christina's `split_recursive`. Callers must already have
/// applied lockfile truncation to `file_diffs` (see [`chunk_diff`]) — this
/// function only packs, it does not special-case any path by name.
fn split_recursive(file_diffs: Vec<FileDiff>, token_limit: TokenCount) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut buffer_content = String::new();
    let mut buffer_files: Vec<String> = Vec::new();
    let mut current_tokens: Option<TokenCount> = None;

    for file_diff in file_diffs {
        if file_diff.token_count.get() <= token_limit.get() {
            let combined = current_tokens.map(|c| c.get()).unwrap_or(0) + file_diff.token_count.get();

            if combined <= token_limit.get() {
                buffer_content.push_str(&file_diff.content);
                buffer_files.push(file_diff.path);
                current_tokens = TokenCount::new(combined);
            } else {
                if !buffer_content.is_empty() {
                    chunks.push(Chunk {
                        content: std::mem::take(&mut buffer_content),
                        file_paths: std::mem::take(&mut buffer_files),
                    });
                }
                buffer_content.push_str(&file_diff.content);
                buffer_files.push(file_diff.path);
                current_tokens = Some(file_diff.token_count);
            }
        } else {
            if !buffer_content.is_empty() {
                chunks.push(Chunk {
                    content: std::mem::take(&mut buffer_content),
                    file_paths: std::mem::take(&mut buffer_files),
                });
                current_tokens = None;
            }
            chunks.extend(split_by_hunks(&file_diff.path, &file_diff.content, token_limit));
        }
    }

    if !buffer_content.is_empty() {
        chunks.push(Chunk { content: buffer_content, file_paths: buffer_files });
    }

    chunks
}

/// The native core's chunking entry point (`02-native-core-and-ffi.md`).
/// Splits `diff` into files, applies the deletion-only and lockfile
/// truncation tiers per file, then packs the results into token-bounded
/// chunks.
///
/// Truncation always runs before the whole-diff-fits-in-one-chunk check:
/// a diff can sit under `token_limit` in total while still containing a
/// lockfile whose own content exceeds `lockfile_token_limit`, and that
/// lockfile still needs truncating even though no further chunk-splitting
/// is required. An earlier version checked total size first and skipped
/// per-file truncation whenever everything fit in one chunk, which left
/// oversized lockfiles untruncated in exactly that case.
pub fn chunk_diff(diff: &str, token_limit: u32, lockfile_token_limit: u32) -> Vec<Chunk> {
    if diff.is_empty() {
        return Vec::new();
    }

    let token_limit = TokenCount::new_at_least_one(token_limit);
    let lockfile_token_limit = TokenCount::new_at_least_one(lockfile_token_limit);

    let mut file_diffs = split_by_files(diff);
    for file_diff in &mut file_diffs {
        if is_deletion_only(&file_diff.content) {
            let limit = deletion_line_limit(file_diff.content.len());
            file_diff.content = truncate_deletion_diff(&file_diff.content, limit);
            file_diff.token_count = TokenCount::new_at_least_one(tokenizer::count_tokens(&file_diff.content));
        }
        if is_lockfile(&file_diff.path) && file_diff.token_count.get() > lockfile_token_limit.get() {
            let mut truncated = truncate_to_token_limit(&file_diff.content, lockfile_token_limit);
            truncated.push_str("\n[... truncated lockfile ...]\n");
            file_diff.content = truncated;
            file_diff.token_count = TokenCount::new_at_least_one(tokenizer::count_tokens(&file_diff.content));
        }
    }

    let total_tokens: u32 = file_diffs.iter().map(|f| f.token_count.get()).sum();
    if total_tokens <= token_limit.get() {
        let content = file_diffs.iter().map(|f| f.content.as_str()).collect::<String>();
        let files = file_diffs.into_iter().map(|f| f.path).collect();
        return vec![Chunk { content, file_paths: files }];
    }

    split_recursive(file_diffs, token_limit)
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    reason = "test fixtures build small, known-length vectors in the same test right before indexing them; a panic on out-of-bounds is exactly the desired test failure"
)]
mod tests {
    use super::*;

    fn sample_header(path: &str) -> String {
        format!("diff --git a/{path} b/{path}\nindex 1111111..2222222 100644\n--- a/{path}\n+++ b/{path}")
    }

    fn sample_hunk(path: &str, lines: &[&str]) -> String {
        let mut content = sample_header(path);
        content.push('\n');
        content.push_str("@@ -1,1 +1,1 @@\n");
        for line in lines {
            content.push_str(line);
            content.push('\n');
        }
        content
    }

    #[test]
    fn chunk_diff_empty_input_is_empty() {
        assert!(chunk_diff("", 1000, DEFAULT_LOCKFILE_TOKEN_LIMIT).is_empty());
    }

    #[test]
    fn chunk_diff_small_diff_single_chunk() {
        let diff = sample_hunk("file.txt", &["+hello"]);
        let chunks = chunk_diff(&diff, 10_000, DEFAULT_LOCKFILE_TOKEN_LIMIT);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].file_paths, vec!["file.txt".to_string()]);
    }

    #[test]
    fn chunk_diff_splits_oversized_file_by_hunk() {
        let mut diff = sample_header("big.txt");
        diff.push('\n');
        for i in 0..200 {
            diff.push_str(&format!("@@ -{i},1 +{i},1 @@\n+line number {i} with some extra padding text\n"));
        }
        let chunks = chunk_diff(&diff, 50, DEFAULT_LOCKFILE_TOKEN_LIMIT);
        assert!(chunks.len() > 1, "expected multiple chunks, got {}", chunks.len());
        for chunk in &chunks {
            let tokens = tokenizer::count_tokens(&chunk.content);
            assert!(tokens <= 50 + 5, "chunk exceeded token budget: {tokens} tokens");
        }
    }

    #[test]
    fn chunk_diff_respects_lockfile_cap() {
        let mut diff = sample_header("Cargo.lock");
        diff.push('\n');
        diff.push_str("@@ -1,1 +1,500 @@\n");
        for i in 0..500 {
            diff.push_str(&format!("+dependency-{i} = \"1.0.0\"\n"));
        }
        let chunks = chunk_diff(&diff, 10_000, 20);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("[... truncated lockfile ...]"));
    }

    #[test]
    fn split_by_files_extracts_paths() {
        let diff = format!("{}\n{}", sample_hunk("a.txt", &["+a"]), sample_hunk("b.txt", &["+b"]));
        let files = split_by_files(&diff);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "a.txt");
        assert_eq!(files[1].path, "b.txt");
    }

    #[test]
    fn is_deletion_only_true_for_pure_deletion() {
        let content = "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-line one\n-line two\n";
        assert!(is_deletion_only(content));
    }

    #[test]
    fn is_deletion_only_false_when_additions_present() {
        let content = "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        assert!(!is_deletion_only(content));
    }

    #[test]
    fn truncate_deletion_diff_uses_small_limit() {
        let mut content = String::from("diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ /dev/null\n@@ -1,120 +0,0 @@\n");
        for i in 0..120 {
            content.push_str(&format!("-line {i}\n"));
        }
        let truncated = truncate_deletion_diff(&content, 50);
        assert!(truncated.contains("truncated"));
        assert_eq!(truncated.matches("-line").count(), 50);
    }

    #[test]
    fn deletion_line_limit_picks_larger_tier_for_big_diffs() {
        assert_eq!(deletion_line_limit(100), 50);
        assert_eq!(deletion_line_limit(500 * 1024), 100);
    }

    #[test]
    fn split_oversized_line_by_search_covers_whole_line() {
        let line = "x".repeat(500);
        let chunks = split_oversized_line_by_search("f.txt", &line, TokenCount::new_at_least_one(20));
        let rebuilt: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert_eq!(rebuilt, line);
    }
}
