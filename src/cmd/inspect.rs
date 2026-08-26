//! `stop inspect` — resolve exactly one process.
//!
//! Selection order for non-numeric targets: exact case-insensitive name
//! match first, then case-insensitive substring. Multiple hits are reported
//! as an ambiguity (exit 3) with candidate identities so callers can retry
//! by PID; identity includes start time to guard against PID reuse.

use std::io::Write;

use serde::Serialize;
use sysinfo::Pid;

use crate::cli::{InspectArgs, parse_pid};
use crate::collector;
use crate::error::StopError;
use crate::model::ProcessInfo;
use crate::output;

#[derive(Serialize)]
struct Candidate {
    pid: u32,
    start_time: u64,
    name: String,
    user: Option<String>,
}

impl Candidate {
    fn from_process(p: &ProcessInfo) -> Self {
        Candidate {
            pid: p.pid,
            start_time: p.start_time,
            name: p.name.clone(),
            user: p.user.clone(),
        }
    }
}

#[derive(Serialize)]
struct InspectError {
    code: &'static str,
    message: String,
    target: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    candidates: Vec<Candidate>,
}

pub fn run(args: &InspectArgs) -> Result<crate::cmd::Outcome, StopError> {
    let (_metrics, all) = collector::collect()?;

    let found: Result<ProcessInfo, InspectError> = match parse_pid(&args.target).map(Pid::as_u32) {
        Some(pid) => all
            .iter()
            .find(|p| p.pid == pid)
            .cloned()
            .ok_or_else(|| not_found(&args.target)),
        None => select_by_name(&all, &args.target),
    };

    match found {
        Ok(process) => {
            if args.output.json {
                output::print_json(&process, args.output.pretty)?;
            } else {
                output::print_process_detail(&process)?;
            }
            Ok(crate::cmd::Outcome::Success)
        }
        Err(err) => {
            report_error(&err, args.output.json)?;
            if err.code == "ambiguous" {
                Ok(crate::cmd::Outcome::Ambiguous)
            } else {
                Ok(crate::cmd::Outcome::NoMatch)
            }
        }
    }
}

fn not_found(target: &str) -> InspectError {
    InspectError {
        code: "not_found",
        message: format!("no process matching '{target}'"),
        target: target.to_string(),
        candidates: Vec::new(),
    }
}

fn ambiguous(target: &str, matches: &[&ProcessInfo]) -> InspectError {
    InspectError {
        code: "ambiguous",
        message: format!(
            "{} processes match '{}'; disambiguate by PID",
            matches.len(),
            target
        ),
        target: target.to_string(),
        candidates: matches.iter().map(|p| Candidate::from_process(p)).collect(),
    }
}

fn select_by_name(all: &[ProcessInfo], target: &str) -> Result<ProcessInfo, InspectError> {
    let lower = target.to_lowercase();

    let exact: Vec<&ProcessInfo> = all
        .iter()
        .filter(|p| p.name.to_lowercase() == lower)
        .collect();
    if exact.len() == 1 {
        return Ok(exact[0].clone());
    }
    if exact.len() > 1 {
        return Err(ambiguous(target, &exact));
    }

    let partial: Vec<&ProcessInfo> = all.iter().filter(|p| p.name_matches(target)).collect();
    match partial.as_slice() {
        [p] => Ok((*p).clone()),
        [] => Err(not_found(target)),
        many => Err(ambiguous(target, many)),
    }
}

fn report_error(err: &InspectError, json: bool) -> std::io::Result<()> {
    let mut stderr = std::io::stderr().lock();
    if json {
        writeln!(stderr, "{}", serde_json::to_string(err)?)
    } else {
        writeln!(stderr, "stop: {}", err.message)?;
        for c in &err.candidates {
            writeln!(
                stderr,
                "  pid {} (started {}) {}",
                c.pid, c.start_time, c.name
            )?;
        }
        Ok(())
    }
}
