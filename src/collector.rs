//! Process and system collection. Owns all sysinfo state.

use std::collections::HashMap;
use std::time::Duration;

use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, ProcessRefreshKind, ProcessStatus, RefreshKind, System,
    Users,
};

use crate::error::StopError;
use crate::model::{ProcessInfo, SystemMetrics};

/// sysinfo requires two refreshes separated by an interval to produce
/// accurate CPU usage deltas.
pub const CPU_SAMPLE_INTERVAL_MS: u64 = 200;

/// Only the components stop reads: CPU, memory, processes. Avoids the
/// startup cost of disks/networks/components that `new_all` would load.
fn refresh_kind() -> RefreshKind {
    RefreshKind::nothing()
        .with_cpu(CpuRefreshKind::everything())
        .with_memory(MemoryRefreshKind::everything())
        .with_processes(ProcessRefreshKind::everything())
}

/// Collects a full system + process sample.
///
/// Blocks for [`CPU_SAMPLE_INTERVAL_MS`] between two refreshes so CPU
/// usage reflects real deltas. This is the single owner of the `System`
/// handle; callers receive plain data only.
pub fn collect() -> Result<(SystemMetrics, Vec<ProcessInfo>), StopError> {
    let kind = refresh_kind();
    let mut sys = System::new_with_specifics(kind);
    std::thread::sleep(Duration::from_millis(CPU_SAMPLE_INTERVAL_MS));
    sys.refresh_specifics(kind);

    // Username resolution: uid -> name from the live user database.
    let users: HashMap<String, String> = Users::new_with_refreshed_list()
        .iter()
        .map(|u| (u.id().to_string(), u.name().to_string()))
        .collect();

    let total_memory = sys.total_memory();
    let memory_used_percent = if total_memory > 0 {
        (sys.used_memory() as f64 / total_memory as f64 * 100.0) as f32
    } else {
        0.0
    };

    let metrics = SystemMetrics {
        cpu_percent: sys.global_cpu_usage(),
        memory_total_bytes: total_memory,
        memory_used_bytes: sys.used_memory(),
        memory_used_percent,
    };

    let mut processes = Vec::with_capacity(sys.processes().len());

    for (pid, process) in sys.processes().iter() {
        let uid = process.user_id().map(|id| id.to_string());
        let user = uid.as_ref().and_then(|uid| users.get(uid).cloned());
        let io = process.disk_usage();

        processes.push(ProcessInfo {
            pid: pid.as_u32(),
            start_time: process.start_time(),
            ppid: process.parent().map(|p| p.as_u32()),
            name: process.name().to_string_lossy().into_owned(),
            exe: process.exe().map(|p| p.display().to_string()),
            cmdline: process
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect(),
            cwd: process.cwd().map(|p| p.display().to_string()),
            state: state_name(process.status()).to_string(),
            user,
            uid,
            cpu_percent: process.cpu_usage(),
            rss_bytes: process.memory(),
            virtual_bytes: process.virtual_memory(),
            threads: process.tasks().map(|t| t.len() as u32),
            io_read_bytes: io.total_read_bytes,
            io_written_bytes: io.total_written_bytes,
        });
    }

    Ok((metrics, processes))
}

fn state_name(status: ProcessStatus) -> &'static str {
    match status {
        ProcessStatus::Idle => "idle",
        ProcessStatus::Run => "run",
        ProcessStatus::Sleep => "sleep",
        ProcessStatus::Stop => "stop",
        ProcessStatus::Zombie => "zombie",
        ProcessStatus::Tracing => "tracing",
        ProcessStatus::Dead => "dead",
        ProcessStatus::Wakekill => "wakekill",
        ProcessStatus::Waking => "waking",
        ProcessStatus::Parked => "parked",
        ProcessStatus::LockBlocked => "lock_blocked",
        ProcessStatus::UninterruptibleDiskSleep => "disk_sleep",
        _ => "unknown",
    }
}
