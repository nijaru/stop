//! Error types for the stop CLI tool.

use crate::filter::FilterError;
use std::io;
use thiserror::Error;

/// Custom error type for stop CLI operations.
#[derive(Debug, Error)]
pub enum StopError {
    /// Error occurred during filter parsing or evaluation
    #[error("filter error: {0}")]
    Filter(#[from] FilterError),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid configuration or argument
    #[error("configuration error: {0}")]
    Config(String),
}

impl StopError {
    /// Creates a new configuration error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
}
