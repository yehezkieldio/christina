//! Abstract AI backend interface.
//!
//! The core only cares that something can turn a request into a result.
//! Concrete implementations live in the `christina` crate.

use crate::error::ProviderError;
use crate::llm::request::LlmRequest;

/// Abstract interface for AI generation backends.
///
/// Implementations handle the concrete details of communicating with
/// specific LLM providers (Azure, etc.).
pub trait LlmBackend: Send + Sync {
    /// Generate a response from the given LLM request.
    fn generate(
        &self,
        request: LlmRequest,
    ) -> impl std::future::Future<Output = Result<String, ProviderError>> + Send;
}
