use std::sync::Arc;

use christina_core::{Tokenizer, types::{FileDiff, FilePath}};

const FILE_HEADER: &str = "diff --git ";
const MAX_FILE_DIFF_SIZE: usize = 1024 * 1024; // 1MB

pub fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }

    &s[..end]
}

/// Extract file paths from a diff string.
///
/// Scans for diff headers at line starts and extracts the file path.
pub fn extract_file_paths(diff: &str) -> Vec<FilePath> {
    diff.lines().filter_map(parse_git_diff_header).collect()
}

/// Parse a git diff header line to extract the file path(s).
///
/// Handles various prefix formats and git configurations:
/// - Standard: `a/path/to/file` `b/path/to/file`
/// - Custom/mnemonic: `c/path` `i/path` `w/path` (git diff --no-index)
/// - No prefix: `path/to/file` (diff.noprefix = true)
/// - Quoted paths with spaces
/// - Flexible whitespace handling
/// - **Renames/Moves**: When `path_a != path_b`, returns `path_b` (destination)
///
/// Returns the destination path (b/ side) after stripping the prefix.
/// For renames, this is the *new* path. For normal diffs, both sides match.
///
/// **Safety**: Git diff output by specification produces only relative paths within
/// the repository. Absolute paths would violate the diff format and are rejected.
pub fn parse_git_diff_header(line: &str) -> Option<FilePath> {
    fn parse_quoted_path(s: &str) -> Option<(String, &str)> {
        let bytes = s.as_bytes();
        if bytes.first()? != &b'"' {
            return None;
        }

        let mut out = Vec::new();
        let mut i = 1usize;
        let mut closed = false;

        while i < bytes.len() {
            match bytes[i] {
                b'"' => {
                    closed = true;
                    i += 1;
                    break;
                }
                b'\\' => {
                    i += 1;
                    if i >= bytes.len() {
                        return None;
                    }
                    let esc = bytes[i];
                    match esc {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0c),
                        b'\\' => out.push(b'\\'),
                        b'"' => out.push(b'"'),
                        b'0'..=b'7' => {
                            let mut value = (esc - b'0') as u32;
                            let mut digits = 1;
                            while digits < 3 && i + 1 < bytes.len() {
                                let next = bytes[i + 1];
                                if !(b'0'..=b'7').contains(&next) {
                                    break;
                                }
                                value = value * 8 + (next - b'0') as u32;
                                i += 1;
                                digits += 1;
                            }
                            out.push(value.min(u32::from(u8::MAX)) as u8);
                        }
                        _ => out.push(esc),
                    }
                }
                other => out.push(other),
            }
            i += 1;
        }

        if !closed {
            return None;
        }

        let path = String::from_utf8(out).ok()?;
        Some((path, &s[i..]))
    }

    fn parse_path(s: &str) -> Option<(String, &str)> {
        let s = s.trim_start();
        if s.is_empty() {
            return None;
        }

        if s.starts_with('"') {
            parse_quoted_path(s)
        } else {
            let path = s.split_whitespace().next()?;
            let rest = &s[path.len()..];
            Some((path.to_string(), rest))
        }
    }

    fn should_strip_prefix(prefix_a: &str, prefix_b: &str) -> bool {
        if prefix_a.len() != 1 || prefix_b.len() != 1 {
            return false;
        }

        let is_known = |prefix: &str| matches!(prefix, "a" | "b" | "c" | "i" | "w");

        if !is_known(prefix_a) || !is_known(prefix_b) {
            return false;
        }

        prefix_a != prefix_b
    }

    fn normalize_paths(path_a: String, path_b: String) -> (String, String) {
        let (Some((prefix_a, rest_a)), Some((prefix_b, rest_b))) =
            (path_a.split_once('/'), path_b.split_once('/'))
        else {
            return (path_a, path_b);
        };

        if should_strip_prefix(prefix_a, prefix_b) {
            (rest_a.to_string(), rest_b.to_string())
        } else {
            (path_a, path_b)
        }
    }

    fn normalize_single_path(path: String) -> String {
        if let Some((prefix, rest)) = path.split_once('/')
            && matches!(prefix, "a" | "b" | "c" | "i" | "w")
            && !rest.is_empty()
        {
            return rest.to_string();
        }
        path
    }

    let trimmed = line.trim();
    let after_diff = trimmed.strip_prefix("diff")?.trim_start();

    if let Some(after_git) = after_diff.strip_prefix("--git") {
        let after_git = after_git.trim_start();
        if after_git.is_empty() {
            return None;
        }

        let (path_a_raw, remaining) = parse_path(after_git)?;
        let (path_b_raw, _) = parse_path(remaining)?;

        let (_path_a, path_b) = normalize_paths(path_a_raw, path_b_raw);

        // Use try_new at parsing boundary to gracefully reject malformed paths.
        // Git diff should never produce absolute paths, but we validate defensively.
        return FilePath::try_new(path_b).ok();
    }

    let after_combined = after_diff
        .strip_prefix("--cc")
        .or_else(|| after_diff.strip_prefix("--combined"))?;
    let after_combined = after_combined.trim_start();
    if after_combined.is_empty() {
        return None;
    }

    let (path_raw, _) = parse_path(after_combined)?;
    let path = normalize_single_path(path_raw);
    FilePath::try_new(path).ok()
}

