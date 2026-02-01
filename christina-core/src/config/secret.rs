use std::fmt;

/// Generic secret container
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Secret<S> {
    Value(S),
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
