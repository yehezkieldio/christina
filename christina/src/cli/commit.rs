use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use git2::Repository;
use tokio::sync::mpsc;

use crate::cli::ui;
use crate::config::Config;
use crate::events::Event;
use crate::generate::generate_commit_message_with_progress;
use crate::io::git::adapter;
use christina_core::GitFile;

pub async fn run(yes: bool, context: Option<&str>, dry_run: bool, trace: bool) -> Result<()> {
    let trace_stats = trace.then(|| Arc::new(Mutex::new(TraceStats::new(dry_run))));
    ui::print_header();
    ui::print_divider();

    if trace {
        ui::print_trace("validating repository state");
    }
    let (repo, diff) = validate_repository()?;

    let repo_path = repo
        .workdir()
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.path().to_path_buf());

    if let Some(stats) = trace_stats.as_ref() {
        let mut stats = match stats.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        stats.repo_path = Some(repo_path.clone());
    }

    if trace {
        ui::print_trace("collecting staged files");
    }
    let files = adapter::get_staged_files(&repo)?;

    if let Some(stats) = trace_stats.as_ref() {
        let mut stats = match stats.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        stats.staged_files = files.len();
    }

    display_changes(&files);

    if trace {
        if let Some(stats) = trace_stats.as_ref() {
            let mut stats = match stats.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let diff_stats = compute_diff_stats(&diff);
            stats.diff_bytes = diff.len();
            stats.diff_lines = diff_stats.lines_total;
            stats.diff_additions = diff_stats.additions;
            stats.diff_deletions = diff_stats.deletions;
        }
        ui::print_trace(&format!(
            "repository: {}",
            repo_path.to_string_lossy()
        ));
        ui::print_trace(&format!("staged files: {}", files.len()));
        ui::print_trace(&format!("diff bytes: {}", diff.len()));
    }

    if trace {
        ui::print_trace("starting commit message generation");
    }
    let message = loop {
        let message = match generate_commit(
            diff.clone(),
            context.map(|s| s.to_string()),
            repo_path.clone(),
            trace,
            trace_stats.as_ref().cloned(),
        )
        .await
        {
            Ok(message) => message,
            Err(err) => return Err(err),
        };

        if trace {
            ui::print_trace("awaiting commit confirmation");
        }
        let action = confirm_commit(&message, yes)?;

        match action {
            CommitAction::Accept => break message,
            CommitAction::Edit => {
                match ui::edit_in_editor(&message) {
                    Ok(edited) => break edited,
                    Err(err) => {
                        ui::print_warning(&format!("Editor failed: {err}"));
                        continue;
                    }
                }
            }
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
        if trace {
            print_trace_summary(trace_stats.as_ref());
        }
        return Ok(());
    }

    if trace {
        ui::print_trace("creating commit");
    }
    if let Err(err) = execute_commit(&repo, &message) {
        if is_gpg_signing_failure(&err) {
            ui::print_warning(
                "GPG signing failed. Configure your GPG key/agent or disable signing with: git config commit.gpgsign false",
            );
        }
        return Err(err);
    }

    if let Some(stats) = trace_stats.as_ref() {
        let mut stats = match stats.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        stats.commit_created = true;
    }

    if trace {
        print_trace_summary(trace_stats.as_ref());
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

async fn generate_commit(
    diff: String,
    context: Option<String>,
    repo_path: PathBuf,
    trace: bool,
    trace_stats: Option<Arc<Mutex<TraceStats>>>,
) -> Result<String> {
    let spinner = ui::create_spinner("Analyzing changes...");
    let config = Config::load_async().await?;

    if trace {
        ui::print_trace("loading configuration");
    }

    if let Some(stats) = trace_stats.as_ref() {
        let mut stats = match stats.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        stats.record_config(&config);
        stats.generation_started = Some(Instant::now());
    }

    let (progress_tx, mut _progress_rx) = mpsc::channel::<Event>(100);
    let _progress_spinner = spinner.clone();
    let trace_stats_handle = trace_stats.clone();
    let trace_enabled = trace;
    let progress_handle = tokio::spawn(async move {
        while let Some(event) = _progress_rx.recv().await {
            match event {
                Event::GenerationProgress { stage, .. } => {
                    let stage_for_trace = stage.clone();
                    _progress_spinner.set_message(stage);
                    if trace_enabled {
                        ui::print_trace(&format!("stage: {}", stage_for_trace));
                    }
                }
                Event::TokenCountUpdate { token_count } => {
                    if let Some(stats) = trace_stats_handle.as_ref() {
                        let mut stats = match stats.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        stats.token_count = Some(token_count);
                    }
                    if trace_enabled {
                        ui::print_trace(&format!("token count: {}", token_count.get()));
                    }
                }
                Event::DiffChunked {
                    chunk_count,
                    binary_only,
                } => {
                    if let Some(stats) = trace_stats_handle.as_ref() {
                        let mut stats = match stats.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        stats.chunk_count = Some(chunk_count);
                        stats.binary_only = Some(binary_only);
                    }
                    if trace_enabled {
                        ui::print_trace(&format!(
                            "diff chunks: {} (binary only: {})",
                            chunk_count, binary_only
                        ));
                    }
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
        if let Some(stats) = trace_stats.as_ref() {
            let mut stats = match stats.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            stats.warnings.push(warning);
        }
    }

    if let Some(stats) = trace_stats.as_ref() {
        let mut stats = match stats.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        stats.generation_completed = Some(Instant::now());
        stats.message_length = Some(generation_result.message.to_string().len());
    }

    Ok(generation_result.message.to_string())
}

fn is_gpg_signing_failure(err: &anyhow::Error) -> bool {
    err.to_string().to_lowercase().contains("gpg signing failed")
}

enum CommitAction {
    Accept,
    Edit,
    Regenerate,
    Decline,
}

fn confirm_commit(message: &str, yes: bool) -> Result<CommitAction> {
    ui::print_section("Proposed");
    ui::print_commit_message(message);

    if yes {
        return Ok(CommitAction::Accept);
    }

    let actions = ["accept", "edit", "regenerate", "decline"];
    let selection = ui::select_action(&actions)
        .map_err(|err| anyhow::anyhow!("Confirmation failed: {}", err))?;

    let action = match selection {
        0 => CommitAction::Accept,
        1 => CommitAction::Edit,
        2 => CommitAction::Regenerate,
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

#[derive(Debug, Default, Clone, Copy)]
struct DiffStats {
    lines_total: usize,
    additions: usize,
    deletions: usize,
}

fn compute_diff_stats(diff: &str) -> DiffStats {
    let mut stats = DiffStats::default();
    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            stats.lines_total += 1;
            continue;
        }
        if line.starts_with('+') {
            stats.additions += 1;
            stats.lines_total += 1;
            continue;
        }
        if line.starts_with('-') {
            stats.deletions += 1;
            stats.lines_total += 1;
            continue;
        }
        stats.lines_total += 1;
    }
    stats
}

#[derive(Debug)]
struct TraceStats {
    started_at: Instant,
    dry_run: bool,
    repo_path: Option<PathBuf>,
    staged_files: usize,
    diff_bytes: usize,
    diff_lines: usize,
    diff_additions: usize,
    diff_deletions: usize,
    chunk_count: Option<usize>,
    binary_only: Option<bool>,
    token_count: Option<christina_core::types::tokens::TokenCount>,
    generation_started: Option<Instant>,
    generation_completed: Option<Instant>,
    message_length: Option<usize>,
    commit_created: bool,
    warnings: Vec<String>,
    provider: Option<christina_core::types::ProviderKind>,
    model: Option<christina_core::types::ModelName>,
    max_input_tokens: Option<christina_core::types::tokens::TokenCount>,
    max_output_tokens: Option<christina_core::types::tokens::TokenCount>,
    usage_tier: Option<christina_core::types::UsageTier>,
    use_commit_history: Option<bool>,
    commit_history_depth: Option<usize>,
    max_concurrent_requests: Option<usize>,
    max_partial_failure_rate: Option<f64>,
}

impl TraceStats {
    fn new(dry_run: bool) -> Self {
        Self {
            started_at: Instant::now(),
            dry_run,
            repo_path: None,
            staged_files: 0,
            diff_bytes: 0,
            diff_lines: 0,
            diff_additions: 0,
            diff_deletions: 0,
            chunk_count: None,
            binary_only: None,
            token_count: None,
            generation_started: None,
            generation_completed: None,
            message_length: None,
            commit_created: false,
            warnings: Vec::new(),
            provider: None,
            model: None,
            max_input_tokens: None,
            max_output_tokens: None,
            usage_tier: None,
            use_commit_history: None,
            commit_history_depth: None,
            max_concurrent_requests: None,
            max_partial_failure_rate: None,
        }
    }

    fn record_config(&mut self, config: &Config) {
        self.provider = Some(config.model_provider);
        self.model = Some(config.model.clone());
        self.max_input_tokens = Some(config.max_input_tokens);
        self.max_output_tokens = Some(config.max_output_tokens);
        self.usage_tier = Some(config.usage_tier);
        self.use_commit_history = Some(config.use_commit_history);
        self.commit_history_depth = Some(config.commit_history_depth);
        self.max_concurrent_requests = Some(config.max_concurrent_requests);
        self.max_partial_failure_rate = Some(config.max_partial_failure_rate);
    }
}

fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 1.0 {
        return format!("{:.0}ms", duration.as_millis());
    }
    if secs < 10.0 {
        return format!("{:.2}s", secs);
    }
    format!("{:.1}s", secs)
}

#[cfg(not(feature = "dhat-heap"))]
fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        return format!("{bytes}B");
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1}KB");
    }
    let mb = kb / 1024.0;
    format!("{mb:.1}MB")
}

fn print_trace_summary(trace_stats: Option<&Arc<Mutex<TraceStats>>>) {
    let Some(stats) = trace_stats else { return };
    let stats = match stats.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    ui::print_trace("summary: trace telemetry");

    if let Some(path) = stats.repo_path.as_ref() {
        ui::print_trace(&format!("repo: {}", path.to_string_lossy()));
    }
    ui::print_trace(&format!("staged files: {}", stats.staged_files));
    ui::print_trace(&format!("diff bytes: {}", stats.diff_bytes));
    ui::print_trace(&format!("diff lines: {}", stats.diff_lines));
    ui::print_trace(&format!(
        "diff changes: +{} -{}",
        stats.diff_additions, stats.diff_deletions
    ));

    if let Some(chunks) = stats.chunk_count {
        ui::print_trace(&format!("diff chunks: {}", chunks));
    }
    if let Some(binary_only) = stats.binary_only {
        ui::print_trace(&format!("diff binary only: {}", binary_only));
    }
    if let Some(token_count) = stats.token_count {
        ui::print_trace(&format!("token count: {}", token_count.get()));
    }

    if let Some(provider) = stats.provider {
        ui::print_trace(&format!("provider: {}", provider));
    }
    if let Some(model) = stats.model.as_ref() {
        ui::print_trace(&format!("model: {}", model));
    }
    if let Some(max_input) = stats.max_input_tokens {
        ui::print_trace(&format!("max input tokens: {}", max_input.get()));
    }
    if let Some(max_output) = stats.max_output_tokens {
        ui::print_trace(&format!("max output tokens: {}", max_output.get()));
    }
    if let Some(usage_tier) = stats.usage_tier {
        ui::print_trace(&format!("usage tier: {}", usage_tier));
    }
    if let Some(use_history) = stats.use_commit_history {
        ui::print_trace(&format!("commit history: {}", use_history));
    }
    if let Some(depth) = stats.commit_history_depth {
        ui::print_trace(&format!("commit history depth: {}", depth));
    }
    if let Some(concurrency) = stats.max_concurrent_requests {
        ui::print_trace(&format!("max concurrent requests: {}", concurrency));
    }
    if let Some(rate) = stats.max_partial_failure_rate {
        ui::print_trace(&format!("max partial failure rate: {:.2}", rate));
    }
    if let Some(length) = stats.message_length {
        ui::print_trace(&format!("message length: {}", length));
    }
    if stats.dry_run {
        ui::print_trace("commit: dry run (no commit created)");
    } else {
        ui::print_trace(&format!("commit created: {}", stats.commit_created));
    }
    if !stats.warnings.is_empty() {
        ui::print_trace(&format!("warnings: {}", stats.warnings.len()));
        for warning in &stats.warnings {
            ui::print_trace(&format!("warning: {}", warning));
        }
    }

    if let (Some(start), Some(end)) = (stats.generation_started, stats.generation_completed) {
        let duration = end.saturating_duration_since(start);
        ui::print_trace(&format!("generation time: {}", format_duration(duration)));
    }

    #[cfg(not(feature = "dhat-heap"))]
    {
        let allocated = crate::GLOBAL.allocated();
        let peak = crate::GLOBAL.max_allocated();
        let total = crate::GLOBAL.total_allocated();
        ui::print_trace(&format!(
            "memory: current {}, peak {}, total {}",
            format_bytes(allocated),
            format_bytes(peak),
            format_bytes(total),
        ));
    }

    let total_duration = stats.started_at.elapsed();
    ui::print_trace(&format!("total time: {}", format_duration(total_duration)));
}
