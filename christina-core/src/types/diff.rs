use super::{FilePath, TokenCount};
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
    ///
    /// Uses `Arc<str>` to avoid allocating intermediate `String` copies when
    /// converting to `DiffChunk`. The Arc is cloned (cheap pointer copy) rather
    /// than allocating new String data.
    #[serde(
        serialize_with = "serialize_arc_str",
        deserialize_with = "deserialize_arc_str"
    )]
    pub content: Arc<str>,
    /// Token count for this file's diff.
    pub token_count: TokenCount,
    /// Whether this file was truncated due to size.
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_chunk_new() {
        let chunk = DiffChunk::new(
            Arc::from("diff --git a/file.txt b/file.txt"),
            vec![FilePath::from("file.txt")],
            TokenCount::new_at_least_one(5),
        );
        assert_eq!(chunk.files.len(), 1);
        assert_eq!(chunk.files[0], FilePath::from("file.txt"));
        assert_eq!(chunk.token_count, TokenCount::new_at_least_one(5));
    }

    #[test]
    fn file_diff_round_trip_fields() {
        let file = FileDiff {
            path: FilePath::from("src/lib.rs"),
            content: Arc::from("diff --git a/src/lib.rs b/src/lib.rs"),
            token_count: TokenCount::new_at_least_one(3),
            truncated: false,
        };
        assert_eq!(file.path, FilePath::from("src/lib.rs"));
        assert_eq!(file.token_count, TokenCount::new_at_least_one(3));
        assert!(!file.truncated);
    }

    #[test]
    fn max_diff_size_constant() {
        assert_eq!(MAX_DIFF_SIZE, 10 * 1024 * 1024);
    }
}
