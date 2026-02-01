pub mod events;
pub mod handlers;
pub mod producers;

pub use events::Event;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::app::App;
use crate::app::state::{AbortOnDrop, GenerationState};
use crate::bootstrap::TerminalHandle;
use crate::generate::generate_commit_message_with_progress;
use crate::tui::render;
use christina_core::{AppState, GitError};

use handlers::{
    format_error_message, handle_generation_complete, handle_generation_error, handle_input,
    handle_tick,
};
use producers::EventProducers;

pub async fn run_event_loop(
    app: &mut App,
    terminal: &mut TerminalHandle,
    mut rx: mpsc::Receiver<Event>,
    tx: mpsc::Sender<Event>,
) -> Result<()> {
    let producers = EventProducers::spawn(tx.clone());

    loop {
        // Start generation if we're in Generating state and no task is running
        // Check this FIRST before rendering to ensure immediate start after navigation
        // Atomic check-and-start prevents race condition where two ticks could both see Idle state
        let should_start = app.state == AppState::Generating
            && matches!(app.generation_state, GenerationState::Idle);

        if should_start {
            try_start_generation(app, tx.clone()).await?;
            app.ui.should_redraw = true;
        }

        // Render if needed
        if app.ui.should_redraw {
            terminal.terminal_mut().draw(|frame| render(frame, app))?;
            app.ui.should_redraw = false;
        }

        // Check for quit
        if app.should_quit {
            break;
        }

        // Process event (blocking wait)
        if let Some(event) = rx.recv().await {
            match event {
                Event::Input(key) => {
                    handle_input(app, key);
                }
                Event::Tick => {
                    handle_tick(app);
                }
                Event::Resize => {
                    app.ui.should_redraw = true;
                }
                Event::GenerationProgress {
                    stage,
                    generation_id,
                } => {
                    handlers::handle_generation_progress(app, stage, generation_id);
                }
                Event::TokenCountUpdate {
                    token_count,
                    generation_id,
                } => {
                    handlers::handle_token_count_update(app, token_count, generation_id);
                }
                Event::GenerationComplete {
                    message,
                    warning_summary,
                    generation_id,
                } => {
                    handle_generation_complete(app, message, warning_summary, generation_id);
                }
                Event::GenerationError {
                    error,
                    generation_id,
                } => {
                    handle_generation_error(app, error, generation_id);
                }
            }
        }
    }

    // Abort generation task if still running
    if matches!(app.generation_state, GenerationState::Running { .. })
        && let GenerationState::Running { task, .. } =
            std::mem::replace(&mut app.generation_state, GenerationState::Idle)
    {
        task.0.abort();
    }

    // Shutdown background tasks
    producers.shutdown().await;

    Ok(())
}

