use std::path::PathBuf;

/// Errors that are safe to display to a user. Payloads never contain file
/// contents, only paths and structural information.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("path `{0}` is outside the approved project root")]
    OutsideRoot(PathBuf),

    #[error("project root `{0}` does not exist or is not a directory")]
    InvalidRoot(PathBuf),

    #[error("operation was cancelled")]
    Cancelled,

    #[error("file `{path}` could not be read: {reason}")]
    Unreadable { path: PathBuf, reason: String },

    #[error("`{0}` is not a git repository")]
    NotAGitRepository(PathBuf),

    #[error("git error: {0}")]
    Git(String),

    #[error("io error: {0}")]
    Io(String),

    #[error("invalid selection: {0}")]
    InvalidSelection(String),

    #[error("policy limit exceeded: {0}")]
    LimitExceeded(String),

    #[error("missing required capability: {0}")]
    MissingCapability(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("invalid model specification: {0}")]
    InvalidModel(String),

    #[error("budget exceeded: {0}")]
    BudgetExceeded(String),
}

impl From<std::io::Error> for CoreError {
    fn from(value: std::io::Error) -> Self {
        CoreError::Io(value.to_string())
    }
}

impl From<git2::Error> for CoreError {
    fn from(value: git2::Error) -> Self {
        CoreError::Git(value.message().to_string())
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
