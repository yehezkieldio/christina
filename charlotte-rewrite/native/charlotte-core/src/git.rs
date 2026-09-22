//! Staged diff reading and commit history walking, ported from Christina's
//! `christina/src/git/adapter.rs` (`collect_staged_changes`,
//! `format_patch_bounded`) and `christina/src/generate.rs`
//! (`get_commit_history_impl`). The commit-creation path stays out of this
//! module: Charlotte shells out to the operator's own `git commit` for the
//! commit step itself, per the open decision in `04-git-integration.md`.

use git2::{DiffOptions, Repository};

/// Matches Christina's `MAX_DIFF_SIZE` (`christina-core/src/types/diff.rs`).
pub const MAX_DIFF_SIZE: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct StagedDiff {
    pub diff: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CommitSummary {
    pub sha: String,
    pub subject: String,
}

#[derive(Debug)]
pub enum GitError {
    /// The repository at the given path could not be opened.
    Open(git2::Error),
    /// A git2 operation failed after the repository was opened.
    Operation(git2::Error),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::Open(err) => write!(f, "failed to open git repository: {err}"),
            GitError::Operation(err) => write!(f, "git operation failed: {err}"),
        }
    }
}

impl std::error::Error for GitError {}

impl From<git2::Error> for GitError {
    fn from(err: git2::Error) -> Self {
        GitError::Operation(err)
    }
}

fn get_delta_path(delta: &git2::DiffDelta) -> Option<String> {
    match delta.status() {
        git2::Delta::Deleted => delta.old_file().path(),
        _ => delta.new_file().path(),
    }
    .map(|p| p.to_string_lossy().to_string())
}

/// Reads the staged diff and the list of staged file paths. Per the
/// staged-only guarantee in `04-git-integration.md`, an empty index returns
/// an empty `StagedDiff` rather than an error, so the caller can fast-exit
/// before any model call.
pub fn read_staged_diff(repo_path: &str) -> Result<StagedDiff, GitError> {
    let repo = Repository::open(repo_path).map_err(GitError::Open)?;

    let head_tree = match repo.head() {
        Ok(head) => Some(head.peel_to_tree()?),
        Err(err) if err.code() == git2::ErrorCode::UnbornBranch => None,
        Err(err) => return Err(err.into()),
    };

    let mut opts = DiffOptions::new();
    opts.include_untracked(false)
        .ignore_whitespace_change(false)
        .context_lines(1)
        .old_prefix("a/")
        .new_prefix("b/");

    let mut diff = repo.diff_tree_to_index(head_tree.as_ref(), Some(&repo.index()?), Some(&mut opts))?;

    let mut find_opts = git2::DiffFindOptions::new();
    find_opts
        .renames(true)
        .copies(true)
        .copies_from_unmodified(true)
        .renames_from_rewrites(true)
        .rename_threshold(40)
        .copy_threshold(40);
    diff.find_similar(Some(&mut find_opts))?;

    let mut files = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            if let Some(path) = get_delta_path(&delta) {
                files.push(path);
            }
            true
        },
        None,
        None,
        None,
    )?;

    if files.is_empty() {
        return Ok(StagedDiff::default());
    }

    let diff_string = format_patch_bounded(&diff)?;
    Ok(StagedDiff { diff: diff_string, files })
}

/// Formats a diff as a patch, truncated to `MAX_DIFF_SIZE` bytes with a
/// trailing notice, matching Christina's `format_patch_bounded` exactly:
/// truncation lands on a UTF-8 char boundary, never mid-codepoint.
fn format_patch_bounded(diff: &git2::Diff) -> Result<String, GitError> {
    use std::fmt::Write;

    let mut diff_string = String::new();
    let notice = "\n[diff truncated: max size reached]";
    let max_without_notice = MAX_DIFF_SIZE.saturating_sub(notice.len());
    let mut truncated = false;

    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        if truncated {
            return false;
        }
        let origin = line.origin();
        let content = String::from_utf8_lossy(line.content());
        let mut line_len = content.len();
        if matches!(origin, '+' | '-' | ' ') {
            line_len += 1;
        }
        if diff_string.len().saturating_add(line_len) > max_without_notice {
            let mut truncate_at = max_without_notice.min(diff_string.len());
            while truncate_at > 0 && !diff_string.is_char_boundary(truncate_at) {
                truncate_at -= 1;
            }
            diff_string.truncate(truncate_at);
            diff_string.push_str(notice);
            truncated = true;
            return false;
        }
        if matches!(origin, '+' | '-' | ' ') {
            let _ = write!(&mut diff_string, "{origin}");
        }
        let _ = write!(&mut diff_string, "{content}");
        true
    })?;

    Ok(diff_string)
}

