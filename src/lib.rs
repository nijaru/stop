//! Process and system monitoring with structured output.
//!
//! JSON-first CLI: `list`, `inspect`, `top`.

pub mod cli;
pub mod cmd;
pub mod collector;
pub mod error;
pub mod model;
pub mod output;

pub use cli::{Cli, Command};
pub use error::StopError;
pub use model::{
    ProcessIdentity, ProcessInfo, Snapshot, SortKey, SystemMetrics, format_bytes_parts,
};
