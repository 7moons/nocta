use serde::Serialize;
use ts_rs::TS;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Operation cancelled.")]
    Cancelled,
    #[error("Operation timed out.")]
    Timeout,
    #[error("Provider completion could not be confirmed by the join precheck in time.")]
    VerificationTimeout,
    #[error("This account already has an active operation. Stop or close it first.")]
    Busy,
    #[error("Network request failed. Check your connection and try again.")]
    Network,
    #[error("{0}")]
    Validation(String),
    #[error("The requested record no longer exists.")]
    NotFound,
    #[error("An account or folder with this name already exists.")]
    Duplicate,
    #[error("The local secret store is unavailable. {0}")]
    Vault(String),
    #[error("Cannot decrypt this data. The password or local key is incorrect.")]
    Decryption,
    #[error("Database operation failed.")]
    Database(#[from] sqlx::Error),
    #[error("File operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Stored data could not be read.")]
    Serialization(#[from] serde_json::Error),
    #[error("Browser operation failed: {0}")]
    Browser(String),
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl From<CoreError> for CommandError {
    fn from(error: CoreError) -> Self {
        let code = match &error {
            CoreError::Cancelled => "cancelled",
            CoreError::Timeout => "timeout",
            CoreError::VerificationTimeout => "verification_timeout",
            CoreError::Busy => "busy",
            CoreError::Network => "network",
            CoreError::Validation(_) => "validation",
            CoreError::NotFound => "not_found",
            CoreError::Duplicate => "duplicate",
            CoreError::Vault(_) | CoreError::Decryption => "secret_store",
            CoreError::Database(_) => "database",
            CoreError::Io(_) => "io",
            CoreError::Serialization(_) => "serialization",
            CoreError::Browser(_) => "browser",
        };
        tracing::warn!(code, "Command failed");
        Self {
            code: code.to_owned(),
            message: error.to_string(),
        }
    }
}
