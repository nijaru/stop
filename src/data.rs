//! Data types and collection for system snapshots.

use crate::error::StopError;
use serde::{Deserialize, Serialize};
use sysinfo::System;

/// Minimum interval for CPU usage calculation (milliseconds).
pub const CPU_SAMPLE_INTERVAL_MS: u64 = 200;

/// Default number of processes to show when --top-n is not specified.
pub const DEFAULT_TOP_N: usize = 20;

/// Byte size constants for human-readable formatting.
const KB: f64 = 1024.0;
const MB: f64 = 1024.0 * 1024.0;
const GB: f64 = 1024.0 * 1024.0 * 1024.0;
const TB: f64 = 1024.0 * 1024.0 * 1024.0 * 1024.0;

/// A snapshot of system and process metrics at a point in time.
#[derive(Serialize, Deserialize, Debug)]
pub struct SystemSnapshot {
    /// ISO 8601 timestamp (RFC3339)
    pub timestamp: String,
    /// System-wide metrics
    pub system: SystemMetrics,
    /// List of process information
    pub processes: Vec<ProcessInfo>,
}

/// System-wide metrics (CPU, memory).
#[derive(Serialize, Deserialize, Debug)]
pub struct SystemMetrics {
    /// Global CPU usage percentage (0-100)
    pub cpu_usage: f32,
    /// Total system memory in bytes
    pub memory_total: u64,
    /// Used system memory in bytes
    pub memory_used: u64,
    /// Memory usage percentage (0-100)
    pub memory_percent: f32,
}

/// Information about a single process.
#[derive(Serialize, Deserialize, Debug)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// CPU usage percentage (0-100+)
    pub cpu_percent: f32,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// Memory usage percentage (0-100)
    pub memory_percent: f32,
    /// User ID (may be numeric string like "501")
    pub user: String,
    /// Full command line
    pub command: String,
    /// Number of threads
    pub thread_count: usize,
    /// Total bytes read from disk
    pub disk_read_bytes: u64,
    /// Total bytes written to disk
    pub disk_write_bytes: u64,
    /// Number of open file descriptors (None if unavailable)
    pub open_files: Option<usize>,
}

/// Formats bytes into human-readable value and unit for aligned display.
///
/// Returns `(value_string, unit_string)` — e.g. `("4.2", "G")` or `("512", "B")`.
pub fn format_bytes_parts(bytes: u64) -> (String, String) {
    let bytes_f = bytes as f64;

    if bytes_f >= TB {
        (format!("{:.1}", bytes_f / TB), "T".to_string())
    } else if bytes_f >= GB {
        (format!("{:.1}", bytes_f / GB), "G".to_string())
    } else if bytes_f >= MB {
        (format!("{:.1}", bytes_f / MB), "M".to_string())
    } else if bytes_f >= KB {
        (format!("{:.1}", bytes_f / KB), "K".to_string())
    } else {
        (format!("{}", bytes), "B".to_string())
    }
}

/// Sorts processes in-place by the specified metric.
///
/// Sort keys: `"cpu"` or `"mem"`/`"memory"` (descending), `"pid"` (ascending),
/// `"name"` (case-insensitive ascending). Defaults to CPU descending for unknown keys.
pub fn sort_processes(processes: &mut [ProcessInfo], sort_by: &str) {
    match sort_by.to_lowercase().as_str() {
        "cpu" => processes.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "mem" | "memory" => processes.sort_by(|a, b| {
            b.memory_percent
                .partial_cmp(&a.memory_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "pid" => processes.sort_by_key(|p| p.pid),
        "name" => processes.sort_by_cached_key(|p| p.name.to_lowercase()),
        _ => {
            eprintln!(
                "Warning: Unknown sort field '{sort_by}', using 'cpu'. Valid: cpu, mem, pid, name"
            );
            processes.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }
}

/// Collects a snapshot of system and process metrics.
///
/// Sleeps for 200ms to allow accurate CPU usage calculation as required by sysinfo.
pub fn collect_snapshot() -> Result<SystemSnapshot, StopError> {
    let mut sys = System::new_all();

    std::thread::sleep(std::time::Duration::from_millis(CPU_SAMPLE_INTERVAL_MS));
    sys.refresh_all();

    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let memory_percent = if total_memory > 0 {
        (used_memory as f64 / total_memory as f64 * 100.0) as f32
    } else {
        0.0
    };

    let global_cpu_usage = sys.global_cpu_usage();

    let process_count = sys.processes().len();
    let mut processes = Vec::with_capacity(process_count);

    for (pid, process) in sys.processes().iter() {
        processes.push({
            let cmd_vec: Vec<String> = process
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect();

            let disk_usage = process.disk_usage();
            let (disk_read, disk_write) =
                (disk_usage.total_read_bytes, disk_usage.total_written_bytes);

            ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().into_owned(),
                cpu_percent: process.cpu_usage(),
                memory_bytes: process.memory(),
                memory_percent: if total_memory > 0 {
                    (process.memory() as f64 / total_memory as f64 * 100.0) as f32
                } else {
                    0.0
                },
                user: process
                    .user_id()
                    .map(|uid| uid.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                command: cmd_vec.join(" "),
                thread_count: process.tasks().map(|t| t.len()).unwrap_or(1),
                disk_read_bytes: disk_read,
                disk_write_bytes: disk_write,
                open_files: process.open_files(),
            }
        });
    }

    Ok(SystemSnapshot {
        timestamp: chrono::Utc::now().to_rfc3339(),
        system: SystemMetrics {
            cpu_usage: global_cpu_usage,
            memory_total: total_memory,
            memory_used: used_memory,
            memory_percent,
        },
        processes,
    })
}
