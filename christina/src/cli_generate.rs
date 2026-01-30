use std::io::Write;
use std::process::Command;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use christina_core::types::CommitMessage;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::{Select, Text};

use crate::config::Config;
use crate::generate::config_to_profile;
use christina_core::prompt::{DIRECT_COMMIT_PROMPT, SYSTEM_PROMPT};
use christina_git::repository::GitRepository;
use christina_llm::{AIOrchestrator, Provider, TokenBudget, get_tokenizer};

// Use crossterm to avoid unused dependency warning
#[allow(unused_imports)]
use crossterm as _;

/// Run the CLI generation flow
pub async fn run_cli_generate(config: &Config, context: Option<String>) -> Result<()> {
    let repo = GitRepository::discover().context("Failed to open git repository")?;

    // Check for staged changes
    if !repo.has_staged_changes()? {
        println!("No staged changes found. Stage some files first with 'git add'.");
        return Ok(());
    }

    // Get optional user context
    let user_context = if context.is_some() {
        context
    } else if config.cli_prompt_context {
        let prompt = Text::new("Additional context (optional):")
            .with_help_message("Provide context to help generate a better commit message")
            .prompt()
            .ok();
        prompt.filter(|s| !s.trim().is_empty())
    } else {
        None
    };

    // Generate commit message with spinner
    let message = generate_with_spinner(config, &repo, user_context.clone()).await?;

    // Show success with checkmark (TUI style)
    display_success(&message);

    // Interactive loop for use/edit/regenerate/exit
    loop {
        let options = vec![
            "Use",
            "Edit",
            "Editor", 
            "Regenerate",
            "Exit",
        ];

        let selection = Select::new("What would you like to do?", options)
            .with_help_message("Use: commit with this message | Edit: edit inline | Editor: open in $EDITOR | Regenerate: generate new | Exit: cancel")
            .prompt();

        match selection {
            Ok(choice) => match choice {
                "Use" => {
                    let commit_msg = CommitMessage::try_from(message.clone())
                        .map_err(|e| anyhow!("Invalid commit message: {}", e))?;
                    let oid = repo.create_commit(&commit_msg)
                        .context("Failed to create commit")?;
                    display_commit_created(&oid.to_string());
                    break;
                }
                "Edit" => {
                    let edited = edit_inline(&message)?;
                    match validate_and_update(&edited, config) {
                        Ok(new_msg) => {
                            display_success(&new_msg);
                            return handle_edited_message(&new_msg, config, &repo).await;
                        }
                        Err(e) => {
                            eprintln!("Invalid message: {}", e);
                            continue;
                        }
                    }
                }
                "Editor" => {
                    let edited = edit_in_editor(&message)?;
                    match validate_and_update(&edited, config) {
                        Ok(new_msg) => {
                            display_success(&new_msg);
                            return handle_edited_message(&new_msg, config, &repo).await;
                        }
                        Err(e) => {
                            eprintln!("Invalid message: {}", e);
                            continue;
                        }
                    }
                }
                "Regenerate" => {
                    print!("\r◐ Regenerating...");
                    std::io::stdout().flush()?;
                    let new_msg = generate_with_spinner(config, &repo, user_context.clone()).await?;
                    print!("\r                    \r"); // Clear the line
                    display_success(&new_msg);
                    continue;
                }
                "Exit" => {
                    println!("Cancelled.");
                    break;
                }
                _ => unreachable!(),
            },
            Err(_) => {
                println!("Cancelled.");
                break;
            }
        }
    }

    Ok(())
}

/// Generate commit message with a spinner
async fn generate_with_spinner(
    config: &Config,
    repo: &GitRepository,
    user_context: Option<String>,
) -> Result<String> {
    let pb = ProgressBar::new_spinner();
    let style = ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .expect("valid progress template")
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
    pb.set_style(style);
    pb.set_message("Generating commit message...");
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let result = generate_message(config, repo, user_context).await;

    pb.finish_and_clear();

    result
}

