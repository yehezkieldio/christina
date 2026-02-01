use anyhow::Context;
use christina_core::types::CommitMessage;
use christina_git::GitRepository;
use std::fs;
use tempfile::TempDir;
use thiserror as _;

fn init_repo() -> anyhow::Result<(TempDir, git2::Repository)> {
    let temp_dir = TempDir::new()?;
    let repo = git2::Repository::init(temp_dir.path())?;
    let mut config = repo.config()?;
    config.set_str("user.name", "Test User")?;
    config.set_str("user.email", "test@example.com")?;
    // Explicitly disable GPG signing to ensure tests pass regardless of user's global config
    config.set_bool("commit.gpgsign", false)?;
    Ok((temp_dir, repo))
}

#[test]
fn full_staging_and_commit_workflow() -> anyhow::Result<()> {
    let (temp_dir, _repo) = init_repo()?;
    let repo_path = temp_dir.path();
    std::env::set_current_dir(repo_path)?;

    let file_path = repo_path.join("example.txt");
    fs::write(&file_path, "hello")?;

    let git_repo = GitRepository::open(Some(repo_path))?;
    git_repo.stage_files(&[(
        std::path::PathBuf::from("example.txt"),
        christina_core::GitFileStatus::Added,
    )])?;

    let message = CommitMessage::try_from("feat: add example".to_string())
        .map_err(anyhow::Error::msg)
        .context("Failed to create commit message")?;
    let commit_id = git_repo.create_commit(&message)?;
    assert_ne!(commit_id, git2::Oid::zero());

    Ok(())
}

#[test]
fn large_diff_processing_pipeline() -> anyhow::Result<()> {
    let (temp_dir, repo) = init_repo()?;
    let repo_path = temp_dir.path();

    let file_path = repo_path.join("large.txt");
    fs::write(&file_path, "line\n".repeat(500))?;

    let mut index = repo.index()?;
    index.add_path(std::path::Path::new("large.txt"))?;
    index.write()?;

    let git_repo = GitRepository::open(Some(repo_path))?;
    let staged = git_repo.get_staged_diff()?;
    let diff = staged.to_string()?;
    assert!(diff.contains("diff --git"));

    Ok(())
}
