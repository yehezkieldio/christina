//! Core domain types with enforced invariants.
//!
//! WHY these types exist: Each type in this module represents a value with
//! constraints that cannot be expressed by primitive types alone. By making
//! invalid states unrepresentable, we push validation to construction time
//! and eliminate defensive checks elsewhere in the codebase.
//!
//! All types here are:
//! - Immutable after construction (no setters)
//! - Validated at creation (invalid values rejected)
//! - Cheap to clone (either Copy or using CompactString/NonZero optimizations)

pub mod backend_id;
pub mod commit;
pub mod diff;
pub mod free_tier;
pub mod model_name;
pub mod path;
pub mod provider_kind;
pub mod temperature;
pub mod tokens;
pub mod usage_tier;

pub use backend_id::GenerationId;
pub use commit::CommitMessage;
pub use diff::{DiffChunk, FileDiff, MAX_DIFF_SIZE};
pub use free_tier::FreeTierLimits;
pub use model_name::ModelName;
pub use path::FilePath;
pub use provider_kind::ProviderKind;
pub use temperature::Temperature;
pub use tokens::TokenCount;
pub use usage_tier::UsageTier;
