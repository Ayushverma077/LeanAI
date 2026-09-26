use serde::Serialize;

/// A structured, display-safe error crossing the IPC boundary.
///
/// Every command returns this shape rather than a bare string, so the frontend
/// can branch on `code` and always has something actionable to render
/// (backlog 1.2).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    /// Stable machine-readable code, e.g. `outside_root`.
    pub code: String,
    /// Message safe to show a user. Contains no file contents and no absolute
    /// paths from OS error strings.
    pub message: String,
    /// What the user can do next, when there is a sensible action.
    pub recovery: Option<String>,
    /// True when retrying the same request could succeed.
    pub retryable: bool,
}

impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            recovery: None,
            retryable: false,
        }
    }

    pub fn with_recovery(mut self, recovery: impl Into<String>) -> Self {
        self.recovery = Some(recovery.into());
        self
    }

    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("internal", message).with_recovery(
            "Try again. If it keeps happening, export a diagnostic bundle from Settings.",
        )
    }
}

impl From<leanai_core::CoreError> for AppError {
    fn from(error: leanai_core::CoreError) -> Self {
        use leanai_core::CoreError as E;
        match &error {
            E::OutsideRoot(_) => AppError::new(
                "outside_root",
                "That path is outside the project you opened.",
            )
            .with_recovery("Open the other directory as its own project instead."),
            E::InvalidRoot(_) => AppError::new(
                "invalid_root",
                "That folder could not be opened as a project.",
            )
            .with_recovery("Choose a directory you have permission to read."),
            E::Cancelled => AppError::new("cancelled", "The operation was cancelled."),
            E::Unreadable { path, .. } => AppError::new(
                "unreadable",
                format!("`{}` could not be read.", path.display()),
            )
            .with_recovery("Check file permissions, then rescan.")
            .retryable(),
            E::NotAGitRepository(_) => AppError::new(
                "not_a_git_repository",
                "This project is not a git repository, so diff selection is unavailable.",
            )
            .with_recovery("Select files manually, or open a git checkout."),
            E::Git(message) => AppError::new("git", format!("Git reported: {message}")),
            E::Io(message) => AppError::new("io", message.clone()).retryable(),
            E::InvalidSelection(message) => AppError::new("invalid_selection", message.clone()),
            E::LimitExceeded(message) => AppError::new("limit_exceeded", message.clone())
                .with_recovery("Reduce the selection, or change the limit in Settings."),
            E::MissingCapability(message) => AppError::new("missing_capability", message.clone())
                .with_recovery("Select a model or provider that supports this capability."),
            E::Provider(message) => AppError::new("provider_error", message.clone()).retryable(),
            E::InvalidModel(message) => AppError::new("invalid_model", message.clone())
                .with_recovery("Verify the model format and checksum before activation."),
            E::BudgetExceeded(message) => AppError::new("budget_exceeded", message.clone())
                .with_recovery("Increase the task budget or switch to a lower-cost tier."),
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        AppError::new("database", format!("Local database error: {error}"))
            .with_recovery("Restart LeanAI. Your projects and presets are stored locally and can be exported from Settings.")
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::new("io", error.to_string()).retryable()
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;