/// Generate the commit message using the LLM
async fn generate_message(
    config: &Config,
    repo: &GitRepository,
    user_context: Option<String>,
) -> Result<String> {
    let staged_diff = repo.get_staged_diff()?;
    
    if staged_diff.is_empty() {
        return Err(anyhow!("No staged changes to generate commit message from"));
    }

    let diff = staged_diff.to_string()?;
    
    let api_key = match config.api_key {
        Some(ref key) => key.clone(),
        None => {
            anyhow::bail!("API key not found in configuration");
        }
    };

    let provider = Provider::from_profile(&config_to_profile(config), &api_key)?;
    let provider = Arc::new(provider);

    let tokenizer = get_tokenizer()?;

    // Dynamic token budgeting
    let system_prompt_tokens = tokenizer.count_tokens(SYSTEM_PROMPT);
    let direct_prompt_tokens = tokenizer.count_tokens(DIRECT_COMMIT_PROMPT);
    let reserved_for_prompt = system_prompt_tokens.max(direct_prompt_tokens);
    let reserved_for_messages = christina_core::types::TokenCount::new_saturating(500);

    let budget = TokenBudget::new(
        config.max_input_tokens,
        config.max_output_tokens,
        reserved_for_prompt,
        reserved_for_messages,
    );
    let token_limit = budget
        .remaining_for_diff()
        .map_err(|e| anyhow::anyhow!("Invalid token budget configuration: {}", e))?;

    let processor = christina_git::DiffProcessor::new(tokenizer, token_limit)
        .with_ignore_files(config.ignore_files.clone());

    let chunks = processor
        .process_safe(&diff)
        .map_err(|e| anyhow::anyhow!("Diff processing error: {}", e))?;

    if chunks.is_empty() {
        anyhow::bail!("No processable diff content found");
    }

    let orchestrator = AIOrchestrator::new(Arc::clone(&provider));

    // Get commit history if enabled
    let history_context = if config.use_commit_history {
        match repo.get_commit_history(config.commit_history_depth) {
            Ok(mut commits) => {
                if commits.is_empty() {
                    None
                } else {
                    let budget_limit =
                        orchestrator.calculate_history_budget(config.max_input_tokens.get());
                    let original_count = commits.len();
                    commits.truncate(budget_limit);

                    if commits.len() < original_count {
                        eprintln!(
                            "Truncated commit history from {} to {} commits to fit token budget",
                            original_count,
                            commits.len()
                        );
                    }

                    let formatted = commits
                        .iter()
                        .map(|c| format!("- {}: {}", c.sha, c.subject))
                        .collect::<Vec<_>>()
                        .join("\n");
                    Some(format!("Recent commits:\n{}", formatted))
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to retrieve commit history: {}", e);
                None
            }
        }
    } else {
        None
    };

    let result = orchestrator
        .generate_commit_message(
            chunks,
            user_context.as_deref(),
            config.commit_message_validation_mode,
            config.commit_message_max_length,
            history_context,
        )
        .await?;

    Ok(result.message.as_str().to_string())
}

/// Display success message with checkmark (TUI style)
fn display_success(message: &str) {
    println!("\x1b[32m✓\x1b[0m {}", message);
}

/// Display commit created message
fn display_commit_created(oid: &str) {
    let short_oid = &oid[..oid.len().min(7)];
    println!("\x1b[32m✓\x1b[0m Created commit \x1b[33m{}\x1b[0m", short_oid);
}

/// Edit message inline
fn edit_inline(message: &str) -> Result<String> {
    let edited = Text::new("Edit commit message:")
        .with_initial_value(message)
        .with_validator(|input: &str| {
            if input.trim().is_empty() {
                Ok(inquire::validator::Validation::Invalid("Message cannot be empty".into()))
            } else {
                Ok(inquire::validator::Validation::Valid)
            }
        })
        .prompt()
        .map_err(|e| anyhow!("Failed to get input: {}", e))?;

    Ok(edited)
}

/// Edit message in $EDITOR
fn edit_in_editor(message: &str) -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    
    let mut temp_file = tempfile::NamedTempFile::new()
        .context("Failed to create temp file")?;
    temp_file.write_all(message.as_bytes())
        .context("Failed to write to temp file")?;
    temp_file.flush()?;

    let status = Command::new(&editor)
        .arg(temp_file.path())
        .status()
        .with_context(|| format!("Failed to open editor: {}", editor))?;

    if !status.success() {
        return Err(anyhow!("Editor exited with non-zero status"));
    }

    let edited = std::fs::read_to_string(temp_file.path())
        .context("Failed to read edited message")?;

    Ok(edited.trim().to_string())
}

/// Validate and update the commit message
fn validate_and_update(message: &str, config: &Config) -> Result<String> {
    let (commit_msg, warnings) = CommitMessage::validate(
        message.to_string(),
        config.commit_message_validation_mode,
        config.commit_message_max_length,
    ).map_err(|e| anyhow!("Validation failed: {}", e))?;

    for warning in warnings {
        eprintln!("Warning: {}", warning);
    }

    Ok(commit_msg.as_str().to_string())
}

/// Handle the edited message flow
async fn handle_edited_message(
    message: &str,
    _config: &Config,
    repo: &GitRepository,
) -> Result<()> {
    let options = vec![
        "Use",
        "Exit",
    ];

    let selection = Select::new("What would you like to do?", options)
        .with_help_message("Use: commit with this message | Exit: cancel without committing")
        .prompt();

    match selection {
        Ok(choice) => match choice {
            "Use" => {
                let commit_msg = CommitMessage::try_from(message.to_string())
                    .map_err(|e| anyhow!("Invalid commit message: {}", e))?;
                let oid = repo.create_commit(&commit_msg)
                    .context("Failed to create commit")?;
                display_commit_created(&oid.to_string());
            }
            _ => println!("Cancelled."),
        },
        Err(_) => println!("Cancelled."),
    }

    Ok(())
}
