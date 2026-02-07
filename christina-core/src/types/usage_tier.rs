//! Usage tier influences conservative defaults and rate limits.

use serde::{Deserialize, Serialize};

/// Usage tier for rate-limit-aware defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum UsageTier {
    /// Default tier with standard limits.
    #[default]
    Standard,
    /// Free tier with stricter limits.
    Free,
}

impl std::fmt::Display for UsageTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsageTier::Standard => write!(f, "standard"),
            UsageTier::Free => write!(f, "free"),
        }
    }
}

impl std::str::FromStr for UsageTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(UsageTier::Standard),
            "free" => Ok(UsageTier::Free),
            _ => Err(format!("Unknown usage tier: {}", s)),
        }
    }
}
