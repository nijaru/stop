//! Process facts and report contracts. Byte quantities are raw bytes in JSON.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
use std::collections::BTreeMap;

/// Native process generation, meaningful within a collection's scope_id.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub birth_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    /// Human-readable birth timestamp, not the native identity token.
    pub start_time: Option<u64>,
    pub birth_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub unavailable: BTreeMap<String, String>,
    pub ppid: Option<u32>,
    pub name: String,
    pub exe: Option<String>,
    pub cmdline: Vec<String>,
    pub cwd: Option<String>,
    pub state: String,
    pub user: Option<String>,
    pub uid: Option<String>,
    /// Percent of one core. None when not requested or without a valid baseline.
    pub cpu_percent: Option<f32>,
    pub rss_bytes: Option<u64>,
    pub virtual_bytes: Option<u64>,
    pub threads: Option<u32>,
    pub io_read_bytes: Option<u64>,
    pub io_written_bytes: Option<u64>,
}

impl ProcessInfo {
    pub fn name_matches(&self, needle: &str) -> bool {
        self.name.to_lowercase().contains(&needle.to_lowercase())
    }
    pub fn user_matches(&self, needle: &str) -> bool {
        self.user.as_deref() == Some(needle) || self.uid.as_deref() == Some(needle)
    }
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Cpu,
    Mem,
    Pid,
    Name,
}

impl SortKey {
    pub fn sort_processes(self, processes: &mut [ProcessInfo]) {
        processes.sort_by(|a, b| {
            let rank = match self {
                SortKey::Cpu => match (a.cpu_percent, b.cpu_percent) {
                    (Some(a), Some(b)) => b.total_cmp(&a),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                },
                SortKey::Mem => b.rss_bytes.cmp(&a.rss_bytes),
                SortKey::Pid => a.pid.cmp(&b.pid),
                SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            };
            rank.then_with(|| a.pid.cmp(&b.pid))
        });
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SystemMetrics {
    /// Normalized across all cores (unlike process CPU).
    pub cpu_percent: Option<f32>,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_used_percent: f32,
}

/// Acquisition is a window, not an atomic machine-wide snapshot.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CollectionMetadata {
    pub schema_version: u32,
    pub collection_started_at: String,
    pub collected_at: String,
    pub collection_duration_ms: u64,
    pub cpu_interval_ms: Option<u64>,
    pub scope_id: Option<String>,
    /// None means all visible PIDs; otherwise collection was scoped to this PID.
    pub scope_pid: Option<u32>,
    /// Processes with limited basic facts, or lost during identity-checked enrichment.
    pub inaccessible_processes: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Snapshot {
    #[serde(flatten)]
    pub collection: CollectionMetadata,
    pub total_processes: usize,
    pub matched: usize,
    pub returned: usize,
    pub truncated: bool,
    pub processes: Vec<ProcessInfo>,
}

impl Snapshot {
    pub fn finish(
        collection: CollectionMetadata,
        total_processes: usize,
        mut matched: Vec<ProcessInfo>,
        limit: Option<usize>,
    ) -> Self {
        let matched_count = matched.len();
        if let Some(n) = limit {
            matched.truncate(n);
        }
        Self {
            collection,
            total_processes,
            matched: matched_count,
            returned: matched.len(),
            truncated: matched.len() < matched_count,
            processes: matched,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SamplePoint {
    pub index: usize,
    pub requested_count: usize,
    pub interval_ms: u64,
    pub skipped_intervals: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_status: Option<String>,
    #[serde(flatten)]
    pub snapshot: Snapshot,
    #[serde(flatten)]
    pub system: SystemMetrics,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SampleReport {
    pub schema_version: u32,
    pub started_at: String,
    pub interval_ms: u64,
    pub requested_count: usize,
    pub count: usize,
    pub completed: bool,
    pub interrupted: bool,
    pub samples: Vec<SamplePoint>,
}

pub fn format_bytes_parts(bytes: u64) -> (String, String) {
    let mut value = bytes as f64;
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    (
        if unit == 0 {
            bytes.to_string()
        } else {
            format!("{value:.1}")
        },
        units[unit].into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, name: &str, cpu: Option<f32>, rss: u64) -> ProcessInfo {
        ProcessInfo {
            pid,
            start_time: Some(1),
            birth_id: Some(format!("test:{pid}")),
            unavailable: BTreeMap::new(),
            ppid: None,
            name: name.into(),
            exe: None,
            cmdline: vec![],
            cwd: None,
            state: "run".into(),
            user: None,
            uid: None,
            cpu_percent: cpu,
            rss_bytes: Some(rss),
            virtual_bytes: Some(0),
            threads: None,
            io_read_bytes: Some(0),
            io_written_bytes: Some(0),
        }
    }
    fn metadata() -> CollectionMetadata {
        CollectionMetadata {
            schema_version: SCHEMA_VERSION,
            collection_started_at: "t".into(),
            collected_at: "t".into(),
            collection_duration_ms: 0,
            cpu_interval_ms: None,
            scope_id: None,
            scope_pid: None,
            inaccessible_processes: 0,
        }
    }
    #[test]
    fn truncation_counts_are_before_limit() {
        for limit in [None, Some(1), Some(3), Some(5)] {
            let rows = (1..=3).map(|pid| process(pid, "p", None, 0)).collect();
            let snap = Snapshot::finish(metadata(), 7, rows, limit);
            assert_eq!(snap.total_processes, 7);
            assert_eq!(snap.matched, 3);
            assert_eq!(snap.returned, limit.unwrap_or(3).min(3));
            assert_eq!(snap.truncated, snap.returned < 3);
        }
    }
    #[test]
    fn sorting_is_deterministic_and_missing_cpu_is_last() {
        let mut rows = vec![
            process(30, "b", None, 100),
            process(20, "c", Some(0.0), 200),
            process(10, "a", Some(0.0), 300),
        ];
        for (sort, expected) in [
            (SortKey::Cpu, vec![10, 20, 30]),
            (SortKey::Mem, vec![10, 20, 30]),
            (SortKey::Name, vec![10, 30, 20]),
            (SortKey::Pid, vec![10, 20, 30]),
        ] {
            sort.sort_processes(&mut rows);
            assert_eq!(rows.iter().map(|p| p.pid).collect::<Vec<_>>(), expected);
        }
    }
    #[test]
    fn name_search_and_owner_matching() {
        let mut p = process(1, "PostgreSQL", None, 0);
        assert!(p.name_matches("SQL"));
        assert!(!p.name_matches("mysql"));
        p.user = Some("nick".into());
        p.uid = Some("501".into());
        assert!(p.user_matches("nick") && p.user_matches("501"));
        assert!(!p.user_matches("root"));
    }
    #[test]
    fn binary_units_are_explicit() {
        assert_eq!(format_bytes_parts(512), ("512".into(), "B".into()));
        assert_eq!(
            format_bytes_parts(2 * 1024 * 1024),
            ("2.0".into(), "MiB".into())
        );
    }
}
