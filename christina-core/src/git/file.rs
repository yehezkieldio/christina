use std::fmt;

use compact_str::CompactString;

use crate::types::FilePath;

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
        if s.len() == 1 {
            #[expect(
                clippy::unwrap_used,
                reason = "len()==1 guarantees chars().next() is Some"
            )]
            Self::from_char(s.chars().next().unwrap())
        } else {
            match s.to_uppercase().as_str() {
                "ADDED" => GitFileStatus::Added,
                "MODIFIED" => GitFileStatus::Modified,
                "DELETED" => GitFileStatus::Deleted,
                "RENAMED" => GitFileStatus::Renamed,
                "COPIED" => GitFileStatus::Copied,
                "UNTRACKED" => GitFileStatus::Untracked,
                _ => GitFileStatus::Unknown,
            }
        }
    }

    pub fn might_be_binary(&self, path: &str) -> bool {
        let binary_extensions = [
            ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico", ".svg", ".pdf", ".zip", ".tar", ".gz",
            ".rar", ".7z", ".exe", ".dll", ".so", ".dylib", ".wasm", ".pyc", ".class", ".ttf",
            ".otf", ".woff", ".woff2", ".mp3", ".mp4", ".avi", ".mov", ".mkv", ".db", ".sqlite",
            ".bin",
        ];
        binary_extensions
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
    pub fn new(path: String, status: String, diff_content: String) -> Self {
        let status_enum = GitFileStatus::parse(&status);
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
