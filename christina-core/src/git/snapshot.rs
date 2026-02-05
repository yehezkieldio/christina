use crate::git::{GitFile, RepoRoot};

#[derive(Debug, Clone)]
pub struct RepoSnapshot {
    pub files: Vec<GitFile>,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub branch: String,
    pub repo_root: RepoRoot,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_repo_snapshot_clone() {
        let snapshot = RepoSnapshot {
            files: vec![],
            staged: vec!["file1.rs".to_string()],
            unstaged: vec!["file2.rs".to_string()],
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
            staged: vec!["file.rs".to_string()],
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
