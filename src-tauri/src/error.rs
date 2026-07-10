use serde::Serialize;

#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "type", content = "message")]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Authentication required")]
    AuthRequired,

    #[error("Wrong password")]
    WrongPassword,

    #[error("Invalid input: {0}")]
    Validation(String),

    #[error("External API error: {0}")]
    ExternalApi(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Sync file is format v{0}, this app version only reads up to v{1} — update the app to keep syncing")]
    UnsupportedBackupVersion(u8, u8),
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Database(e.to_string())
    }
}

impl From<r2d2::Error> for AppError {
    fn from(e: r2d2::Error) -> Self {
        AppError::Database(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Parse(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
