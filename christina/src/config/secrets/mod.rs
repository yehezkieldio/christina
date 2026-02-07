use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;

pub use christina_core::config::{Secret, SecretRef, SecretString};

/// Errors that can occur during secret resolution.
#[derive(Debug, Error)]
pub enum SecretResolveError {
    /// Environment variable not found.
    #[error("Environment variable '{0}' not found")]
    EnvVarNotFound(String),

    /// Keyring lookup failed.
    #[error("Keyring lookup failed for '{0}': {1}")]
    KeyringFailed(String, String),
}

/// Synchronous retry policy for blocking operations.
///
/// Uses exponential backoff with full jitter to prevent thundering herds on transient failures.
#[derive(Debug, Clone)]
struct BlockingRetryPolicy {
    max_retries: usize,
    base_delay_ms: u64,
    with_jitter: bool,
}

impl Default for BlockingRetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1_000,
            with_jitter: true,
        }
    }
}

impl BlockingRetryPolicy {
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let max_delay_ms = self
            .base_delay_ms
            .saturating_mul(2_u64.saturating_pow(attempt));
        let delay_ms = if self.with_jitter {
            rand_jitter(max_delay_ms)
        } else {
            max_delay_ms
        };
        Duration::from_millis(delay_ms)
    }

    /// Retry a fallible blocking operation with exponential backoff.
    ///
    /// Only retries if `is_transient` returns true for the error.
    fn retry_blocking<F, T, E>(
        &self,
        mut operation: F,
        is_transient: impl Fn(&E) -> bool,
    ) -> Result<T, E>
    where
        F: FnMut() -> Result<T, E>,
    {
        let mut attempt = 0usize;

        loop {
            match operation() {
                Ok(result) => return Ok(result),
                Err(err) => {
                    if !is_transient(&err) {
                        return Err(err);
                    }

                    if attempt >= self.max_retries {
                        return Err(err);
                    }

                    let delay = self.calculate_delay(attempt as u32);
                    thread::sleep(delay);
                    attempt += 1;
                }
            }
        }
    }
}

/// Generate random jitter in range [0, max] using time-based entropy.
fn rand_jitter(max: u64) -> u64 {
    if max == 0 {
        return 0;
    }

    if max == u64::MAX {
        let mut hasher = RandomState::new().build_hasher();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        hasher.write_u64(now.as_nanos() as u64);
        return hasher.finish();
    }

    let mut hasher = RandomState::new().build_hasher();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    hasher.write_u64(now.as_nanos() as u64);
    let hash = hasher.finish();
    hash % (max + 1)
}

pub fn resolve_secret(secret: &Secret<String>) -> Result<SecretString, SecretResolveError> {
    match secret {
        Secret::Value(s) => Ok(SecretString::new(s.clone())),
        Secret::EnvVar(name) => std::env::var(name)
            .map(SecretString::new)
            .map_err(|_| SecretResolveError::EnvVarNotFound(name.clone())),
        #[cfg(feature = "keyring-support")]
        Secret::Keyring(key) => resolve_keyring_secret(key),
        #[cfg(not(feature = "keyring-support"))]
        Secret::Keyring(key) => Err(SecretResolveError::KeyringFailed(
            key.clone(),
            "Keyring support not compiled in. Enable the 'keyring-support' feature".to_string(),
        )),
    }
}

pub fn resolve_secret_ref(secret: &SecretRef) -> Result<SecretString, SecretResolveError> {
    match secret {
        SecretRef::EnvVar(name) => std::env::var(name)
            .map(SecretString::new)
            .map_err(|_| SecretResolveError::EnvVarNotFound(name.clone())),
        #[cfg(feature = "keyring-support")]
        SecretRef::Keyring(key) => resolve_keyring_secret(key),
        #[cfg(not(feature = "keyring-support"))]
        SecretRef::Keyring(key) => Err(SecretResolveError::KeyringFailed(
            key.clone(),
            "Keyring support not compiled in. Enable the 'keyring-support' feature".to_string(),
        )),
        SecretRef::Literal(value) => Ok(SecretString::new(value.clone())),
    }
}

#[cfg(feature = "keyring-support")]
fn resolve_keyring_secret(key: &str) -> Result<SecretString, SecretResolveError> {
    let policy = BlockingRetryPolicy::default();
    let key_clone = key.to_string();

    policy.retry_blocking(
        || {
            let entry = keyring::Entry::new("christina", &key_clone)
                .map_err(|e: keyring::Error| {
                    SecretResolveError::KeyringFailed(key_clone.clone(), e.to_string())
                })?;

            entry
                .get_password()
                .map(SecretString::new)
                .map_err(|e: keyring::Error| {
                    SecretResolveError::KeyringFailed(key_clone.clone(), e.to_string())
                })
        },
        |err| match err {
            SecretResolveError::KeyringFailed(_, msg) => {
                let is_not_found = msg.contains("entry not found") || msg.contains("not found");
                !is_not_found
            }
            _ => false,
        },
    )
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
    fn resolve_literal_ref() {
        let secret = SecretRef::Literal("ref_value".to_string());
        let resolved = resolve_secret_ref(&secret).unwrap();
        assert_eq!(resolved.expose_secret(), "ref_value");
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

    #[cfg(not(feature = "keyring-support"))]
    #[test]
    fn resolve_keyring_without_feature() {
        let secret = Secret::Keyring("test".to_string());
        let result = resolve_secret(&secret);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Keyring support not compiled"));
    }
}
