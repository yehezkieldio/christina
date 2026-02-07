//! Configuration types and resolution stages.
//!
//! WHY split files: keep input (config file), derived values (resolved), and
//! provider-specific helpers (Azure endpoint) distinct so validation happens
//! once and downstream code consumes a stable, fully-populated shape.

pub mod azure_endpoint;
pub mod config_file;
pub mod resolved;
pub mod secret;

pub use azure_endpoint::{AzureEndpoint, AzureEndpointError};
pub use config_file::{AdvancedConfig, ConfigFile, ExperimentalConfig, StandardConfig};
pub use resolved::ResolvedConfig;
pub use secret::{Secret, SecretRef, SecretString};
