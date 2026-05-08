//! Unified error types for the Christina workspace.
//!
//! WHY single error module: Centralizes error handling across christina-core, christina-git,
//! and christina-llm crates. Prevents duplication, ensures consistent retry/transient logic,
//! and simplifies error propagation across subsystem boundaries.
//!
//! WHY IsTransient trait: Separates transient (retryable) from permanent errors at the
//! type level. Enables generic retry logic without inspecting error details. Each error
//! type implements its own transient classification based on domain knowledge.
//!
//! WHY AppError enum: Application-level error that can represent any subsystem error.
//! Acts as error boundary between internal subsystems and user-facing code. Implements
//! automatic From conversions to reduce boilerplate.

use std::fmt;
use std::time::Duration;
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
    RateLimited { retry_after: Option<Duration> },

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

    /// User-initiated cancellation.
    #[error("Generation cancelled")]
    Cancelled,

    /// Unknown or unclassified error
    #[error("Unknown error: {0}")]
    UnknownError(String),
}

impl CompletionError {
    /// Check if this error is transient and may succeed on retry
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            CompletionError::RateLimited { .. }
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
                | CompletionError::RateLimited { .. }
                | CompletionError::ServerError(_)
        )
    }

    /// Returns Retry-After duration when available.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            CompletionError::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }

    /// Parse an API error message to create appropriate error variant.
    ///
    /// WHY string parsing: API providers return errors as unstructured text/HTML.
    /// No standardized error schema across Azure. Regex would be
    /// brittle; simple substring matching is sufficient and maintainable.
    ///
    /// WHY default to ServerError: Unknown errors assumed retryable. False
    /// positive (retrying permanent error) wastes time but preserves correctness.
    /// False negative (not retrying transient error) surfaces user-visible failures.
    ///
    /// Explicit non-transient patterns are detected to prevent wasting retries
    /// on errors that will never succeed (validation, resource not found, etc.).
    pub fn from_api_error(msg: &str) -> Self {
        let msg_lower = msg.to_lowercase();

        // Authentication and authorization errors (permanent)
        if contains_status_code(&msg_lower, "401")
            || msg_lower.contains("unauthorized")
            || msg_lower.contains("invalid api key")
        {
            CompletionError::Unauthorized(msg.to_string())
        }
        // Rate limiting (transient)
        else if contains_status_code(&msg_lower, "429")
            || msg_lower.contains("rate limit")
            || msg_lower.contains("quota")
        {
            let retry_after = parse_retry_after_seconds(msg);
            CompletionError::RateLimited { retry_after }
        }
        // Request validation and resource not found errors (permanent)
        else if contains_status_code(&msg_lower, "400")
            || contains_status_code(&msg_lower, "404")
            || msg_lower.contains("bad request")
            || msg_lower.contains("invalid request")
            || msg_lower.contains("malformed")
            || msg_lower.contains("context length")
            || msg_lower.contains("context_length")
            || msg_lower.contains("token limit")
            || msg_lower.contains("too many tokens")
            || msg_lower.contains("maximum context")
            || msg_lower.contains("not found")
            || msg_lower.contains("does not exist")
            || msg_lower.contains("model not found")
            || msg_lower.contains("no such model")
            || msg_lower.contains("unknown model")
        {
            CompletionError::InvalidResponse(msg.to_string())
        }
        // Timeout (transient)
        else if msg_lower.contains("timeout") || msg_lower.contains("timed out") {
            CompletionError::Timeout
        }
        // Server errors (transient)
        else if contains_status_code(&msg_lower, "500")
            || contains_status_code(&msg_lower, "502")
            || contains_status_code(&msg_lower, "503")
            || contains_status_code(&msg_lower, "504")
            || msg_lower.contains("server error")
            || msg_lower.contains("overloaded")
            || msg_lower.contains("internal error")
            || msg_lower.contains("bad gateway")
            || msg_lower.contains("gateway timeout")
            || msg_lower.contains("service unavailable")
        {
            CompletionError::ServerError(msg.to_string())
        }
        // Network errors (transient)
        else if msg_lower.contains("network")
            || msg_lower.contains("connection")
            || msg_lower.contains("dns")
            || msg_lower.contains("resolve")
        {
            CompletionError::NetworkError(msg.to_string())
        }
        // Unknown errors default to ServerError (transient, but conservative)
        else {
            CompletionError::ServerError(msg.to_string())
        }
    }
}

