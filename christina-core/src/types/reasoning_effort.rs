//! Reasoning effort level for reasoning-capable models.
//!
//! WHY: Some models (GPT-5 series, o-series) allow configuring reasoning effort to trade
//! latency/cost for quality. This type constrains values to supported levels.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Supported reasoning effort levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    pub fn as_str(self) -> &'static str {
        match self {
            ReasoningEffort::Low => "low",
            ReasoningEffort::Medium => "medium",
            ReasoningEffort::High => "high",
        }
    }
}

impl fmt::Display for ReasoningEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ReasoningEffort {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(ReasoningEffort::Low),
            "medium" => Ok(ReasoningEffort::Medium),
            "high" => Ok(ReasoningEffort::High),
            other => Err(format!(
                "Invalid reasoning effort: {other} (expected low, medium, or high)"
            )),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_reasoning_effort() {
        assert_eq!(
            "low".parse::<ReasoningEffort>().unwrap(),
            ReasoningEffort::Low
        );
        assert_eq!(
            "MEDIUM".parse::<ReasoningEffort>().unwrap(),
            ReasoningEffort::Medium
        );
        assert_eq!(
            "High".parse::<ReasoningEffort>().unwrap(),
            ReasoningEffort::High
        );
    }

    #[test]
    fn display_reasoning_effort() {
        assert_eq!(ReasoningEffort::Low.to_string(), "low");
    }
}
