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

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

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
