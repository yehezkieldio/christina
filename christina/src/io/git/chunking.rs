use std::sync::Arc;

use christina_core::{
    Tokenizer,
    git::{DiffChunk, FileDiff},
    types::{FilePath, TokenCount},
};

use crate::io::git::buffer_pool;

pub const LOCKFILE_TOKEN_LIMIT: u32 = 100;

/// Split diff into chunks recursively by files, hunks, then lines.
///
/// Uses a greedy packing algorithm (First-Fit) to fill chunks up to the token limit
/// while maintaining semantic coherence at file boundaries.
///
/// - All file_diffs must contain valid UTF-8 content
/// - tokenizer must be initialized and functional
pub(crate) fn split_recursive(
    file_diffs: Vec<FileDiff>,
    token_limit: TokenCount,
    ignore_patterns: &[String],
    tokenizer: &dyn Tokenizer,
) -> Vec<DiffChunk> {
    let mut chunks = Vec::new();
    let mut buffer = buffer_pool::acquire_buffer();
    let mut current_tokens: Option<TokenCount> = None;

    for file_diff in file_diffs {
        let is_lockfile = should_limit_file(&file_diff.path, ignore_patterns);

        // If lockfile, strictly limit its tokens
        let effective_token_count = if is_lockfile {
            file_diff
                .token_count
                .min(TokenCount::new_saturating(LOCKFILE_TOKEN_LIMIT))
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
                if is_lockfile
                    && file_diff.token_count > TokenCount::new_saturating(LOCKFILE_TOKEN_LIMIT)
                {
                    let truncated = truncate_to_token_limit(
                        &file_diff.content,
                        TokenCount::new_saturating(LOCKFILE_TOKEN_LIMIT),
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
                        current_tokens.unwrap_or_else(|| TokenCount::new_saturating(1)),
                    ));
                    buffer.clear();
                }

                // Add new file to fresh buffer
                if is_lockfile
                    && file_diff.token_count > TokenCount::new_saturating(LOCKFILE_TOKEN_LIMIT)
                {
                    let mut truncated = truncate_to_token_limit(
                        &file_diff.content,
                        TokenCount::new_saturating(LOCKFILE_TOKEN_LIMIT),
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
                    current_tokens.unwrap_or_else(|| TokenCount::new_saturating(1)),
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
            current_tokens.unwrap_or_else(|| TokenCount::new_saturating(1)),
        ));
    }

    // Return buffer to pool
    buffer_pool::release_buffer(buffer);

    chunks
}

/// Check if a file should be limited based on ignore patterns.
fn should_limit_file(path: &FilePath, ignore_patterns: &[String]) -> bool {
    ignore_patterns
        .iter()
        .any(|pattern| path.as_str().ends_with(pattern))
}

/// Truncate content to a maximum number of tokens, line by line.
///
/// Uses token-level truncation for exact precision. Encodes the entire content once,
/// takes the first `limit` tokens, and decodes back to a string.
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
            // Try to end at a line boundary for cleaner output
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
/// While less precise than token-level truncation due to BPE boundary effects,
/// it guarantees we won't exceed the limit.
fn truncate_to_token_limit_fallback(
    content: &str,
    limit: TokenCount,
    tokenizer: &dyn Tokenizer,
) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut truncated = String::new();
    let mut current_tokens: Option<TokenCount> = None;

    for line in lines {
        let line_with_newline = format!("{}\n", line);
        let line_tokens = tokenizer.count_tokens(&line_with_newline);

        // Check if adding this line would exceed the limit
        if current_tokens
            .map(|current| current.get() + line_tokens.get())
            .unwrap_or(line_tokens.get())
            > limit.get()
        {
            break;
        }

        truncated.push_str(&line_with_newline);
        current_tokens = Some(match current_tokens {
            Some(current) => TokenCount::new_saturating(current.get() + line_tokens.get()),
            None => line_tokens,
        });
    }

    truncated
}

/// Split a single file's diff by hunks (`@@`).
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
    let mut buffer = buffer_pool::acquire_buffer();
    buffer.content_mut().push_str(header);
    let mut current_tokens = header_tokens_count;

    // Find all hunk positions
    let hunk_positions: Vec<usize> = content
        .match_indices(HUNK_HEADER)
        .map(|(idx, _)| idx + 1)
        .collect();

    if hunk_positions.is_empty() {
        // No hunks found, treat as single chunk
        buffer_pool::release_buffer(buffer);
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
                    TokenCount::new_saturating(current_tokens),
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
                    TokenCount::new_saturating(current_tokens),
                ));
                buffer.clear();
            }
            // Start new chunk with header + hunk
            buffer.content_mut().push_str(header);
            buffer.content_mut().push('\n');
            buffer.content_mut().push_str(hunk);
            current_tokens = header_tokens_count + hunk_tokens_count + 1;
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
            TokenCount::new_saturating(current_tokens),
        ));
    }

    // Return buffer to pool
    buffer_pool::release_buffer(buffer);

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
    let mut buffer = buffer_pool::acquire_buffer();
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
                    TokenCount::new_saturating(current_tokens),
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
                TokenCount::new_saturating(current_tokens),
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
            TokenCount::new_saturating(current_tokens),
        ));
    }

    // Return buffer to pool
    buffer_pool::release_buffer(buffer);

    chunks
}

/// Split an oversized line into smaller chunks.
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
