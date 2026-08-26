//! Error types for stop.

use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StopError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
