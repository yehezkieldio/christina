use anyhow::{Context, Result};
use git2::{DiffOptions, Repository};
use std::io::{IsTerminal, Write};

use christina_core::git::GitFile;

/// Get the appropriate path from a delta based on its status.
/// For deletions, use old_file path; otherwise use new_file path.
fn get_delta_path(delta: &git2::DiffDelta) -> Option<String> {
    match delta.status() {
        git2::Delta::Deleted => delta.old_file().path(),
        _ => delta.new_file().path(),
    }
    .map(|p| p.to_string_lossy().to_string())
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
            if let Some(path) = get_delta_path(&delta) {
                let status = match delta.status() {
                    git2::Delta::Added => "A",
                    git2::Delta::Deleted => "D",
                    git2::Delta::Modified => "M",
                    git2::Delta::Renamed => "R",
                    git2::Delta::Copied => "C",
                    _ => "?",
                };
                files.push((path, status.to_string()));
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
        let path = get_delta_path(&delta).unwrap_or_default();

        // If we've moved to a new file, save the previous one
        if !current_path.is_empty() && path != current_path {
            diff_map.insert(current_path.clone(), current_diff.clone());
            current_diff.clear();
        }

        current_path = path;

        // Include origin character for context/add/delete lines
        let origin = line.origin();
        if matches!(origin, '+' | '-' | ' ') {
            current_diff.push(origin);
        }
        // Use lossy UTF-8 conversion to include content with replacement characters
        // rather than silently dropping non-UTF8 lines
        let content = String::from_utf8_lossy(line.content());
        current_diff.push_str(&content);

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

#[cfg(test)]
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
            if let Some(path) = get_delta_path(&delta) {
                let status = match delta.status() {
                    git2::Delta::Added => "A",
                    git2::Delta::Deleted => "D",
                    git2::Delta::Modified => "M",
                    git2::Delta::Renamed => "R",
                    git2::Delta::Copied => "C",
                    _ => "?",
                };
                files.push((path, status.to_string()));
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
        let path = get_delta_path(&delta).unwrap_or_default();

        // If we've moved to a new file, save the previous one
        if !current_path.is_empty() && path != current_path {
            diff_map.insert(current_path.clone(), current_diff.clone());
            current_diff.clear();
        }

        current_path = path;

        // Include origin character for context/add/delete lines
        let origin = line.origin();
        if matches!(origin, '+' | '-' | ' ') {
            current_diff.push(origin);
        }
        // Use lossy UTF-8 conversion to include content with replacement characters
        // rather than silently dropping non-UTF8 lines
        let content = String::from_utf8_lossy(line.content());
        current_diff.push_str(&content);

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
#[cfg(test)]
pub fn stage_files(repo: &Repository, paths: &[String]) -> Result<()> {
    let mut index = repo.index()?;
    let workdir = repo
        .workdir()
        .context("Cannot stage files in bare repository")?;

    for path in paths {
        let path_buf = std::path::Path::new(path);
        let full_path = workdir.join(path_buf);
        if full_path.exists() {
            index.add_path(path_buf)?;
        } else {
            index.remove_path(path_buf)?;
        }
    }

    index.write()?;
    Ok(())
}

/// Unstage files by path
#[cfg(test)]
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

/// Validate that git user.name and user.email are configured
fn validate_git_author(repo: &Repository) -> Result<()> {
    let config = repo.config()?;

    let name_err = config.get_string("user.name").is_err();
    let email_err = config.get_string("user.email").is_err();

    if name_err || email_err {
        return Err(anyhow::anyhow!(
            "Git user.name and/or user.email not configured. Run:\n  git config --global user.name 'Your Name'\n  git config --global user.email 'your@email.com'"
        ));
    }

    Ok(())
}

pub fn create_commit(repo: &Repository, message: &str) -> Result<git2::Oid> {
    validate_git_author(repo)?;
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

        let stdin_is_tty = std::io::stdin().is_terminal();
        let gpg_tty = std::env::var("GPG_TTY").ok().or_else(|| {
            #[cfg(unix)]
            {
                if stdin_is_tty {
                    Some("/dev/tty".to_string())
                } else {
                    None
                }
            }
            #[cfg(not(unix))]
            {
                None
            }
        });

        let mut cmd = std::process::Command::new(&program);
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .arg("-bsa");

        if !stdin_is_tty {
            cmd.arg("--batch").arg("--pinentry-mode").arg("loopback");
        }

        if let Some(ref key) = signing_key {
            cmd.arg("-u").arg(key);
        }

        if let Some(tty) = gpg_tty {
            cmd.env("GPG_TTY", tty);
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
            let stderr_trimmed = stderr.trim();
            let mut message = format!("GPG signing failed: {}", stderr_trimmed);
            if is_noninteractive_gpg_error(stderr_trimmed) {
                message.push_str(
                    " (non-interactive signing failed). Configure gpg-agent/pinentry for \
                     loopback mode, set GPG_TTY if running in a terminal, or disable signing \
                     with: git config commit.gpgsign false",
                );
            }
            anyhow::bail!(message);
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

fn is_noninteractive_gpg_error(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    lower.contains("pinentry")
        || lower.contains("no pinentry")
        || lower.contains("batchmode")
        || lower.contains("inappropriate ioctl")
        || lower.contains("no tty")
        || lower.contains("cannot open")
        || lower.contains("tty")
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
#[cfg(test)]
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
        // Include origin character for context/add/delete lines
        let origin = line.origin();
        if matches!(origin, '+' | '-' | ' ') {
            let _ = write!(&mut diff_string, "{}", origin);
        }
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use christina_core::test_helpers::TempRepo;
    use git2::build::CheckoutBuilder;
    use std::path::Path;

    fn new_repo() -> TempRepo {
        let temp_repo = TempRepo::new();
        disable_gpg(temp_repo.repo());
        temp_repo
    }

    fn disable_gpg(repo: &Repository) {
        let mut config = repo.config().unwrap();
        config.set_str("commit.gpgsign", "false").unwrap();
    }

    fn write_file(repo: &Repository, path: &str, content: &str) {
        let file_path = repo.workdir().unwrap().join(path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(file_path, content).unwrap();
    }

    fn stage(repo: &Repository, paths: &[&str]) {
        let paths = paths.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        stage_files(repo, &paths).unwrap();
    }

    fn checkout_branch(repo: &Repository, reference: &str) {
        repo.set_head(reference).unwrap();
        let mut checkout = CheckoutBuilder::new();
        checkout.force();
        repo.checkout_head(Some(&mut checkout)).unwrap();
    }

    #[test]
    fn test_get_staged_files_empty() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        let files = get_staged_files(repo).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_get_staged_files_with_changes() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        write_file(repo, "file.txt", "hello\n");
        stage(repo, &["file.txt"]);

        let files = get_staged_files(repo).unwrap();
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path.as_str(), "file.txt");
        assert_eq!(file.status.as_str(), "A");
        assert!(file.diff_content.contains("+hello"));
    }

    #[test]
    fn test_get_unstaged_files() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        temp_repo.commit_file("file.txt", "base\n");
        write_file(repo, "file.txt", "changed\n");

        let files = get_unstaged_files(repo).unwrap();
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path.as_str(), "file.txt");
        assert_eq!(file.status.as_str(), "M");
        assert!(file.diff_content.contains("+changed"));
    }

    #[test]
    fn test_stage_files_adds_to_index() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        write_file(repo, "file.txt", "content\n");
        stage(repo, &["file.txt"]);

        let index = repo.index().unwrap();
        assert!(index.get_path(Path::new("file.txt"), 0).is_some());
    }

    #[test]
    fn test_unstage_files_removes_from_index() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        temp_repo.commit_file("base.txt", "base\n");
        write_file(repo, "added.txt", "content\n");
        stage(repo, &["added.txt"]);

        let index = repo.index().unwrap();
        assert!(index.get_path(Path::new("added.txt"), 0).is_some());

        unstage_files(repo, &["added.txt".to_string()]).unwrap();

        let index = repo.index().unwrap();
        assert!(index.get_path(Path::new("added.txt"), 0).is_none());
    }

    #[test]
    fn test_create_commit_initial() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        write_file(repo, "file.txt", "initial\n");
        stage(repo, &["file.txt"]);

        let oid = create_commit(repo, "Initial commit").unwrap();
        let commit = repo.find_commit(oid).unwrap();
        assert_eq!(commit.parent_count(), 0);
        assert!(commit.message().unwrap().starts_with("Initial commit"));
    }

    #[test]
    fn test_create_commit_normal() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        let parent_oid = temp_repo.commit_file("file.txt", "base\n");
        write_file(repo, "file.txt", "updated\n");
        stage(repo, &["file.txt"]);

        let oid = create_commit(repo, "Second commit").unwrap();
        let commit = repo.find_commit(oid).unwrap();
        assert_eq!(commit.parent_count(), 1);
        assert_eq!(commit.parent_id(0).unwrap(), parent_oid);
    }

    #[test]
    fn test_has_staged_changes_true() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        write_file(repo, "file.txt", "content\n");
        stage(repo, &["file.txt"]);

        assert!(has_staged_changes(repo).unwrap());
    }

    #[test]
    fn test_has_staged_changes_false() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        assert!(!has_staged_changes(repo).unwrap());
    }

    #[test]
    fn test_validate_for_commit_no_changes() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        let err = validate_for_commit(repo).unwrap_err();
        assert!(err.to_string().contains("No staged changes"));
    }

    #[test]
    fn test_validate_for_commit_with_conflicts() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        let base_oid = temp_repo.commit_file("file.txt", "base\n");
        let base_commit = repo.find_commit(base_oid).unwrap();
        let base_ref = repo.head().unwrap().name().unwrap().to_string();
        repo.branch("feature", &base_commit, false).unwrap();

        checkout_branch(repo, "refs/heads/feature");
        write_file(repo, "file.txt", "feature\n");
        stage(repo, &["file.txt"]);
        create_commit(repo, "feature commit").unwrap();

        checkout_branch(repo, &base_ref);
        write_file(repo, "file.txt", "main\n");
        stage(repo, &["file.txt"]);
        create_commit(repo, "main commit").unwrap();

        let feature_ref = repo.find_reference("refs/heads/feature").unwrap();
        let annotated = repo.reference_to_annotated_commit(&feature_ref).unwrap();
        let mut checkout = CheckoutBuilder::new();
        checkout.allow_conflicts(true).force();
        repo.merge(&[&annotated], None, Some(&mut checkout))
            .unwrap();

        let index = repo.index().unwrap();
        assert!(index.has_conflicts());

        write_file(repo, "extra.txt", "extra\n");
        stage(repo, &["extra.txt"]);

        let err = validate_for_commit(repo).unwrap_err();
        assert!(err.to_string().contains("conflicts"));

        repo.cleanup_state().unwrap();
    }

    #[test]
    fn test_build_staged_diff() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        temp_repo.commit_file("file.txt", "base\n");
        write_file(repo, "file.txt", "changed\n");
        stage(repo, &["file.txt"]);

        let diff = build_staged_diff(repo).unwrap();
        assert!(diff.contains("diff --git"));
        assert!(diff.contains("file.txt"));
        assert!(diff.contains("+changed"));
    }

    #[test]
    fn test_build_staged_diff_empty() {
        let temp_repo = new_repo();
        let repo = temp_repo.repo();

        let err = build_staged_diff(repo).unwrap_err();
        assert!(err.to_string().contains("No staged changes"));
    }
}
