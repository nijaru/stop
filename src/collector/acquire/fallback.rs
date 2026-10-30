//! Fresh sysinfo process acquisition on platforms without a native backend.

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use super::{Acquisition, Facts, Request, UserNames};
use crate::{StopError, identity};

pub(super) fn acquire(request: &Request, users: &mut UserNames) -> Result<Acquisition, StopError> {
    let basic = ProcessRefreshKind::nothing()
        .without_tasks()
        .with_memory()
        .with_user(UpdateKind::Always);
    let (selected, preliminary_total) = candidates(request, users, basic.without_memory())?;
    let births: Vec<_> = selected
        .iter()
        .map(|pid| identity::birth_id(pid.as_u32()))
        .collect();
    let mut kind = basic;
    if request.cpu {
        kind = kind.with_cpu();
    }
    if request.details {
        kind = kind
            .with_exe(UpdateKind::Always)
            .with_cwd(UpdateKind::Always)
            .with_disk_usage()
            .with_cmd(UpdateKind::Always);
    }
    // Never retain process records: sysinfo caches names across exec.
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&selected), true, kind);
    let total_processes = preliminary_total.unwrap_or_else(|| system.processes().len());
    let cpu_at = std::time::Instant::now();
    let mut records = Vec::with_capacity(selected.len());
    let mut inaccessible_processes = 0;
    for (pid, birth_before) in selected.into_iter().zip(births) {
        let id = pid.as_u32();
        let Some(p) = system.process(pid) else {
            if request.pid != Some(id) || birth_before.is_some() {
                inaccessible_processes += 1;
            }
            continue;
        };
        let cmdline = if request.details {
            p.cmd()
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect()
        } else {
            Vec::new()
        };
        let io = p.disk_usage();
        records.push(Facts {
            pid: id,
            birth_before,
            birth_after: identity::birth_id(id),
            name: p.name().to_string_lossy().into_owned(),
            ppid: p.parent().map(Pid::as_u32),
            uid: p.user_id().cloned(),
            state: p.status(),
            start_time: Some(p.start_time()),
            rss_bytes: Some(p.memory()),
            virtual_bytes: Some(p.virtual_memory()),
            cpu_counter: request.cpu.then(|| p.accumulated_cpu_time()),
            cpu_at,
            exe: request
                .details
                .then(|| p.exe().map(|p| p.to_string_lossy().into_owned()))
                .flatten(),
            cwd: request
                .details
                .then(|| p.cwd().map(|p| p.to_string_lossy().into_owned()))
                .flatten(),
            cmdline,
            threads: None,
            io_read_bytes: request.details.then_some(io.total_read_bytes),
            io_written_bytes: request.details.then_some(io.total_written_bytes),
        });
    }
    Ok(Acquisition {
        records,
        total_processes,
        inaccessible_processes,
    })
}

fn candidates(
    request: &Request,
    users: &mut UserNames,
    selectors: ProcessRefreshKind,
) -> Result<(Vec<Pid>, Option<usize>), StopError> {
    if let Some(pid) = request.pid {
        return Ok((vec![Pid::from_u32(pid)], None));
    }
    // Preliminary records only choose candidates; they are never emitted.
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
