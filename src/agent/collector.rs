use super::model::{IoCounters, Observation, ProcessIdentity, ProcessRecord, SCHEMA_VERSION, SystemRecord};
use crate::error::StopError;
use sysinfo::System;

pub const DEFAULT_SAMPLE_WINDOW_MS: u64 = 200;

pub fn collect_observation() -> Result<Observation, StopError> {
    let mut system = System::new_all();

    std::thread::sleep(std::time::Duration::from_millis(DEFAULT_SAMPLE_WINDOW_MS));
    system.refresh_all();

    let mut processes = Vec::with_capacity(system.processes().len());

    for (pid, process) in system.processes() {
        let disk = process.disk_usage();
        let argv = process
            .cmd()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        processes.push(ProcessRecord {
            id: ProcessIdentity {
                pid: pid.as_u32(),
                start_time_secs: process.start_time(),
            },
            ppid: process.parent().map(|parent| parent.as_u32()),
            name: process.name().to_string_lossy().into_owned(),
            exe: process
                .exe()
                .map(|path| path.to_string_lossy().into_owned()),
            argv,
            cwd: process
                .cwd()
                .map(|path| path.to_string_lossy().into_owned()),
            state: format!("{:?}", process.status()).to_lowercase(),
            user: process.user_id().map(ToString::to_string),
            cpu_percent: process.cpu_usage(),
            rss_bytes: process.memory(),
            threads: process.tasks().map(|tasks| tasks.len()),
            io: IoCounters {
                read_bytes: disk.total_read_bytes,
                write_bytes: disk.total_written_bytes,
            },
        });
    }

    Ok(Observation {
        schema: SCHEMA_VERSION.to_string(),
        observed_at: chrono::Utc::now().to_rfc3339(),
        sample_window_ms: DEFAULT_SAMPLE_WINDOW_MS,
        system: SystemRecord {
            cpu_percent: system.global_cpu_usage(),
            memory_total_bytes: system.total_memory(),
            memory_used_bytes: system.used_memory(),
        },
        processes,
    })
}
