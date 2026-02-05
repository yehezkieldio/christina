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

pub mod commit_message;
pub mod file_path;
pub mod free_tier;
pub mod model_name;
pub mod provider_kind;
pub mod temperature;
pub mod token_count;
pub mod usage_tier;

pub use commit_message::CommitMessage;
pub use file_path::FilePath;
pub use free_tier::FreeTierLimits;
pub use model_name::ModelName;
pub use provider_kind::ProviderKind;
pub use temperature::Temperature;
pub use token_count::TokenCount;
pub use usage_tier::UsageTier;
