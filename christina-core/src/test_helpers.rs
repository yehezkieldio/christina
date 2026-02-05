//! Test utilities and helpers for christina-core tests.
//!
//! This module provides reusable testing infrastructure including:
//! - Temporary Git repositories for integration testing
//! - Deterministic tokenizers for predictable token counting
//! - Mock stdin for CLI input simulation
//! - Isolated config helpers for test isolation

#![cfg(any(test, feature = "test-helpers"))]

use std::path::Path;

use crate::{config::ResolvedConfig, tokenizer::Tokenizer, types::TokenCount};

/// Temporary Git repository for testing.
///
/// Creates an isolated temporary directory with an initialized Git repository.
/// The directory and repository are automatically cleaned up when dropped.
///
/// # Examples
///
/// ```
/// use christina_core::test_helpers::TempRepo;
///
/// let temp_repo = TempRepo::new();
/// let oid = temp_repo.commit_file("README.md", "# Test Project");
/// assert!(temp_repo.path().exists());
/// ```
pub struct TempRepo {
    _temp_dir: tempfile::TempDir,
    repo: git2::Repository,
}

impl TempRepo {
    /// Creates a new temporary directory with an initialized Git repository.
    ///
    /// # Panics
    ///
    /// Panics if the temporary directory or Git repository cannot be created.
    /// This is acceptable in test code.
    #[allow(clippy::unwrap_used)]
    pub fn new() -> Self {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let repo = git2::Repository::init(temp_dir.path()).unwrap();

        // Configure user for commits
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        Self {
            _temp_dir: temp_dir,
            repo,
        }
    }

    /// Creates a file with the given content, stages it, and commits it.
    ///
    /// # Arguments
    ///
    /// * `path` - Relative path within the repository
    /// * `content` - File content as a string
    ///
    /// # Returns
    ///
    /// The Git object ID (OID) of the created commit.
    ///
    /// # Panics
    ///
    /// Panics if file creation, staging, or committing fails.
    /// This is acceptable in test code.
    #[allow(clippy::unwrap_used)]
    pub fn commit_file(&self, path: &str, content: &str) -> git2::Oid {
        let file_path = self.repo.path().parent().unwrap().join(path);

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }

        std::fs::write(&file_path, content).unwrap();

        // Stage the file
        let mut index = self.repo.index().unwrap();
        index.add_path(Path::new(path)).unwrap();
        index.write().unwrap();

        // Create commit
        let tree_id = index.write_tree().unwrap();
        let tree = self.repo.find_tree(tree_id).unwrap();
        let sig = self.repo.signature().unwrap();
        let parent_commit = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok());

        let commit_message = format!("Add {}", path);

        if let Some(parent) = parent_commit {
            self.repo
                .commit(Some("HEAD"), &sig, &sig, &commit_message, &tree, &[&parent])
                .unwrap()
        } else {
            self.repo
                .commit(Some("HEAD"), &sig, &sig, &commit_message, &tree, &[])
                .unwrap()
        }
    }

    /// Returns a reference to the underlying Git repository.
    pub fn repo(&self) -> &git2::Repository {
        &self.repo
    }

    /// Returns the path to the temporary directory.
    pub fn path(&self) -> &Path {
        self.repo.path().parent().unwrap_or(self.repo.path())
    }
}

impl Default for TempRepo {
    fn default() -> Self {
        Self::new()
    }
}

/// Deterministic tokenizer that counts tokens by whitespace splitting.
///
/// This tokenizer provides predictable, fast token counting for tests
/// by treating each whitespace-separated word as a token.
///
/// # Examples
///
/// ```
/// use christina_core::test_helpers::DeterministicTokenizer;
/// use christina_core::tokenizer::Tokenizer;
/// use christina_core::types::TokenCount;
///
/// let tokenizer = DeterministicTokenizer;
/// let count = tokenizer.count_tokens("hello world");
/// assert_eq!(count, TokenCount::new(2).unwrap());
/// ```
pub struct DeterministicTokenizer;

