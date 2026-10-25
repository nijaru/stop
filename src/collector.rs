//! Owns sysinfo state, native identity validation and CPU counter baselines.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use chrono::Utc;
use sysinfo::{
    MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System,
    UpdateKind, Users,
};

use crate::cli::QueryArgs;
use crate::error::StopError;
use crate::identity;
use crate::model::{CollectionMetadata, ProcessIdentity, ProcessInfo, SystemMetrics};
use crate::runtime::Cancellation;

#[derive(Default)]
pub struct Request {
    pub pid: Option<u32>,
    pub name: Option<String>,
    pub user: Option<String>,
    pub details: bool,
    pub cpu: bool,
}

impl Request {
    pub fn query(query: &QueryArgs, cpu: bool) -> Self {
        Self {
            pid: query.pid,
            name: query.name.clone(),
            user: query.user.clone(),
            cpu,
            details: false,
        }
    }
}

#[derive(Default)]
pub enum TargetObservation {
    #[default]
    NotObserved,
    Unavailable,
    Observed(ProcessIdentity),
}

pub struct Collection {
    pub target: TargetObservation,
    pub metadata: CollectionMetadata,
    pub system: SystemMetrics,
    pub total_processes: usize,
    pub processes: Vec<ProcessInfo>,
}

struct Baseline {
    birth_id: String,
    counter: u64,
    at: Instant,
}

pub struct Sampler {
    sys: System,
    request: Request,
    users: HashMap<String, String>,
    baselines: HashMap<u32, Baseline>,
    system_baseline: Option<Instant>,
    scope_id: Option<String>,
}

impl Sampler {
    pub fn new(request: Request) -> Self {
        // Resolve account names once per finite command, never once per process.
        let users = Users::new_with_refreshed_list()
            .iter()
            .map(|u| (u.id().to_string(), u.name().into()))
            .collect();
        Self {
            sys: System::new(),
            request,
            users,
            baselines: HashMap::new(),
            system_baseline: None,
            scope_id: identity::scope_id(),
        }
    }

    /// Earliest acquisition that can produce fresh counters for this sampler.
    pub fn next_cpu_deadline(&self) -> Option<Instant> {
        self.baselines
            .values()
            .map(|b| b.at)
            .chain(self.system_baseline)
            .max()
            .map(|at| at + sysinfo::MINIMUM_CPU_UPDATE_INTERVAL)
    }

