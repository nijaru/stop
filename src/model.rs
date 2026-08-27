//! Core process model: identity, per-process facts, and snapshot envelope.
//!
//! JSON contract: `snake_case` keys, unavailable values serialize as `null`,
//! byte quantities are raw bytes (suffix-free), and timestamps are RFC 3339.

use serde::{Deserialize, Serialize};

/// Stable process identity. A PID alone is not unique over time; the start
/// time anchors identity against PID reuse so callers can correlate records
/// across invocations (`stop inspect`, future `wait`/`diff`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    /// Unix epoch seconds at which the process started.
    pub start_time: u64,
}

impl ProcessIdentity {
    pub fn new(pid: u32, start_time: u64) -> Self {
        Self { pid, start_time }
    }
}

/// P0 per-process facts: identity, lineage, location, ownership, resources.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    /// Unix epoch seconds at which the process started.
    pub start_time: u64,
    /// Parent PID; `null` when unavailable or the parent has exited.
    pub ppid: Option<u32>,
    pub name: String,
    /// Executable path; `null` when unavailable (kernel threads, permissions).
    pub exe: Option<String>,
    /// Full argument vector as reported by the OS.
    pub cmdline: Vec<String>,
    /// Working directory; `null` when unavailable.
    pub cwd: Option<String>,
    /// Lifecycle state (one of: idle, run, sleep, stop, zombie, tracing,
    /// dead, wakekill, waking, parked, lock_blocked, disk_sleep, unknown).
    pub state: String,
    /// Resolved owner username; `null` when it cannot be resolved.
    pub user: Option<String>,
    /// Raw user ID as a string; `null` when unavailable.
    pub uid: Option<String>,
    /// Total CPU usage percent across cores; can exceed 100.
    /// `null` when collection ran with `--fast` (no warm-up sample).
    pub cpu_percent: Option<f32>,
    /// Resident set size in bytes.
    pub rss_bytes: u64,
    /// Virtual memory size in bytes.
    pub virtual_bytes: u64,
    /// Thread count; `null` on platforms that do not report it (macOS).
    pub threads: Option<u32>,
    /// Cumulative bytes read from storage.
    pub io_read_bytes: u64,
    /// Cumulative bytes written to storage.
    pub io_written_bytes: u64,
}

impl ProcessInfo {
    /// Case-insensitive substring match against the process name.
    pub fn name_matches(&self, needle: &str) -> bool {
        let needle = needle.to_lowercase();
        self.name.to_lowercase().contains(&needle)
    }

    /// Equality match against resolved username or raw UID.
    pub fn user_matches(&self, needle: &str) -> bool {
        self.user.as_deref() == Some(needle) || self.uid.as_deref() == Some(needle)
    }
}

/// Sort keys for process lists.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Cpu,
    Mem,
    Pid,
    Name,
}

impl SortKey {
    pub fn sort_processes(self, processes: &mut [ProcessInfo]) {
        match self {
            SortKey::Cpu => processes.sort_by(|a, b| {
                b.cpu_percent
                    .unwrap_or(0.0)
                    .partial_cmp(&a.cpu_percent.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortKey::Mem => processes.sort_by_key(|p| std::cmp::Reverse(p.rss_bytes)),
            SortKey::Pid => processes.sort_by_key(|p| p.pid),
            SortKey::Name => processes.sort_by_cached_key(|p| p.name.to_lowercase()),
        }
    }
}

impl std::fmt::Display for SortKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SortKey::Cpu => "cpu",
            SortKey::Mem => "mem",
            SortKey::Pid => "pid",
            SortKey::Name => "name",
        };
        write!(f, "{s}")
    }
}

/// System-wide summary metrics (used by `top`).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SystemMetrics {
    /// Global CPU usage percent across all cores.
    /// `null` when collection ran with `--fast` (no warm-up sample).
    pub cpu_percent: Option<f32>,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_used_percent: f32,
}

/// One point in a `sample` time series.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SamplePoint {
    /// RFC 3339 collection timestamp.
    pub collected_at: String,
    /// Number of live processes seen by this collection.
    pub total_processes: usize,
    /// System metrics are flattened to match `top`'s JSON contract.
    #[serde(flatten)]
    pub system: SystemMetrics,
    pub processes: Vec<ProcessInfo>,
}

/// Bounded time-series result for `sample`.
#[derive(Serialize, Deserialize, Debug)]
pub struct SampleReport {
    /// RFC 3339 timestamp when sampling began.
    pub started_at: String,
    /// Requested target start-to-start period, rounded down to milliseconds.
    pub interval_ms: u64,
    /// Number of points requested and returned.
    pub count: usize,
    pub samples: Vec<SamplePoint>,
}

