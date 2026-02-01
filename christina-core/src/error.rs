//! Unified error types for the Christina workspace
//!
//! This module provides a centralized error handling system that standardizes
//! error types across christina-git and christina-llm crates.

use std::fmt;
use thiserror::Error;

/// Trait for errors that can be retried
pub trait IsTransient {
    /// Returns true if this error is transient and may succeed on retry
    fn is_transient(&self) -> bool;
}

/// Primary error type for git operations
#[derive(Debug, Error)]
pub enum GitError {
    /// Generic git operation error
    #[error("Git error: {0}")]
    Git(String),

    /// Repository resource not found
    #[error("Resource not found")]
    NotFound,

    /// Authentication failed
    #[error("Authentication failed")]
    AuthFailed,

    /// Repository is locked
    #[error("Repository is locked")]
    Locked,

    /// Other git-related error with details
    #[error("Git operation failed: {0}")]
    Other(String),

    /// Invalid GPG configuration
    #[error("GPG config invalid: {0}")]
    GpgConfigInvalid(String),

    /// GPG signing operation failed
    #[error("GPG signing failed: {0}")]
    GpgSigningFailed(String),
}

impl GitError {
    /// Check if this error is related to signing operations
    pub fn is_signing_error(&self) -> bool {
        matches!(
            self,
            GitError::GpgConfigInvalid(_) | GitError::GpgSigningFailed(_)
        )
    }
}

impl IsTransient for GitError {
    fn is_transient(&self) -> bool {
        matches!(self, GitError::Locked)
    }
}

#[cfg(feature = "git2-support")]
impl From<git2::Error> for GitError {
    fn from(e: git2::Error) -> Self {
        match e.code() {
            git2::ErrorCode::NotFound => GitError::NotFound,
            git2::ErrorCode::Auth => GitError::AuthFailed,
            git2::ErrorCode::Locked => GitError::Locked,
            _ => GitError::Other(e.to_string()),
        }
    }
}

/// Result type alias for git operations
pub type GitResult<T> = std::result::Result<T, GitError>;

/// Error type for LLM completion operations
#[derive(Debug, Error)]
pub enum CompletionError {
    /// Authentication failure (invalid API key, etc.)
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Rate limit exceeded
    #[error("Rate limited")]
    RateLimited,

    /// Request timeout
    #[error("Request timed out")]
    Timeout,

    /// Server-side error (5xx responses)
    #[error("Server error: {0}")]
    ServerError(String),

    /// Network connectivity issues
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Invalid or unexpected response from API
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Unknown or unclassified error
    #[error("Unknown error: {0}")]
    UnknownError(String),
}

impl CompletionError {
    /// Check if this error is transient and may succeed on retry
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            CompletionError::RateLimited
                | CompletionError::Timeout
                | CompletionError::ServerError(_)
                | CompletionError::NetworkError(_)
        )
    }

    /// Check if this error is a systemic provider issue
    pub fn is_provider_error(&self) -> bool {
        matches!(
            self,
            CompletionError::Unauthorized(_)
                | CompletionError::RateLimited
                | CompletionError::ServerError(_)
        )
    }

    /// Parse an API error message to create appropriate error variant
    pub fn from_api_error(msg: &str) -> Self {
        let msg_lower = msg.to_lowercase();

        if msg_lower.contains("401")
            || msg_lower.contains("unauthorized")
            || msg_lower.contains("invalid api key")
        {
            CompletionError::Unauthorized(msg.to_string())
        } else if msg_lower.contains("429")
            || msg_lower.contains("rate limit")
            || msg_lower.contains("quota")
        {
            CompletionError::RateLimited
        } else if msg_lower.contains("timeout") || msg_lower.contains("timed out") {
            CompletionError::Timeout
        } else if msg_lower.contains("5")
            || msg_lower.contains("server error")
            || msg_lower.contains("overloaded")
        {
            CompletionError::ServerError(msg.to_string())
        } else if msg_lower.contains("network")
            || msg_lower.contains("connection")
            || msg_lower.contains("dns")
            || msg_lower.contains("resolve")
        {
            CompletionError::NetworkError(msg.to_string())
        } else {
            CompletionError::ServerError(msg.to_string())
        }
    }
}

impl IsTransient for CompletionError {
    fn is_transient(&self) -> bool {
        matches!(
            self,
            CompletionError::RateLimited
                | CompletionError::Timeout
                | CompletionError::ServerError(_)
                | CompletionError::NetworkError(_)
        )
    }
}

/// Error type for LLM provider configuration
#[derive(Debug, Error)]
pub enum ProviderError {
    /// Missing required configuration field
    #[error("Missing required configuration: {0}")]
    MissingConfig(String),

    /// Invalid provider configuration value
    #[error("Invalid provider configuration: {0}")]
    InvalidConfig(String),
}

