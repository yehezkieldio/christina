use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{config::SecretRef, profile::ProviderProfile};

/// On-disk configuration representation (serde-friendly)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigFile {
    /// Active profile name
    pub active_profile: Option<String>,

    /// Provider profiles (with SecretRef for file storage)
    pub profiles: HashMap<String, ProviderProfile<SecretRef>>,

    /// Maximum commit message length (None = 72)
    pub commit_message_max_length: Option<usize>,

    /// Files to exclude from AI processing
    pub ignore_files: Vec<String>,

    /// Whether to enable file diffs in output
    pub include_file_diffs: bool,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            active_profile: None,
            profiles: HashMap::new(),
            commit_message_max_length: None,
            ignore_files: vec![
                "Cargo.lock".to_string(),
                "package-lock.json".to_string(),
                "yarn.lock".to_string(),
                "pnpm-lock.yaml".to_string(),
                "*.lock".to_string(),
            ],
            include_file_diffs: false,
        }
    }
}
