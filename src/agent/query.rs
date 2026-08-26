use super::model::{Observation, ProcessRecord, ProcessResult, ResultMeta};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortKey {
    #[default]
    Cpu,
    Memory,
    Pid,
    Name,
}

#[derive(Clone, Debug, Default)]
pub struct ProcessSelector {
    pub pid: Option<u32>,
    pub name: Option<String>,
    pub user: Option<String>,
    pub cwd: Option<String>,
    pub parent: Option<u32>,
    pub min_cpu: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct ProcessQuery {
    pub selector: ProcessSelector,
    pub sort: SortKey,
    pub limit: Option<usize>,
}

impl Default for ProcessQuery {
    fn default() -> Self {
        Self {
            selector: ProcessSelector::default(),
            sort: SortKey::Cpu,
            limit: None,
        }
    }
}

impl ProcessQuery {
    #[must_use]
    pub fn execute(&self, observation: Observation) -> ProcessResult {
        let mut processes: Vec<ProcessRecord> = observation
            .processes
            .into_iter()
            .filter(|process| self.selector.matches(process))
            .collect();

        sort_processes(&mut processes, self.sort);

        let matched = processes.len();
        if let Some(limit) = self.limit {
            processes.truncate(limit);
        }
        let returned = processes.len();

        ProcessResult {
            schema: observation.schema,
            observed_at: observation.observed_at,
            system: observation.system,
            meta: ResultMeta {
                complete: true,
                matched,
                returned,
                truncated: returned < matched,
                sample_window_ms: observation.sample_window_ms,
            },
            processes,
        }
    }
}

impl ProcessSelector {
    fn matches(&self, process: &ProcessRecord) -> bool {
        if self.pid.is_some_and(|pid| process.id.pid != pid) {
            return false;
        }

        if let Some(name) = self.name.as_deref() {
            let needle = name.to_lowercase();
            let in_name = process.name.to_lowercase().contains(&needle);
            let in_argv = process
                .argv
                .iter()
                .any(|arg| arg.to_lowercase().contains(&needle));
            if !in_name && !in_argv {
                return false;
            }
        }

        if let Some(user) = self.user.as_deref() {
            if process.user.as_deref() != Some(user) {
                return false;
            }
        }

        if let Some(cwd) = self.cwd.as_deref() {
            if process.cwd.as_deref() != Some(cwd) {
                return false;
            }
        }

        if self.parent.is_some_and(|ppid| process.ppid != Some(ppid)) {
            return false;
        }

        if self
            .min_cpu
            .is_some_and(|minimum| process.cpu_percent < minimum)
        {
            return false;
        }

        true
    }
}

fn sort_processes(processes: &mut [ProcessRecord], sort: SortKey) {
    match sort {
        SortKey::Cpu => processes.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent)),
        SortKey::Memory => processes.sort_by_key(|process| std::cmp::Reverse(process.rss_bytes)),
        SortKey::Pid => processes.sort_by_key(|process| process.id.pid),
        SortKey::Name => processes.sort_by_cached_key(|process| process.name.to_lowercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::model::{IoCounters, ProcessIdentity};

    fn process(pid: u32, name: &str, cpu: f32, rss: u64) -> ProcessRecord {
        ProcessRecord {
            id: ProcessIdentity {
                pid,
                start_time_secs: 1,
            },
            ppid: None,
            name: name.to_string(),
            exe: None,
            argv: vec![name.to_string()],
            cwd: None,
            state: "run".to_string(),
            user: None,
            cpu_percent: cpu,
            rss_bytes: rss,
            threads: None,
            io: IoCounters {
                read_bytes: 0,
                write_bytes: 0,
            },
        }
    }

    #[test]
    fn list_is_exhaustive_without_limit() {
        let observation = Observation {
            schema: "stop/1".to_string(),
            observed_at: "now".to_string(),
            sample_window_ms: 200,
            system: crate::agent::model::SystemRecord {
                cpu_percent: 0.0,
                memory_total_bytes: 1,
                memory_used_bytes: 0,
            },
            processes: vec![process(1, "a", 1.0, 1), process(2, "b", 2.0, 2)],
        };

        let result = ProcessQuery::default().execute(observation);
        assert_eq!(result.meta.matched, 2);
        assert_eq!(result.meta.returned, 2);
        assert!(!result.meta.truncated);
    }

    #[test]
    fn explicit_limit_reports_truncation() {
        let observation = Observation {
            schema: "stop/1".to_string(),
            observed_at: "now".to_string(),
            sample_window_ms: 200,
            system: crate::agent::model::SystemRecord {
                cpu_percent: 0.0,
                memory_total_bytes: 1,
                memory_used_bytes: 0,
            },
            processes: vec![process(1, "a", 1.0, 1), process(2, "b", 2.0, 2)],
        };

        let result = ProcessQuery {
            limit: Some(1),
            ..ProcessQuery::default()
        }
        .execute(observation);

        assert_eq!(result.meta.matched, 2);
        assert_eq!(result.meta.returned, 1);
        assert!(result.meta.truncated);
    }
}
