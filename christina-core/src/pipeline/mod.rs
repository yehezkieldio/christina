//! Generation pipeline protocol and state management.
//!
//! Defines the abstract interface for AI backends and the pipeline state machine.

pub mod backend;
pub mod state;

pub use backend::LlmBackend;
pub use state::PipelineState;