impl Tokenizer for DeterministicTokenizer {
    /// Counts tokens by splitting on whitespace.
    ///
    /// Empty strings and whitespace-only strings return a token count of 0.
    fn count_tokens(&self, text: &str) -> TokenCount {
        if text.is_empty() {
            return TokenCount::new_at_least_one(0);
        }
        let count = text.split_whitespace().count();
        TokenCount::new_at_least_one(count as u32)
    }

    fn encoding_name(&self) -> &str {
        "deterministic-whitespace"
    }

    /// Encodes text by converting each character to its Unicode code point.
    fn encode(&self, text: &str) -> Vec<u32> {
        text.chars().map(|c| c as u32).collect()
    }

    /// Decodes token IDs back to text by converting code points to characters.
    ///
    /// Invalid Unicode code points are filtered out.
    fn decode(&self, tokens: &[u32]) -> Option<String> {
        tokens
            .iter()
            .filter_map(|&token| char::from_u32(token))
            .collect::<String>()
            .into()
    }
}

/// Mock stdin for simulating user input in CLI tests.
///
/// Provides a sequence of preset responses that can be consumed
/// one at a time via `read_line`.
///
/// # Examples
///
/// ```
/// use christina_core::test_helpers::MockStdin;
///
/// let mut mock = MockStdin::new(vec!["yes".to_string(), "no".to_string()]);
/// assert_eq!(mock.read_line(), Some("yes".to_string()));
/// assert_eq!(mock.read_line(), Some("no".to_string()));
/// assert_eq!(mock.read_line(), None);
/// ```
pub struct MockStdin {
    inputs: Vec<String>,
    position: usize,
}

impl MockStdin {
    /// Creates a new mock stdin with preset responses.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Vector of strings to return in sequence
    pub fn new(inputs: Vec<String>) -> Self {
        Self {
            inputs,
            position: 0,
        }
    }

    /// Returns the next input string, or `None` if all inputs have been consumed.
    pub fn read_line(&mut self) -> Option<String> {
        if self.position < self.inputs.len() {
            let result = self.inputs[self.position].clone();
            self.position += 1;
            Some(result)
        } else {
            None
        }
    }
}

