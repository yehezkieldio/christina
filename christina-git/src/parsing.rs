use christina_core::{
    git::FileDiff,
    types::{FilePath, TokenCount},
};

fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    match s.get(..max_bytes) {
        Some(slice) => slice,
        None => {
            let mut end = max_bytes;
            while end > 0 && !s.is_char_boundary(end) {
                end -= 1;
            }
            s.get(..end).unwrap_or("")
        }
    }
}

/// Extract file paths from a diff string.
///
/// Scans for "diff --git" headers at line starts and extracts the file path.
pub fn extract_file_paths(diff: &str) -> Vec<FilePath> {
    let mut paths = Vec::new();

    for line in diff.lines() {
        if line.starts_with("diff --git ")
            && let Some(path) = parse_git_diff_header(line)
        {
            paths.push(path);
        }
    }

    paths
}

/// Extract a single file path from diff content.
///
/// Looks for the first "diff --git" header and extracts the path.
pub fn extract_file_path(content: &str) -> Option<FilePath> {
    for line in content.lines() {
        if line.starts_with("diff --git ") {
            return parse_git_diff_header(line);
        }
    }
    None
}

/// Parse a git diff header line to extract the file path(s).
///
/// Handles various prefix formats and git configurations:
/// - Standard: `a/path/to/file` `b/path/to/file`
/// - Custom/mnemonic: `c/path` `i/path` `w/path` (git diff --no-index)
/// - No prefix: `path/to/file` (diff.noprefix = true)
/// - Flexible whitespace handling
/// - **Renames/Moves**: When `path_a != path_b`, returns `path_b` (destination)
///
/// Returns the destination path (b/ side) after stripping the prefix.
/// For renames, this is the *new* path. For normal diffs, both sides match.
pub fn parse_git_diff_header(line: &str) -> Option<FilePath> {
    fn parse_path(s: &str) -> Option<(FilePath, &str)> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let (path, rest) = if let Some(s_without_quote) = s.strip_prefix('"') {
            // Handle quoted paths with spaces or special characters
            let end_quote = s_without_quote.find('"')?;
            let path = &s_without_quote[..end_quote];
            let rest = &s[end_quote + 2..]; // Skip closing quote
            (path, rest)
        } else {
            // Unquoted path - take until whitespace
            let path = s.split_whitespace().next()?;
            let rest = &s[path.len()..];
            (path, rest)
        };

        // Strip prefix (a/, b/, c/, w/, etc.) by removing first path component
        // If no slash exists, assume no prefix (diff.noprefix = true)
        let stripped = if let Some(slash_pos) = path.find('/') {
            &path[slash_pos + 1..]
        } else {
            path
        };

        Some((FilePath::from(stripped), rest))
    }

    // Format: "diff --git <path1> <path2>"
    // Be flexible with whitespace between "diff", "--git", and paths
    let trimmed = line.trim();
    let after_git = trimmed
        .strip_prefix("diff")
        .and_then(|s| s.trim_start().strip_prefix("--git"))
        .and_then(|s| {
            let s = s.trim_start();
            if s.is_empty() { None } else { Some(s) }
        })?;

    let (_path_a, remaining) = parse_path(after_git)?;
    let (path_b, _) = parse_path(remaining)?;

    // For renames/moves, path_a != path_b, return the destination (b/ side)
    // For normal diffs, both sides match - doesn't matter which we return
    Some(path_b)
}

/// Split a diff string by file headers (`diff --git`).
///
/// Returns zero-copy slices for each file's diff with metadata.
///
/// Only treats lines that START with "diff --git " as headers,
/// preventing content injection attacks where diff content could contain
/// the header string mid-line.
pub fn split_by_files(diff: &str, tokenizer: impl Fn(&str) -> TokenCount) -> Vec<FileDiff> {
    const FILE_HEADER: &str = "diff --git ";

    // Collect all file header positions - only match at LINE START
    let mut positions: Vec<usize> = Vec::new();
    let mut current_pos = 0;

    for line in diff.lines() {
        if line.starts_with(FILE_HEADER) {
            positions.push(current_pos);
        }
        // +1 for the newline character (or end of string)
        current_pos += line.len() + 1;
    }

    if positions.is_empty() {
        return Vec::new();
    }

    let mut files = Vec::new();

    for (i, &start) in positions.iter().enumerate() {
        // End is either the next file's start or the end of the diff
        let end = positions.get(i + 1).copied().unwrap_or(diff.len());
        let end = end.min(diff.len());
        let raw_content = &diff[start..end];

        if let Some(path) = extract_file_path(raw_content) {
            // Check for oversized file diffs
            const MAX_FILE_DIFF_SIZE: usize = 1024 * 1024; // 1MB
            let (content_str, truncated) = if raw_content.len() > MAX_FILE_DIFF_SIZE {
                (safe_truncate(raw_content, MAX_FILE_DIFF_SIZE), true)
            } else {
                (raw_content, false)
            };

            let token_count = tokenizer(content_str);
            files.push(FileDiff {
                path,
                content: content_str.to_string(),
                token_count,
                truncated,
            });
        }
    }

    files
}

