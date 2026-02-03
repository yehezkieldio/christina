#[cfg(not(feature = "dhat-heap"))]
use mimalloc::MiMalloc;

#[cfg(not(feature = "dhat-heap"))]
use cap::Cap;

// Satisfy unused_crate_dependencies lint - these are used via global_allocator
#[cfg(feature = "dhat-heap")]
use cap as _;
#[cfg(feature = "dhat-heap")]
use mimalloc as _;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(not(feature = "dhat-heap"))]
#[global_allocator]
static GLOBAL: Cap<MiMalloc> = Cap::new(MiMalloc, usize::MAX);

use anyhow::Result;
use clap::Parser;
use std::panic::{AssertUnwindSafe, catch_unwind};
use tokio::sync::mpsc;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use tracing_appender::rolling;

// Satisfy unused_crate_dependencies lint for CLI UI crates
use console as _;
use dialoguer as _;
use indicatif as _;

mod app;
mod bootstrap;
mod cli;
mod config;
mod event_loop;
mod generate;
mod io;
mod tui;

use git2 as _;

use app::App;
use bootstrap::TerminalHandle;
use cli::{Cli, Commands};
use event_loop::{events::Event, run_event_loop};

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };

    // Get log directory using same pattern as config
    let log_dir = directories::ProjectDirs::from("", "", "christina")
        .map(|dirs| dirs.data_dir().join("logs"))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // Create log directory if it doesn't exist
    let _ = std::fs::create_dir_all(&log_dir);

    // Daily rolling file appender (TUI-safe - no stdout)
    let file_appender = rolling::daily(&log_dir, "christina.log");

    // Build subscriber with env filter (RUST_LOG takes precedence)
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level.to_string()));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(file_appender).with_ansi(false))
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let cli = Cli::parse();

    init_tracing(cli.verbose);

    match cli.command {
        Some(Commands::Config(cmd)) => {
            config::cli::handle_config_command(cmd)?;
            Ok(())
        }
        Some(Commands::Profile(cmd)) => {
            config::profile_cli::handle_profile_command(cmd)?;
            Ok(())
        }
        None => {
            if cli.tui {
                run_tui().await
            } else {
                cli::commit::run(cli.yes, cli.context).await
            }
        }
    }
}

async fn run_tui() -> Result<()> {
    let mut terminal = TerminalHandle::init()?;
    let mut app = App::new();

    if let Err(e) = app.validate_configuration() {
        terminal.cleanup(Some(format!("Configuration error: {}", e)))?;
        anyhow::bail!("Configuration validation failed: {}", e);
    }

    let (tx, rx) = mpsc::channel::<Event>(100);

    // Run event loop and ensure terminal cleanup even on panic
    // Use AssertUnwindSafe since App contains RefCell-like structures that are
    // safe to unwind through in this single-threaded TUI context
    let event_loop_result = {
        let result = catch_unwind(AssertUnwindSafe(|| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(async { run_event_loop(&mut app, &mut terminal, rx, tx).await })
            })
        }));

        // Always cleanup terminal state, even on panic
        let exit_msg = app.exit_message.clone();
        terminal.cleanup(exit_msg)?;

        result
    };

    // Propagate panic or event loop error
    match event_loop_result {
        Ok(event_loop_result) => event_loop_result,
        Err(panic_payload) => {
            // WHY downcast attempts: Rust panics use `Any` trait for payload, which can be
            // &str (panic!("message")), String (panic!(format!(...)), or arbitrary types.
            // We attempt both common string types to provide actionable error messages.
            // Without this, users would only see "unknown panic" which hides the root cause.
            let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };

            Err(anyhow::anyhow!("Event loop panicked: {}", panic_msg))
        }
    }
}
