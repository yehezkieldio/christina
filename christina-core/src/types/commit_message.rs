use std::fmt;

use serde::{Deserialize, Serialize};

/// Validation mode for commit message length checks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ValidationMode {
    /// Reject messages exceeding the limit
    Strict,
    /// Warn but allow messages exceeding the limit
    #[default]
    Soft,
    /// Skip length check entirely
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommitMessage(String);

impl CommitMessage {
    const DEFAULT_MAX_LENGTH: usize = 72;

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate a commit message with configurable validation mode and length limit.
    pub fn validate(
        value: String,
        mode: ValidationMode,
        max_length: Option<usize>,
    ) -> Result<(Self, Vec<String>), String> {
        let trimmed = value.trim();
        let mut warnings = Vec::new();

        if trimmed.is_empty() {
            return Err("Commit message cannot be empty".to_string());
        }

        let limit = max_length.unwrap_or(Self::DEFAULT_MAX_LENGTH);
        if trimmed.len() > limit {
            match mode {
                ValidationMode::Strict => {
                    return Err(format!("Commit message exceeds {} characters", limit));
                }
                ValidationMode::Soft => {
                    warnings.push(format!(
                        "Commit message exceeds recommended {} character limit ({} chars)",
                        limit,
                        trimmed.len()
                    ));
                }
                ValidationMode::Disabled => {}
            }
        }

        if trimmed.contains('\n') {
            return Err("Commit message must be single line".to_string());
        }

        let pattern = regex::Regex::new(r"^[a-z]+(\([a-z0-9_-]+\))?:.+$")
            .map_err(|e| format!("Regex error: {}", e))?;

        if !pattern.is_match(trimmed) {
            return Err(
                "Commit message must follow conventional commits format: type(scope): description"
                    .to_string(),
            );
        }

        Ok((Self(trimmed.to_string()), warnings))
    }
}

impl TryFrom<String> for CommitMessage {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let (msg, _warnings) = Self::validate(value, ValidationMode::default(), None)?;
        Ok(msg)
    }
}

impl fmt::Display for CommitMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for CommitMessage {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for CommitMessage {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}