async fn try_start_generation(app: &mut App, tx: mpsc::Sender<Event>) -> Result<()> {
    let config = app.app_context.config.clone();
    let generation_id = app.data.state_machine.next_generation_id();
    let user_context = app.data.base.user_context.clone();

    // Get repository path to reopen it in the background task
    let repo_path = app
        .app_context
        .repo
        .as_ref()
        .and_then(|r| r.workdir())
        .map(|p| p.to_path_buf());

    let Some(repo_path) = repo_path else {
        app.data.base.error_message = Some("No git repository found".to_string());
        app.transition_to(AppState::Error);
        return Ok(());
    };

    // Spawn async task for AI generation
    let tx_progress = tx.clone();
    let repo_path_display = repo_path.clone(); // For error messages
    let handle = tokio::spawn(async move {
        if tx_progress
            .send(Event::GenerationProgress {
                stage: "Analyzing repository...".to_string(),
                generation_id,
            })
            .await
            .is_err()
        {
            return;
        }

        // Perform heavy git operations in background using spawn_blocking
        // This prevents blocking the main thread/UI while reading large diffs
        let diff_result = tokio::task::spawn_blocking(move || {
            // Re-open repository in background thread. This is necessary because:
            //
            // 1. git2::Repository is NOT Send - cannot be shared across threads
            // 2. spawn_blocking runs on a separate thread pool (blocking thread pool)
            // 3. The repository must be opened in the worker thread that uses it
            //
            // Reopening can fail if:
            // - Repository was moved or deleted
            // - Permissions changed
            // - Network mount disconnected
            // - .git directory corrupted
            let repo = git2::Repository::open(&repo_path)
                .map_err(|e| GitError::Git(format!("Failed to open repository: {}", e)))?;

            // Validate for commit
            if repo.state() != git2::RepositoryState::Clean {
                return Err(GitError::Git(format!(
                    "Repository is in {:?} state",
                    repo.state()
                )));
            }

            // Get staged diff as string
            let mut index = repo
                .index()
                .map_err(|e| GitError::Git(format!("Failed to get index: {}", e)))?;
            let oid = index
                .write_tree()
                .map_err(|e| GitError::Git(format!("Failed to write tree: {}", e)))?;
            let tree = repo
                .find_tree(oid)
                .map_err(|e| GitError::Git(format!("Failed to find tree: {}", e)))?;

            let head = repo
                .head()
                .ok()
                .and_then(|h| h.target())
                .and_then(|oid| repo.find_commit(oid).ok());
            let parent_tree = head.as_ref().and_then(|c| c.tree().ok());

            let diff = repo
                .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
                .map_err(|e| GitError::Git(format!("Failed to create diff: {}", e)))?;

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
            .map_err(|e| GitError::Git(format!("Failed to format diff: {}", e)))?;

            if diff_string.is_empty() {
                return Err(GitError::Git("No staged changes to process".to_string()));
            }

            Ok(diff_string)
        })
        .await;

        let staged_diff = match diff_result {
            Ok(Ok(diff)) => diff,
            Ok(Err(e)) => {
                // Downcast and preserve specific error variants
                let error_msg = match &e {
                    GitError::NotFound => {
                        format!(
                            "Repository no longer accessible at {:?}. It may have been moved or deleted.",
                            repo_path_display
                        )
                    }
                    GitError::Locked => {
                        "Repository is locked (git operation in progress). Wait and try again."
                            .to_string()
                    }
                    GitError::AuthFailed => {
                        "Authentication failed - check your Git credentials.".to_string()
                    }
                    GitError::Other(msg) => msg.clone(),
                    GitError::Git(msg) => msg.clone(),
                    GitError::GpgConfigInvalid(msg) => msg.clone(),
                    GitError::GpgSigningFailed(msg) => msg.clone(),
                };
                if tx_progress
                    .send(Event::GenerationError {
                        error: error_msg,
                        generation_id,
                    })
                    .await
                    .is_err()
                {
                    return;
                }
                return;
            }
            Err(e) => {
                if tx_progress
                    .send(Event::GenerationError {
                        error: format!("Task panicked: {}", e),
                        generation_id,
                    })
                    .await
                    .is_err()
                {
                    return;
                }
                return;
            }
        };

        match generate_commit_message_with_progress(
            config,
            staged_diff,
            tx_progress.clone(),
            generation_id,
            user_context,
        )
        .await
        {
            Ok(result) => {
                let warning_summary = result.warning_summary();
                let _ = tx
                    .send(Event::GenerationComplete {
                        message: result.message,
                        warning_summary,
                        generation_id,
                    })
                    .await;
            }
            Err(e) => {
                let error_msg = format_error_message(&e);
                let _ = tx
                    .send(Event::GenerationError {
                        error: error_msg,
                        generation_id,
                    })
                    .await;
            }
        }
    });

    // Wrap in AbortOnDrop to ensure task is aborted on state transition
    app.generation_state = GenerationState::Running {
        task: AbortOnDrop(handle),
        generation_id,
    };

    Ok(())
}
