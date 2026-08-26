//! `stop top` — system summary plus ranked top-N processes.

use chrono::Utc;

use crate::cli::TopArgs;
use crate::collector;
use crate::error::StopError;
use crate::model::{Snapshot, SystemMetrics};
use crate::output;

/// One-shot system view. JSON shape: `{ collected_at, system metrics, snapshot }`.
/// `Snapshot.collected_at` doubles as the report timestamp via flatten.
#[derive(serde::Serialize)]
struct TopReport {
    #[serde(flatten)]
    system: SystemMetrics,
    #[serde(flatten)]
    snapshot: Snapshot,
}

pub fn run(args: &TopArgs) -> Result<crate::cmd::Outcome, StopError> {
    let (system, all) = collector::collect()?;

    let total_processes = all.len();
    let mut ranked = all; // consume: nothing else reads the full table
    args.sort.sort_processes(&mut ranked);

    let snapshot = Snapshot::finish(
        Utc::now().to_rfc3339(),
        total_processes,
        ranked,
        Some(args.limit),
    );

    if snapshot.returned == 0 {
        return Ok(crate::cmd::Outcome::NoMatch);
    }

    if args.output.json {
        let report = TopReport { system, snapshot };
        output::print_json(&report, args.output.pretty)?;
    } else {
        output::print_process_table(&snapshot.processes, Some(&output::system_header(&system)))?;
    }

    Ok(crate::cmd::Outcome::Success)
}
