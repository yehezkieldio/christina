use std::fmt;

use compact_str::CompactString;

use crate::types::FilePath;

pub const BINARY_EXTENSIONS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico", ".svg", ".pdf", ".zip", ".tar", ".gz", ".rar",
    ".7z", ".exe", ".dll", ".so", ".dylib", ".wasm", ".pyc", ".class", ".ttf", ".otf", ".woff",
    ".woff2", ".mp3", ".mp4", ".avi", ".mov", ".mkv", ".db", ".sqlite", ".bin",
];

/// Unified file status type representing the state of a file in git.
///
/// This is the canonical type used across all crates to represent file status,
/// eliminating the previous duplication between `FileStatus` and `GitFileStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Unknown,
}

impl FileStatus {
    /// Get the single-character representation used by git.
    pub fn as_char(&self) -> char {
        match self {
            FileStatus::Added => 'A',
            FileStatus::Modified => 'M',
            FileStatus::Deleted => 'D',
            FileStatus::Renamed => 'R',
            FileStatus::Copied => 'C',
            FileStatus::Untracked => '?',
            FileStatus::Unknown => '?',
        }
    }

    /// Create from a single character.
    pub fn from_char(c: char) -> Self {
        match c {
            'A' => FileStatus::Added,
            'M' => FileStatus::Modified,
            'D' => FileStatus::Deleted,
            'R' => FileStatus::Renamed,
            'C' => FileStatus::Copied,
            '?' => FileStatus::Untracked,
            _ => FileStatus::Unknown,
        }
    }

    /// Parse from a string (single char or full word).
    pub fn parse(s: &str) -> Self {
        if s.len() == 1 {
            #[expect(
                clippy::unwrap_used,
                reason = "len()==1 guarantees chars().next() is Some"
            )]
            Self::from_char(s.chars().next().unwrap())
        } else {
            match s.to_uppercase().as_str() {
                "ADDED" => FileStatus::Added,
                "MODIFIED" => FileStatus::Modified,
                "DELETED" => FileStatus::Deleted,
                "RENAMED" => FileStatus::Renamed,
                "COPIED" => FileStatus::Copied,
                "UNTRACKED" => FileStatus::Untracked,
                _ => FileStatus::Unknown,
            }
        }
    }

    /// Check if a file path might be binary based on extension.
    pub fn might_be_binary(&self, path: &str) -> bool {
        BINARY_EXTENSIONS
            .iter()
            .any(|ext| path.to_lowercase().ends_with(ext))
    }
}

impl fmt::Display for FileStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_char())
    }
}

/// Legacy alias for backward compatibility.
///
/// New code should use `FileStatus` directly.
pub type GitFileStatus = FileStatus;

#[derive(Debug, Clone)]
pub struct GitFile {
    pub path: FilePath,
    pub status: CompactString,
    pub status_enum: FileStatus,
    pub diff_content: String,
    pub is_binary: bool,
}

impl GitFile {
    pub fn new(path: String, status: String, diff_content: String) -> Self {
        let status_enum = FileStatus::parse(&status);
        let is_binary = status_enum.might_be_binary(&path)
            || diff_content.contains("Binary files")
            || diff_content.bytes().any(|b| b == 0);

        Self {
            path: FilePath::from(path.as_str()),
            status: CompactString::new(&status),
            status_enum,
            diff_content,
            is_binary,
        }
    }

    pub fn extension(&self) -> Option<&str> {
        std::path::Path::new(self.path.as_str())
            .extension()
            .and_then(|s| s.to_str())
    }
}

impl fmt::Display for GitFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.status, self.path)
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn file_status_parse_char() {
        assert_eq!(FileStatus::parse("A"), FileStatus::Added);
        assert_eq!(FileStatus::parse("M"), FileStatus::Modified);
        assert_eq!(FileStatus::parse("D"), FileStatus::Deleted);
        assert_eq!(FileStatus::parse("R"), FileStatus::Renamed);
        assert_eq!(FileStatus::parse("C"), FileStatus::Copied);
        assert_eq!(FileStatus::parse("?"), FileStatus::Untracked);
        assert_eq!(FileStatus::parse("X"), FileStatus::Unknown);
    }

    #[test]
    fn file_status_parse_words() {
        assert_eq!(FileStatus::parse("added"), FileStatus::Added);
        assert_eq!(FileStatus::parse("MODIFIED"), FileStatus::Modified);
        assert_eq!(FileStatus::parse("deleted"), FileStatus::Deleted);
        assert_eq!(FileStatus::parse("RENAMED"), FileStatus::Renamed);
        assert_eq!(FileStatus::parse("copied"), FileStatus::Copied);
        assert_eq!(FileStatus::parse("untracked"), FileStatus::Untracked);
        assert_eq!(FileStatus::parse("unknown"), FileStatus::Unknown);
    }

    #[test]
    fn file_status_binary_heuristic() {
        assert!(FileStatus::Added.might_be_binary("image.PNG"));
        assert!(FileStatus::Added.might_be_binary("archive.tar.gz"));
        assert!(!FileStatus::Added.might_be_binary("src/main.rs"));
    }

    #[test]
    fn git_file_binary_marker() {
        let diff = "Binary files a/image.png and b/image.png differ";
        let file = GitFile::new("image.png".to_string(), "M".to_string(), diff.to_string());
        assert!(file.is_binary);
    }

    #[test]
    fn git_file_extension() {
        let file = GitFile::new("src/lib.rs".to_string(), "A".to_string(), "".to_string());
        assert_eq!(file.extension(), Some("rs"));
    }

    // Legacy alias tests
    #[test]
    fn git_file_status_alias() {
        // Ensure the alias works
        let _: GitFileStatus = FileStatus::Added;
        assert_eq!(FileStatus::Added, GitFileStatus::Added);
    }
}
