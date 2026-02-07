// Allow unused dev-dependencies that are only used in benchmarks
// The unused_crate_dependencies lint cannot distinguish between
// dependencies used in benches vs lib.
#![allow(unused_crate_dependencies)]
// Allow unwrap(), expect(), and panic!() in test code
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

/// Christina library modules exposed for benchmarking and testing.
///
/// Internal modules for the christina CLI application.
/// These are exposed primarily for integration tests and benchmarks.
pub mod git;

// Placeholder uses for dependencies used in main.rs but not lib.rs
// This satisfies clippy's unused-crate-dependencies lint
use cap as _;
use clap as _;
use clap_complete as _;
use config as _;
use console as _;
#[cfg(feature = "dhat-heap")]
use dhat as _;
use dialoguer as _;
use directories as _;
use fs2 as _;
use indicatif as _;
#[cfg(feature = "keyring-support")]
use keyring as _;
use mimalloc as _;
use tokio as _;
use toml as _;
use tracing as _;
use tracing_appender as _;
use tracing_subscriber as _;

// Placeholder uses for dev-dependencies used in tests and benchmarks
#[cfg(test)]
use christina_core as _;
#[cfg(test)]
use criterion as _;
