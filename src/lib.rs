//! Process and system monitoring with structured output.
//!
//! The existing modules back the stable `stop` binary. The `agent` module is the
//! experimental ground-up path for the agent-first redesign on this branch.

pub mod agent;
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
