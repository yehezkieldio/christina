use std::path::Path;

use compact_str::CompactString;
use serde::{Deserialize, Serialize};

/// Relative file path within a Git repository.
///
/// WHY CompactString: Most repository file paths are short (≤16 bytes on average:
/// "src/main.rs" = 11 bytes), and CompactString stores these inline without heap
/// allocation. This eliminates pointer indirection for the common case while still
/// gracefully handling longer paths via the heap.
///
/// WHY relative paths only: Absolute paths break portability across different
/// checkout directories and deployment environments. By enforcing relative paths,
/// we ensure diffs and file references work consistently regardless of where
/// the repository is cloned. The invariant is enforced in all builds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FilePath(CompactString);

#[cfg(feature = "jsonschema")]
impl schemars::JsonSchema for FilePath {
    fn schema_name() -> String {
        "FilePath".to_string()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            metadata: Some(Box::new(schemars::schema::Metadata {
                description: Some("Relative file path within a Git repository".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

impl FilePath {
    pub fn new(path: impl Into<CompactString>) -> Self {
        let compact = path.into();
        assert!(
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
    fn filepath_absolute_panics() {
        FilePath::new("/absolute/path");
    }

    #[test]
    fn filepath_extension() {
        let path = FilePath::from("file.rs");
        let p: &Path = path.as_ref();
        assert_eq!(p.extension().and_then(|e| e.to_str()), Some("rs"));
    }
}
