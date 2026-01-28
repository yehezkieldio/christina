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

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_tokenizer(text: &str) -> TokenCount {
        TokenCount::new_saturating((text.len() / 4).max(1) as u32)
    }

    #[test]
    fn extract_file_paths_from_git_diff() {
        let diff = "\
diff --git a/src/main.rs b/src/main.rs
index 1234567..abcdefg 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
+use std::io;
 fn main() {
 }
diff --git a/Cargo.toml b/Cargo.toml
index 2345678..bcdefgh 100644
";

        let paths = extract_file_paths(diff);
        assert_eq!(
            paths,
            vec![FilePath::from("src/main.rs"), FilePath::from("Cargo.toml")]
        );
    }

    #[test]
    fn parse_git_diff_header_standard() {
        assert_eq!(
            parse_git_diff_header("diff --git a/src/main.rs b/src/main.rs"),
            Some(FilePath::from("src/main.rs"))
        );
        assert_eq!(
            parse_git_diff_header("diff --git a/Cargo.toml b/Cargo.toml"),
            Some(FilePath::from("Cargo.toml"))
        );
    }

    #[test]
    fn parse_git_diff_header_with_mnemonic_prefixes() {
        // git diff --no-index uses c/ and i/ prefixes (mnemonic)
        assert_eq!(
            parse_git_diff_header("diff --git c/file.txt i/file.txt"),
            Some(FilePath::from("file.txt"))
        );
        // git diff --no-index with w/ prefix
        assert_eq!(
            parse_git_diff_header("diff --git w/file.txt i/file.txt"),
            Some(FilePath::from("file.txt"))
        );
    }

    #[test]
    fn parse_git_diff_header_handles_whitespace() {
        // Multiple spaces between diff and --git
        assert_eq!(
            parse_git_diff_header("diff  --git a/file.txt b/file.txt"),
            Some(FilePath::from("file.txt"))
        );
        // Tab character
        assert_eq!(
            parse_git_diff_header("diff\t--git a/file.txt b/file.txt"),
            Some(FilePath::from("file.txt"))
        );
        // Leading/trailing whitespace
        assert_eq!(
            parse_git_diff_header("  diff --git a/file.txt b/file.txt  "),
            Some(FilePath::from("file.txt"))
        );
    }

    #[test]
    fn parse_git_diff_header_without_prefixes() {
        // No prefix (diff.noprefix = true)
        // When path has no slash, it's returned as-is
        assert_eq!(
            parse_git_diff_header("diff --git file.txt file.txt"),
            Some(FilePath::from("file.txt"))
        );
        // When path has slashes, first component is treated as prefix and stripped
        assert_eq!(
            parse_git_diff_header("diff --git path/to/file.txt path/to/file.txt"),
            Some(FilePath::from("to/file.txt"))
        );
    }

    #[test]
    fn split_diff_by_individual_files() {
        let diff = "\
diff --git a/file1.txt b/file1.txt
content1
diff --git a/file2.txt b/file2.txt
content2";

        let files = split_by_files(diff, mock_tokenizer);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, FilePath::from("file1.txt"));
        assert_eq!(files[1].path, FilePath::from("file2.txt"));
    }

    #[test]
    fn split_by_files_ignores_inline_headers() {
        let diff = "\
diff --git a/real.txt b/real.txt
Some content with diff --git a/fake.txt b/fake.txt embedded
More content";

        let files = split_by_files(diff, mock_tokenizer);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, FilePath::from("real.txt"));
    }

    #[test]
    fn parse_git_diff_header_extracts_rename_destination() {
        // Test rename handling - should return destination path (b/ side)
        assert_eq!(
            parse_git_diff_header("diff --git a/old/path.rs b/new/path.rs"),
            Some(FilePath::from("new/path.rs"))
        );

        // Same path on both sides (normal diff)
        assert_eq!(
            parse_git_diff_header("diff --git a/file.rs b/file.rs"),
            Some(FilePath::from("file.rs"))
        );
    }

    #[test]
    fn is_deletion_only_correctly_identifies_pure_deletions() {
        // Pure deletion
        let deletion_diff = "\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt
@@ -1,3 +0,0 @@
-line 1
-line 2
-line 3";
        assert!(is_deletion_only(deletion_diff));

        // Mixed (deletion and addition)
        let mixed_diff = "\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
-old line
+new line
 context line";
        assert!(!is_deletion_only(mixed_diff));

        // Addition only
        let addition_diff = "\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt
@@ -0,0 +1,2 @@
+new line 1
+new line 2";
        assert!(!is_deletion_only(addition_diff));
    }

    #[test]
    fn is_all_file_deletions_verifies_multiple_file_states() {
        // Single file deletion
        let single_deletion = "\
diff --git a/file.txt b/file.txt
deleted file mode 100644
index abcdef..0000000";
        assert!(is_all_file_deletions(single_deletion));

        // Multiple file deletions
        let multi_deletion = "\
diff --git a/file1.txt b/file1.txt
deleted file mode 100644
diff --git a/file2.txt b/file2.txt
deleted file mode 100644";
        assert!(is_all_file_deletions(multi_deletion));

        // Mixed (deletion and modification)
        let mixed = "\
diff --git a/deleted.txt b/deleted.txt
deleted file mode 100644
diff --git a/modified.txt b/modified.txt
--- a/modified.txt
+++ b/modified.txt";
        assert!(!is_all_file_deletions(mixed));

        // No deletions
        let no_deletions = "\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt";
        assert!(!is_all_file_deletions(no_deletions));
    }

    #[test]
    fn truncate_deletion_diff_respects_line_limit() {
        let deletion_diff = "\
diff --git a/file.txt b/file.txt
deleted file mode 100644
--- a/file.txt
+++ /dev/null
@@ -1,10 +0,0 @@
-line 1
-line 2
-line 3
-line 4
-line 5
-line 6
-line 7
-line 8
-line 9
-line 10";

        let truncated = truncate_deletion_diff(deletion_diff, 3);

        // Should keep metadata headers
        assert!(truncated.contains("diff --git"));
        assert!(truncated.contains("deleted file mode"));
        assert!(truncated.contains("---"));
        assert!(truncated.contains("+++"));
        assert!(truncated.contains("@@"));

        // Should keep first 3 deletion lines
        assert!(truncated.contains("-line 1"));
        assert!(truncated.contains("-line 2"));
        assert!(truncated.contains("-line 3"));

        // Should have truncation notice
        assert!(truncated.contains("[... deleted content truncated"));

        // Should not have all 10 lines
        assert!(!truncated.contains("-line 10"));
    }
}
