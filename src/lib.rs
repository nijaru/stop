//! Process and system monitoring with structured output.
//!
//! JSON-first CLI: `list`, `inspect`, `top`, `tree`.

pub mod cli;
pub mod cmd;
pub mod collector;
pub mod error;
pub mod model;
pub mod output;
pub mod ports;

pub use cli::{Cli, Command};
pub use error::StopError;
pub use model::{
    ProcessIdentity, ProcessInfo, SamplePoint, SampleReport, Snapshot, SortKey, SystemMetrics,
    format_bytes_parts,
};
