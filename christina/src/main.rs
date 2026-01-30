#[cfg(not(feature = "dhat-heap"))]
use mimalloc::MiMalloc;

#[cfg(not(feature = "dhat-heap"))]
use cap::Cap;

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

mod app;
mod bootstrap;
mod cli;
mod config;
mod event_loop;
mod generate;
mod tui;

use app::App;
use bootstrap::TerminalHandle;
use cli::{Cli, Commands};
use event_loop::{events::Event, run_event_loop};

#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config) => config::cli::handle_config_command(),
        None => run_tui().await,
    }
}

async fn run_tui() -> Result<()> {
    let mut terminal = TerminalHandle::init()?;
    let mut app = App::new();
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
            // Try to extract panic message for better error reporting
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
