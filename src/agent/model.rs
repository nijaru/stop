use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "stop/1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub start_time_secs: u64,
}

impl ProcessIdentity {
    #[must_use]
    pub fn stable_id(&self) -> String {
        format!("{}:{}", self.pid, self.start_time_secs)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IoCounters {
    pub read_bytes: u64,
    pub write_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessRecord {
    pub id: ProcessIdentity,
    pub ppid: Option<u32>,
    pub name: String,
    pub exe: Option<String>,
    pub argv: Vec<String>,
    pub cwd: Option<String>,
    pub state: String,
    pub user: Option<String>,
    pub cpu_percent: f32,
    pub rss_bytes: u64,
    pub threads: Option<usize>,
    pub io: IoCounters,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemRecord {
    pub cpu_percent: f32,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub schema: String,
    pub observed_at: String,
    pub sample_window_ms: u64,
    pub system: SystemRecord,
    pub processes: Vec<ProcessRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultMeta {
    pub complete: bool,
    pub matched: usize,
    pub returned: usize,
    pub truncated: bool,
    pub sample_window_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessResult {
    pub schema: String,
    pub observed_at: String,
    pub system: SystemRecord,
    pub processes: Vec<ProcessRecord>,
    pub meta: ResultMeta,
}
