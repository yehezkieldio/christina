//! Git integration layer for the CLI crate.
//!
//! Provides staged diff extraction and parsing atop git2.

pub mod adapter;
pub mod diff_processor;
pub mod parsing;
