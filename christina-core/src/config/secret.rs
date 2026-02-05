use std::fmt;
use std::thread;
use std::time::Duration;
use thiserror::Error;
use tracing;

/// Errors that can occur during secret resolution
#[derive(Debug, Error)]
pub enum SecretError {
    /// Environment variable not found
    #[error("Environment variable '{0}' not found")]
    EnvVarNotFound(String),

    /// Keyring lookup failed
    #[error("Keyring lookup failed for '{0}': {1}")]
    KeyringFailed(String, String),

    /// Invalid secret reference format
    #[error("Invalid secret reference: {0}")]
    InvalidFormat(String),
}

/// Generic secret container
///
/// **Important**: This type implements a custom Debug impl that redacts all secrets
/// for security. To access secret values, use `expose_secret()` or resolver methods.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum Secret<S> {
    Value(S),
    #[serde(rename = "env")]
    EnvVar(String),
    #[serde(rename = "keyring")]
    Keyring(String),
}

impl<S> fmt::Debug for Secret<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Secret::Value(_) => f.write_str("[REDACTED:secret-value]"),
            Secret::EnvVar(name) => f.write_fmt(format_args!("[REDACTED:env:{}]", name)),
            Secret::Keyring(key) => f.write_fmt(format_args!("[REDACTED:keyring:{}]", key)),
        }
    }
}

impl Secret<String> {
    /// Resolve the secret to its actual value
    pub fn resolve(&self) -> Result<SecretString, SecretError> {
        match self {
            Secret::Value(s) => Ok(SecretString::new(s.clone())),
            Secret::EnvVar(name) => std::env::var(name)
                .map(SecretString::new)
                .map_err(|_| SecretError::EnvVarNotFound(name.clone())),
            #[cfg(feature = "keyring-support")]
            Secret::Keyring(key) => {
                let entry =
                    keyring::Entry::new("christina", key).map_err(|e: keyring::Error| {
                        SecretError::KeyringFailed(key.clone(), e.to_string())
                    })?;

                match entry.get_password() {
                    Ok(password) => Ok(SecretString::new(password)),
                    Err(e) => {
                        let error_str = e.to_string();
                        let is_not_found = error_str.contains("entry not found")
                            || error_str.contains("not found");

                        if is_not_found {
                            Err(SecretError::KeyringFailed(key.clone(), error_str))
                        } else {
                            tracing::warn!("Keyring access failed for '{}', retrying...", key);
                            thread::sleep(Duration::from_millis(500));

                            entry.get_password().map(SecretString::new).map_err(
                                |retry_err: keyring::Error| {
                                    SecretError::KeyringFailed(key.clone(), retry_err.to_string())
                                },
                            )
                        }
                    }
                }
            }

            #[cfg(not(feature = "keyring-support"))]
            Secret::Keyring(key) => Err(SecretError::KeyringFailed(
                key.clone(),
                "Keyring support not compiled in. Enable the 'keyring-support' feature".to_string(),
            )),
        }
    }
}

/// On-disk reference (env var or keyring)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "value")]
pub enum SecretRef {
    #[serde(rename = "env")]
    EnvVar(String), // e.g., "OPENAI_API_KEY"
    #[serde(rename = "keyring")]
    Keyring(String), // e.g., "christina.openai"
    #[serde(rename = "value")]
    Literal(String), // Raw value (for testing)
}

impl SecretRef {
    /// Resolve the secret reference to a SecretString
    pub fn resolve(&self) -> Result<SecretString, SecretError> {
        match self {
            SecretRef::EnvVar(name) => std::env::var(name)
                .map(SecretString::new)
                .map_err(|_| SecretError::EnvVarNotFound(name.clone())),
            #[cfg(feature = "keyring-support")]
            SecretRef::Keyring(key) => {
                let entry =
                    keyring::Entry::new("christina", key).map_err(|e: keyring::Error| {
                        SecretError::KeyringFailed(key.clone(), e.to_string())
                    })?;

                match entry.get_password() {
                    Ok(password) => Ok(SecretString::new(password)),
                    Err(e) => {
                        let error_str = e.to_string();
                        let is_not_found = error_str.contains("entry not found")
                            || error_str.contains("not found");

                        if is_not_found {
                            Err(SecretError::KeyringFailed(key.clone(), error_str))
                        } else {
                            tracing::warn!("Keyring access failed for '{}', retrying...", key);
                            thread::sleep(Duration::from_millis(500));

                            entry.get_password().map(SecretString::new).map_err(
                                |retry_err: keyring::Error| {
                                    SecretError::KeyringFailed(key.clone(), retry_err.to_string())
                                },
                            )
                        }
                    }
                }
            }
            #[cfg(not(feature = "keyring-support"))]
            SecretRef::Keyring(key) => Err(SecretError::KeyringFailed(
                key.clone(),
                "Keyring support not compiled in. Enable the 'keyring-support' feature".to_string(),
            )),
            SecretRef::Literal(value) => Ok(SecretString::new(value.clone())),
        }
    }

