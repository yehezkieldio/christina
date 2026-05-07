//! Secret resolution for runtime use.
//!
//! WHY here: env access is runtime IO, so it lives in the CLI crate rather than
//! `christina-core`.

use thiserror::Error;

pub use christina_core::config::{Secret, SecretRef, SecretString};

/// Errors that can occur during secret resolution.
#[derive(Debug, Error)]
pub enum SecretResolveError {
    /// Environment variable not found.
    #[error("Environment variable '{0}' not found")]
    EnvVarNotFound(String),
}

pub fn resolve_secret(secret: &Secret<String>) -> Result<SecretString, SecretResolveError> {
    // Keep resolution centralized to avoid leaking plaintext secrets into config layers.
    match secret {
        Secret::Value(s) => Ok(SecretString::new(s.clone())),
        Secret::EnvVar(name) => std::env::var(name)
            .map(SecretString::new)
            .map_err(|_| SecretResolveError::EnvVarNotFound(name.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_env_var_missing() {
        let secret = Secret::EnvVar("DEFINITELY_NOT_SET_VAR_12345".to_string());
        let result = resolve_secret(&secret);
        assert!(matches!(result, Err(SecretResolveError::EnvVarNotFound(_))));
    }

    #[test]
    fn resolve_literal_secret() {
        let secret = Secret::Value("test_value".to_string());
        let resolved = resolve_secret(&secret).unwrap();
        assert_eq!(resolved.expose_secret(), "test_value");
    }

    #[test]
    #[allow(unsafe_code)]
    fn resolve_env_var_actually_exists() {
        unsafe {
            std::env::set_var("CHRISTINA_TEST_SECRET_VAR_12345", "test_value");
        }
        let secret = Secret::EnvVar("CHRISTINA_TEST_SECRET_VAR_12345".to_string());
        let resolved = resolve_secret(&secret).unwrap();
        assert_eq!(resolved.expose_secret(), "test_value");
        unsafe {
            std::env::remove_var("CHRISTINA_TEST_SECRET_VAR_12345");
        }
    }
}
