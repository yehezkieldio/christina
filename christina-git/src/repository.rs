use std::borrow::Cow;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use git2::{Diff, DiffFormat, DiffOptions, Repository};

use christina_core::git::GitFileStatus;
use christina_core::types::{CommitMessage, FilePath};

/// Information about a commit for historical context.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    /// Short SHA (first 7 characters).
    pub sha: String,
    /// Commit subject line only (no body).
    pub subject: String,
}

/// Error type for git operations
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Git error: {0}")]
    Git(String),
    #[error("Git2 error: {0}")]
    Git2(#[from] git2::Error),
    #[error("GPG config invalid: {0}")]
    GpgConfigInvalid(String),
    #[error("GPG signing failed: {0}")]
    GpgSigningFailed(String),
}

pub type Result<T> = std::result::Result<T, GitError>;

pub struct GitRepository {
    inner: Repository,
}

impl GitRepository {
    pub fn open(path: Option<&Path>) -> Result<Self> {
        let repo = match path {
            Some(p) => Repository::open(p)?,
            None => Repository::discover(".")?,
        };
        Ok(Self { inner: repo })
    }

    pub fn discover() -> Result<Self> {
        let repo = Repository::discover(".")?;
        Ok(Self { inner: repo })
    }

    pub fn workdir(&self) -> Option<&Path> {
        self.inner.workdir()
    }

