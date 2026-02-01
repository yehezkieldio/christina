pub mod adapter;
pub mod chunking;
pub mod diff_processor;
pub mod parsing;

#[expect(unused_imports, reason = "Public API re-export for future use")]
pub use adapter::status;
