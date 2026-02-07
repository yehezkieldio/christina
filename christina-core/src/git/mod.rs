//! Git domain types used by higher-level crates.
//!
//! WHY in core: keeps git-specific data structures (repo root, snapshot shapes)
//! independent of IO and libgit2 bindings, enabling testing and reuse.

pub mod repository;
pub mod stage;

pub use repository::{RepoRoot, RepoRootError};
pub use stage::{GitFile, GitFileStatus, RepoSnapshot};
