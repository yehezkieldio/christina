//! Data-only secret representations for config and runtime.
//!
//! WHY keep in core: lets downstream crates share a single schema without
//! pulling in environment resolution logic, which lives in `christina`.

use std::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// Generic secret container.
///
/// This type is pure data. Resolution is handled in the `christina` crate.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub enum Secret<S> {
    #[serde(rename = "value", alias = "Value")]
    Value(S),
    #[serde(rename = "env")]
    EnvVar(String),
}

impl<S> fmt::Debug for Secret<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Secret::Value(_) => f.write_str("[REDACTED:secret-value]"),
            Secret::EnvVar(name) => f.write_fmt(format_args!("[REDACTED:env:{name}]")),
        }
    }
}

/// On-disk reference (env var or literal value).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "value")]
pub enum SecretRef {
    #[serde(rename = "env")]
    EnvVar(String),
    #[serde(rename = "value")]
    Literal(String),
}

impl SecretRef {
    /// Parse a secret reference from a string.
    ///
    /// Supports formats:
    /// - `env:VAR_NAME`
    /// - `value:SECRET_VALUE`
    /// - Plain string (treated as literal)
    #[must_use]
    pub fn parse(s: &str) -> Self {
        // Infallible parse keeps CLI/config parsing simple; invalid prefixes fall back to literal.
        if let Some(rest) = s.strip_prefix("env:") {
            SecretRef::EnvVar(rest.to_string())
        } else if let Some(rest) = s.strip_prefix("value:") {
            SecretRef::Literal(rest.to_string())
        } else {
            SecretRef::Literal(s.to_string())
        }
    }
}

/// Runtime secret (redacted in Debug).
///
/// `PartialEq` is deliberately not implemented for `SecretString`.
/// This forces explicit comparisons via `expose_secret()`.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecretString(String);

impl SecretString {
    #[must_use]
    pub fn new(s: String) -> Self {
        Self(s)
    }

    #[must_use]
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
        let secret = SecretRef::parse("env:OPENAI_API_KEY");
        assert!(matches!(secret, SecretRef::EnvVar(s) if s == "OPENAI_API_KEY"));
    }

    #[test]
    fn secret_ref_parse_value() {
        let secret = SecretRef::parse("value:secret123");
        assert!(matches!(secret, SecretRef::Literal(s) if s == "secret123"));
    }

    #[test]
    fn secret_ref_parse_literal() {
        let secret = SecretRef::parse("just_a_plain_secret");
        assert!(matches!(secret, SecretRef::Literal(s) if s == "just_a_plain_secret"));
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
}
