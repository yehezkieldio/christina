#![allow(unused_crate_dependencies)]
use anyhow::{Context, Result};
use christina_core::ConfigFile;
use schemars::schema_for;
use std::env;
use std::fs;
use std::path::PathBuf;

// Suppress unused crate dependency warnings inherited from the library
use compact_str as _;
use regex as _;
use serde as _;
use thiserror as _;
use tracing as _;
use url as _;
use zeroize as _;

fn main() -> Result<()> {
    let schema = schema_for!(ConfigFile);
    let schema_json =
        serde_json::to_string_pretty(&schema).context("Failed to serialize JSON schema")?;

    let mut out_path = if let Some(arg) = env::args().nth(1) {
        PathBuf::from(arg)
    } else {
        // Default to config.schema.json in the project root
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_root = manifest_dir
            .parent()
            .context("Failed to resolve project root directory")?;
        project_root.join("config.schema.json")
    };

    // If out_path is a directory, append the default filename
    if out_path.is_dir() {
        out_path.push("config.schema.json");
    }

    println!("Generating JSON schema to {}...", out_path.display());
    fs::write(&out_path, schema_json)
        .with_context(|| format!("Failed to write schema file to {}", out_path.display()))?;
    println!("Done!");
    Ok(())
}