    /// Get the staged diff (changes between HEAD and the index).
    ///
    /// Automatically detects renames and copies to provide semantic correctness:
    ///
    /// - Moved files appear as \"R\" (rename) instead of \"D\" + \"A\"
    /// - Copied files appear as \"C\" (copy) instead of large \"A\" diffs
    ///
    /// This should hopefully saves tokens when sending diffs to LLMs.
    pub fn get_staged_diff(&self) -> Result<StagedDiff<'_>> {
        // Get HEAD tree (or empty tree for initial commit)
        let head_tree = match self.inner.head() {
            Ok(head) => Some(head.peel_to_tree()?),
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) => return Err(e.into()),
        };

        let mut opts = DiffOptions::new();
        opts.include_untracked(false)
            .ignore_whitespace_change(false)
            .context_lines(1) // Reduced from 3 to save tokens
            .old_prefix("a/")
            .new_prefix("b/");

        let mut diff = self.inner.diff_tree_to_index(
            head_tree.as_ref(),
            Some(&self.inner.index()?),
            Some(&mut opts),
        )?;

        // Transforms "Delete A + Add B" into "Rename A -> B" when content is similar
        // and "Add large file" into "Copy A -> B" when duplicating content.
        let mut find_opts = git2::DiffFindOptions::new();
        find_opts
            .renames(true)
            .copies(true)
            .copies_from_unmodified(true) // Detect copies from unmodified files
            .renames_from_rewrites(true) // Detect renames even with heavy edits
            .rename_threshold(40) // Lower threshold (40% vs default 50%) to catch more renames
            .copy_threshold(40); // Lower threshold for copy detection
        diff.find_similar(Some(&mut find_opts))?;

        Ok(StagedDiff { diff })
    }

    pub fn has_staged_changes(&self) -> Result<bool> {
        let staged_diff = self.get_staged_diff()?;
        Ok(staged_diff.delta_count() > 0)
    }

    /// Get recent commit history for context
    ///
    /// Returns up to `limit` commits, filtering out:
    /// - Merge commits (parent_count > 1)
    /// - Fixup commits (subject starts with "fixup!", "squash!", "amend!")
    ///
    /// Returns empty vec gracefully if repo is empty or HEAD doesn't exist.
    pub fn get_commit_history(&self, limit: usize) -> Result<Vec<CommitInfo>> {
        // Handle empty repo (no HEAD)
        if self.inner.head().is_err() {
            return Ok(vec![]);
        }

        let mut revwalk = self.inner.revwalk()?;
        revwalk.push_head()?;

        let mut commits = vec![];

        for oid_result in revwalk {
            if commits.len() >= limit {
                break;
            }

            let oid = oid_result?;
            let commit = self.inner.find_commit(oid)?;

            // Skip merge commits
            if commit.parent_count() > 1 {
                continue;
            }

            // Get subject (first line)
            let subject = commit
                .message()
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("")
                .to_string();

            // Skip fixup/squash/amend commits
            if subject.starts_with("fixup!")
                || subject.starts_with("squash!")
                || subject.starts_with("amend!")
            {
                continue;
            }

            // Get short SHA (7 chars)
            let sha = format!("{}", oid)[..7].to_string();

            commits.push(CommitInfo { sha, subject });
        }

        Ok(commits)
    }

    pub fn get_unstaged_diff(&self) -> Result<StagedDiff<'_>> {
        let mut opts = DiffOptions::new();
        opts.include_untracked(true)
            .ignore_whitespace_change(false)
            .context_lines(1) // Reduced from 3 to save tokens
            .old_prefix("a/")
            .new_prefix("b/");

        let mut diff = self
            .inner
            .diff_index_to_workdir(Some(&self.inner.index()?), Some(&mut opts))?;

        // Rename and copy detection for unstaged changes too
        let mut find_opts = git2::DiffFindOptions::new();
        find_opts
            .renames(true)
            .copies(true)
            .copies_from_unmodified(true) // Detect copies from unmodified files
            .renames_from_rewrites(true); // Detect renames even with content changes
        diff.find_similar(Some(&mut find_opts))?;

        Ok(StagedDiff { diff })
    }

    /// Stage multiple files atomically.
    /// If any file fails validation, no changes are made to the index.
    pub fn stage_files(&self, files: &[(std::path::PathBuf, GitFileStatus)]) -> Result<()> {
        // Phase 1: Validate all operations before modifying index
        for (path, status) in files {
            match status {
                GitFileStatus::Deleted => {
                    // No validation needed - remove_path will fail gracefully if not in index
                }
                GitFileStatus::Added | GitFileStatus::Modified | GitFileStatus::Untracked => {
                    if !path.exists() {
                        return Err(GitError::Git(format!(
                            "Cannot stage '{}': file does not exist",
                            path.display()
                        )));
                    }
                }
                GitFileStatus::Renamed | GitFileStatus::Copied => {
                    // These are conditional operations - validation done during application
                }
                GitFileStatus::Unknown => {
                    // Conditional operation - no validation needed
                }
            }
        }

        // Phase 2: All validations passed - now apply changes to index
        let mut index = self.inner.index()?;

        for (path, status) in files {
            match status {
                GitFileStatus::Deleted => {
                    index.remove_path(path)?;
                }
                GitFileStatus::Added | GitFileStatus::Modified | GitFileStatus::Untracked => {
                    index.add_path(path)?;
                }
                GitFileStatus::Renamed | GitFileStatus::Copied => {
                    if path.exists() {
                        index.add_path(path)?;
                    }
                }
                GitFileStatus::Unknown => {
                    if path.exists() {
                        index.add_path(path)?;
                    } else {
                        index.remove_path(path)?;
                    }
                }
            }
        }

        // Phase 3: Write changes only after all operations succeed
        index.write()?;
        Ok(())
    }

    pub fn unstage_files(&self, paths: &[std::path::PathBuf]) -> Result<()> {
        match self.inner.head() {
            Ok(head) => {
                let commit = head.peel_to_commit()?;
                let path_refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
                self.inner
                    .reset_default(Some(&commit.into_object()), path_refs)?;
            }
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                let mut index = self.inner.index()?;
                for path in paths {
                    index.remove_path(path)?;
                }
                index.write()?;
            }
            Err(e) => return Err(e.into()),
        }
        Ok(())
    }

    pub fn validate_for_commit(&self) -> Result<()> {
        // Check for staged changes
        if !self.has_staged_changes()? {
            return Err(GitError::Git("No staged changes to commit".to_string()));
        }

        // Check for merge conflicts
        let index = self.inner.index()?;
        if index.has_conflicts() {
            return Err(GitError::Git(
                "Repository has unresolved merge conflicts".to_string(),
            ));
        }

        Ok(())
    }

    pub fn create_commit(&self, message: &CommitMessage) -> Result<git2::Oid> {
        // Validate GPG config if signing is enabled
        let config = self.inner.config()?;
        let gpg_sign = config.get_bool("commit.gpgsign").unwrap_or(false);

        let signature = self.inner.signature()?;
        let mut index = self.inner.index()?;
        let tree_id = index.write_tree()?;
        let tree = self.inner.find_tree(tree_id)?;

        let parent = match self.inner.head() {
            Ok(head) => Some(head.peel_to_commit()?),
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) => return Err(e.into()),
        };

        let parents: Vec<&git2::Commit> = parent.as_ref().map(|p| vec![p]).unwrap_or_default();

        if gpg_sign {
            // Check if user.signingkey is set
            let signing_key = config.get_string("user.signingkey").ok();

            // Create the commit buffer content to be signed
            let buffer = self.inner.commit_create_buffer(
                &signature,
                &signature,
                message.as_ref(),
                &tree,
                &parents,
            )?;

            let content = std::str::from_utf8(&buffer)
                .map_err(|e| GitError::Git(format!("Invalid UTF-8 in commit buffer: {}", e)))?;

            // Sign the content using GPG CLI
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

            let mut cmd = Command::new(&program);
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .arg("-bsa"); // detach, sign, armor

            if let Some(ref key) = signing_key {
                cmd.arg("-u").arg(key);
            }

            let mut child = cmd.spawn().map_err(|e| {
                GitError::GpgSigningFailed(format!("Failed to spawn {}: {}", program, e))
            })?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(content.as_bytes()).map_err(|e| {
                    GitError::GpgSigningFailed(format!("Failed to write to gpg: {}", e))
                })?;
            }

            let output = child.wait_with_output().map_err(|e| {
                GitError::GpgSigningFailed(format!("Failed to wait for gpg: {}", e))
            })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(GitError::GpgSigningFailed(format!(
                    "GPG failed: {}",
                    stderr
                )));
            }

            let gpg_sig = String::from_utf8(output.stdout).map_err(|e| {
                GitError::GpgSigningFailed(format!("Invalid UTF-8 in signature: {}", e))
            })?;

            // Create the signed commit
            let oid = self
                .inner
                .commit_signed(content, &gpg_sig, Some("gpgsig"))?;

            // Manually update HEAD reference since commit_signed doesn't do it
            match self.inner.head() {
                Ok(head) => {
                    if let Some(name) = head.name() {
                        self.inner.reference(name, oid, true, "commit (signed)")?;
                    } else {
                        // Detached HEAD - update HEAD to point to new commit
                        self.inner.set_head_detached(oid)?;
                    }
                }
                Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                    // Initial commit on unborn branch (e.g. master)
                    // Try to determine the default branch name
                    let branch_name = config
                        .get_string("init.defaultBranch")
                        .unwrap_or_else(|_| "master".to_string());
                    let ref_name = format!("refs/heads/{}", branch_name);
                    // Create the branch ref and set HEAD to point to it
                    self.inner
                        .reference(&ref_name, oid, false, "initial commit (signed)")?;
                    self.inner.set_head(&ref_name)?;
                }
                Err(e) => return Err(e.into()),
            }

            Ok(oid)
        } else {
            // Fallback to standard unsigned commit
            self.inner
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    message.as_ref(),
                    &tree,
                    &parents,
                )
                .map_err(|e| {
                    let err_msg = e.message().to_lowercase();
                    if err_msg.contains("gpg")
                        || err_msg.contains("signing")
                        || err_msg.contains("sign")
                    {
                        GitError::GpgSigningFailed(format!(
                            "Failed to sign commit with GPG: {}",
                            e.message()
                        ))
                    } else {
                        e.into()
                    }
                })
        }
    }

    pub fn get_staged_files_as_model(&self) -> Result<Vec<christina_core::git::GitFile>> {
        let diff = self.get_staged_diff()?;
        self.diff_to_model_files(&diff)
    }

    pub fn get_unstaged_files_as_model(&self) -> Result<Vec<christina_core::git::GitFile>> {
        let diff = self.get_unstaged_diff()?;
        self.diff_to_model_files(&diff)
    }

    /// Convert a StagedDiff to a Vec of File.
    fn diff_to_model_files(
        &self,
        diff: &StagedDiff<'_>,
    ) -> Result<Vec<christina_core::git::GitFile>> {
        use std::collections::HashMap;

        // Build all file diffs in a single pass to avoid O(n^2) complexity
        let patches: Vec<FilePatch> = diff.file_patches().collect();
        let mut file_contents: Vec<String> = vec![String::new(); patches.len()];

        // Pre-build a HashMap for O(1) delta lookup instead of O(n) .position() calls
        let delta_index: HashMap<(Option<FilePath>, Option<FilePath>), usize> = diff
            .inner()
            .deltas()
            .enumerate()
            .map(|(idx, d)| {
                let key = (
                    d.new_file()
                        .path()
                        .map(|p| FilePath::from(p.to_string_lossy().into_owned())),
                    d.old_file()
                        .path()
                        .map(|p| FilePath::from(p.to_string_lossy().into_owned())),
                );
                (key, idx)
            })
            .collect();

        let mut current_file_idx = 0;

        // Single-pass extraction of all file diffs
        diff.inner()
            .print(git2::DiffFormat::Patch, |delta, _hunk, line| {
                // Track which file we're on using precomputed index
                if line.origin() == 'F' {
                    let key = (
                        delta
                            .new_file()
                            .path()
                            .map(|p| FilePath::from(p.to_string_lossy().into_owned())),
                        delta
                            .old_file()
                            .path()
                            .map(|p| FilePath::from(p.to_string_lossy().into_owned())),
                    );
                    current_file_idx = delta_index.get(&key).copied().unwrap_or(0);
                }

                // Append content to the appropriate file buffer
                if current_file_idx < file_contents.len() {
                    let content = match std::str::from_utf8(line.content()) {
                        Ok(s) => std::borrow::Cow::Borrowed(s),
                        Err(_) => std::borrow::Cow::Owned(
                            String::from_utf8_lossy(line.content()).into_owned(),
                        ),
                    };

                    match line.origin() {
                        '+' | '-' | ' ' => file_contents[current_file_idx].push(line.origin()),
                        _ => {}
                    }
                    file_contents[current_file_idx].push_str(&content);
                }

                true
            })
            .map_err(|e| GitError::Git(format!("Failed to format diff: {}", e)))?;

        // Create File objects with their respective content
        let files = patches
            .into_iter()
            .zip(file_contents)
            .map(|(patch, content)| {
                // For renames and copies, show "old -> new" format
                let display_path = if let Some(old_path) = &patch.old_path {
                    format!("{} -> {}", old_path, patch.path)
                } else {
                    patch.path.to_string()
                };
                christina_core::git::GitFile::new(display_path, patch.status.to_string(), content)
            })
            .collect();

        Ok(files)
    }

    /// Get a reference to the underlying git2 repository.
    #[inline]
    pub fn inner(&self) -> &Repository {
        &self.inner
    }
}

