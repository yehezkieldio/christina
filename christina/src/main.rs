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

use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use tracing_appender::rolling;

// Satisfy unused_crate_dependencies lint for CLI UI crates
use console as _;
use dialoguer as _;
use indicatif as _;

mod cli;
mod config;
mod events;
mod generate;
mod io;

use git2 as _;

use cli::{Cli, Commands};
use clap::CommandFactory;

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
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "christina", &mut std::io::stdout());
            Ok(())
        }
        None => cli::commit::run(cli.yes, cli.context.as_deref(), cli.dry_run).await,
    }
}

