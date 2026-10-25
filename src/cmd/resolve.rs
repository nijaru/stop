//! Exact single-target selection. Searching is an explicit list operation.

use serde::Serialize;
use thiserror::Error;

use crate::cli::parse_pid;
use crate::model::ProcessInfo;

#[derive(Debug, Serialize)]
pub struct Candidate {
    pub pid: u32,
    pub start_time: Option<u64>,
    pub birth_id: Option<String>,
    pub name: String,
    pub user: Option<String>,
    pub exe: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Serialize, Error)]
#[error("{message}")]
pub struct ResolveError {
    pub code: &'static str,
    pub message: String,
    pub target: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<Candidate>,
}

pub fn resolve(
    all: &[ProcessInfo],
    target: &str,
    explicit_name: bool,
) -> Result<usize, ResolveError> {
    let indices: Vec<usize> =
        if let Some(pid) = (!explicit_name).then(|| parse_pid(target)).flatten() {
            all.iter()
                .enumerate()
                .filter(|(_, p)| p.pid == pid)
                .map(|(i, _)| i)
                .collect()
        } else {
            all.iter()
                .enumerate()
                .filter(|(_, p)| {
                    p.name.eq_ignore_ascii_case(target)
                        || p.name.to_lowercase() == target.to_lowercase()
                })
                .map(|(i, _)| i)
                .collect()
        };
    match indices.as_slice() {
        [index] => Ok(*index),
        [] => Err(ResolveError {
            code: "not_found",
            message: format!(
                "no visible process matching '{target}'; search with stop list --name '{target}'"
            ),
            target: target.into(),
            candidates: vec![],
        }),
        _ => {
            let mut candidates: Vec<_> = indices
                .iter()
                .map(|&i| {
                    let p = &all[i];
                    Candidate {
                        pid: p.pid,
                        start_time: p.start_time,
                        birth_id: p.birth_id.clone(),
                        name: p.name.clone(),
                        user: p.user.clone(),
                        exe: p.exe.clone(),
                        cwd: p.cwd.clone(),
                    }
                })
                .collect();
            candidates.sort_by_key(|c| c.pid);
            Err(ResolveError {
                code: "ambiguous",
                message: format!(
                    "{} processes match '{target}'; inspect one with stop inspect <pid>",
                    candidates.len()
                ),
                target: target.into(),
                candidates,
            })
        }
    }
}