/// Represents a staged diff in the repositor with lazy evaluation.
pub struct StagedDiff<'repo> {
    diff: Diff<'repo>,
}

impl<'repo> StagedDiff<'repo> {
    #[inline]
    pub fn delta_count(&self) -> usize {
        self.diff.deltas().len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.delta_count() == 0
    }

    /// Convert the diff to a UTF-8 validated string.
    ///
    /// This method performs the actual work of iterating through the diff
    /// and converting it to a string. It validates UTF-8 encoding and
    /// replaces invalid sequences with the replacement character.
    ///
    /// Captures ALL diff lines including:
    /// - File headers (origin 'F'): diff --git, similarity index, rename from/to, copy from/to
    /// - Hunk headers (origin 'H'): @@ line ranges
    /// - Content lines (origin '+'/'-'/' '): actual diff content
    ///
    /// This ensures the LLM receives complete rename/copy metadata for semantic correctness.
    pub fn to_string(&self) -> Result<String> {
        let mut output = String::new();

        self.diff
            .print(DiffFormat::Patch, |_delta, _hunk, line| {
                let origin = line.origin();
                let content = match std::str::from_utf8(line.content()) {
                    Ok(s) => Cow::Borrowed(s),
                    Err(_) => Cow::Owned(String::from_utf8_lossy(line.content()).into_owned()),
                };

                match origin {
                    // Content lines - add origin prefix
                    '+' | '-' | ' ' => {
                        output.push(origin);
                        output.push_str(&content);
                    }
                    // File headers (diff --git, similarity, rename, copy, mode changes)
                    // Hunk headers (@@) and other metadata
                    // These don't need origin prefix - output as-is
                    'F' | 'H' | '<' | '>' | '=' => {
                        output.push_str(&content);
                    }
                    // Binary file marker
                    'B' => {
                        output.push_str(&content);
                    }
                    // Ignore other origins
                    _ => {}
                }

                true
            })
            .map_err(|e| GitError::Git(format!("Failed to format diff: {}", e)))?;

        Ok(output)
    }

