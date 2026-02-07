//! CLI entrypoint for the christina application.
//!
//! WHY allocator switching: use dhat when profiling heap usage; otherwise cap
//! mimalloc to guard against pathological allocations.

// Allow unwrap(), expect(), and panic!() in test code
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
// Allow unused dev-dependencies in binary tests
#![allow(unused_crate_dependencies)]

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
pub static GLOBAL: Cap<MiMalloc> = Cap::new(MiMalloc, 1024 * 1024 * 1024);

use anyhow::Result;
use clap::Parser;

// Satisfy unused_crate_dependencies lint for CLI UI crates
use console as _;
use dialoguer as _;
use indicatif as _;

mod cli;
mod config;
mod engines;
mod generate;
mod git;
mod orchestrator;
mod telemetry;
mod ui;

use git2 as _;

use clap::CommandFactory;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> std::process::ExitCode {
    #[cfg(feature = "dhat-heap")]
    // Alloc profiler is opt-in to avoid overhead in normal runs.
    let _profiler = dhat::Profiler::new_heap();

    let cli = Cli::parse();

    telemetry::init(cli.verbose);

    let result: Result<()> = match cli.command {
        Some(Commands::Config(cmd)) => cli::config::handle_config_command(cmd),
        Some(Commands::Profile(cmd)) => cli::profile::handle_profile_command(cmd),
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "christina", &mut std::io::stdout());
            Ok(())
        }
        None => cli::commit::run(cli.yes, cli.context.as_deref(), cli.dry_run, cli.trace).await,
    };

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            ui::print_error(&format!("{err}"));
            std::process::ExitCode::FAILURE
        }
    }
}
