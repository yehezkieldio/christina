// Allow unused dev-dependencies that are only used in benchmarks
// The unused_crate_dependencies lint cannot distinguish between
// dependencies used in benches vs lib.
#![allow(unused_crate_dependencies)]

// Allow unwrap(), expect(), and panic!() in test code
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod config;
pub mod error;
pub mod git;
pub mod ids;
pub mod llm;
pub mod profile;
pub mod prompt;
#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers;
pub mod tokenizer;
pub mod types;

pub use config::{
    AzureEndpoint, AzureEndpointError, ConfigFile, ResolvedConfig, Secret, SecretError, SecretRef,
    SecretString,
};
pub use error::{
    AppError, CompletionError, DiffError, DiffResult, ErrorCategory, GitError, GitResult,
    ProviderError, TokenizerError, TokenizerResult,
};
pub use git::{GitFile, GitFileStatus, RepoRoot, RepoRootError};
pub use ids::GenerationId;
pub use llm::{ChatMessage, LlmRequest, LlmResponse, ProviderEndpoint, ProviderSpec, Role};
pub use profile::{Profiles, ProviderProfile};
pub use tokenizer::Tokenizer;
