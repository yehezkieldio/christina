use anyhow::{Context, Result};
use git2::{DiffOptions, Repository, StatusOptions};
use std::path::PathBuf;

use christina_core::git::{GitFile, GitFileStatus, RepoSnapshot};

/// Get the repository status including staged, unstaged, and changed files.
///
/// This adapter function discovers the git repository from the current directory
/// and returns a `RepoSnapshot` containing information about files
/// and repository status.
#[expect(
    dead_code,
    reason = "Will be used when Cmd executor is wired to event loop"
)]
pub fn status() -> Result<RepoSnapshot> {
    // Discover repository from current directory
    let repo = Repository::discover(".").context("Failed to discover git repository")?;

    // Get repository root
    let root = repo
        .workdir()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    // Get current branch name
    let branch = get_branch_name(&repo).unwrap_or_else(|_| "HEAD".to_string());

    // Get file statuses
    let mut status_opts = StatusOptions::new();
    status_opts
        .include_untracked(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);

    let statuses = repo
        .statuses(Some(&mut status_opts))
        .context("Failed to get repository status")?;

    let files: Vec<GitFile> = statuses
        .iter()
        .filter_map(|entry| {
            let path = entry.path()?;
            let status = convert_status(entry.status());
            let status_char = status.as_char().to_string();
            Some(GitFile::new(
                path.to_string(),
                status_char,
                String::new(), // diff_content - empty for now
            ))
        })
        .collect();

    Ok(RepoSnapshot {
        files,
        staged: vec![],   // TODO: populate from get_staged_files
        unstaged: vec![], // TODO: populate from get_unstaged_files
        branch,
        repo_root: root,
    })
}

/// Open a repository at a specific path or discover from current directory
#[expect(
    dead_code,
    reason = "Will be used when Cmd executor is wired to event loop"
)]
pub fn open(path: Option<&std::path::Path>) -> Result<Repository> {
    match path {
        Some(p) => Repository::open(p).context("Failed to open repository"),
        None => Repository::discover(".").context("Failed to discover repository"),
    }
}

/// Get current branch name from repository
fn get_branch_name(repo: &Repository) -> Result<String> {
    let head = repo.head()?;
    if let Some(name) = head.shorthand() {
        Ok(name.to_string())
    } else {
        Ok("HEAD".to_string())
    }
}

/// Convert git2::Status to core::GitFileStatus
fn convert_status(status: git2::Status) -> GitFileStatus {
    use git2::Status;

    if status.contains(Status::WT_NEW) || status.contains(Status::INDEX_NEW) {
        GitFileStatus::Added
    } else if status.contains(Status::WT_MODIFIED) || status.contains(Status::INDEX_MODIFIED) {
        GitFileStatus::Modified
    } else if status.contains(Status::WT_DELETED) || status.contains(Status::INDEX_DELETED) {
        GitFileStatus::Deleted
    } else if status.contains(Status::WT_RENAMED) || status.contains(Status::INDEX_RENAMED) {
        GitFileStatus::Renamed
    } else if status.contains(Status::WT_TYPECHANGE) || status.contains(Status::INDEX_TYPECHANGE) {
        GitFileStatus::Modified
    } else {
        GitFileStatus::Unknown
    }
}

/// Get staged files (changes between HEAD and index)
#[expect(
    dead_code,
    reason = "Will be used when Cmd executor is wired to event loop"
)]
pub fn get_staged_files(repo: &Repository) -> Result<Vec<GitFile>> {
    let head_tree = match repo.head() {
        Ok(head) => Some(head.peel_to_tree()?),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };

    let mut opts = DiffOptions::new();
    opts.include_untracked(false)
        .ignore_whitespace_change(false)
        .context_lines(1)
        .old_prefix("a/")
        .new_prefix("b/");

    let mut diff =
        repo.diff_tree_to_index(head_tree.as_ref(), Some(&repo.index()?), Some(&mut opts))?;

    // Detect renames and copies
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
            if let Some(path) = delta.new_file().path() {
                let status = match delta.status() {
                    git2::Delta::Added => "A",
                    git2::Delta::Deleted => "D",
                    git2::Delta::Modified => "M",
                    git2::Delta::Renamed => "R",
                    git2::Delta::Copied => "C",
                    _ => "?",
                };
                files.push(GitFile::new(
                    path.to_string_lossy().to_string(),
                    status.to_string(),
                    String::new(),
                ));
            }
            true
        },
        None,
        None,
        None,
    )?;

    Ok(files)
}

