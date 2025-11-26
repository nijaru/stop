//! Error types for the stop CLI tool.

use crate::filter::FilterError;
use std::io;
use thiserror::Error;

/// Custom error type for stop CLI operations.
#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
pub enum StopError {
    /// Error occurred during filter parsing or evaluation
    #[error("Filter error: {0}")]
    FilterError(#[from] FilterError),

    /// IO error (reading, writing, etc.)
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Invalid configuration or argument
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// System information collection failed
    #[error("System metrics collection failed: {0}")]
    SystemError(String),
}

impl StopError {
    /// Creates a new configuration error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::ConfigError(msg.into())
    }

    /// Creates a new system error.
    pub fn system(msg: impl Into<String>) -> Self {
        Self::SystemError(msg.into())
    }
}
