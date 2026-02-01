use std::path::Path;

use compact_str::CompactString;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FilePath(CompactString);

impl FilePath {
    pub fn new(path: impl Into<CompactString>) -> Self {
        let compact = path.into();
        debug_assert!(
            !compact.starts_with('/'),
            "FilePath must be relative, got: {}",
            compact
        );
        Self(compact)
    }

    pub fn try_new(path: impl Into<CompactString>) -> Result<Self, FilePathError> {
        let compact = path.into();
        if compact.starts_with('/') {
            return Err(FilePathError::AbsolutePath(compact.to_string()));
        }
        Ok(Self(compact))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilePathError {
    AbsolutePath(String),
}

impl std::fmt::Display for FilePathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilePathError::AbsolutePath(path) => {
                write!(f, "FilePath must be relative, got absolute path: {}", path)
            }
        }
    }
}

impl std::error::Error for FilePathError {}

impl std::fmt::Display for FilePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for FilePath {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for FilePath {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for FilePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<Path> for FilePath {
    fn as_ref(&self) -> &Path {
        Path::new(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn filepath_from_str() {
        let path = FilePath::from("src/main.rs");
        let as_str: &str = path.as_ref();
        assert_eq!(as_str, "src/main.rs");
    }

    #[test]
    fn filepath_equality() {
        let p1 = FilePath::from("file.txt");
        let p2 = FilePath::from("file.txt");
        assert_eq!(p1, p2);
    }

    #[test]
    fn filepath_display() {
        let path = FilePath::from("path/to/file.rs");
        assert_eq!(format!("{}", path), "path/to/file.rs");
    }

    #[test]
    fn filepath_as_path() {
        let fp = FilePath::from("src/lib.rs");
        let p: &Path = fp.as_ref();
        assert_eq!(p.to_str(), Some("src/lib.rs"));
    }

    #[test]
    #[should_panic(expected = "FilePath must be relative")]
    #[cfg(debug_assertions)]
    fn filepath_absolute_panics_debug() {
        FilePath::new("/absolute/path");
    }

    #[test]
    fn filepath_extension() {
        let path = FilePath::from("file.rs");
        let p: &Path = path.as_ref();
        assert_eq!(p.extension().and_then(|e| e.to_str()), Some("rs"));
    }
}
