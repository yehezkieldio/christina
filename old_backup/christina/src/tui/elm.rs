/// Toast severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    #[allow(dead_code)]
    Success,
    Warning,
    #[allow(dead_code)]
    Error,
}

/// Application-level messages that trigger side effects.
///
/// Components return these from their update functions to request
/// actions that require I/O or interact with the application context.
#[derive(Debug, Clone)]
#[expect(
    dead_code,
    reason = "All variants constructed and handled in app (false positive)"
)]
pub enum AppMsg {
    /// Request file staging
    StageFile(christina_core::types::FilePath),
    /// Request multiple files staging
    StageFiles(Vec<christina_core::types::FilePath>),
    /// Request file unstaging
    UnstageFile(christina_core::types::FilePath),
    /// Request navigation to a different state
    Navigate(christina_core::AppState),
    /// Commit the message (with validation)
    CommitMessage(christina_core::types::CommitMessage),
    /// Edit the message
    EditMessage(christina_core::types::CommitMessage),
    /// Edit a raw message string (e.g. invalid candidate)
    EditRawMessage(String),
    /// Regenerate the message
    RegenerateMessage,
    /// Request generation of commit message
    GenerateMessage,
    /// Cancel ongoing generation
    CancelGeneration,
    /// Request diff refresh
    RefreshDiff,
    /// Show toast notification with level
    ShowToast(String, ToastLevel),
    /// Save edited message
    SaveEditedMessage(christina_core::types::CommitMessage),
    /// Cancel editing
    CancelEdit,
    /// Restore textarea state
    RestoreTextArea(String, (usize, usize)),
    /// Set user context
    SetUserContext(Option<String>),
    /// Quit application
    Quit,
    /// No operation
    None,
}

/// Trait for components that follow Elm architecture.
///
/// Components implementing this trait:
/// 1. Have pure state (Model)
/// 2. Define their own message types (Msg)
/// 3. Provide pure update functions
/// 4. Provide pure render functions
pub trait Component {
    /// Component-specific message type
    type Msg;

    /// Update the component state based on a message.
    ///
    /// Returns a list of app-level messages that require side effects.
    /// State transitions are pure - no I/O happens in this function.
    fn update(&mut self, msg: Self::Msg) -> Vec<AppMsg>;
}