/// Check if a diff content is deletion-only (no additions, only deletions).
///
/// Returns `true` if the diff contains only deleted lines (lines starting with '-')
/// and no added lines (lines starting with '+'), excluding metadata lines.
///
/// deletion-only diffs can be heavily truncated since
/// the LLM doesn't need to see all deleted content to generate
/// \"delete file\" or \"remove code\" commit messages.
pub fn is_deletion_only(content: &str) -> bool {
    let mut has_deletion = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('+') && !line.starts_with("+++") {
            // Found an addition line (not metadata), not deletion-only
            return false;
        }
        if line.starts_with('-') && !line.starts_with("---") {
            // Found a deletion line (not metadata)
            has_deletion = true;
        }
    }

    has_deletion
}

/// Check if all file diffs are deletions (entire files deleted).
///
/// Returns `true` if every file in the diff is a complete file deletion
/// (contains "deleted file mode" header).
pub fn is_all_file_deletions(content: &str) -> bool {
    let file_count = content
        .lines()
        .filter(|line| line.starts_with("diff --git "))
        .count();
    if file_count == 0 {
        return false;
    }

    let deletion_count = content
        .lines()
        .filter(|line| line.starts_with("deleted file mode"))
        .count();

    // All files are deletions if deletion count matches file count
    file_count == deletion_count && file_count > 0
}

/// Truncate a deletion-only diff to save tokens.
///
/// For deletion-only diffs, the LLM doesn't need to see all the deleted content.
/// This function keeps:
/// - All metadata headers (diff --git, index, ---, +++, @@)
/// - First few deletion lines from each hunk (for context)
/// - A truncation notice
///
/// This can save massive amounts of tokens when deleting large files.
pub fn truncate_deletion_diff(content: &str, max_deletion_lines: usize) -> String {
    let mut result = String::new();
    let mut deletion_lines_shown = 0;
    let mut in_deletion_block = false;
    let mut total_deletions_skipped = 0;

    for line in content.lines() {
        // Always keep metadata and hunk headers
        if line.starts_with("diff --git ")
            || line.starts_with("index ")
            || line.starts_with("---")
            || line.starts_with("+++")
            || line.starts_with("@@")
            || line.starts_with("deleted file mode")
            || line.starts_with("new file mode")
            || line.starts_with("similarity index")
            || line.starts_with("rename from")
            || line.starts_with("rename to")
            || line.starts_with("copy from")
            || line.starts_with("copy to")
        {
            result.push_str(line);
            result.push('\n');

            // Reset deletion counter at hunk boundaries
            if line.starts_with("@@") {
                deletion_lines_shown = 0;
                in_deletion_block = false;
            }
            continue;
        }

        // Handle deletion lines
        if line.starts_with('-') && !line.starts_with("---") {
            if deletion_lines_shown < max_deletion_lines {
                result.push_str(line);
                result.push('\n');
                deletion_lines_shown += 1;
                in_deletion_block = true;
            } else if in_deletion_block {
                // First line we're skipping - add truncation notice
                result.push_str("[... deleted content truncated to save tokens ...]\n");
                in_deletion_block = false;
            }
            total_deletions_skipped += 1;
        } else {
            // Keep other lines (context lines, additions)
            result.push_str(line);
            result.push('\n');
        }
    }

    // Add summary at the end
    if total_deletions_skipped > max_deletion_lines {
        result.push_str(&format!(
            "\n[Truncated {} deletion lines - full deletion diff not needed for commit message generation]\n",
            total_deletions_skipped - max_deletion_lines
        ));
    }

    result
}
