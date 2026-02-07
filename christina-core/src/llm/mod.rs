//! LLM request/response domain types and provider specifications.
//!
//! WHY in core: keeps the wire shapes and provider metadata stable across CLI,
//! orchestration, and testing without pulling in any HTTP client dependencies.

pub mod provider_spec;
pub mod request;

pub use provider_spec::{ProviderEndpoint, ProviderSpec};
pub use request::{ChatMessage, LlmRequest, LlmResponse, Role};