    /// Iterate over file patches, yielding the diff content for each file.
    ///
    /// This provides a more granular view than `to_string()`, allowing
    /// processing of individual file diffs.
    ///
    /// For renames and copies, both old_path and path are populated.
    pub fn file_patches(&self) -> impl Iterator<Item = FilePatch> + '_ {
        self.diff.deltas().map(move |delta| {
            let new_path = delta.new_file().path();
            let old_path = delta.old_file().path();

            let path = new_path
                .or(old_path)
                .map(|p| FilePath::from(p.to_string_lossy().into_owned()))
                .unwrap_or_else(|| FilePath::from("<unknown>"));

            // For renames and copies, track the old path if different
            let old_path_str = match delta.status() {
                git2::Delta::Renamed | git2::Delta::Copied => old_path.and_then(|old| {
                    let old_str = FilePath::from(old.to_string_lossy().into_owned());
                    let new_str = new_path
                        .map(|p| FilePath::from(p.to_string_lossy().into_owned()))
                        .unwrap_or_else(|| FilePath::from(""));
                    if old_str != new_str {
                        Some(old_str)
                    } else {
                        None
                    }
                }),
                _ => None,
            };

            let status = match delta.status() {
                git2::Delta::Added => FileStatus::Added,
                git2::Delta::Deleted => FileStatus::Deleted,
                git2::Delta::Modified => FileStatus::Modified,
                git2::Delta::Renamed => FileStatus::Renamed,
                git2::Delta::Copied => FileStatus::Copied,
                _ => FileStatus::Other,
            };

            FilePatch {
                path,
                old_path: old_path_str,
                status,
            }
        })
    }

    /// Get a reference to the underlying git2 diff.
    #[inline]
    pub fn inner(&self) -> &Diff<'repo> {
        &self.diff
    }
}

