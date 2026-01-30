pub mod chunking;
pub mod diff_processor;
pub mod parsing;
pub mod repository;

mod buffer_pool;

pub use christina_core::error::GitError;
pub use diff_processor::DiffProcessor;
pub use repository::GitRepository;
