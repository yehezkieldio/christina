use crate::git::GitFile;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RepoSnapshot {
    pub files: Vec<GitFile>,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub branch: String,
    pub repo_root: PathBuf,
}
