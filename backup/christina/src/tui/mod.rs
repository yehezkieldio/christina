pub(crate) mod diff_executor;
pub(crate) mod diff_renderer;
pub(crate) mod elm;
pub(crate) mod form;
pub(crate) mod layout;
pub(crate) mod screens;

pub(crate) mod components_elm;

pub mod config;
pub mod context;
pub mod profiles;
pub mod theme;
pub mod widgets;

pub use components_elm::{handle_key, render};
pub use config::{ConfigTuiOptions, ConfigTuiResult, run_config_tui};
pub use context::{DataState, UiState};
pub use profiles::{ProfileTuiOptions, run_profile_tui};
pub use theme::*;
pub use widgets::ToastManager;
