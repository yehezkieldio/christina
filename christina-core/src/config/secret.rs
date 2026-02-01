use std::fmt;
use thiserror::Error;

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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Secret<S> {
    Value(S),
    #[serde(rename = "env")]
    EnvVar(String),
    #[serde(rename = "keyring")]
    Keyring(String),
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
                entry
                    .get_password()
                    .map(SecretString::new)
                    .map_err(|e: keyring::Error| {
                        SecretError::KeyringFailed(key.clone(), e.to_string())
                    })
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
                entry
                    .get_password()
                    .map(SecretString::new)
                    .map_err(|e: keyring::Error| {
                        SecretError::KeyringFailed(key.clone(), e.to_string())
                    })
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
            Ok(SecretRef::Literal(s.to_string()))
        }
    }
}

/// Runtime secret (redacted in Debug)
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

impl PartialEq for SecretString {
    fn eq(&self, _other: &Self) -> bool {
        // Secrets are never equal (security)
        false
    }
}

impl Eq for SecretString {}

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
    fn secret_string_never_equal() {
        let s1 = SecretString::new("secret".to_string());
        let s2 = SecretString::new("secret".to_string());
        assert_ne!(s1, s2);
    }

    #[test]
    fn secret_string_debug_redacted() {
        let secret = SecretString::new("my_secret".to_string());
        let debug = format!("{:?}", secret);
        assert_eq!(debug, "[REDACTED]");
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
}