/// Represents a single file's patch information.
#[derive(Debug, Clone)]
pub struct FilePatch {
    /// Path to the file (new path for renames/copies, only path otherwise).
    pub path: FilePath,
    /// Old path (only different for renames/copies).
    pub old_path: Option<FilePath>,
    /// Status of the file change.
    pub status: FileStatus,
}

/// Status of a file in the diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
    Other,
}

impl FileStatus {
    /// Get a single-character representation of the status.
    pub fn as_char(&self) -> char {
        match self {
            FileStatus::Added => 'A',
            FileStatus::Deleted => 'D',
            FileStatus::Modified => 'M',
            FileStatus::Renamed => 'R',
            FileStatus::Copied => 'C',
            FileStatus::Other => '?',
        }
    }
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_char())
    }
}

// Bidirectional conversion between FileStatus and christina_core::GitFileStatus
impl From<FileStatus> for christina_core::GitFileStatus {
    fn from(status: FileStatus) -> Self {
        match status {
            FileStatus::Added => christina_core::GitFileStatus::Added,
            FileStatus::Deleted => christina_core::GitFileStatus::Deleted,
            FileStatus::Modified => christina_core::GitFileStatus::Modified,
            FileStatus::Renamed => christina_core::GitFileStatus::Renamed,
            FileStatus::Copied => christina_core::GitFileStatus::Copied,
            FileStatus::Other => christina_core::GitFileStatus::Unknown,
        }
    }
}

impl From<christina_core::GitFileStatus> for FileStatus {
    fn from(status: christina_core::GitFileStatus) -> Self {
        match status {
            christina_core::GitFileStatus::Added => FileStatus::Added,
            christina_core::GitFileStatus::Deleted => FileStatus::Deleted,
            christina_core::GitFileStatus::Modified => FileStatus::Modified,
            christina_core::GitFileStatus::Renamed => FileStatus::Renamed,
            christina_core::GitFileStatus::Copied => FileStatus::Copied,
            christina_core::GitFileStatus::Untracked => FileStatus::Other,
            christina_core::GitFileStatus::Unknown => FileStatus::Other,
        }
    }
}
