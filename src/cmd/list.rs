//! `stop list` — query the live process table.

use chrono::Utc;

use crate::cli::ListArgs;
use crate::collector;
use crate::error::StopError;
use crate::model::{ProcessInfo, Snapshot};
use crate::output;

pub fn run(args: &ListArgs) -> Result<crate::cmd::Outcome, StopError> {
    let (_metrics, all) = collector::collect(!args.collection.fast)?;

    let total_processes = all.len();
    let mut matched: Vec<ProcessInfo> = all
        .iter()
        .filter(|p| args.name.as_ref().is_none_or(|n| p.name_matches(n)))
        .filter(|p| args.user.as_ref().is_none_or(|u| p.user_matches(u)))
        .cloned()
        .collect();

    args.sort.sort_processes(&mut matched);

    let snapshot = Snapshot::finish(
        Utc::now().to_rfc3339(),
        total_processes,
        matched,
        args.limit,
    );

    if args.output.json {
        // Structured consumers get the envelope even on no-match.
        output::print_json(&snapshot, args.output.pretty)?;
        if snapshot.returned == 0 {
            return Ok(crate::cmd::Outcome::NoMatch);
        }
    } else {
        if snapshot.returned == 0 {
            eprintln!(
                "stop: no processes matched{}",
                args.name
                    .as_deref()
                    .map(|n| format!(" name='{n}'"))
                    .unwrap_or_default()
            );
            return Ok(crate::cmd::Outcome::NoMatch);
        }
        output::print_process_table(&snapshot.processes, None)?;
    }

    Ok(crate::cmd::Outcome::Success)
}
