//! Resolves a user-supplied target (PID or process name) to a single
//! process in a collected snapshot.
//!
//! Numeric targets resolve by PID; otherwise by name — exact
//! case-insensitive match first, then case-insensitive substring. Multiple
//! hits are an ambiguity (exit 3): candidates carry stable identities
//! (pid + start_time) so callers can retry by PID.

use std::io::Write;

use serde::Serialize;
use sysinfo::Pid;

use crate::cli::parse_pid;
use crate::model::ProcessInfo;

#[derive(Serialize)]
pub struct Candidate {
    pub pid: u32,
    pub start_time: u64,
    pub name: String,
    pub user: Option<String>,
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
pub struct ResolveError {
    pub code: &'static str,
    pub message: String,
    pub target: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<Candidate>,
}

pub fn resolve(all: &[ProcessInfo], target: &str) -> Result<usize, ResolveError> {
    match parse_pid(target).map(Pid::as_u32) {
        Some(pid) => all
            .iter()
            .position(|p| p.pid == pid)
            .ok_or_else(|| not_found(target)),
        None => select_by_name(all, target),
    }
}

fn not_found(target: &str) -> ResolveError {
    ResolveError {
        code: "not_found",
        message: format!("no process matching '{target}'"),
        target: target.to_string(),
        candidates: Vec::new(),
    }
}

fn ambiguous(target: &str, matches: &[&ProcessInfo]) -> ResolveError {
    ResolveError {
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

fn select_by_name(all: &[ProcessInfo], target: &str) -> Result<usize, ResolveError> {
    let lower = target.to_lowercase();

    let exact: Vec<usize> = all
        .iter()
        .enumerate()
        .filter(|(_, p)| p.name.to_lowercase() == lower)
        .map(|(i, _)| i)
        .collect();
    if exact.len() == 1 {
        return Ok(exact[0]);
    }
    if exact.len() > 1 {
        let refs: Vec<&ProcessInfo> = exact.iter().map(|&i| &all[i]).collect();
        return Err(ambiguous(target, &refs));
    }

    let partial: Vec<usize> = all
        .iter()
        .enumerate()
        .filter(|(_, p)| p.name_matches(target))
        .map(|(i, _)| i)
        .collect();
    match partial.as_slice() {
        [i] => Ok(*i),
        [] => Err(not_found(target)),
        many => {
            let refs: Vec<&ProcessInfo> = many.iter().map(|&i| &all[i]).collect();
            Err(ambiguous(target, &refs))
        }
    }
}

/// Renders a resolution failure: JSON envelope on stderr in `--json` mode,
/// plain text plus one line per candidate otherwise.
pub fn report_error(err: &ResolveError, json: bool) -> std::io::Result<()> {
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
