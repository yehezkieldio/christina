//! Git status and staging domain types.
//!
//! WHY keep heuristics here: binary detection and status parsing are part of the
//! git snapshot model, letting higher layers stay agnostic about raw diff text.

use std::fmt;

use compact_str::CompactString;

use crate::types::FilePath;

// Coarse extension list to quickly skip obvious binaries without parsing diffs.
pub const BINARY_EXTENSIONS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico", ".svg", ".pdf", ".zip", ".tar", ".gz", ".rar",
    ".7z", ".exe", ".dll", ".so", ".dylib", ".wasm", ".pyc", ".class", ".ttf", ".otf", ".woff",
    ".woff2", ".mp3", ".mp4", ".avi", ".mov", ".mkv", ".db", ".sqlite", ".bin",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Unknown,
}

impl GitFileStatus {
    pub fn as_char(&self) -> char {
        match self {
            GitFileStatus::Added => 'A',
            GitFileStatus::Modified => 'M',
            GitFileStatus::Deleted => 'D',
            GitFileStatus::Renamed => 'R',
            GitFileStatus::Copied => 'C',
            GitFileStatus::Untracked => '?',
            GitFileStatus::Unknown => '?',
        }
    }

    pub fn from_char(c: char) -> Self {
        match c {
            'A' => GitFileStatus::Added,
            'M' => GitFileStatus::Modified,
            'D' => GitFileStatus::Deleted,
            'R' => GitFileStatus::Renamed,
            'C' => GitFileStatus::Copied,
            '?' => GitFileStatus::Untracked,
            _ => GitFileStatus::Unknown,
        }
    }

    pub fn parse(s: &str) -> Self {
        let mut chars = s.chars();
        if let (Some(status), None) = (chars.next(), chars.next()) {
            Self::from_char(status)
        } else {
            if s.eq_ignore_ascii_case("ADDED") {
                Self::Added
            } else if s.eq_ignore_ascii_case("MODIFIED") {
                Self::Modified
            } else if s.eq_ignore_ascii_case("DELETED") {
                Self::Deleted
            } else if s.eq_ignore_ascii_case("RENAMED") {
                Self::Renamed
            } else if s.eq_ignore_ascii_case("COPIED") {
                Self::Copied
            } else if s.eq_ignore_ascii_case("UNTRACKED") {
                Self::Untracked
            } else {
                Self::Unknown
            }
        }
    }

    pub fn might_be_binary(&self, path: &str) -> bool {
        BINARY_EXTENSIONS
            .iter()
            .any(|ext| path.to_lowercase().ends_with(ext))
    }
}

impl fmt::Display for GitFileStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_char())
    }
}

#[derive(Debug, Clone)]
pub struct GitFile {
    pub path: FilePath,
    pub status: CompactString,
    pub status_enum: GitFileStatus,
    pub diff_content: String,
    pub is_binary: bool,
}

impl GitFile {
    pub fn new(
        path: impl Into<CompactString>,
        status: impl Into<CompactString>,
        diff_content: impl Into<String>,
    ) -> Self {
        let path = path.into();
        let status = status.into();
        let diff_content = diff_content.into();
        let status_enum = GitFileStatus::parse(&status);
        // Binary detection is intentionally conservative: any signal marks the file as binary.
        let is_binary = status_enum.might_be_binary(path.as_str())
            || diff_content.contains("Binary files")
            || diff_content.bytes().any(|b| b == 0);

        Self {
            path: FilePath::from(path.as_str()),
            status,
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

// --- RepoSnapshot (merged from snapshot.rs) ---

use super::repository::RepoRoot;

#[derive(Debug, Clone)]
pub struct RepoSnapshot {
    pub files: Vec<GitFile>,
    pub staged: Vec<FilePath>,
    pub unstaged: Vec<FilePath>,
    pub branch: String,
    pub repo_root: RepoRoot,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // --- GitFile tests ---

    #[test]
    fn git_file_status_parse_char() {
        assert_eq!(GitFileStatus::parse("A"), GitFileStatus::Added);
        assert_eq!(GitFileStatus::parse("M"), GitFileStatus::Modified);
        assert_eq!(GitFileStatus::parse("D"), GitFileStatus::Deleted);
        assert_eq!(GitFileStatus::parse("R"), GitFileStatus::Renamed);
        assert_eq!(GitFileStatus::parse("C"), GitFileStatus::Copied);
        assert_eq!(GitFileStatus::parse("?"), GitFileStatus::Untracked);
        assert_eq!(GitFileStatus::parse("X"), GitFileStatus::Unknown);
    }

    #[test]
    fn git_file_status_parse_words() {
        assert_eq!(GitFileStatus::parse("added"), GitFileStatus::Added);
        assert_eq!(GitFileStatus::parse("MODIFIED"), GitFileStatus::Modified);
        assert_eq!(GitFileStatus::parse("deleted"), GitFileStatus::Deleted);
        assert_eq!(GitFileStatus::parse("RENAMED"), GitFileStatus::Renamed);
        assert_eq!(GitFileStatus::parse("copied"), GitFileStatus::Copied);
        assert_eq!(GitFileStatus::parse("untracked"), GitFileStatus::Untracked);
        assert_eq!(GitFileStatus::parse("unknown"), GitFileStatus::Unknown);
    }

    #[test]
    fn git_file_status_binary_heuristic() {
        assert!(GitFileStatus::Added.might_be_binary("image.PNG"));
        assert!(GitFileStatus::Added.might_be_binary("archive.tar.gz"));
        assert!(!GitFileStatus::Added.might_be_binary("src/main.rs"));
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

    // --- RepoSnapshot tests ---

    #[test]
    fn test_repo_snapshot_clone() {
        let snapshot = RepoSnapshot {
            files: vec![],
            staged: vec![FilePath::from("file1.rs")],
            unstaged: vec![FilePath::from("file2.rs")],
            branch: "main".to_string(),
            repo_root: RepoRoot::new(PathBuf::from("/home/user/repo")),
        };

        let cloned = snapshot.clone();

        assert_eq!(cloned.staged, snapshot.staged);
        assert_eq!(cloned.unstaged, snapshot.unstaged);
        assert_eq!(cloned.branch, snapshot.branch);
        assert_eq!(cloned.repo_root, snapshot.repo_root);
        assert_eq!(cloned.files.len(), snapshot.files.len());
    }

    #[test]
    fn test_repo_snapshot_debug() {
        let snapshot = RepoSnapshot {
            files: vec![],
            staged: vec![FilePath::from("file.rs")],
            unstaged: vec![],
            branch: "develop".to_string(),
            repo_root: RepoRoot::new(PathBuf::from("/repo")),
        };

        let debug_str = format!("{:?}", snapshot);

        assert!(debug_str.contains("RepoSnapshot"));
        assert!(debug_str.contains("develop"));
        assert!(debug_str.contains("file.rs"));
    }
}