/// Creates an isolated configuration for testing.
///
/// Returns a default `ResolvedConfig` and a temporary directory.
/// Both must be kept alive for the duration of the test.
///
/// # Returns
///
/// A tuple of `(ResolvedConfig, TempDir)` where the `TempDir` must be kept
/// in scope to prevent premature cleanup.
///
/// # Examples
///
/// ```
/// use christina_core::test_helpers::temp_config;
///
/// let (config, _temp_dir) = temp_config();
/// assert_eq!(config.commit_message_max_length, 72);
/// ```
///
/// # Panics
///
/// Panics if temporary directory creation fails. This is acceptable in test code.
#[allow(clippy::expect_used)]
pub fn temp_config() -> (ResolvedConfig, tempfile::TempDir) {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir for config");
    let config = ResolvedConfig::default();
    (config, temp_dir)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn temp_repo_creates_repository() {
        let temp_repo = TempRepo::new();
        assert!(temp_repo.path().exists());
        assert!(temp_repo.repo().path().exists());
    }

    #[test]
    fn temp_repo_commit_file_creates_commit() {
        let temp_repo = TempRepo::new();
        let oid = temp_repo.commit_file("test.txt", "Hello, World!");

        // Verify commit exists
        let commit = temp_repo.repo().find_commit(oid).unwrap();
        assert!(commit.message().unwrap().contains("Add test.txt"));

        // Verify file exists and has correct content
        let file_path = temp_repo.path().join("test.txt");
        assert!(file_path.exists());
        let content = std::fs::read_to_string(file_path).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn temp_repo_multiple_commits() {
        let temp_repo = TempRepo::new();
        let oid1 = temp_repo.commit_file("file1.txt", "First");
        let oid2 = temp_repo.commit_file("file2.txt", "Second");

        assert_ne!(oid1, oid2);
        assert!(temp_repo.repo().find_commit(oid1).is_ok());
        assert!(temp_repo.repo().find_commit(oid2).is_ok());
    }

    #[test]
    fn temp_repo_nested_paths() {
        let temp_repo = TempRepo::new();
        let oid = temp_repo.commit_file("src/main.rs", "fn main() {}");

        let commit = temp_repo.repo().find_commit(oid).unwrap();
        assert!(commit.message().unwrap().contains("Add src/main.rs"));

        let file_path = temp_repo.path().join("src/main.rs");
        assert!(file_path.exists());
    }

    #[test]
    fn deterministic_tokenizer_count_tokens() {
        let tokenizer = DeterministicTokenizer;
        assert_eq!(
            tokenizer.count_tokens("hello world"),
            TokenCount::new(2).unwrap()
        );
        assert_eq!(tokenizer.count_tokens("one"), TokenCount::new(1).unwrap());
        assert_eq!(tokenizer.count_tokens(""), TokenCount::new_at_least_one(0));
        assert_eq!(
            tokenizer.count_tokens("   \t  \n  "),
            TokenCount::new_at_least_one(0)
        );
    }

    #[test]
    fn deterministic_tokenizer_encoding_name() {
        let tokenizer = DeterministicTokenizer;
        assert_eq!(tokenizer.encoding_name(), "deterministic-whitespace");
    }

    #[test]
    fn deterministic_tokenizer_encode_decode() {
        let tokenizer = DeterministicTokenizer;
        let text = "Hello World";
        let encoded = tokenizer.encode(text);
        let decoded = tokenizer.decode(&encoded).unwrap();
        assert_eq!(decoded, text);
    }

    #[test]
    fn deterministic_tokenizer_slice_to_token_limit() {
        let tokenizer = DeterministicTokenizer;
        let text = "hello world test sentence";
        let result = tokenizer.slice_to_token_limit(text, TokenCount::new(2).unwrap());
        assert_eq!(tokenizer.count_tokens(result), TokenCount::new(2).unwrap());
    }

    #[test]
    fn deterministic_tokenizer_utf8_boundaries() {
        let tokenizer = DeterministicTokenizer;
        let text = "Hello 👋 World";
        let result = tokenizer.slice_to_token_limit(text, TokenCount::new(2).unwrap());

        // Result should preserve UTF-8 boundaries
        assert!(result.is_char_boundary(result.len()));
        assert_eq!(tokenizer.count_tokens(result), TokenCount::new(2).unwrap());
    }

    #[test]
    fn mock_stdin_basic_usage() {
        let mut mock = MockStdin::new(vec!["first".to_string(), "second".to_string()]);
        assert_eq!(mock.read_line(), Some("first".to_string()));
        assert_eq!(mock.read_line(), Some("second".to_string()));
        assert_eq!(mock.read_line(), None);
        assert_eq!(mock.read_line(), None);
    }

    #[test]
    fn mock_stdin_empty() {
        let mut mock = MockStdin::new(vec![]);
        assert_eq!(mock.read_line(), None);
    }

    #[test]
    fn mock_stdin_single_input() {
        let mut mock = MockStdin::new(vec!["only".to_string()]);
        assert_eq!(mock.read_line(), Some("only".to_string()));
        assert_eq!(mock.read_line(), None);
    }

    #[test]
    fn temp_config_returns_default() {
        let (config, _temp_dir) = temp_config();
        assert_eq!(config.commit_message_max_length, 72);
        assert!(!config.include_file_diffs);
        assert!(config.active_profile.is_none());
    }

    #[test]
    fn temp_config_temp_dir_persists() {
        let (config, temp_dir) = temp_config();
        assert!(temp_dir.path().exists());
        assert_eq!(config.commit_message_max_length, 72);
    }
}
