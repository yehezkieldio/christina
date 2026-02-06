//! LLM provider discriminator for API client selection.
//!
//! WHY enum instead of trait: The set of supported providers is fixed and known
//! at compile time. An enum gives us exhaustive matching, zero overhead dispatch,
//! and clear error messages when an unsupported provider is specified.
//!
//! WHY case-insensitive parsing: User-facing config (TOML, CLI args) should be
//! forgiving. "OpenAI", "openai", "OPENAI" all mean the same thing.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// Supported LLM API providers.
///
/// This enum drives client instantiation and request formatting.
/// Each variant corresponds to a distinct API contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[serde(rename = "openai", alias = "open_ai", alias = "open_a_i")]
    OpenAI,
    Azure,
    Groq,
}

impl FromStr for ProviderKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "openai" | "open_ai" | "open_a_i" => Ok(ProviderKind::OpenAI),
            "azure" => Ok(ProviderKind::Azure),
            "groq" => Ok(ProviderKind::Groq),
            _ => Err(format!("Unknown provider kind: {}", s)),
        }
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ProviderKind::OpenAI => "openai",
            ProviderKind::Azure => "azure",
            ProviderKind::Groq => "groq",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn provider_kind_from_str() {
        match ProviderKind::from_str("openai") {
            Ok(kind) => assert_eq!(kind, ProviderKind::OpenAI),
            Err(err) => panic!("unexpected error: {}", err),
        }
        match ProviderKind::from_str("azure") {
            Ok(kind) => assert_eq!(kind, ProviderKind::Azure),
            Err(err) => panic!("unexpected error: {}", err),
        }
    }

    #[test]
    fn provider_kind_case_insensitive() {
        match ProviderKind::from_str("OpenAI") {
            Ok(kind) => assert_eq!(kind, ProviderKind::OpenAI),
            Err(err) => panic!("unexpected error: {}", err),
        }
        match ProviderKind::from_str("AZURE") {
            Ok(kind) => assert_eq!(kind, ProviderKind::Azure),
            Err(err) => panic!("unexpected error: {}", err),
        }
    }

    #[test]
    fn provider_kind_invalid() {
        assert!(ProviderKind::from_str("invalid").is_err());
        assert!(ProviderKind::from_str("").is_err());
    }

    #[test]
    fn provider_kind_display() {
        assert_eq!(ProviderKind::OpenAI.to_string(), "openai");
        assert_eq!(ProviderKind::Azure.to_string(), "azure");
        assert_eq!(ProviderKind::Groq.to_string(), "groq");
    }

    #[test]
    fn provider_kind_serde_openai() {
        let serialized = serde_json::to_string(&ProviderKind::OpenAI).expect("serialize OpenAI");
        assert_eq!(serialized, "\"openai\"");

        for raw in ["openai", "open_ai", "open_a_i"] {
            let parsed: ProviderKind =
                serde_json::from_str(&format!("\"{}\"", raw)).expect("deserialize OpenAI");
            assert_eq!(parsed, ProviderKind::OpenAI);
        }
    }

    #[test]
    fn provider_kind_groq_from_str() {
        match ProviderKind::from_str("groq") {
            Ok(kind) => assert_eq!(kind, ProviderKind::Groq),
            Err(err) => panic!("unexpected error: {}", err),
        }
        match ProviderKind::from_str("GROQ") {
            Ok(kind) => assert_eq!(kind, ProviderKind::Groq),
            Err(err) => panic!("unexpected error: {}", err),
        }
    }
}
