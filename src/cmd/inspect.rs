//! Detailed single-target inspection and visible port ownership.

use super::{Outcome, resolve};
use crate::cli::{CollectionArgs, InspectArgs, OutputArgs, parse_pid};
use crate::collector::{self, Request};
use crate::error::StopError;
use crate::model::{CollectionMetadata, ProcessInfo};
use crate::runtime::Cancellation;
use crate::{output, ports};
use chrono::Utc;
use serde::Serialize;
use std::time::Instant;

#[derive(Serialize)]
struct PortInspectReport<'a> {
    schema_version: u32,
    scope_id: &'a Option<String>,
    process_collection: &'a CollectionMetadata,
    scan_started_at: String,
    scanned_at: String,
    scan_duration_ms: u64,
    #[serde(flatten)]
    report: &'a ports::PortReport,
}

#[derive(Serialize)]
struct InspectReport<'a> {
    #[serde(flatten)]
    collection: &'a CollectionMetadata,
    target: &'a str,
    process: Option<&'a ProcessInfo>,
}

pub fn run(
    args: &InspectArgs,
    collection: &CollectionArgs,
    output_args: &OutputArgs,
    cancel: &Cancellation,
) -> Result<Outcome, StopError> {
    let window = collection.cpu;
    if let Some(port) = args.port {
        let data = collector::collect(
            Request {
                cpu: window.is_some(),
                ..Request::default()
            },
            window,
            cancel,
        )?;
        let scan_start = Instant::now();
        let scan_started_at = Utc::now().to_rfc3339();
        let report = ports::inspect(port, &data.processes)?;
        let envelope = PortInspectReport {
            schema_version: crate::model::SCHEMA_VERSION,
            scope_id: &data.metadata.scope_id,
            process_collection: &data.metadata,
            scan_started_at,
            scanned_at: Utc::now().to_rfc3339(),
            scan_duration_ms: collector::millis(scan_start.elapsed()),
            report: &report,
        };
        if output_args.document() {
            output::print_json(&envelope, output_args.pretty, cancel)?;
        } else {
            output::print_port_report(&report, window.is_some(), cancel)?;
        }
        return Ok(if report.owners.is_empty() {
            Outcome::NoMatch
        } else {
            Outcome::Success
        });
    }
    let explicit = args.name.is_some();
    let target = args
        .name
        .as_deref()
        .or(args.target.as_deref())
        .expect("clap requires one inspect selector");
    let request = Request {
        pid: (!explicit).then(|| parse_pid(target)).flatten(),
        name: if explicit || parse_pid(target).is_none() {
            Some(target.into())
        } else {
            None
        },
        details: true,
        cpu: window.is_some(),
        ..Request::default()
    };
    let data = collector::collect(request, window, cancel)?;
    let index = match resolve::resolve(&data.processes, target, explicit) {
        Ok(index) => Some(index),
        Err(err) if err.code == "not_found" => None,
        Err(err) => return Err(err.into()),
    };
    let process = index.map(|i| &data.processes[i]);
    if output_args.document() {
        output::print_json(
            &InspectReport {
                collection: &data.metadata,
                target,
                process,
            },
            output_args.pretty,
            cancel,
        )?;
    } else if let Some(process) = process {
        output::print_process_detail(process, &data.metadata, window.is_some(), cancel)?;
    } else {
        output::print_no_match(target, &data.metadata, cancel)?;
    }
    Ok(if process.is_some() {
        Outcome::Success
    } else {
        Outcome::NoMatch
    })
}
