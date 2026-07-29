use serde::Serialize;
use thiserror::Error;

/// Application-wide error type. Serialized to a string for the frontend;
/// the "duplicate:" prefix is used by the UI to detect FR-9 duplicate adds.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("network error: {0}")]
    Network(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("duplicate: {0}")]
    Duplicate(String),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("sync not configured: set a Bingqilin server URL and token first")]
    SyncNotConfigured,
    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
