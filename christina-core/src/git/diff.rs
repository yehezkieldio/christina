use crate::types::{FilePath, TokenCount};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;

fn serialize_arc_str<S>(arc: &Arc<str>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(arc)
}

fn deserialize_arc_str<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer).map(|s| Arc::from(s.as_str()))
}

pub const MAX_DIFF_SIZE: usize = 10 * 1024 * 1024; // 10MB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffChunk {
    #[serde(
        serialize_with = "serialize_arc_str",
        deserialize_with = "deserialize_arc_str"
    )]
    pub content: Arc<str>,
    pub files: Vec<FilePath>,
    pub token_count: TokenCount,
}

impl DiffChunk {
    pub fn new(content: Arc<str>, files: Vec<FilePath>, token_count: TokenCount) -> Self {
        Self {
            content,
            files,
            token_count,
        }
    }
}

/// A single file's diff content with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    /// The file path (extracted from diff header).
    pub path: FilePath,
    /// The full diff content for this file.
    pub content: String,
    /// Token count for this file's diff.
    pub token_count: TokenCount,
    /// Whether this file was truncated due to size.
    pub truncated: bool,
}
