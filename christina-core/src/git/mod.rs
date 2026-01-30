pub mod diff;
pub mod file;

pub use diff::{DiffChunk, FileDiff, MAX_DIFF_SIZE};
pub use file::{GitFile, GitFileStatus};
