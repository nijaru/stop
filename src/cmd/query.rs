//! Inventory and ranking share one pipeline, with distinct defaults.

use super::Outcome;
use crate::cli::{CollectionArgs, OutputArgs, QueryArgs};
use crate::collector::{self, Request};
use crate::error::StopError;
use crate::model::{Snapshot, SortKey, SystemMetrics};
use crate::output;
use crate::runtime::Cancellation;
use serde::Serialize;

#[derive(Serialize)]
struct TopReport<'a> {
    #[serde(flatten)]
    system: &'a SystemMetrics,
    #[serde(flatten)]
    snapshot: &'a Snapshot,
}

pub fn run(
    args: &QueryArgs,
    top: bool,
    collection: &CollectionArgs,
    output_args: &OutputArgs,
    cancel: &Cancellation,
) -> Result<Outcome, StopError> {
    let sort = args
        .sort
        .unwrap_or(if top { SortKey::Cpu } else { SortKey::Pid });
    let window = collection.window(sort);
    let data = collector::collect(Request::query(args, window.is_some()), window, cancel)?;
    let mut processes = data.processes;
    sort.sort_processes(&mut processes);
    let snapshot = Snapshot::finish(
        data.metadata,
        data.total_processes,
        processes,
        args.limit.or(top.then_some(10)),
    );
    if output_args.document() {
        if top {
            output::print_json(
                &TopReport {
                    system: &data.system,
                    snapshot: &snapshot,
                },
                output_args.pretty,
                cancel,
            )?;
        } else {
            output::print_json(&snapshot, output_args.pretty, cancel)?;
        }
    } else {
        output::print_snapshot(
            &snapshot,
            top.then_some(&data.system),
            window.is_some(),
            cancel,
        )?;
    }
    Ok(if snapshot.returned == 0 {
        Outcome::NoMatch
    } else {
        Outcome::Success
    })
}