impl ProviderError {
    /// Get the name of the missing or invalid configuration field
    pub fn field_name(&self) -> &str {
        match self {
            ProviderError::MissingConfig(field) => field,
            ProviderError::InvalidConfig(field) => field,
        }
    }
}

impl IsTransient for ProviderError {
    fn is_transient(&self) -> bool {
        false
    }
}

/// Error type for tokenizer operations
#[derive(Debug, Error)]
pub enum TokenizerError {
    /// General tokenizer operation failure
    #[error("Tokenizer error: {0}")]
    Tokenizer(String),

    /// Failed to load tokenizer model
    #[error("Failed to load tokenizer model: {0}")]
    ModelLoadFailed(String),

    /// Token encoding/decoding failure
    #[error("Token encoding error: {0}")]
    EncodingFailed(String),
}

impl TokenizerError {
    /// Check if this is a model loading error
    pub fn is_model_error(&self) -> bool {
        matches!(self, TokenizerError::ModelLoadFailed(_))
    }
}

impl IsTransient for TokenizerError {
    fn is_transient(&self) -> bool {
        false
    }
}

/// Result type alias for tokenizer operations
pub type TokenizerResult<T> = std::result::Result<T, TokenizerError>;

/// Unified application error that can represent any subsystem error
#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Git(#[from] GitError),

    #[error(transparent)]
    Completion(#[from] CompletionError),

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error(transparent)]
    Tokenizer(#[from] TokenizerError),

    #[error("Application error: {0}")]
    Other(String),
}

impl AppError {
    /// Check if this error is transient and may succeed on retry
    pub fn is_transient(&self) -> bool {
        match self {
            AppError::Completion(e) => e.is_transient(),
            AppError::Git(e) => matches!(e, GitError::Locked),
            _ => false,
        }
    }

    /// Check if this is a configuration-related error
    pub fn is_config_error(&self) -> bool {
        matches!(self, AppError::Provider(_))
    }

    /// Get a user-friendly error category
    pub fn category(&self) -> ErrorCategory {
        match self {
            AppError::Git(_) => ErrorCategory::Git,
            AppError::Completion(_) | AppError::Provider(_) => ErrorCategory::Llm,
            AppError::Tokenizer(_) => ErrorCategory::Tokenizer,
            AppError::Other(_) => ErrorCategory::General,
        }
    }
}

impl IsTransient for AppError {
    fn is_transient(&self) -> bool {
        match self {
            AppError::Completion(e) => e.is_transient(),
            AppError::Git(e) => e.is_transient(),
            _ => false,
        }
    }
}

/// Error categories for grouping and display purposes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Git,
    Llm,
    Tokenizer,
    General,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorCategory::Git => write!(f, "Git"),
            ErrorCategory::Llm => write!(f, "LLM"),
            ErrorCategory::Tokenizer => write!(f, "Tokenizer"),
            ErrorCategory::General => write!(f, "General"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_error_is_signing_error() {
        let err = GitError::GpgConfigInvalid("test".to_string());
        assert!(err.is_signing_error());

        let err2 = GitError::Git("test".to_string());
        assert!(!err2.is_signing_error());
    }

    #[test]
    fn completion_error_is_transient() {
        assert!(CompletionError::Timeout.is_transient());
        assert!(CompletionError::RateLimited.is_transient());
        assert!(CompletionError::ServerError("test".to_string()).is_transient());
        assert!(CompletionError::NetworkError("test".to_string()).is_transient());

        assert!(!CompletionError::Unauthorized("test".to_string()).is_transient());
        assert!(!CompletionError::InvalidResponse("test".to_string()).is_transient());
    }

    #[test]
    fn completion_error_from_api_error() {
        let err = CompletionError::from_api_error("Error 401: Unauthorized");
        assert!(matches!(err, CompletionError::Unauthorized(_)));

        let err = CompletionError::from_api_error("rate limit exceeded");
        assert!(matches!(err, CompletionError::RateLimited));

        let err = CompletionError::from_api_error("Request timeout");
        assert!(matches!(err, CompletionError::Timeout));

        let err = CompletionError::from_api_error("Server error 500");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn provider_error_field_name() {
        let err = ProviderError::MissingConfig("api_key".to_string());
        assert_eq!(err.field_name(), "api_key");
    }

    #[test]
    fn app_error_categories() {
        let git_err = AppError::from(GitError::Git("test".to_string()));
        assert_eq!(git_err.category(), ErrorCategory::Git);

        let llm_err = AppError::from(CompletionError::Timeout);
        assert_eq!(llm_err.category(), ErrorCategory::Llm);

        let other_err = AppError::Other("test".to_string());
        assert_eq!(other_err.category(), ErrorCategory::General);
    }

    #[test]
    fn error_category_display() {
        assert_eq!(format!("{}", ErrorCategory::Git), "Git");
        assert_eq!(format!("{}", ErrorCategory::Llm), "LLM");
    }
}