impl IsTransient for CompletionError {
    fn is_transient(&self) -> bool {
        matches!(
            self,
            CompletionError::RateLimited { .. }
                | CompletionError::Timeout
                | CompletionError::ServerError(_)
                | CompletionError::NetworkError(_)
        )
    }
}

fn contains_status_code(msg_lower: &str, code: &str) -> bool {
    if starts_with_status_code(msg_lower, code) {
        return true;
    }

    let patterns = [
        format!("http {code}"),
        format!("http/1.1 {code}"),
        format!("http/2 {code}"),
        format!("status {code}"),
        format!("status: {code}"),
        format!("status={code}"),
        format!("status code {code}"),
        format!("code {code}"),
        format!("error {code}"),
        format!("response {code}"),
        format!("response: {code}"),
    ];

    patterns.iter().any(|pattern| msg_lower.contains(pattern))
}

fn starts_with_status_code(msg_lower: &str, code: &str) -> bool {
    if !msg_lower.starts_with(code) {
        return false;
    }

    msg_lower[code.len()..]
        .chars()
        .next()
        .is_none_or(|c| !c.is_ascii_digit())
}

fn parse_retry_after_seconds(msg: &str) -> Option<Duration> {
    let lower = msg.to_lowercase();
    let markers = ["retry-after", "retry after", "retry_after"];

    for marker in markers {
        if let Some(index) = lower.find(marker) {
            let rest = &lower[index + marker.len()..];
            let digits: String = rest
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();

            if !digits.is_empty()
                && let Ok(seconds) = digits.parse::<u64>()
            {
                return Some(Duration::from_secs(seconds));
            }
        }
    }

    None
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
#[derive(Debug, Clone, Error)]
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

/// Error type for diff processing operations
#[derive(Debug, Error)]
pub enum DiffError {
    /// Diff size exceeds maximum allowed
    #[error("Diff size exceeds maximum: {actual} bytes > {max} bytes")]
    SizeExceeded { actual: usize, max: usize },

    /// No processable diff content found
    #[error("No processable diff content found")]
    NoContent,
}

impl IsTransient for DiffError {
    fn is_transient(&self) -> bool {
        false
    }
}

/// Result type alias for diff processing operations
pub type DiffResult<T> = std::result::Result<T, DiffError>;

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

    #[error(transparent)]
    Diff(#[from] DiffError),

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
            AppError::Diff(_) | AppError::Other(_) => ErrorCategory::General,
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
        assert!(CompletionError::RateLimited { retry_after: None }.is_transient());
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
        assert!(matches!(err, CompletionError::RateLimited { .. }));

        let err = CompletionError::from_api_error("Request timeout");
        assert!(matches!(err, CompletionError::Timeout));

        let err = CompletionError::from_api_error("Server error 500");
        assert!(matches!(err, CompletionError::ServerError(_)));
    }

    #[test]
    fn completion_error_from_api_error_validation() {
        // Bad request errors should be non-transient
        let err = CompletionError::from_api_error("400 Bad Request: invalid request");
        assert!(matches!(err, CompletionError::InvalidResponse(_)));
        assert!(!err.is_transient());

        let err = CompletionError::from_api_error("context length exceeded");
        assert!(matches!(err, CompletionError::InvalidResponse(_)));
        assert!(!err.is_transient());

        let err = CompletionError::from_api_error("too many tokens in request");
        assert!(matches!(err, CompletionError::InvalidResponse(_)));
        assert!(!err.is_transient());
    }

    #[test]
    fn completion_error_from_api_error_not_found() {
        // Not found errors should be non-transient
        let err = CompletionError::from_api_error("404 model not found");
        assert!(matches!(err, CompletionError::InvalidResponse(_)));
        assert!(!err.is_transient());

        let err = CompletionError::from_api_error("The model 'gpt-5' does not exist");
        assert!(matches!(err, CompletionError::InvalidResponse(_)));
        assert!(!err.is_transient());
    }

    #[test]
    fn completion_error_from_api_error_server_codes() {
        // Server error codes should be transient
        let err = CompletionError::from_api_error("500 Internal Server Error");
        assert!(matches!(err, CompletionError::ServerError(_)));
        assert!(err.is_transient());

        let err = CompletionError::from_api_error("502 Bad Gateway");
        assert!(matches!(err, CompletionError::ServerError(_)));
        assert!(err.is_transient());

        let err = CompletionError::from_api_error("503 Service Unavailable");
        assert!(matches!(err, CompletionError::ServerError(_)));
        assert!(err.is_transient());
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

    #[test]
    fn completion_error_is_provider_error() {
        assert!(CompletionError::Unauthorized("test".to_string()).is_provider_error());
        assert!(CompletionError::RateLimited { retry_after: None }.is_provider_error());
        assert!(CompletionError::ServerError("test".to_string()).is_provider_error());

        assert!(!CompletionError::Timeout.is_provider_error());
        assert!(!CompletionError::NetworkError("test".to_string()).is_provider_error());
    }

    #[test]
    fn completion_error_from_api_error_unauthorized_variations() {
        let err = CompletionError::from_api_error("401 Unauthorized");
        assert!(matches!(err, CompletionError::Unauthorized(_)));

        let err = CompletionError::from_api_error("Invalid API key provided");
        assert!(matches!(err, CompletionError::Unauthorized(_)));

        let err = CompletionError::from_api_error("unauthorized access");
        assert!(matches!(err, CompletionError::Unauthorized(_)));
    }

    #[test]
    fn completion_error_from_api_error_rate_limit_variations() {
        let err = CompletionError::from_api_error("429 Too Many Requests");
        assert!(matches!(err, CompletionError::RateLimited { .. }));

        let err = CompletionError::from_api_error("Rate limit exceeded");
        assert!(matches!(err, CompletionError::RateLimited { .. }));

        let err = CompletionError::from_api_error("Quota exceeded");
        assert!(matches!(err, CompletionError::RateLimited { .. }));
    }

    #[test]
    fn completion_error_rate_limit_parses_retry_after() {
        let err = CompletionError::from_api_error("429 Too Many Requests; Retry-After: 12");
        match err {
            CompletionError::RateLimited { retry_after } => {
                assert_eq!(retry_after, Some(Duration::from_secs(12)));
            }
            _ => panic!("Expected rate limited error"),
        }
    }

    #[test]
    fn completion_error_from_api_error_context_length() {
        let err = CompletionError::from_api_error("context_length_exceeded");
        assert!(matches!(err, CompletionError::InvalidResponse(_)));
        assert!(!err.is_transient());

        let err = CompletionError::from_api_error("Maximum context length exceeded");
        assert!(matches!(err, CompletionError::InvalidResponse(_)));
        assert!(!err.is_transient());

        let err = CompletionError::from_api_error("Token limit exceeded");
        assert!(matches!(err, CompletionError::InvalidResponse(_)));
        assert!(!err.is_transient());
    }

    #[test]
    fn completion_error_from_api_error_malformed() {
        let err = CompletionError::from_api_error("Malformed request");
        assert!(matches!(err, CompletionError::InvalidResponse(_)));
        assert!(!err.is_transient());
    }

    #[test]
    fn completion_error_from_api_error_network() {
        let err = CompletionError::from_api_error("Network connection failed");
        assert!(matches!(err, CompletionError::NetworkError(_)));
        assert!(err.is_transient());

        let err = CompletionError::from_api_error("DNS resolution failed");
        assert!(matches!(err, CompletionError::NetworkError(_)));
        assert!(err.is_transient());
    }

    #[test]
    fn completion_error_from_api_error_unknown_defaults_to_server() {
        let err = CompletionError::from_api_error("Unknown mysterious error");
        assert!(matches!(err, CompletionError::ServerError(_)));
        assert!(err.is_transient());
    }

    #[test]
    fn git_error_transient_only_locked() {
        assert!(GitError::Locked.is_transient());
        assert!(!GitError::NotFound.is_transient());
        assert!(!GitError::AuthFailed.is_transient());
        assert!(!GitError::Git("test".to_string()).is_transient());
    }

    #[test]
    fn provider_error_never_transient() {
        let err = ProviderError::MissingConfig("test".to_string());
        assert!(!err.is_transient());

        let err = ProviderError::InvalidConfig("test".to_string());
        assert!(!err.is_transient());
    }

    #[test]
    fn tokenizer_error_never_transient() {
        let err = TokenizerError::Tokenizer("test".to_string());
        assert!(!err.is_transient());

        let err = TokenizerError::ModelLoadFailed("test".to_string());
        assert!(!err.is_transient());

        let err = TokenizerError::EncodingFailed("test".to_string());
        assert!(!err.is_transient());
    }

    #[test]
    fn tokenizer_error_is_model_error() {
        let err = TokenizerError::ModelLoadFailed("test".to_string());
        assert!(err.is_model_error());

        let err = TokenizerError::Tokenizer("test".to_string());
        assert!(!err.is_model_error());
    }

    #[test]
    fn diff_error_never_transient() {
        let err = DiffError::SizeExceeded {
            actual: 100,
            max: 50,
        };
        assert!(!err.is_transient());

        let err = DiffError::NoContent;
        assert!(!err.is_transient());
    }

    #[test]
    fn app_error_is_transient() {
        let transient = AppError::Completion(CompletionError::Timeout);
        assert!(transient.is_transient());

        let non_transient = AppError::Provider(ProviderError::MissingConfig("test".to_string()));
        assert!(!non_transient.is_transient());
    }

    #[test]
    fn app_error_is_config_error() {
        let config_err = AppError::Provider(ProviderError::MissingConfig("test".to_string()));
        assert!(config_err.is_config_error());

        let non_config = AppError::Completion(CompletionError::Timeout);
        assert!(!non_config.is_config_error());
    }

    #[test]
    fn git_error_signing_error_variants() {
        let gpg_config = GitError::GpgConfigInvalid("bad config".to_string());
        assert!(gpg_config.is_signing_error());

        let gpg_signing = GitError::GpgSigningFailed("signature failed".to_string());
        assert!(gpg_signing.is_signing_error());

        let other = GitError::Other("unrelated".to_string());
        assert!(!other.is_signing_error());
    }

    #[test]
    fn provider_error_field_name_consistency() {
        let missing = ProviderError::MissingConfig("api_key".to_string());
        assert_eq!(missing.field_name(), "api_key");

        let invalid = ProviderError::InvalidConfig("temperature".to_string());
        assert_eq!(invalid.field_name(), "temperature");
    }

    #[test]
    fn app_error_category_completeness() {
        let git = AppError::Git(GitError::NotFound);
        assert_eq!(git.category(), ErrorCategory::Git);

        let completion = AppError::Completion(CompletionError::Timeout);
        assert_eq!(completion.category(), ErrorCategory::Llm);

        let provider = AppError::Provider(ProviderError::MissingConfig("x".to_string()));
        assert_eq!(provider.category(), ErrorCategory::Llm);

        let tokenizer = AppError::Tokenizer(TokenizerError::Tokenizer("x".to_string()));
        assert_eq!(tokenizer.category(), ErrorCategory::Tokenizer);

        let diff = AppError::Diff(DiffError::NoContent);
        assert_eq!(diff.category(), ErrorCategory::General);

        let other = AppError::Other("x".to_string());
        assert_eq!(other.category(), ErrorCategory::General);
    }

    #[test]
    fn git_error_from_conversion() {
        #[cfg(feature = "git2-support")]
        {
            use git2::Error;

            let git2_err = Error::from_str("not found");
            let err: GitError = git2_err.into();
            // Basic conversion test - exact variant depends on git2 implementation
            assert!(matches!(err, GitError::Git(_) | GitError::Other(_)));
        }
    }
}
