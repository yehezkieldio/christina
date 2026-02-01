pub mod dashboard;
pub mod editing;
pub mod error;
pub mod generating;
pub mod review;
pub mod staging;

pub use dashboard::{DashboardState, key_to_message as dashboard_key_to_message, render_dashboard};
pub use editing::{EditingState, key_to_message as editing_key_to_message, render_editing};
pub use error::{ErrorState, key_to_message as error_key_to_message, render_error};
pub use generating::{
    GeneratingState, key_to_message as generating_key_to_message, render_generating,
};
pub use review::{
    ReviewState, handle_enter as review_handle_enter, key_to_message as review_key_to_message,
    render_review,
};
pub use staging::{StagingState, key_to_message as staging_key_to_message, render_staging};
