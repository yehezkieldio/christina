//! Shared validation logic for the christina codebase.
//!
//! This module provides centralized validation functions that are used
//! across multiple modules, ensuring consistent validation rules.

use crate::constants;

/// Validates a commit message according to conventional commit standards.
///
/// Returns Ok(()) if valid, or an error message describing the issue.
pub fn validate_commit_message(message: &str, max_length: Option<usize>) -> Result<(), String> {
    let max_len = max_length.unwrap_or(constants::git::MAX_COMMIT_MESSAGE_LENGTH);

    if message.is_empty() {
        return Err("Commit message cannot be empty".to_string());
    }

    // Check overall length
    if message.len() > max_len {
        return Err(format!(
            "Commit message exceeds maximum length of {} characters",
            max_len
        ));
    }

    // Check for conventional commit format: type(scope): description
    let first_line = message.lines().next().unwrap_or("");

    // Must have a colon to separate type from description
    if !first_line.contains(':') {
        return Err(
            "Commit message must follow conventional commit format: type: description".to_string(),
        );
    }

    // Extract the part before the colon
    let type_part = first_line.split(':').next().unwrap_or("");

    // Validate the type portion
    if type_part.is_empty() {
        return Err("Commit message type cannot be empty".to_string());
    }

    // Check for valid conventional commit type
    let valid_types = [
        "feat", "fix", "docs", "style", "refactor", "perf", "test", "chore", "ci", "build",
    ];
    let base_type = type_part.split('(').next().unwrap_or("");
    if !valid_types.contains(&base_type) {
        return Err(format!(
            "Invalid commit type: {}. Must be one of: {}",
            base_type,
            valid_types.join(", ")
        ));
    }

    // Check that there's content after the colon
    let parts: Vec<&str> = first_line.splitn(2, ':').collect();
    if parts.len() < 2 || parts[1].trim().is_empty() {
        return Err("Commit message must have a description after the type".to_string());
    }

    Ok(())
}

/// Validates a provider profile name.
///
/// Profile names must be non-empty and contain only alphanumeric characters,
/// hyphens, and underscores.
pub fn validate_profile_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Profile name cannot be empty".to_string());
    }

    if name.len() > 50 {
        return Err("Profile name cannot exceed 50 characters".to_string());
    }

    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(
            "Profile name can only contain alphanumeric characters, hyphens, and underscores"
                .to_string(),
        );
    }

    Ok(())
}

/// Validates a temperature value for LLM sampling.
///
/// Temperature must be between 0.0 and 2.0 inclusive.
pub fn validate_temperature(temp: f32) -> Result<(), String> {
    if !(constants::llm::MIN_TEMPERATURE..=constants::llm::MAX_TEMPERATURE).contains(&temp) {
        return Err(format!(
            "Temperature must be between {} and {}",
            constants::llm::MIN_TEMPERATURE,
            constants::llm::MAX_TEMPERATURE
        ));
    }
    Ok(())
}

/// Validates token count limits.
///
/// Ensures the token count is within valid bounds for the given type.
pub fn validate_token_count(tokens: u32, is_input: bool) -> Result<(), String> {
    let max = if is_input {
        constants::llm::MAX_INPUT_TOKENS
    } else {
        constants::llm::MAX_OUTPUT_TOKENS
    };

    if tokens == 0 {
        return Err("Token count must be greater than 0".to_string());
    }

    if tokens > max {
        return Err(format!("Token count cannot exceed maximum of {}", max));
    }

    Ok(())
}

/// Validates commit history depth.
///
/// Depth must be between MIN_COMMIT_HISTORY_DEPTH and MAX_COMMIT_HISTORY_DEPTH.
pub fn validate_commit_history_depth(depth: usize) -> Result<(), String> {
    let min = constants::git::MIN_COMMIT_HISTORY_DEPTH;
    let max = constants::git::MAX_COMMIT_HISTORY_DEPTH;

    if depth < min || depth > max {
        return Err(format!(
            "Commit history depth must be between {} and {}",
            min, max
        ));
    }

    Ok(())
}

/// Trait for types that can be validated.
///
/// This allows consistent validation patterns across the codebase.
pub trait Validatable {
    /// Validate the instance and return an error if invalid.
    fn validate(&self) -> Result<(), String>;
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_commit_message_valid() {
        assert!(validate_commit_message("feat: add new feature", None).is_ok());
        assert!(validate_commit_message("fix(parser): handle edge case", None).is_ok());
        assert!(validate_commit_message("docs: update readme", Some(100)).is_ok());
    }

    #[test]
    fn test_validate_commit_message_empty() {
        assert!(validate_commit_message("", None).is_err());
    }

    #[test]
    fn test_validate_commit_message_no_colon() {
        assert!(validate_commit_message("invalid message", None).is_err());
    }

    #[test]
    fn test_validate_commit_message_invalid_type() {
        assert!(validate_commit_message("invalid: message", None).is_err());
    }

    #[test]
    fn test_validate_profile_name() {
        assert!(validate_profile_name("default").is_ok());
        assert!(validate_profile_name("my-profile").is_ok());
        assert!(validate_profile_name("my_profile").is_ok());
        assert!(validate_profile_name("").is_err());
        assert!(validate_profile_name("a").is_ok());
    }

    #[test]
    fn test_validate_temperature() {
        assert!(validate_temperature(0.0).is_ok());
        assert!(validate_temperature(1.0).is_ok());
        assert!(validate_temperature(2.0).is_ok());
        assert!(validate_temperature(-0.1).is_err());
        assert!(validate_temperature(2.1).is_err());
    }

    #[test]
    fn test_validate_token_count() {
        assert!(validate_token_count(1000, true).is_ok());
        assert!(validate_token_count(1000, false).is_ok());
        assert!(validate_token_count(0, true).is_err());
        assert!(validate_token_count(constants::llm::MAX_INPUT_TOKENS + 1, true).is_err());
    }

    #[test]
    fn test_validate_commit_history_depth() {
        assert!(validate_commit_history_depth(5).is_ok());
        assert!(validate_commit_history_depth(10).is_ok());
        assert!(validate_commit_history_depth(20).is_ok());
        assert!(validate_commit_history_depth(4).is_err());
        assert!(validate_commit_history_depth(21).is_err());
    }
}
