//! Core domain crate for Christina.
//!
//! This crate is intentionally **headless**: no CLI, no IO orchestration, and no
//! runtime secret resolution. It provides the stable domain types, processing
//! pipeline, and prompt construction primitives that the `christina` binary wires
//! together. Public re-exports below form the supported API surface for the app.

// Allow unused dev-dependencies that are only used in benchmarks.
// The unused_crate_dependencies lint cannot distinguish between
// dependencies used in benches vs lib.
#![allow(unused_crate_dependencies)]
// Allow unwrap(), expect(), and panic!() in test code
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod config;
pub mod error;
pub mod git;
pub mod llm;
pub mod pipeline;
pub mod processing;
pub mod profile;
pub mod prompt;
#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers;
pub mod tokenizer;
pub mod types;

pub use config::{
    AdvancedConfig, AzureEndpoint, AzureEndpointError, ConfigFile, ExperimentalConfig,
    ResolvedConfig, Secret, SecretRef, SecretString, StandardConfig,
};
pub use error::{
    AppError, CompletionError, DiffError, DiffResult, ErrorCategory, GitError, GitResult,
    ProviderError, TokenizerError, TokenizerResult,
};
pub use git::{GitFile, GitFileStatus, RepoRoot, RepoRootError};
pub use llm::{
    ChatMessage, LlmRequest, LlmResponse, ProviderEndpoint, ProviderSpec, Role,
    StructuredOutputFormat,
};
pub use pipeline::LlmBackend;
pub use profile::{Profiles, ProviderProfile};
pub use tokenizer::Tokenizer;
pub use types::backend_id::GenerationId;