/// Split a diff string by file headers (`diff --git`).
///
/// Returns per-file diff content with metadata.
///
/// Only treats lines that START with "diff --git " as headers,
/// preventing content injection attacks where diff content could contain
/// the header string mid-line.
pub fn split_by_files(diff: &str, tokenizer: &dyn Tokenizer) -> Vec<FileDiff> {
    let mut positions = Vec::new();

    if diff.starts_with(FILE_HEADER) {
        positions.push(0);
    }

    for (idx, byte) in diff.bytes().enumerate() {
        if byte == b'\n' {
            let start = idx + 1;
            if start < diff.len() && diff[start..].starts_with(FILE_HEADER) {
                positions.push(start);
            }
        }
    }

    if positions.is_empty() {
        return Vec::new();
    }

    let mut files = Vec::new();

    for (i, &start) in positions.iter().enumerate() {
        let end = positions.get(i + 1).copied().unwrap_or(diff.len());
        let raw_content = &diff[start..end];
        let header_line = raw_content.lines().next().unwrap_or("");

        let Some(path) = parse_git_diff_header(header_line) else {
            continue;
        };

        let (content, truncated) = if raw_content.len() > MAX_FILE_DIFF_SIZE {
            (
                Arc::from(safe_truncate(raw_content, MAX_FILE_DIFF_SIZE)),
                true,
            )
        } else {
            (Arc::from(raw_content), false)
        };

        let token_count = tokenizer.count_tokens(&content);

        files.push(FileDiff {
            path,
            content,
            token_count,
            truncated,
        });
    }

    files
}

/// Check if a diff content is deletion-only (no additions, only deletions).
///
/// Returns `true` if the diff contains only deleted lines (lines starting with '-')
/// and no added lines (lines starting with '+'), excluding metadata lines.
pub fn is_deletion_only(content: &str) -> bool {
    let mut has_deletion = false;

    for line in content.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            return false;
        }
        if line.starts_with('-') && !line.starts_with("---") {
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
        .filter(|line| line.starts_with(FILE_HEADER))
        .count();

    if file_count == 0 {
        return false;
    }

    let deletion_count = content
        .lines()
        .filter(|line| line.starts_with("deleted file mode"))
        .count();

    file_count == deletion_count
}

