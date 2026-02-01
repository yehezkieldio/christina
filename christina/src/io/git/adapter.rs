use anyhow::{Context, Result};
use git2::{DiffOptions, Repository, StatusOptions};
use std::io::Write;
use std::path::PathBuf;

use christina_core::git::{GitFile, GitFileStatus, RepoSnapshot};

/// Get the repository status including staged, unstaged, and changed files.
///
/// This adapter function discovers the git repository from the current directory
/// and returns a `RepoSnapshot` containing information about files
/// and repository status.
#[allow(dead_code)]
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

    // Get staged and unstaged file lists
    let staged_files = get_staged_files(&repo)?;
    let unstaged_files = get_unstaged_files(&repo)?;

    // Extract just the paths for staged and unstaged lists
    let staged: Vec<String> = staged_files
        .iter()
        .map(|f| f.path.as_str().to_string())
        .collect();
    let unstaged: Vec<String> = unstaged_files
        .iter()
        .map(|f| f.path.as_str().to_string())
        .collect();

    Ok(RepoSnapshot {
        files,
        staged,
        unstaged,
        branch,
        repo_root: root,
    })
}

/// Open a repository at a specific path or discover from current directory
#[allow(dead_code)]
pub fn open(path: Option<&std::path::Path>) -> Result<Repository> {
    match path {
        Some(p) => Repository::open(p).context("Failed to open repository"),
        None => Repository::discover(".").context("Failed to discover repository"),
    }
}

/// Get current branch name from repository
#[allow(dead_code)]
fn get_branch_name(repo: &Repository) -> Result<String> {
    let head = repo.head()?;
    if let Some(name) = head.shorthand() {
        Ok(name.to_string())
    } else {
        Ok("HEAD".to_string())
    }
}

/// Convert git2::Status to core::GitFileStatus
#[allow(dead_code)]
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

    // Collect file metadata first
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
                files.push((path.to_string_lossy().to_string(), status.to_string()));
            }
            true
        },
        None,
        None,
        None,
    )?;

    // Capture diff content per file
    use std::collections::HashMap;
    let mut diff_map: HashMap<String, String> = HashMap::new();
    let mut current_path = String::new();
    let mut current_diff = String::new();

    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        let new_path = delta
            .new_file()
            .path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        // If we've moved to a new file, save the previous one
        if !current_path.is_empty() && new_path != current_path {
            diff_map.insert(current_path.clone(), current_diff.clone());
            current_diff.clear();
        }

        current_path = new_path;

        // Append line content to current diff
        if let Ok(content) = std::str::from_utf8(line.content()) {
            current_diff.push_str(content);
        }

        true
    })?;

    // Save the last file's diff
    if !current_path.is_empty() {
        diff_map.insert(current_path, current_diff);
    }

    // Create GitFile objects with diff content
    let result = files
        .into_iter()
        .map(|(path, status)| {
            let diff_content = diff_map.get(&path).cloned().unwrap_or_default();
            GitFile::new(path, status, diff_content)
        })
        .collect();

    Ok(result)
}

/// Get unstaged files (changes between index and workdir)
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

    // Collect file metadata first
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
                files.push((path.to_string_lossy().to_string(), status.to_string()));
            }
            true
        },
        None,
        None,
        None,
    )?;

    // Capture diff content per file
    use std::collections::HashMap;
    let mut diff_map: HashMap<String, String> = HashMap::new();
    let mut current_path = String::new();
    let mut current_diff = String::new();

    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        let new_path = delta
            .new_file()
            .path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        // If we've moved to a new file, save the previous one
        if !current_path.is_empty() && new_path != current_path {
            diff_map.insert(current_path.clone(), current_diff.clone());
            current_diff.clear();
        }

        current_path = new_path;

        // Append line content to current diff
        if let Ok(content) = std::str::from_utf8(line.content()) {
            current_diff.push_str(content);
        }

        true
    })?;

    // Save the last file's diff
    if !current_path.is_empty() {
        diff_map.insert(current_path, current_diff);
    }

    // Create GitFile objects with diff content
    let result = files
        .into_iter()
        .map(|(path, status)| {
            let diff_content = diff_map.get(&path).cloned().unwrap_or_default();
            GitFile::new(path, status, diff_content)
        })
        .collect();

    Ok(result)
}

/// Stage files by path
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

    let config = repo.config()?;
    let gpg_sign = config.get_bool("commit.gpgsign").unwrap_or(false);

    if gpg_sign {
        let signing_key = config.get_string("user.signingkey").ok();

        let buffer = repo.commit_create_buffer(&signature, &signature, message, &tree, &parents)?;

        let content = std::str::from_utf8(&buffer)
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in commit buffer: {}", e))?;

        let program = config
            .get_string("gpg.program")
            .map(|s| {
                if s.trim().is_empty() {
                    "gpg".to_string()
                } else {
                    s
                }
            })
            .unwrap_or_else(|_| "gpg".to_string());

        let mut cmd = std::process::Command::new(&program);
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .arg("-bsa");

        if let Some(ref key) = signing_key {
            cmd.arg("-u").arg(key);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn {}: {}", program, e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(content.as_bytes())
                .map_err(|e| anyhow::anyhow!("Failed to write to gpg: {}", e))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| anyhow::anyhow!("Failed to wait for gpg: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("GPG signing failed: {}", stderr);
        }

        let gpg_sig = String::from_utf8(output.stdout)
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in signature: {}", e))?;

        let oid = repo.commit_signed(content, &gpg_sig, Some("gpgsig"))?;

        match repo.head() {
            Ok(head) => {
                if let Some(name) = head.name() {
                    repo.reference(name, oid, true, "commit (signed)")?;
                } else {
                    repo.set_head_detached(oid)?;
                }
            }
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                let branch_name = config
                    .get_string("init.defaultBranch")
                    .unwrap_or_else(|_| "master".to_string());
                let ref_name = format!("refs/heads/{}", branch_name);
                repo.reference(&ref_name, oid, false, "initial commit (signed)")?;
                repo.set_head(&ref_name)?;
            }
            Err(e) => return Err(e.into()),
        }

        Ok(oid)
    } else {
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

pub fn build_staged_diff(repo: &Repository) -> Result<String> {
    if repo.state() != git2::RepositoryState::Clean {
        return Err(anyhow::anyhow!("Repository is in {:?} state", repo.state()));
    }

    let mut index = repo.index().context("Failed to get index")?;
    let oid = index.write_tree().context("Failed to write tree")?;
    let tree = repo.find_tree(oid).context("Failed to find tree")?;

    let head = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    let parent_tree = head.as_ref().and_then(|c| c.tree().ok());

    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
        .context("Failed to create diff")?;

    let mut diff_string = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        use std::fmt::Write;
        let _ = write!(
            &mut diff_string,
            "{}",
            String::from_utf8_lossy(line.content())
        );
        true
    })
    .context("Failed to format diff")?;

    if diff_string.is_empty() {
        return Err(anyhow::anyhow!("No staged changes to process"));
    }

    Ok(diff_string)
}