/// Get unstaged files (changes between index and workdir)
#[expect(
    dead_code,
    reason = "Will be used when Cmd executor is wired to event loop"
)]
pub fn get_unstaged_files(repo: &Repository) -> Result<Vec<GitFile>> {
    let mut opts = DiffOptions::new();
    opts.include_untracked(true)
        .ignore_whitespace_change(false)
        .context_lines(1)
        .old_prefix("a/")
        .new_prefix("b/");

    let mut diff = repo.diff_index_to_workdir(Some(&repo.index()?), Some(&mut opts))?;

    // Detect renames and copies
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts
        .renames(true)
        .copies(true)
        .copies_from_unmodified(true)
        .renames_from_rewrites(true);
    diff.find_similar(Some(&mut find_opts))?;

    let mut files = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            if let Some(path) = delta.new_file().path() {
                let status = match delta.status() {
                    git2::Delta::Added => "A",
                    git2::Delta::Deleted => "D",
                    git2::Delta::Modified => "M",
                    git2::Delta::Renamed => "R",
                    git2::Delta::Copied => "C",
                    _ => "?",
                };
                files.push(GitFile::new(
                    path.to_string_lossy().to_string(),
                    status.to_string(),
                    String::new(),
                ));
            }
            true
        },
        None,
        None,
        None,
    )?;

    Ok(files)
}

/// Stage files by path
#[expect(
    dead_code,
    reason = "Will be used when Cmd executor is wired to event loop"
)]
pub fn stage_files(repo: &Repository, paths: &[String]) -> Result<()> {
    let mut index = repo.index()?;

    for path in paths {
        let path_buf = std::path::Path::new(path);
        if path_buf.exists() {
            index.add_path(path_buf)?;
        } else {
            index.remove_path(path_buf)?;
        }
    }

    index.write()?;
    Ok(())
}

/// Unstage files by path
#[expect(
    dead_code,
    reason = "Will be used when Cmd executor is wired to event loop"
)]
pub fn unstage_files(repo: &Repository, paths: &[String]) -> Result<()> {
    let path_refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_ref()).collect();

    match repo.head() {
        Ok(head) => {
            let commit = head.peel_to_commit()?;
            repo.reset_default(Some(&commit.into_object()), path_refs)?;
        }
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            let mut index = repo.index()?;
            for path in &path_refs {
                index.remove_path(path)?;
            }
            index.write()?;
        }
        Err(e) => return Err(e.into()),
    }

    Ok(())
}

/// Create a commit with the given message
#[expect(
    dead_code,
    reason = "Will be used when Cmd executor is wired to event loop"
)]
pub fn create_commit(repo: &Repository, message: &str) -> Result<git2::Oid> {
    let signature = repo.signature()?;
    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };

    let parents: Vec<&git2::Commit> = parent.as_ref().map(|p| vec![p]).unwrap_or_default();

    let oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )?;

    Ok(oid)
}

/// Check if there are staged changes
pub fn has_staged_changes(repo: &Repository) -> Result<bool> {
    let head_tree = match repo.head() {
        Ok(head) => Some(head.peel_to_tree()?),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };

    let mut opts = DiffOptions::new();
    opts.include_untracked(false);

    let diff =
        repo.diff_tree_to_index(head_tree.as_ref(), Some(&repo.index()?), Some(&mut opts))?;

    Ok(diff.deltas().count() > 0)
}

/// Validate that the repository is ready for commit
#[expect(
    dead_code,
    reason = "Will be used when Cmd executor is wired to event loop"
)]
pub fn validate_for_commit(repo: &Repository) -> Result<()> {
    // Check for staged changes
    if !has_staged_changes(repo)? {
        return Err(anyhow::anyhow!("No staged changes to commit"));
    }

    // Check for merge conflicts
    let index = repo.index()?;
    if index.has_conflicts() {
        return Err(anyhow::anyhow!("Repository has unresolved merge conflicts"));
    }

    Ok(())
}