/// Truncate a deletion-only diff to save tokens.
///
/// For deletion-only diffs, the LLM doesn't need to see all the deleted content.
/// This function keeps:
/// - All metadata headers (diff --git, index, ---, +++, @@)
/// - First few deletion lines from each hunk (for context)
/// - A truncation notice
pub fn truncate_deletion_diff(content: &str, max_deletion_lines: usize) -> String {
    let mut result = String::new();
    let mut deletion_lines_shown = 0usize;
    let mut truncation_notice_emitted = false;
    let mut total_deletions_skipped = 0usize;

    for line in content.lines() {
        let is_metadata = line.starts_with(FILE_HEADER)
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
            || line.starts_with("copy to");

        if is_metadata {
            result.push_str(line);
            result.push('\n');

            if line.starts_with("@@") {
                deletion_lines_shown = 0;
                truncation_notice_emitted = false;
            }
            continue;
        }

        if line.starts_with('-') && !line.starts_with("---") {
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
            "\n[Truncated {} deletion lines - full deletion diff not needed for commit message generation]\n",
            total_deletions_skipped
        ));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTokenizer;

    impl Tokenizer for MockTokenizer {
        fn count_tokens_exact(&self, text: &str) -> u32 {
            text.len() as u32
        }

        fn count_tokens(&self, text: &str) -> TokenCount {
            TokenCount::new_at_least_one(self.count_tokens_exact(text))
        }

        fn encoding_name(&self) -> &str {
            "mock-bytes"
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
    fn safe_truncate_ascii() {
        let s = "hello world";
        assert_eq!(safe_truncate(s, 5), "hello");
    }

    #[test]
    fn safe_truncate_utf8_multibyte() {
        let s = "café";
        assert_eq!(safe_truncate(s, 4), "caf");
        assert_eq!(safe_truncate(s, 5), "café");
    }

    #[test]
    fn safe_truncate_emoji_boundary() {
        let s = "hi 👋";
        assert_eq!(safe_truncate(s, 3), "hi ");
        assert_eq!(safe_truncate(s, 6), "hi ");
        assert_eq!(safe_truncate(s, s.len()), s);
    }

    #[test]
    fn safe_truncate_zero_bytes() {
        assert_eq!(safe_truncate("test", 0), "");
    }

    #[test]
    fn extract_file_paths_from_git_diff() {
        let diff = "\
diff --git a/src/main.rs b/src/main.rs
index 1234567..abcdefg 100644
diff --git \"a/path with space.txt\" \"b/path with space.txt\"
diff --cc src/combined.rs
diff --git c/noindex.txt i/noindex.txt
 context diff --git a/fake.txt b/fake.txt
diff --git file.txt file.txt
";

        let paths = extract_file_paths(diff);
        assert_eq!(
            paths,
            vec![
                FilePath::from("src/main.rs"),
                FilePath::from("path with space.txt"),
                FilePath::from("src/combined.rs"),
                FilePath::from("noindex.txt"),
                FilePath::from("file.txt"),
            ]
        );
    }

    #[test]
    fn parse_git_diff_header_standard() {
        assert_eq!(
            parse_git_diff_header("diff --git a/src/main.rs b/src/main.rs"),
            Some(FilePath::from("src/main.rs"))
        );
    }

    #[test]
    fn parse_git_diff_header_with_mnemonic_prefixes() {
        assert_eq!(
            parse_git_diff_header("diff --git c/file.txt i/file.txt"),
            Some(FilePath::from("file.txt"))
        );
        assert_eq!(
            parse_git_diff_header("diff --git w/file.txt i/file.txt"),
            Some(FilePath::from("file.txt"))
        );
    }

    #[test]
    fn parse_git_diff_header_handles_whitespace() {
        assert_eq!(
            parse_git_diff_header("diff  --git a/file.txt b/file.txt"),
            Some(FilePath::from("file.txt"))
        );
        assert_eq!(
            parse_git_diff_header("diff\t--git\ta/file.txt\tb/file.txt"),
            Some(FilePath::from("file.txt"))
        );
        assert_eq!(
            parse_git_diff_header("  diff --git a/file.txt b/file.txt  "),
            Some(FilePath::from("file.txt"))
        );
    }

    #[test]
    fn parse_git_diff_header_without_prefixes() {
        assert_eq!(
            parse_git_diff_header("diff --git file.txt file.txt"),
            Some(FilePath::from("file.txt"))
        );
        assert_eq!(
            parse_git_diff_header("diff --git path/to/file.txt path/to/file.txt"),
            Some(FilePath::from("path/to/file.txt"))
        );
    }

    #[test]
    fn parse_git_diff_header_quoted_paths() {
        assert_eq!(
            parse_git_diff_header("diff --git \"a/path with space.txt\" \"b/path with space.txt\""),
            Some(FilePath::from("path with space.txt"))
        );
    }

    #[test]
    fn parse_git_diff_header_escaped_quotes() {
        assert_eq!(
            parse_git_diff_header("diff --git \"a/path\\\"with\\\"quote.txt\" \"b/path\\\"with\\\"quote.txt\""),
            Some(FilePath::from("path\"with\"quote.txt"))
        );
    }

    #[test]
    fn parse_git_diff_header_combined() {
        assert_eq!(
            parse_git_diff_header("diff --cc src/combined.rs"),
            Some(FilePath::from("src/combined.rs"))
        );
        assert_eq!(
            parse_git_diff_header("diff --combined \"path with space.rs\""),
            Some(FilePath::from("path with space.rs"))
        );
    }

    #[test]
    fn parse_git_diff_header_extracts_rename_destination() {
        assert_eq!(
            parse_git_diff_header("diff --git a/old/path.rs b/new/path.rs"),
            Some(FilePath::from("new/path.rs"))
        );
    }

    #[test]
    fn parse_git_diff_header_invalid_inputs() {
        assert_eq!(parse_git_diff_header("diff --git a/onlyone"), None);
        assert_eq!(parse_git_diff_header("not a diff header"), None);
    }

    #[test]
    fn split_diff_by_individual_files() {
        let diff = "\
diff --git a/file1.txt b/file1.txt
index 1234567..abcdefg 100644
--- a/file1.txt
+++ b/file1.txt
@@ -1 +1 @@
-old
+new
diff --git a/file2.txt b/file2.txt
index 2345678..bcdefgh 100644
--- a/file2.txt
+++ b/file2.txt
@@ -1 +1 @@
-old2
+new2
";

        let tokenizer = MockTokenizer;
        let files = split_by_files(diff, &tokenizer);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, FilePath::from("file1.txt"));
        assert_eq!(files[1].path, FilePath::from("file2.txt"));
        assert!(!files[0].truncated);
        assert_eq!(
            files[0].token_count,
            TokenCount::new_at_least_one(files[0].content.len() as u32)
        );
    }

    #[test]
    fn split_by_files_ignores_inline_headers() {
        let diff = "\
diff --git a/real.txt b/real.txt
+Some content with diff --git a/fake.txt b/fake.txt embedded
More content
";

        let tokenizer = MockTokenizer;
        let files = split_by_files(diff, &tokenizer);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, FilePath::from("real.txt"));
    }

    #[test]
    fn split_by_files_truncates_large_file() {
        let header = "diff --git a/huge.txt b/huge.txt\n";
        let body = "a".repeat(MAX_FILE_DIFF_SIZE + 64);
        let diff = format!("{header}{body}");

        let tokenizer = MockTokenizer;
        let files = split_by_files(&diff, &tokenizer);
        assert_eq!(files.len(), 1);
        assert!(files[0].truncated);
        assert_eq!(files[0].content.len(), MAX_FILE_DIFF_SIZE);
        assert_eq!(
            files[0].token_count,
            TokenCount::new_at_least_one(MAX_FILE_DIFF_SIZE as u32)
        );
    }

    #[test]
    fn is_deletion_only_correctly_identifies_pure_deletions() {
        let deletion_diff = "\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt
@@ -1,3 +0,0 @@
-line 1
-line 2
-line 3";
        assert!(is_deletion_only(deletion_diff));

        let mixed_diff = "\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
-old line
+new line
 context line";
        assert!(!is_deletion_only(mixed_diff));

        let addition_diff = "\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt
@@ -0,0 +1,2 @@
+new line 1
+new line 2";
        assert!(!is_deletion_only(addition_diff));

        let empty_diff = "diff --git a/file.txt b/file.txt";
        assert!(!is_deletion_only(empty_diff));
    }

    #[test]
    fn is_all_file_deletions_verifies_multiple_file_states() {
        let single_deletion = "\
diff --git a/file.txt b/file.txt
deleted file mode 100644
index abcdef..0000000";
        assert!(is_all_file_deletions(single_deletion));

        let multi_deletion = "\
diff --git a/file1.txt b/file1.txt
deleted file mode 100644
diff --git a/file2.txt b/file2.txt
deleted file mode 100644";
        assert!(is_all_file_deletions(multi_deletion));

        let mixed = "\
diff --git a/deleted.txt b/deleted.txt
deleted file mode 100644
diff --git a/modified.txt b/modified.txt
--- a/modified.txt
+++ b/modified.txt";
        assert!(!is_all_file_deletions(mixed));

        let no_deletions = "\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt";
        assert!(!is_all_file_deletions(no_deletions));
    }

    #[test]
    fn truncate_deletion_diff_respects_line_limit_per_hunk() {
        let deletion_diff = "\
diff --git a/file.txt b/file.txt
deleted file mode 100644
index abcdef..0000000
--- a/file.txt
+++ /dev/null
@@ -1,4 +0,0 @@
-line 1
-line 2
-line 3
-line 4
@@ -10,3 +0,0 @@
-line 5
-line 6
-line 7";

        let truncated = truncate_deletion_diff(deletion_diff, 2);

        assert!(truncated.contains("diff --git"));
        assert!(truncated.contains("deleted file mode"));
        assert!(truncated.contains("---"));
        assert!(truncated.contains("+++"));
        assert!(truncated.contains("@@ -1,4 +0,0 @@"));

        assert!(truncated.contains("-line 1"));
        assert!(truncated.contains("-line 2"));
        assert!(!truncated.contains("-line 3"));
        assert!(!truncated.contains("-line 4"));

        assert!(truncated.contains("-line 5"));
        assert!(truncated.contains("-line 6"));
        assert!(!truncated.contains("-line 7"));

        assert!(truncated.contains("[... deleted content truncated"));
        assert!(truncated.contains("Truncated 3 deletion lines"));
    }

    #[test]
    fn truncate_deletion_diff_without_truncation_keeps_content() {
        let deletion_diff = "\
diff --git a/file.txt b/file.txt
deleted file mode 100644
index abcdef..0000000
--- a/file.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-line 1
-line 2";

        let truncated = truncate_deletion_diff(deletion_diff, 5);
        assert!(!truncated.contains("deleted content truncated"));
        assert!(!truncated.contains("Truncated "));
        assert!(truncated.contains("-line 1"));
        assert!(truncated.contains("-line 2"));
    }

    #[test]
    fn parse_git_diff_header_rejects_absolute_paths() {
        // Git diff should never produce absolute paths, but we validate defensively
        // at parsing boundaries. Absolute paths violate the diff format spec.
        assert_eq!(
            parse_git_diff_header("diff --git /absolute/path.txt /absolute/path.txt"),
            None
        );
        assert_eq!(
            parse_git_diff_header("diff --git a//root/file.txt b//root/file.txt"),
            None
        );
    }

    #[test]
    fn filepath_try_new_absolute_returns_error() {
        // Verify that FilePath::try_new properly rejects absolute paths
        assert!(FilePath::try_new("/absolute/path.rs").is_err());
        assert!(FilePath::try_new("relative/path.rs").is_ok());
    }
}
