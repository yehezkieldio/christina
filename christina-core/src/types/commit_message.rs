//! Validated commit message type enforcing Conventional Commits format.
//!
//! WHY single-line only: Git tooling (GitHub, GitLab, `git log --oneline`) expects
//! the first line to stand alone as a summary. Multi-line messages would be truncated
//! in most UI contexts, breaking user expectations.
//!
//! WHY 72 character default: Git best practice for first-line summary length.
//! Ensures readability in terminals (80 columns - 8 for indentation) and prevents
//! truncation in GitHub/GitLab UI. Configurable via ValidationMode for flexibility.
//!
//! WHY regex-based validation: Conventional Commits format (`type(scope): description`)
//! has a simple, stable grammar. Regex is sufficient, fast, and avoids parser complexity.
//! We enforce lowercase to maintain consistency across generated messages.

use std::fmt;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// Regex pattern for validating Conventional Commits format.
/// Compiled once and reused across all validations.
/// Pattern: type(scope)?: description
#[allow(clippy::expect_used)]
static CONVENTIONAL_COMMIT_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^[a-z]+(\([a-z0-9_-]+\))?:\s*\S.*$")
        .expect("Conventional commit regex pattern must be valid")
});

/// Validation mode for commit message length checks.
///
/// WHY three modes: Different teams have different policies. Strict enforces limits
/// (CI integration), Soft guides without blocking (developer experience), Disabled
/// allows custom workflows (automated tools, migrations).
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

/// A validated commit message following Conventional Commits format.
///
/// Invariants enforced at construction:
/// - Non-empty after trimming
/// - Single line (no '\n')
/// - Matches `^[a-z]+(\([a-z0-9_-]+\))?:.+$` (lowercase conventional format)
/// - Optional length limit based on ValidationMode
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommitMessage(String);

impl CommitMessage {
    /// WHY 72: Git/GitHub convention for first-line summary length.
    /// Balances readability in terminals (80 cols - indent) and UI truncation.
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

        // WHY reject newlines: Single-line summaries are a Git convention.
        // Multi-line messages would be truncated in `git log --oneline`, GitHub PR lists, etc.
        if trimmed.contains('\n') {
            return Err("Commit message must be single line".to_string());
        }

        // WHY this regex pattern:
        // - `^[a-z]+`: type in lowercase (feat, fix, docs, etc.)
        // - `(\([a-z0-9_-]+\))?`: optional scope in parens (api, ui, core)
        // - `:\s*\S.*$`: colon + optional whitespace + non-whitespace description
        // Enforces Conventional Commits with lowercase consistency for generated messages.
        if !CONVENTIONAL_COMMIT_PATTERN.is_match(trimmed) {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn valid_commit_messages() {
        assert!(CommitMessage::try_from("feat: add new feature".to_string()).is_ok());
        assert!(CommitMessage::try_from("fix: resolve bug".to_string()).is_ok());
        assert!(CommitMessage::try_from("chore: update deps".to_string()).is_ok());
        assert!(CommitMessage::try_from("docs: improve readme".to_string()).is_ok());
        assert!(CommitMessage::try_from("refactor: simplify code".to_string()).is_ok());
        assert!(CommitMessage::try_from("test: add tests".to_string()).is_ok());
    }

    #[test]
    fn invalid_commit_messages() {
        assert!(CommitMessage::try_from("no prefix".to_string()).is_err());
        assert!(CommitMessage::try_from("FEAT: wrong case".to_string()).is_err());
        assert!(CommitMessage::try_from("feat: ".to_string()).is_err());
        assert!(CommitMessage::try_from("feat:   ".to_string()).is_err());
        assert!(CommitMessage::try_from("".to_string()).is_err());
    }

    #[test]
    fn commit_message_with_scope() {
        assert!(CommitMessage::try_from("feat(api): add endpoint".to_string()).is_ok());
        assert!(CommitMessage::try_from("fix(ui): button alignment".to_string()).is_ok());
    }

    #[test]
    fn commit_message_multiline_rejected() {
        let msg = "feat: add feature\n\nDetailed description here\n\nBREAKING CHANGE: something";
        assert!(CommitMessage::try_from(msg.to_string()).is_err());
    }

    #[test]
    fn commit_message_as_ref() {
        let msg = match CommitMessage::try_from("feat: test".to_string()) {
            Ok(value) => value,
            Err(err) => panic!("unexpected error: {}", err),
        };
        let s: &str = msg.as_ref();
        assert_eq!(s, "feat: test");
    }

    #[test]
    fn validation_mode_strict() {
        let long_msg = "a".repeat(73);
        let msg = format!("feat: {}", long_msg);
        let result = CommitMessage::validate(msg, ValidationMode::Strict, Some(72));
        assert!(result.is_err());
    }

    #[test]
    fn validation_mode_soft() {
        let long_msg = "a".repeat(73);
        let msg = format!("feat: {}", long_msg);
        let result = CommitMessage::validate(msg, ValidationMode::Soft, Some(72));
        assert!(result.is_ok());
        let (_, warnings) = result.unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("exceeds recommended"));
    }

    #[test]
    fn validation_mode_disabled() {
        let long_msg = "a".repeat(200);
        let msg = format!("feat: {}", long_msg);
        let result = CommitMessage::validate(msg, ValidationMode::Disabled, Some(72));
        assert!(result.is_ok());
        let (_, warnings) = result.unwrap();
        assert!(warnings.is_empty());
    }
}