    pub fn sample(&mut self) -> Collection {
        let start = Instant::now();
        let collection_started_at = Utc::now().to_rfc3339();
        self.sys
            .refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        let cpu_at = Instant::now();
        let cpu_interval = self.system_baseline.map(|prev| cpu_at.duration_since(prev));
        let refresh_cpu = self.request.cpu
            && cpu_interval.is_none_or(|d| d >= sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let cpu_percent = if refresh_cpu {
            self.sys.refresh_cpu_usage();
            self.system_baseline = Some(cpu_at);
            cpu_interval.map(|_| self.sys.global_cpu_usage())
        } else {
            None
        };

        let pids = self.request.pid.map(|p| [Pid::from_u32(p)]);
        let scope = match &pids {
            Some(pids) => ProcessesToUpdate::Some(pids),
            None => ProcessesToUpdate::All,
        };
        let basic = ProcessRefreshKind::nothing()
            .without_tasks()
            .with_memory()
            .with_user(UpdateKind::Always);
        // sysinfo caches names across exec and checks macOS generations only to
        // whole seconds. Reacquire process records; retain only our strong CPU baselines.
        let mut inventory = System::new();
        inventory.refresh_processes_specifics(scope, true, basic);
        let total_processes = inventory.processes().len();
        let selected: Vec<Pid> = inventory
            .processes()
            .iter()
            .filter(|(_, p)| {
                let name = p.name().to_string_lossy();
                let uid = p.user_id().map(|u| u.to_string());
                self.request.pid.is_some()
                    || (self
                        .request
                        .name
                        .as_ref()
                        .is_none_or(|n| name.to_lowercase().contains(&n.to_lowercase()))
                        && self.request.user.as_ref().is_none_or(|u| {
                            uid.as_ref() == Some(u)
                                || uid.as_ref().and_then(|id| self.users.get(id)) == Some(u)
                        }))
            })
            .map(|(pid, _)| *pid)
            .collect();
        let births: HashMap<_, _> = selected
            .iter()
            .map(|pid| (pid.as_u32(), identity::birth_id(pid.as_u32())))
            .collect();
        let mut kind = basic;
        if self.request.cpu {
            kind = kind.with_cpu();
        }
        if self.request.details {
            kind = kind
                .with_exe(UpdateKind::Always)
                .with_cmd(UpdateKind::Always)
                .with_cwd(UpdateKind::Always)
                .with_disk_usage()
                .with_tasks();
        }
        // Start fresh again so *all* emitted facts fall inside the native birth checks.
        let mut selected_sys = System::new();
        selected_sys.refresh_processes_specifics(ProcessesToUpdate::Some(&selected), true, kind);
        let measured_at = Instant::now();
        let mut next_baselines = HashMap::new();
        let mut processes = Vec::with_capacity(selected.len());
        let mut inaccessible_processes = 0;
        let mut target = TargetObservation::NotObserved;
        for pid in selected {
            let id = pid.as_u32();
            let Some(p) = selected_sys.process(pid) else {
                inaccessible_processes += 1;
                continue;
            };
            let birth_id = identity::birth_id(id);
            if births.get(&id) != Some(&birth_id) {
                inaccessible_processes += 1;
                continue;
            }
            if self.request.pid == Some(id) {
                target = match &birth_id {
                    Some(birth_id) => TargetObservation::Observed(ProcessIdentity {
                        pid: id,
                        birth_id: birth_id.clone(),
                    }),
                    None => TargetObservation::Unavailable,
                };
            }
            let start_time = nonzero(p.start_time());
            let rss_bytes = nonzero(p.memory());
            let virtual_bytes = nonzero(p.virtual_memory());
            if birth_id.is_none()
                || start_time.is_none()
                || rss_bytes.is_none()
                || virtual_bytes.is_none()
            {
                inaccessible_processes += 1;
            }
            let mut unavailable = BTreeMap::new();
            for (field, missing) in [
                ("start_time", start_time.is_none()),
                ("rss_bytes", rss_bytes.is_none()),
                ("virtual_bytes", virtual_bytes.is_none()),
                ("ppid", p.parent().is_none()),
                ("uid", p.user_id().is_none()),
            ] {
                if missing {
                    unavailable.insert(field.into(), "unknown".into());
                }
            }
            if birth_id.is_none() {
                unavailable.insert("birth_id".into(), "unknown".into());
            }
            // On macOS CPU and memory come from the same task-info read; an
            // inaccessible task otherwise looks like a permanently idle process.
            let counters_available =
                !cfg!(target_os = "macos") || rss_bytes.is_some() || virtual_bytes.is_some();
            let cpu_percent = if self.request.cpu && counters_available {
                birth_id.as_ref().and_then(|birth| {
                    let previous = self.baselines.get(&id)?;
                    cpu_delta(previous, birth, p.accumulated_cpu_time(), measured_at)
                })
            } else {
                None
            };
            if cpu_percent.is_none() {
                unavailable.insert(
                    "cpu_percent".into(),
                    if self.request.cpu {
                        "missing_baseline"
                    } else {
                        "not_requested"
                    }
                    .into(),
                );
            }
            if self.request.cpu
                && counters_available
                && let Some(birth) = &birth_id
            {
                next_baselines.insert(
                    id,
                    Baseline {
                        birth_id: birth.clone(),
                        counter: p.accumulated_cpu_time(),
                        at: measured_at,
                    },
                );
            }
            let uid = p.user_id().map(|u| u.to_string());
            let user = uid.as_ref().and_then(|id| self.users.get(id).cloned());
            let exe = self
                .request
                .details
                .then(|| p.exe().map(|p| p.to_string_lossy().into_owned()))
                .flatten();
            let cwd = self
                .request
                .details
                .then(|| p.cwd().map(|p| p.to_string_lossy().into_owned()))
                .flatten();
            let cmdline: Vec<String> = if self.request.details {
                p.cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect()
            } else {
                vec![]
            };
            let threads = if self.request.details {
                p.tasks().and_then(|t| u32::try_from(t.len()).ok())
            } else {
                None
            };
            let io = p.disk_usage();
            // sysinfo does not expose read failure separately from zero counters.
            let io_read_bytes =
                (self.request.details && io.total_read_bytes > 0).then_some(io.total_read_bytes);
            let io_written_bytes = (self.request.details && io.total_written_bytes > 0)
                .then_some(io.total_written_bytes);
            for (field, missing) in [
                ("exe", exe.is_none()),
                ("cwd", cwd.is_none()),
                ("cmdline", cmdline.is_empty()),
                ("threads", threads.is_none()),
                ("io_read_bytes", io_read_bytes.is_none()),
                ("io_written_bytes", io_written_bytes.is_none()),
            ] {
                if missing {
                    unavailable.insert(
                        field.into(),
                        if self.request.details {
                            if field == "threads" && cfg!(target_os = "macos") {
                                "unsupported"
                            } else {
                                "unknown"
                            }
                        } else {
                            "not_requested"
                        }
                        .into(),
                    );
                }
            }
            if user.is_none() {
                unavailable.insert("user".into(), "unknown".into());
            }
            let process = ProcessInfo {
                pid: id,
                start_time,
                birth_id,
                unavailable,
                ppid: p.parent().map(Pid::as_u32),
                name: p.name().to_string_lossy().into_owned(),
                exe,
                cmdline,
                cwd,
                state: state_name(p.status()).into(),
                user,
                uid,
                cpu_percent,
                rss_bytes,
                virtual_bytes,
                threads,
                io_read_bytes,
                io_written_bytes,
            };
            // Recheck selectors on the identity-validated facts, after binding the PID.
            if self
                .request
                .name
                .as_ref()
                .is_none_or(|name| process.name_matches(name))
                && self
                    .request
                    .user
                    .as_ref()
                    .is_none_or(|user| process.user_matches(user))
            {
                processes.push(process);
            }
        }
        self.baselines = next_baselines;
        let total = self.sys.total_memory();
        Collection {
            target,
            metadata: CollectionMetadata {
                schema_version: crate::model::SCHEMA_VERSION,
                collection_started_at,
                collected_at: Utc::now().to_rfc3339(),
                collection_duration_ms: millis(start.elapsed()),
                cpu_interval_ms: cpu_interval.filter(|_| self.request.cpu).map(millis),
                scope_id: self.scope_id.clone(),
                scope_pid: self.request.pid,
                inaccessible_processes,
            },
            system: SystemMetrics {
                cpu_percent,
                memory_total_bytes: total,
                memory_used_bytes: self.sys.used_memory(),
                memory_used_percent: if total == 0 {
                    0.0
                } else {
                    (self.sys.used_memory() as f64 / total as f64 * 100.0) as f32
                },
            },
            total_processes,
            processes,
        }
    }
}

// The backend cannot distinguish inaccessible scalar counters from zero.
fn nonzero(value: u64) -> Option<u64> {
    (value != 0).then_some(value)
}

fn cpu_delta(previous: &Baseline, birth: &str, counter: u64, at: Instant) -> Option<f32> {
    let elapsed = at.duration_since(previous.at);
    if previous.birth_id != birth || elapsed < sysinfo::MINIMUM_CPU_UPDATE_INTERVAL {
        return None;
    }
    let delta = counter.checked_sub(previous.counter)?;
    let value = delta as f64 / elapsed.as_secs_f64() / 10.0;
    value.is_finite().then_some(value as f32)
}

pub fn collect(
    request: Request,
    window: Option<Duration>,
    cancel: &Cancellation,
) -> Result<Collection, StopError> {
    let mut sampler = Sampler::new(request);
    if let Some(window) = window {
        sampler.sample();
        if !cancel.wait_until(Instant::now() + window)? {
            return Err(StopError::Interrupted);
        }
    }
    if cancel.interrupted() {
        return Err(StopError::Interrupted);
    }
    Ok(sampler.sample())
}

pub fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpu_requires_same_generation_and_nondecreasing_counter() {
        let at = Instant::now();
        let baseline = Baseline {
            birth_id: "one".into(),
            counter: 1000,
            at,
        };
        let later = at + Duration::from_secs(1);
        assert_eq!(cpu_delta(&baseline, "one", 1250, later), Some(25.0));
        assert_eq!(cpu_delta(&baseline, "two", 1250, later), None);
        assert_eq!(cpu_delta(&baseline, "one", 999, later), None);
        assert_eq!(cpu_delta(&baseline, "one", 1250, at), None);
    }
}
