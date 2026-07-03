//! Process and system monitoring with structured output.
//!
//! Provides cross-platform system metrics collection with filtering, sorting,
//! and multiple output formats (JSON, CSV, human-readable).

pub mod data;
pub mod error;
pub mod filter;
pub mod output;
pub mod pipeline;
pub mod watch;

pub use data::{DEFAULT_TOP_N, ProcessInfo, SystemMetrics, SystemSnapshot, collect_snapshot};
pub use error::StopError;
pub use filter::FilterExpr;
pub use output::{Output, ignore_broken_pipe};
pub use pipeline::Pipeline;
