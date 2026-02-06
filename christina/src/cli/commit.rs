use std::path::PathBuf;

use anyhow::Result;
use git2::Repository;
use tokio::sync::mpsc;

use crate::cli::ui;
use crate::config::Config;
use crate::events::Event;
use crate::generate::generate_commit_message_with_progress;
use crate::io::git::adapter;
use christina_core::GitFile;

pub async fn run(yes: bool, context: Option<&str>, dry_run: bool) -> Result<()> {
    ui::print_header();
    ui::print_divider();

    let (repo, diff) = match validate_repository() {
        Ok(values) => values,
        Err(err) => {
            ui::print_error(&err.to_string());
            return Err(err);
        }
    };

    let repo_path = repo
        .workdir()
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.path().to_path_buf());

    let files = match adapter::get_staged_files(&repo) {
        Ok(files) => files,
        Err(err) => {
            ui::print_error(&format!("Failed to read staged files: {}", err));
            return Err(err);
        }
    };

    display_changes(&files);

    let message = loop {
        let message = match generate_commit(diff.clone(), context.map(|s| s.to_string()), repo_path.clone()).await {
            Ok(message) => message,
            Err(err) => {
                ui::print_error(&format!("Commit message generation failed: {}", err));
                return Err(err);
            }
        };

        let action = match confirm_commit(&message, yes) {
            Ok(value) => value,
            Err(err) => {
                ui::print_error(&format!("Failed to confirm commit: {}", err));
                return Err(err);
            }
        };

        match action {
            CommitAction::Accept => break message,
            CommitAction::Regenerate => continue,
            CommitAction::Decline => {
                ui::print_info("Commit cancelled.");
                return Ok(());
            }
        }
    };

    if dry_run {
        println!("\n{}", "═".repeat(60));
        println!("DRY RUN MODE - Commit NOT created");
        println!("{}\n", "═".repeat(60));
        println!("The following commit message would have been used:\n");
        println!("{}", message);
        return Ok(());
    }

    if let Err(err) = execute_commit(&repo, &message) {
        if is_gpg_signing_failure(&err) {
            ui::print_warning(
                "GPG signing failed. Configure your GPG key/agent or disable signing with: git config commit.gpgsign false",
            );
        }
        ui::print_error(&format!("Failed to create commit: {}", err));
        return Err(err);
    }

    Ok(())
}

fn validate_repository() -> Result<(Repository, String)> {
    let repo = Repository::open(".").map_err(|err| {
        if err.code() == git2::ErrorCode::NotFound {
            anyhow::anyhow!(
                "No git repository found in the current directory. Run this from the repository root."
            )
        } else {
            anyhow::anyhow!("Failed to open git repository: {}", err)
        }
    })?;

    if !adapter::has_staged_changes(&repo)? {
        anyhow::bail!("No staged changes to commit. Stage your changes and try again.");
    }

    let diff = adapter::build_staged_diff(&repo)?;
    Ok((repo, diff))
}

fn display_changes(files: &[GitFile]) {
    ui::print_section("Staged");
    ui::print_info(&format!("{} file(s) staged", files.len()));
    let file_paths = files
        .iter()
        .map(|file| file.path.to_string())
        .collect::<Vec<_>>();
    ui::print_file_list(&file_paths, 10);
}

async fn generate_commit(diff: String, context: Option<String>, repo_path: PathBuf) -> Result<String> {
    let spinner = ui::create_spinner("Analyzing changes...");
    let config = Config::load_async().await?;

    let (progress_tx, mut _progress_rx) = mpsc::channel::<Event>(100);
    let _progress_spinner = spinner.clone();
    let progress_handle = tokio::spawn(async move {
        while let Some(event) = _progress_rx.recv().await {
            match event {
                Event::GenerationProgress { stage, .. } => {
                    _progress_spinner.set_message(stage);
                }
                Event::TokenCountUpdate { token_count } => {
                    let _ = token_count.get();
                }
                _ => {
                    // Ignore other events for now
                }
            }
        }
    });

    let generation_result =
        generate_commit_message_with_progress(config, diff, repo_path, progress_tx, context).await;

    let _ = progress_handle.await;
    spinner.finish_and_clear();

    let generation_result = generation_result?;
    if let Some(warning) = generation_result.warning_summary() {
        ui::print_warning(&warning);
    }

    Ok(generation_result.message.to_string())
}

fn is_gpg_signing_failure(err: &anyhow::Error) -> bool {
    err.to_string().to_lowercase().contains("gpg signing failed")
}

enum CommitAction {
    Accept,
    Regenerate,
    Decline,
}

fn confirm_commit(message: &str, yes: bool) -> Result<CommitAction> {
    ui::print_section("Proposed");
    ui::print_commit_message(message);

    if yes {
        return Ok(CommitAction::Accept);
    }

    let actions = ["accept", "regenerate", "decline"];
    let selection = ui::select_action(&actions)
        .map_err(|err| anyhow::anyhow!("Confirmation failed: {}", err))?;

    let action = match selection {
        0 => CommitAction::Accept,
        1 => CommitAction::Regenerate,
        _ => CommitAction::Decline,
    };

    Ok(action)
}

fn execute_commit(repo: &Repository, message: &str) -> Result<()> {
    let oid = adapter::create_commit(repo, message)?;
    let oid_str = oid.to_string();
    let short = oid_str.get(..7).unwrap_or(oid_str.as_str());
    ui::print_success(&format!("Created commit {}", short));
    Ok(())
}
