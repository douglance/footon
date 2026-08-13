use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Core(#[from] footon_core::Error),
    #[error("no user or assistant prose was found")]
    NoMessages,
    #[error("unsupported source {0}; use auto, claude, or codex")]
    Source(String),
    #[error("safety scanner failed: {0}")]
    Safety(String),
    #[error("invalid share: {0}")]
    Share(String),
    #[error("invalid endpoint: {0}")]
    Endpoint(String),
    #[error("publish request failed: {0}")]
    Publish(String),
    #[error("fetch request failed: {0}")]
    Fetch(String),
}

pub type Result<T> = std::result::Result<T, Error>;
