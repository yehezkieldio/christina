use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persistent application state for crash recovery.
///
/// This struct captures the minimal state needed to recover
/// from an unexpected crash during commit message generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    /// The generated commit message that was pending user review
    pub pending_message: Option<String>,
    /// The repository path where generation occurred
    pub repo_path: Option<String>,
    /// Timestamp of when the state was saved
    pub timestamp: u64,
    /// Version of the state format for migrations
    pub version: u32,
}

impl PersistentState {
    const CURRENT_VERSION: u32 = 1;
    const STATE_FILENAME: &'static str = "christina_state.json";

    pub fn new() -> Self {
        Self {
            pending_message: None,
            repo_path: None,
            timestamp: 0,
            version: Self::CURRENT_VERSION,
        }
    }

    /// Save state to disk for crash recovery.
    pub fn save(&self) -> Result<()> {
        let state_path = Self::state_path()?;
        let json = serde_json::to_string_pretty(self).context("Failed to serialize state")?;
        std::fs::write(&state_path, json).context("Failed to write state file")?;
        Ok(())
    }

    /// Load state from disk if it exists.
    pub fn load() -> Result<Option<Self>> {
        let state_path = Self::state_path()?;
        if !state_path.exists() {
            return Ok(None);
        }

        let json = std::fs::read_to_string(&state_path).context("Failed to read state file")?;
        let state: PersistentState =
            serde_json::from_str(&json).context("Failed to deserialize state")?;

        // Check version for migrations
        if state.version != Self::CURRENT_VERSION {
            // For now, just ignore old versions
            // In the future, we could implement migrations here
            return Ok(None);
        }

        Ok(Some(state))
    }

    /// Clear saved state after successful operation.
    pub fn clear() -> Result<()> {
        let state_path = Self::state_path()?;
        if state_path.exists() {
            std::fs::remove_file(&state_path).context("Failed to remove state file")?;
        }
        Ok(())
    }

    /// Check if there's a pending message to recover.
    pub fn has_pending_recovery(&self) -> bool {
        self.pending_message.is_some()
    }

    fn state_path() -> Result<PathBuf> {
        let cache_dir = directories::ProjectDirs::from("", "", "christina")
            .map(|dirs| dirs.cache_dir().to_path_buf())
            .context("Could not determine cache directory")?;

        std::fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;

        Ok(cache_dir.join(Self::STATE_FILENAME))
    }
}

impl Default for PersistentState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn persistent_state_new() {
        let state = PersistentState::new();
        assert!(state.pending_message.is_none());
        assert!(state.repo_path.is_none());
        assert_eq!(state.version, PersistentState::CURRENT_VERSION);
    }

    #[test]
    fn persistent_state_has_pending_recovery() {
        let mut state = PersistentState::new();
        assert!(!state.has_pending_recovery());

        state.pending_message = Some("feat: test".to_string());
        assert!(state.has_pending_recovery());
    }
}
