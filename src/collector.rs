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

/// Owns one `System` across a sequence of samples.
///
/// The initial refresh establishes CPU baselines. Each [`Self::sample`]
/// refresh then compares against the previous refresh, so callers can choose
/// the spacing between samples without rebuilding the collector state.
pub struct Sampler {
    sys: System,
    first_sample: bool,
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            sys: System::new_with_specifics(refresh_kind()),
            first_sample: true,
        }
    }

    /// Refreshes and returns one plain-data sample.
    ///
    /// With `warm_up_cpu`, blocks for [`CPU_SAMPLE_INTERVAL_MS`] before the
    /// refresh so CPU usage reflects a real delta. `include_cpu` controls
    /// whether the resulting CPU readings are returned; `--fast` sets both
    /// options to false, while later points in a normal series set only
    /// `warm_up_cpu` to false. The first fast point uses the constructor's
    /// initial refresh; later points refresh against their predecessor.
    pub fn sample(
        &mut self,
        warm_up_cpu: bool,
        include_cpu: bool,
    ) -> Result<(SystemMetrics, Vec<ProcessInfo>), StopError> {
        if self.first_sample {
            if warm_up_cpu {
                std::thread::sleep(Duration::from_millis(CPU_SAMPLE_INTERVAL_MS));
                self.sys.refresh_specifics(refresh_kind());
            }
            self.first_sample = false;
        } else {
            self.sys.refresh_specifics(refresh_kind());
        }

        let users: HashMap<String, String> = Users::new_with_refreshed_list()
            .iter()
            .map(|u| (u.id().to_string(), u.name().to_string()))
            .collect();
        Ok(build_sample(&self.sys, &users, include_cpu))
    }
}

/// Collects one full system + process sample with a short-lived sampler.
pub fn collect(warm_up_cpu: bool) -> Result<(SystemMetrics, Vec<ProcessInfo>), StopError> {
    Sampler::new().sample(warm_up_cpu, warm_up_cpu)
}

fn build_sample(
    sys: &System,
    users: &HashMap<String, String>,
    include_cpu: bool,
) -> (SystemMetrics, Vec<ProcessInfo>) {
    let total_memory = sys.total_memory();
    let memory_used_percent = if total_memory > 0 {
        (sys.used_memory() as f64 / total_memory as f64 * 100.0) as f32
    } else {
        0.0
    };

    let metrics = SystemMetrics {
        cpu_percent: include_cpu.then(|| sys.global_cpu_usage()),
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
            cpu_percent: include_cpu.then(|| process.cpu_usage()),
            rss_bytes: process.memory(),
            virtual_bytes: process.virtual_memory(),
            threads: process.tasks().map(|t| t.len() as u32),
            io_read_bytes: io.total_read_bytes,
            io_written_bytes: io.total_written_bytes,
        });
    }

    (metrics, processes)
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
