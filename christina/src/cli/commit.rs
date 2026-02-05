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

    let files = match adapter::get_staged_files(&repo) {
        Ok(files) => files,
        Err(err) => {
            ui::print_error(&format!("Failed to read staged files: {}", err));
            return Err(err);
        }
    };

    display_changes(&files);

    let message = match generate_commit(diff, context.map(|s| s.to_string())).await {
        Ok(message) => message,
        Err(err) => {
            ui::print_error(&format!("Commit message generation failed: {}", err));
            return Err(err);
        }
    };

    let confirmed = match confirm_commit(&message, yes) {
        Ok(value) => value,
        Err(err) => {
            ui::print_error(&format!("Failed to confirm commit: {}", err));
            return Err(err);
        }
    };

    if !confirmed {
        ui::print_info("Commit cancelled.");
        return Ok(());
    }

    if dry_run {
        println!("\n{}", "═".repeat(60));
        println!("DRY RUN MODE - Commit NOT created");
        println!("{}\n", "═".repeat(60));
        println!("The following commit message would have been used:\n");
        println!("{}", message);
        return Ok(());
    }

    if let Err(err) = execute_commit(&repo, &message) {
        ui::print_error(&format!("Failed to create commit: {}", err));
        return Err(err);
    }

    Ok(())
}

fn validate_repository() -> Result<(Repository, String)> {
    let repo = Repository::discover(".").map_err(|err| {
        if err.code() == git2::ErrorCode::NotFound {
            anyhow::anyhow!("No git repository found. Run this inside a git repository.")
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
    ui::print_section("Staged changes");
    let file_paths = files
        .iter()
        .map(|file| file.path.to_string())
        .collect::<Vec<_>>();
    ui::print_file_list(&file_paths);

    let diff_preview = files
        .iter()
        .map(|file| file.diff_content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    if !diff_preview.trim().is_empty() {
        ui::print_diff_preview(&diff_preview, 120);
    }
}

async fn generate_commit(diff: String, context: Option<String>) -> Result<String> {
    let spinner = ui::create_spinner("Analyzing changes...");
    let config = Config::load()?;
    let repo = Repository::discover(".")?;
    let repo_path = repo
        .workdir()
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.path().to_path_buf());

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

fn confirm_commit(message: &str, yes: bool) -> Result<bool> {
    ui::print_section("Proposed commit");
    ui::print_commit_message(message);

    if yes {
        return Ok(true);
    }

    ui::confirm("Create commit with this message?")
        .map_err(|err| anyhow::anyhow!("Confirmation failed: {}", err))
}

fn execute_commit(repo: &Repository, message: &str) -> Result<()> {
    let oid = adapter::create_commit(repo, message)?;
    let oid_str = oid.to_string();
    let short = oid_str.get(..7).unwrap_or(oid_str.as_str());
    ui::print_success(&format!("Created commit {}", short));
    Ok(())
}