/// Walks recent commit subjects for style context, matching Christina's
/// `get_commit_history_impl`: merge commits and `fixup!`/`squash!`/`amend!`
/// subjects are skipped since they add noise to style inference, a shallow
/// clone caps the effective depth at 3, and each sha is the short 7-char
/// form.
pub fn read_commit_history(repo_path: &str, depth: u32) -> Result<Vec<CommitSummary>, GitError> {
    let repo = Repository::open(repo_path).map_err(GitError::Open)?;

    if repo.head().is_err() {
        return Ok(Vec::new());
    }

    let is_shallow = repo.is_shallow();
    let effective_limit = if is_shallow { (depth as usize).min(3) } else { depth as usize };

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    let mut commits = Vec::new();
    for oid_result in revwalk {
        if commits.len() >= effective_limit {
            break;
        }

        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;

        if commit.parent_count() > 1 {
            continue;
        }

        let subject = commit.message().unwrap_or("").lines().next().unwrap_or("").to_string();

        if subject.starts_with("fixup!") || subject.starts_with("squash!") || subject.starts_with("amend!") {
            continue;
        }

        let oid_str = oid.to_string();
        let sha = oid_str.get(..7).unwrap_or(oid_str.as_str()).to_string();

        commits.push(CommitSummary { sha, subject });
    }

    Ok(commits)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    struct TempRepo {
        dir: TempDir,
        repo: Repository,
    }

    impl TempRepo {
        fn new() -> Self {
            let dir = TempDir::new().expect("create temp dir");
            let repo = Repository::init(dir.path()).expect("init repo");
            {
                let mut config = repo.config().expect("repo config");
                config.set_str("user.name", "Test User").expect("set user.name");
                config.set_str("user.email", "test@example.com").expect("set user.email");
                config.set_str("commit.gpgsign", "false").expect("disable gpgsign");
            }
            Self { dir, repo }
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn write(&self, name: &str, content: &str) {
            std::fs::write(self.dir.path().join(name), content).expect("write fixture file");
        }

        fn stage(&self, name: &str) {
            let mut index = self.repo.index().expect("repo index");
            index.add_path(Path::new(name)).expect("stage file");
            index.write().expect("write index");
        }

        fn commit(&self, message: &str) -> git2::Oid {
            let mut index = self.repo.index().expect("repo index");
            let tree_id = index.write_tree().expect("write tree");
            let tree = self.repo.find_tree(tree_id).expect("find tree");
            let signature = self.repo.signature().expect("signature");
            let parent = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok());
            let parents: Vec<&git2::Commit> = parent.as_ref().map(|p| vec![p]).unwrap_or_default();
            self.repo
                .commit(Some("HEAD"), &signature, &signature, message, &tree, &parents)
                .expect("create commit")
        }
    }

    #[test]
    fn read_staged_diff_on_fresh_staged_file() {
        let repo = TempRepo::new();
        repo.write("file.txt", "hello\n");
        repo.stage("file.txt");

        let staged = read_staged_diff(repo.path().to_str().unwrap()).unwrap();
        assert_eq!(staged.files, vec!["file.txt".to_string()]);
        assert!(staged.diff.contains("+hello"));
    }

    #[test]
    fn read_staged_diff_empty_index_fast_exits() {
        let repo = TempRepo::new();
        let staged = read_staged_diff(repo.path().to_str().unwrap()).unwrap();
        assert!(staged.files.is_empty());
        assert!(staged.diff.is_empty());
    }

    #[test]
    fn read_commit_history_walks_to_depth() {
        let repo = TempRepo::new();
        repo.write("a.txt", "a\n");
        repo.stage("a.txt");
        repo.commit("first commit");
        repo.write("b.txt", "b\n");
        repo.stage("b.txt");
        repo.commit("second commit");
        repo.write("c.txt", "c\n");
        repo.stage("c.txt");
        repo.commit("third commit");

        let history = read_commit_history(repo.path().to_str().unwrap(), 2).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].subject, "third commit");
        assert_eq!(history[1].subject, "second commit");
        assert_eq!(history[0].sha.len(), 7);
    }

    #[test]
    fn read_commit_history_skips_fixup_subjects() {
        let repo = TempRepo::new();
        repo.write("a.txt", "a\n");
        repo.stage("a.txt");
        repo.commit("real commit");
        repo.write("b.txt", "b\n");
        repo.stage("b.txt");
        repo.commit("fixup! real commit");

        let history = read_commit_history(repo.path().to_str().unwrap(), 5).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].subject, "real commit");
    }

    #[test]
    fn read_commit_history_on_unborn_branch_is_empty() {
        let repo = TempRepo::new();
        let history = read_commit_history(repo.path().to_str().unwrap(), 5).unwrap();
        assert!(history.is_empty());
    }
}
