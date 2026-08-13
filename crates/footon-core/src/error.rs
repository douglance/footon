#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no user or assistant prose was found")]
    NoMessages,
    #[error("unsupported source {0}; use auto, claude, or codex")]
    Source(String),
    #[error("safety scanner failed: {0}")]
    Safety(String),
    #[error("invalid share: {0}")]
    Share(String),
}

pub type Result<T> = std::result::Result<T, Error>;