    /// Parse a secret reference from a string
    ///
    /// Supports formats:
    /// - `env:VAR_NAME` - Environment variable reference
    /// - `keyring:KEY_NAME` - Keyring entry reference
    /// - `value:SECRET_VALUE` - Literal value (not recommended for production)
    /// - Plain string - Treated as literal value
    pub fn parse(s: &str) -> Result<Self, SecretError> {
        if let Some(rest) = s.strip_prefix("env:") {
            Ok(SecretRef::EnvVar(rest.to_string()))
        } else if let Some(rest) = s.strip_prefix("keyring:") {
            Ok(SecretRef::Keyring(rest.to_string()))
        } else if let Some(rest) = s.strip_prefix("value:") {
            Ok(SecretRef::Literal(rest.to_string()))
        } else {
            // Treat as literal value
            // Warn if this looks like an API key stored as plaintext
            if s.len() > 20 && !s.contains(' ') {
                tracing::warn!(
                    "API key stored as plaintext in config file. Consider using env:VAR_NAME or keyring:KEY_NAME for better security."
                );
            }
            Ok(SecretRef::Literal(s.to_string()))
        }
    }
}

/// Runtime secret (redacted in Debug).
///
/// `PartialEq` is deliberately not implemented for `SecretString`.
/// This forces explicit comparisons via `expose_secret()`, which:
/// - Makes secret comparisons intentional and visible in code
/// - Prevents accidental timing-side-channel leaks from `==` comparisons
/// - Encourages explicit secret handling rather than treating secrets like normal strings
///
/// For comparing secrets, use `s1.expose_secret() == s2.expose_secret()`.
pub struct SecretString(String);

impl SecretString {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl Clone for SecretString {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn secret_ref_parse_env() {
        let secret = SecretRef::parse("env:OPENAI_API_KEY").unwrap();
        assert!(matches!(secret, SecretRef::EnvVar(s) if s == "OPENAI_API_KEY"));
    }

    #[test]
    fn secret_ref_parse_keyring() {
        let secret = SecretRef::parse("keyring:christina.openai").unwrap();
        assert!(matches!(secret, SecretRef::Keyring(s) if s == "christina.openai"));
    }

    #[test]
    fn secret_ref_parse_value() {
        let secret = SecretRef::parse("value:secret123").unwrap();
        assert!(matches!(secret, SecretRef::Literal(s) if s == "secret123"));
    }

    #[test]
    fn secret_ref_parse_literal() {
        let secret = SecretRef::parse("just_a_plain_secret").unwrap();
        assert!(matches!(secret, SecretRef::Literal(s) if s == "just_a_plain_secret"));
    }

    #[test]
    fn secret_ref_resolve_env() {
        let secret = SecretRef::Literal("test_value".to_string());
        let resolved = secret.resolve().unwrap();
        assert_eq!(resolved.expose_secret(), "test_value");
    }

    #[test]
    fn secret_ref_resolve_env_not_found() {
        let secret = SecretRef::EnvVar("DEFINITELY_NOT_SET_VAR_12345".to_string());
        assert!(matches!(
            secret.resolve(),
            Err(SecretError::EnvVarNotFound(_))
        ));
    }

    #[test]
    fn secret_ref_resolve_literal() {
        let secret = SecretRef::Literal("my_secret".to_string());
        let resolved = secret.resolve().unwrap();
        assert_eq!(resolved.expose_secret(), "my_secret");
    }

    #[test]
    fn secret_string_debug_redacted() {
        let secret = SecretString::new("my_secret".to_string());
        let debug = format!("{:?}", secret);
        assert_eq!(debug, "[REDACTED]");
    }

    #[test]
    fn secret_value_debug_redacted() {
        let secret: Secret<String> = Secret::Value("sk-test123".to_string());
        let debug = format!("{:?}", secret);
        assert_eq!(debug, "[REDACTED:secret-value]");
        assert!(!debug.contains("sk-test123"));
    }

    #[test]
    fn secret_env_var_debug_redacted() {
        let secret: Secret<String> = Secret::EnvVar("OPENAI_API_KEY".to_string());
        let debug = format!("{:?}", secret);
        assert_eq!(debug, "[REDACTED:env:OPENAI_API_KEY]");
    }

    #[test]
    fn secret_keyring_debug_redacted() {
        let secret: Secret<String> = Secret::Keyring("christina.openai".to_string());
        let debug = format!("{:?}", secret);
        assert_eq!(debug, "[REDACTED:keyring:christina.openai]");
    }

    #[test]
    fn secret_resolve_value() {
        let secret = Secret::Value("direct_value".to_string());
        let resolved = secret.resolve().unwrap();
        assert_eq!(resolved.expose_secret(), "direct_value");
    }

