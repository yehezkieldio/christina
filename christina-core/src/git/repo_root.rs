//! Repository root path type enforcing absolute path invariants.
//!
//! WHY absolute paths for repo_root: Git operations require an absolute path to
//! the repository root for reliable operations. Relative paths could resolve
//! differently depending on the current working directory, leading to incorrect
//! or failed git operations.
//!
//! WHY separate from FilePath: FilePath represents relative paths within the repo
//! (e.g., "src/main.rs"), while RepoRoot represents the absolute filesystem location
//! of the repository itself (e.g., "/home/user/project"). These are fundamentally
//! different concepts with opposite requirements for absolute vs relative paths.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Absolute path to a Git repository root directory.
///
/// WHY PathBuf instead of CompactString: Repository root paths are typically longer
/// than individual file paths and always require owned data for git operations.
/// PathBuf provides the necessary OS path handling and ownership without the
/// size optimization tradeoffs needed for file paths.
///
/// WHY absolute paths required: Git commands need an absolute path to operate
/// consistently regardless of the current working directory. The invariant is
/// enforced in all builds to catch programming errors early.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoRoot(PathBuf);

impl RepoRoot {
    /// Create a new RepoRoot from an absolute path.
    ///
    /// # Panics
    /// Panics if the path is not absolute. Use `try_new` for fallible construction.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        assert!(
            path.is_absolute(),
            "RepoRoot must be absolute, got: {}",
            path.display()
        );
        Self(path)
    }

    /// Try to create a new RepoRoot from a path.
    ///
    /// Returns an error if the path is not absolute.
    pub fn try_new(path: impl Into<PathBuf>) -> Result<Self, RepoRootError> {
        let path = path.into();
        if !path.is_absolute() {
            return Err(RepoRootError::RelativePath(
                path.to_string_lossy().to_string(),
            ));
        }
        Ok(Self(path))
    }

    /// Get the path as a reference.
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Convert into the inner PathBuf.
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoRootError {
    RelativePath(String),
}

impl std::fmt::Display for RepoRootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoRootError::RelativePath(path) => {
                write!(f, "RepoRoot must be absolute, got relative path: {}", path)
            }
        }
    }
}

impl std::error::Error for RepoRootError {}

impl std::fmt::Display for RepoRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl AsRef<Path> for RepoRoot {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl From<RepoRoot> for PathBuf {
    fn from(root: RepoRoot) -> Self {
        root.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reporoot_absolute_path() {
        let root = RepoRoot::new(PathBuf::from("/home/user/repo"));
        assert_eq!(root.as_path(), Path::new("/home/user/repo"));
    }

    #[test]
    fn reporoot_try_new_success() {
        let result = RepoRoot::try_new(PathBuf::from("/absolute/path"));
        assert!(result.is_ok());
    }

    #[test]
    fn reporoot_try_new_failure() {
        let result = RepoRoot::try_new(PathBuf::from("relative/path"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RepoRootError::RelativePath(_)));
    }

    #[test]
    fn reporoot_display() {
        let root = RepoRoot::new(PathBuf::from("/repo/path"));
        let display = format!("{}", root);
        assert!(display.contains("/repo/path"));
    }

    #[test]
    fn reporoot_as_ref() {
        let root = RepoRoot::new(PathBuf::from("/home/project"));
        let path_ref: &Path = root.as_ref();
        assert_eq!(path_ref, Path::new("/home/project"));
    }

    #[test]
    fn reporoot_into_pathbuf() {
        let root = RepoRoot::new(PathBuf::from("/repo"));
        let path = root.into_path_buf();
        assert_eq!(path, PathBuf::from("/repo"));
    }

    #[test]
    fn reporoot_equality() {
        let r1 = RepoRoot::new(PathBuf::from("/same/path"));
        let r2 = RepoRoot::new(PathBuf::from("/same/path"));
        assert_eq!(r1, r2);
    }

    #[test]
    #[should_panic(expected = "RepoRoot must be absolute")]
    fn reporoot_relative_panics() {
        RepoRoot::new(PathBuf::from("relative/path"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn reporoot_windows_path() {
        let root = RepoRoot::new(PathBuf::from("C:\\Users\\repo"));
        assert!(root.as_path().is_absolute());
    }
}
