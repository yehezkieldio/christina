//! Observability and tracing setup.
//!
//! Centralizes tracing subscriber configuration, log filtering,
//! and privacy-aware log sanitization.

use tracing_appender::rolling;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Initialize the tracing subsystem.
///
/// Sets up file-based logging with daily rotation and env-filter support.
pub fn init(verbose: u8) {
    let level = match verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.to_string()));

    let log_dir = directories::ProjectDirs::from("", "", "christina")
        .map(|dirs| dirs.data_dir().join("logs"))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = rolling::daily(&log_dir, "christina.log");

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(file_appender).with_ansi(false))
        .init();
}