/// Result set for `list` and `top`: matched rows plus completeness metadata.
///
/// Invariants enforced by [`Snapshot::finish`]:
/// - `returned <= matched`
/// - `truncated == true` iff `limit` cut rows from `matched`
#[derive(Serialize, Deserialize, Debug)]
pub struct Snapshot {
    /// RFC 3339 collection timestamp.
    pub collected_at: String,
    /// Number of live processes seen by the collector.
    pub total_processes: usize,
    /// Number of processes matching the query filters.
    pub matched: usize,
    /// Number of processes actually returned after sort + limit.
    pub returned: usize,
    /// True if `matched > returned` because of a limit.
    pub truncated: bool,
    pub processes: Vec<ProcessInfo>,
}

impl Snapshot {
    pub fn finish(
        collected_at: String,
        total_processes: usize,
        mut matched: Vec<ProcessInfo>,
        limit: Option<usize>,
    ) -> Self {
        let matched_count = matched.len();
        let truncated = limit.is_some_and(|n| n < matched_count);
        if let Some(n) = limit {
            matched.truncate(n);
        }
        let returned = matched.len();
        Snapshot {
            collected_at,
            total_processes,
            matched: matched_count,
            returned,
            truncated,
            processes: matched,
        }
    }
}

/// Formats bytes into `(value, unit)` pairs for human output, e.g. `("4.2", "G")`.
pub fn format_bytes_parts(bytes: u64) -> (String, String) {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

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
        (bytes.to_string(), "B".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pid: u32, name: &str, cpu: f32, rss: u64) -> ProcessInfo {
        ProcessInfo {
            pid,
            start_time: 1_700_000_000,
            ppid: None,
            name: name.to_string(),
            exe: None,
            cmdline: vec![],
            cwd: None,
            state: "run".to_string(),
            user: None,
            uid: None,
            cpu_percent: Some(cpu),
            rss_bytes: rss,
            virtual_bytes: 0,
            threads: None,
            io_read_bytes: 0,
            io_written_bytes: 0,
        }
    }

    #[test]
    fn finish_reports_truncation_metadata() {
        let procs: Vec<_> = (0..5).map(|i| sample(i, "p", 0.0, 0)).collect();
        let snap = Snapshot::finish("t".into(), 10, procs, Some(3));
        assert_eq!(snap.total_processes, 10);
        assert_eq!(snap.matched, 5);
        assert_eq!(snap.returned, 3);
        assert!(snap.truncated);
    }

    #[test]
    fn finish_without_limit_returns_all() {
        let procs: Vec<_> = (0..5).map(|i| sample(i, "p", 0.0, 0)).collect();
        let snap = Snapshot::finish("t".into(), 7, procs.clone(), None);
        assert_eq!(snap.matched, 5);
        assert_eq!(snap.returned, 5);
        assert!(!snap.truncated);

        // Limit >= matched is not truncation.
        let snap = Snapshot::finish("t".into(), 7, procs, Some(5));
        assert!(!snap.truncated);
    }

    #[test]
    fn sorts_by_each_key() {
        let mut procs = vec![
            sample(30, "b", 50.0, 100),
            sample(10, "a", 90.0, 300),
            sample(20, "c", 10.0, 200),
        ];

        SortKey::Cpu.sort_processes(&mut procs);
        assert_eq!(
            procs.iter().map(|p| p.pid).collect::<Vec<_>>(),
            vec![10, 30, 20]
        );

        SortKey::Mem.sort_processes(&mut procs);
        assert_eq!(
            procs.iter().map(|p| p.pid).collect::<Vec<_>>(),
            vec![10, 20, 30]
        );

        SortKey::Pid.sort_processes(&mut procs);
        assert_eq!(
            procs.iter().map(|p| p.pid).collect::<Vec<_>>(),
            vec![10, 20, 30]
        );

        SortKey::Name.sort_processes(&mut procs);
        assert_eq!(
            procs.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn name_match_is_case_insensitive_substring() {
        let p = sample(1, "PostgreSQL", 0.0, 0);
        assert!(p.name_matches("postgres"));
        assert!(p.name_matches("SQL"));
        assert!(!p.name_matches("mysql"));
    }

    #[test]
    fn format_bytes_parts_uses_largest_unit() {
        assert_eq!(
            format_bytes_parts(512),
            ("512".to_string(), "B".to_string())
        );
        assert_eq!(
            format_bytes_parts(2 * 1024 * 1024),
            ("2.0".to_string(), "M".to_string())
        );
    }
}
