//! JSON schema generator for config files.
//!
//! WHY standalone binary: keeps schema generation out of runtime code paths and
//! allows CI or contributors to refresh schema deterministically.

#![allow(unused_crate_dependencies)]
use anyhow::{Context, Result};
use christina_core::ConfigFile;
use schemars::schema_for;
use std::env;
use std::fs;
use std::path::PathBuf;

// Suppress unused crate dependency warnings inherited from the library; the schema
// macro expands to types that may not be referenced directly in this binary.
use compact_str as _;
use regex as _;
use serde as _;
use thiserror as _;
use tracing as _;
use url as _;
use zeroize as _;

const SCHEMA_FILE_NAME: &str = "config.schema.json";

fn main() -> Result<()> {
    let schema = schema_for!(ConfigFile);
    let schema_json =
        serde_json::to_string_pretty(&schema).context("Failed to serialize JSON schema")?;

    let out_path = output_path(env::args().nth(1))?;

    println!("Generating JSON schema to {}...", out_path.display());
    fs::write(&out_path, schema_json)
        .with_context(|| format!("Failed to write schema file to {}", out_path.display()))?;
    println!("Done!");
    Ok(())
}

fn output_path(arg: Option<String>) -> Result<PathBuf> {
    let mut path = match arg {
        Some(path) => PathBuf::from(path),
        None => default_schema_path()?,
    };

    if path.is_dir() {
        path.push(SCHEMA_FILE_NAME);
    }

    Ok(path)
}

fn default_schema_path() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir
        .parent()
        .context("Failed to resolve project root directory")?;

    Ok(project_root.join(SCHEMA_FILE_NAME))
}