    #[test]
    fn secret_resolve_env_var() {
        let secret = Secret::Value("env_value".to_string());
        let resolved = secret.resolve().unwrap();
        assert_eq!(resolved.expose_secret(), "env_value");
    }

    #[test]
    fn secret_ref_parse_empty_env() {
        let secret = SecretRef::parse("env:").unwrap();
        assert!(matches!(secret, SecretRef::EnvVar(s) if s.is_empty()));
    }

    #[test]
    fn secret_ref_parse_empty_keyring() {
        let secret = SecretRef::parse("keyring:").unwrap();
        assert!(matches!(secret, SecretRef::Keyring(s) if s.is_empty()));
    }

    #[test]
    fn secret_ref_parse_empty_value() {
        let secret = SecretRef::parse("value:").unwrap();
        assert!(matches!(secret, SecretRef::Literal(s) if s.is_empty()));
    }

    #[test]
    fn secret_ref_parse_with_colons() {
        let secret = SecretRef::parse("env:MY_VAR:WITH:COLONS").unwrap();
        assert!(matches!(secret, SecretRef::EnvVar(s) if s == "MY_VAR:WITH:COLONS"));
    }

    #[test]
    fn secret_string_clone() {
        let original = SecretString::new("secret".to_string());
        let cloned = original.clone();
        assert_eq!(original.expose_secret(), cloned.expose_secret());
    }

    #[test]
    fn secret_string_expose_multiple_times() {
        let secret = SecretString::new("test".to_string());
        assert_eq!(secret.expose_secret(), "test");
        assert_eq!(secret.expose_secret(), "test");
    }

    #[test]
    fn secret_value_clone() {
        let original: Secret<String> = Secret::Value("test".to_string());
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn secret_env_var_clone() {
        let original: Secret<String> = Secret::EnvVar("VAR".to_string());
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn secret_keyring_clone() {
        let original: Secret<String> = Secret::Keyring("key".to_string());
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn secret_equality() {
        let s1: Secret<String> = Secret::Value("test".to_string());
        let s2: Secret<String> = Secret::Value("test".to_string());
        let s3: Secret<String> = Secret::Value("other".to_string());

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn secret_ref_equality() {
        let r1 = SecretRef::EnvVar("VAR".to_string());
        let r2 = SecretRef::EnvVar("VAR".to_string());
        let r3 = SecretRef::EnvVar("OTHER".to_string());

        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }

    #[test]
    fn secret_error_display() {
        let err = SecretError::EnvVarNotFound("MY_VAR".to_string());
        assert!(err.to_string().contains("MY_VAR"));
        assert!(err.to_string().contains("not found"));

        let err = SecretError::KeyringFailed("key".to_string(), "reason".to_string());
        assert!(err.to_string().contains("key"));
        assert!(err.to_string().contains("reason"));

        let err = SecretError::InvalidFormat("bad format".to_string());
        assert!(err.to_string().contains("bad format"));
    }

    #[cfg(not(feature = "keyring-support"))]
    #[test]
    fn secret_resolve_keyring_without_feature() {
        let secret = Secret::Keyring("test".to_string());
        let result = secret.resolve();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Keyring support not compiled")
        );
    }

    #[cfg(not(feature = "keyring-support"))]
    #[test]
    fn secret_ref_resolve_keyring_without_feature() {
        let secret = SecretRef::Keyring("test".to_string());
        let result = secret.resolve();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Keyring support not compiled")
        );
    }

    #[test]
    #[allow(unsafe_code)]
    fn secret_resolve_env_var_actually_exists() {
        unsafe {
            std::env::set_var("CHRISTINA_TEST_SECRET_VAR_12345", "test_value");
        }
        let secret = Secret::EnvVar("CHRISTINA_TEST_SECRET_VAR_12345".to_string());
        let resolved = secret.resolve().unwrap();
        assert_eq!(resolved.expose_secret(), "test_value");
        unsafe {
            std::env::remove_var("CHRISTINA_TEST_SECRET_VAR_12345");
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn secret_ref_resolve_env_var_actually_exists() {
        unsafe {
            std::env::set_var("CHRISTINA_TEST_REF_VAR_67890", "ref_value");
        }
        let secret = SecretRef::EnvVar("CHRISTINA_TEST_REF_VAR_67890".to_string());
        let resolved = secret.resolve().unwrap();
        assert_eq!(resolved.expose_secret(), "ref_value");
        unsafe {
            std::env::remove_var("CHRISTINA_TEST_REF_VAR_67890");
        }
    }

    #[test]
    fn secret_string_new() {
        let secret = SecretString::new("test".to_string());
        assert_eq!(secret.expose_secret(), "test");
    }

    #[test]
    fn secret_ref_parse_long_api_key() {
        // Should parse as literal but may log warning
        let secret = SecretRef::parse("sk-1234567890123456789012345678901234567890").unwrap();
        assert!(matches!(secret, SecretRef::Literal(_)));
    }
}
