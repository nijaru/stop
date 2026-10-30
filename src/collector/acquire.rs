//! Fresh backend records bracketed by native process births.

use std::collections::HashMap;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use super::{Request, UserNames};
use crate::{StopError, identity};

pub(super) struct Acquisition {
    pub system: System,
    pub selected: Vec<Pid>,
    pub births: HashMap<u32, Option<String>>,
    pub total_processes: usize,
}

pub(super) fn acquire(request: &Request, users: &mut UserNames) -> Result<Acquisition, StopError> {
    let basic = ProcessRefreshKind::nothing()
        .without_tasks()
        .with_memory()
        .with_user(UpdateKind::Always);
    let (selected, preliminary_total) = candidates(request, users, basic.without_memory())?;
    let births = selected
        .iter()
        .map(|pid| (pid.as_u32(), identity::birth_id(pid.as_u32())))
        .collect();
    let mut kind = basic;
    if request.cpu {
        kind = kind.with_cpu();
    }
    if request.details {
        kind = kind
            .with_exe(UpdateKind::Always)
            .with_cwd(UpdateKind::Always)
            .with_disk_usage();
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            kind = kind.with_cmd(UpdateKind::Always);
        }
    }
    // Never retain backend process records across acquisitions: sysinfo caches
    // names across exec and its macOS generation checks round births to seconds.
    // Every emitted fact must be acquired after its native before-birth check.
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&selected), true, kind);
    let total_processes = preliminary_total.unwrap_or_else(|| system.processes().len());
    Ok(Acquisition {
        system,
        selected,
        births,
        total_processes,
    })
}

fn candidates(
    request: &Request,
    users: &mut UserNames,
    selectors: ProcessRefreshKind,
) -> Result<(Vec<Pid>, Option<usize>), StopError> {
    // Bind an explicit PID before any other filters. No preliminary process
    // facts are needed just to learn the PID the caller already supplied.
    if let Some(pid) = request.pid {
        return Ok((vec![Pid::from_u32(pid)], None));
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if request.name.is_none() && request.user.is_none() {
        return Ok((native_pids()?, None));
    }
    // Filters need names/UIDs first, but only selected processes should incur
    // detail/CPU acquisition. These preliminary records are never emitted.
    let mut inventory = System::new();
    inventory.refresh_processes_specifics(ProcessesToUpdate::All, true, selectors);
    let total = inventory.processes().len();
    let name = request.name.as_ref().map(|name| name.to_lowercase());
    let selected = inventory
        .processes()
        .iter()
        .filter(|(_, p)| {
            name.as_ref()
                .is_none_or(|name| p.name().to_string_lossy().to_lowercase().contains(name))
                && request.user.as_ref().is_none_or(|user| {
                    p.user_id().is_some_and(|uid| {
                        uid.to_string() == *user || users.name(uid) == Some(user.as_str())
                    })
                })
        })
        .map(|(pid, _)| *pid)
        .collect();
    Ok((selected, Some(total)))
}

#[cfg(target_os = "linux")]
fn native_pids() -> std::io::Result<Vec<Pid>> {
    let mut pids = Vec::new();
    for entry in std::fs::read_dir("/proc")? {
        let name = entry?.file_name();
        if let Some(pid) = name.to_str().and_then(|name| name.parse::<u32>().ok())
            && pid > 0
        {
            pids.push(Pid::from_u32(pid));
        }
    }
    Ok(pids)
}

#[cfg(target_os = "macos")]
fn native_pids() -> std::io::Result<Vec<Pid>> {
    Ok(crate::libproc::pids()?
        .into_iter()
        .filter_map(|pid| u32::try_from(pid).ok().map(Pid::from_u32))
        .collect())
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn native_inventory_contains_the_current_process() {
        let pids = native_pids().unwrap();
        assert!(pids.contains(&Pid::from_u32(std::process::id())));
        assert!(pids.iter().all(|pid| pid.as_u32() > 0));
    }
}
