//! One-shot process inspection and finite sampling, with explicit structured output.

pub mod cli;
pub mod cmd;
pub mod collector;
pub mod error;
pub mod identity;
pub mod model;
pub mod output;
pub mod ports;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod process_args;
pub mod runtime;

pub use cli::{Cli, Command};
pub use error::StopError;
pub use model::{
    ProcessIdentity, ProcessInfo, SamplePoint, SampleReport, Snapshot, SortKey, SystemMetrics,
    format_bytes_parts,
};
