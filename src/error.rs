//! Error types for stop.

use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StopError {
    #[error("interrupted")]
    Interrupted,

    #[error("{0}")]
    InvalidRequest(String),

    #[error("{0}")]
    Operational(String),

    #[error(transparent)]
    Resolve(#[from] crate::cmd::resolve::ResolveError),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("port lookup is unsupported on this platform")]
    UnsupportedPlatform,
}
