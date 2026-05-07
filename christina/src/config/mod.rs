//! CLI-side configuration assembly and secret resolution.
//!
//! WHY split: `settings` owns user-facing config; `profiles` re-exports core
//! profile types; `secrets` resolves env references into runtime values.

pub mod profiles;
pub mod secrets;
pub mod settings;

pub use settings::Config;
