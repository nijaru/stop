//! Linux process facts without cached process records or task enumeration.

use std::fs;
use std::io::{self, Read};
use std::path::Path;
use sysinfo::{ProcessStatus, Uid};

use super::{Acquisition, Facts, Request, UserNames};
use crate::{StopError, identity};
use identity::proc_stat::{self, Stat};

struct Units {
    ticks: Option<u64>,
    page: Option<u64>,
    boot: Option<u64>,
}

impl Units {
    fn read() -> Self {
        // SAFETY: sysconf accepts these constant names and has no pointer args.
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        // SAFETY: as above, querying the host page size.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        let boot = fs::read_to_string("/proc/stat").ok().and_then(|s| {
            s.lines()
                .find_map(|line| line.strip_prefix("btime ")?.trim().parse().ok())
        });
        Self {
            ticks: u64::try_from(ticks).ok().filter(|&v| v > 0),
            page: u64::try_from(page).ok().filter(|&v| v > 0),
            boot,
        }
    }
}

pub(super) fn acquire(request: &Request, users: &mut UserNames) -> Result<Acquisition, StopError> {
    // /proc enumeration lists thread-group leaders, just like sysinfo's
    // without_tasks inventory. Explicit TIDs remain addressable via /proc/TID.
    let pids = if let Some(pid) = request.pid {
        vec![pid]
    } else {
        let mut pids = Vec::new();
        for entry in fs::read_dir("/proc")? {
            let name = entry?.file_name();
            if let Some(pid) = name
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|&pid| pid > 0)
            {
                pids.push(pid);
            }
        }
        pids
    };
    let units = Units::read();
    let mut records = Vec::with_capacity(pids.len());
    let mut total_processes = 0;
    let mut inaccessible_processes = 0;
    let mut bytes = Vec::with_capacity(4096);
    for pid in pids {
        // This record is both the before-birth check and the basic stat facts.
        let Some(mut facts) = proc_stat::read(pid, |stat| basic(pid, stat, &units, request.cpu))
        else {
            // An absent explicit PID was never an observed candidate.
            if request.pid.is_none() {
                inaccessible_processes += 1;
            }
            continue;
        };
        total_processes += 1;
        if read_file(format!("/proc/{pid}/status"), &mut bytes).is_ok() {
            let status = Status::parse(&bytes);
            facts.uid = status.uid;
            // sysinfo uses the thread-group leader as a nonleader TID's parent,
            // not stat's process parent. Preserve that explicit-PID behavior.
            if let Some(tgid) = status.tgid.filter(|&tgid| tgid != pid && tgid != 0) {
                facts.ppid = Some(tgid);
            }
            if request.details {
                facts.threads = status.threads;
            }
        }
        // Explicit PIDs must reach the shared identity binding before filters.
        // For inventory filters, avoid enriching nonmatches (as sysinfo did).
        if request.pid.is_none() && (request.name.is_some() || request.user.is_some()) {
            let uid = facts.uid.as_ref().map(|uid| uid.to_string());
            let user = if request.user.is_some() {
                facts.uid.as_ref().and_then(|id| users.name(id))
            } else {
                None
            };
            if !request.matches(&facts.name, uid.as_deref(), user) {
                continue;
            }
        }
        // sysinfo prefers statm over stat for memory. Preserve that source (and
        // its stat fallback on read failure), rather than assuming equal RSS.
        if read_file(format!("/proc/{pid}/statm"), &mut bytes).is_ok() {
            let mut fields = bytes
                .split(u8::is_ascii_whitespace)
                .filter(|f| !f.is_empty());
            facts.virtual_bytes = number(fields.next()).and_then(|v| v.checked_mul(units.page?));
            facts.rss_bytes = number(fields.next()).and_then(|v| v.checked_mul(units.page?));
        }
        if request.details {
            facts.exe = executable(pid);
            facts.cwd = fs::read_link(format!("/proc/{pid}/cwd"))
                .ok()
                .map(|p| p.to_string_lossy().into_owned());
            facts.cmdline = crate::process_args::read(pid);
            if read_file(format!("/proc/{pid}/io"), &mut bytes).is_ok() {
                for line in bytes.split(|&b| b == b'\n') {
                    if let Some(value) = line.strip_prefix(b"read_bytes:") {
                        facts.io_read_bytes = number(Some(value.trim_ascii()));
                    } else if let Some(value) = line.strip_prefix(b"write_bytes:") {
                        facts.io_written_bytes = number(Some(value.trim_ascii()));
                    }
                }
            }
        }
        facts.birth_after = identity::birth_id(pid);
        records.push(facts);
    }
    Ok(Acquisition {
        records,
        total_processes,
        inaccessible_processes,
    })
}

fn basic(pid: u32, stat: Stat<'_>, units: &Units, cpu: bool) -> Option<Facts> {
    let ticks = stat.number::<u64>(22)?;
    let cpu_counter = if cpu {
        stat.number::<u64>(14)
            .and_then(|u| u.checked_add(stat.number(15)?))
            .and_then(|v| v.checked_mul(1000))
            .and_then(|v| v.checked_div(units.ticks?))
    } else {
        None
    };
    Some(Facts {
        pid,
        birth_before: Some(format!("linux:{ticks}")),
        birth_after: None,
        name: String::from_utf8_lossy(stat.name).into_owned(),
        ppid: stat.number::<u32>(4).filter(|&pid| pid > 0),
        uid: None,
        state: stat
            .state()
            .map(ProcessStatus::from)
            .unwrap_or(ProcessStatus::Unknown(0)),
        start_time: units
            .ticks
            .and_then(|hz| (ticks / hz).checked_add(units.boot?)),
        rss_bytes: stat
            .number::<u64>(24)
            .and_then(|v| v.checked_mul(units.page?)),
        virtual_bytes: stat.number(23),
        cpu_counter,
        cpu_at: std::time::Instant::now(),
        exe: None,
        cwd: None,
        cmdline: Vec::new(),
        threads: None,
        io_read_bytes: None,
        io_written_bytes: None,
    })
}

#[derive(Default)]
struct Status {
    uid: Option<Uid>,
    tgid: Option<u32>,
    threads: Option<u32>,
}

impl Status {
    fn parse(bytes: &[u8]) -> Self {
        let mut status = Self::default();
        for line in bytes.split(|&b| b == b'\n') {
            if let Some(value) = line.strip_prefix(b"Uid:") {
                // The first ID is real, not effective/saved/filesystem UID.
                status.uid = number(value.split(u8::is_ascii_whitespace).find(|f| !f.is_empty()))
                    .and_then(|v| usize::try_from(v).ok())
                    .and_then(|v| Uid::try_from(v).ok());
            } else if let Some(value) = line.strip_prefix(b"Tgid:") {
                status.tgid = number(Some(value.trim_ascii())).and_then(|v| u32::try_from(v).ok());
            } else if let Some(value) = line.strip_prefix(b"Threads:") {
                status.threads = number(Some(value.trim_ascii()))
                    .and_then(|v| u32::try_from(v).ok())
                    .filter(|&v| v > 0);
            }
        }
        status
    }
}

fn read_file(path: impl AsRef<Path>, bytes: &mut Vec<u8>) -> io::Result<()> {
    bytes.clear();
    // Procfs has no useful file size. Take's streaming read avoids File's
    // metadata/seek sizing calls, and one buffer serves the whole acquisition.
    fs::File::open(path)?.take(u64::MAX).read_to_end(bytes)?;
    Ok(())
}

fn number(value: Option<&[u8]>) -> Option<u64> {
    std::str::from_utf8(value?).ok()?.parse().ok()
}

fn executable(pid: u32) -> Option<String> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let mut path = fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    // sysinfo strips the kernel's deleted marker from the final component.
    if let Some(name) = path
        .file_name()
        .and_then(|s| s.as_bytes().strip_suffix(b" (deleted)"))
    {
        let name = std::ffi::OsString::from_vec(name.to_vec());
        path.set_file_name(name);
    }
    Some(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reads_real_uid_tgid_and_total_threads_independently() {
        let status =
            Status::parse(b"Name:\tfoo\xff\nUid:\t123 456 789 99\nTgid:\t42\nThreads:\t7\n");
        assert_eq!(status.uid.unwrap().to_string(), "123");
        assert_eq!(status.tgid, Some(42));
        assert_eq!(status.threads, Some(7));
        let status = Status::parse(b"Uid:\tinvalid 42\nThreads:\t0\n");
        assert!(status.uid.is_none());
        assert!(status.tgid.is_none());
        assert!(status.threads.is_none());
    }

    #[test]
    fn inventory_excludes_threads_but_explicit_tid_preserves_sysinfo_parent() {
        // Rust test bodies already run on nonleader threads. No extra live
        // fixture or timing assumption is needed to keep this TID observable.
        // SAFETY: gettid has no arguments or memory safety preconditions.
        let tid = u32::try_from(unsafe { libc::gettid() }).unwrap();
        let pid = std::process::id();
        assert_ne!(tid, pid);
        let mut users = UserNames::new();
        let inventory = acquire(&Request::default(), &mut users).unwrap();
        assert!(inventory.records.iter().any(|p| p.pid == pid));
        assert!(!inventory.records.iter().any(|p| p.pid == tid));
        let scoped = acquire(
            &Request {
                pid: Some(tid),
                details: true,
                ..Request::default()
            },
            &mut users,
        )
        .unwrap();
        assert_eq!(scoped.total_processes, 1);
        assert_eq!(scoped.records.len(), 1);
        let facts = &scoped.records[0];
        assert_eq!(facts.pid, tid);
        assert_eq!(facts.ppid, Some(pid));
        assert!(facts.threads.is_some_and(|n| n >= 2));
        assert_eq!(facts.birth_before, facts.birth_after);
        let mut reference = sysinfo::System::new();
        let sys_pid = sysinfo::Pid::from_u32(tid);
        reference.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[sys_pid]),
            true,
            sysinfo::ProcessRefreshKind::nothing().without_tasks(),
        );
        let reference = reference.process(sys_pid).unwrap();
        assert_eq!(facts.ppid, reference.parent().map(sysinfo::Pid::as_u32));
        assert_eq!(facts.name, reference.name().to_string_lossy());
    }

    #[test]
    fn filtered_inventory_counts_basic_records_and_pid_scope_bypasses_prefilter() {
        let mut users = UserNames::new();
        let request = Request {
            name: Some("stop-no-matching-process-name-xyz".into()),
            ..Request::default()
        };
        let filtered = acquire(&request, &mut users).unwrap();
        assert!(filtered.total_processes > 0);
        assert!(filtered.records.is_empty());
        let scoped = acquire(
            &Request {
                pid: Some(std::process::id()),
                ..request
            },
            &mut users,
        )
        .unwrap();
        assert_eq!(scoped.total_processes, 1);
        assert_eq!(scoped.records.len(), 1);
    }

    #[test]
    fn stat_units_use_host_scales_and_leave_failed_queries_unknown() {
        let record = b"1 (name) R 0 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 200 999 2";
        let units = Units {
            ticks: Some(250),
            page: Some(4096),
            boot: Some(1000),
        };
        let facts = basic(1, Stat::parse(record).unwrap(), &units, true).unwrap();
        assert_eq!(facts.birth_before.as_deref(), Some("linux:200"));
        assert_eq!(facts.cpu_counter, Some(92)); // (11 + 12) * 1000 / 250 ms
        assert_eq!(facts.start_time, Some(1000)); // floor ticks / Hz + btime
        assert_eq!(facts.rss_bytes, Some(8192));
        assert_eq!(facts.virtual_bytes, Some(999)); // stat vsize is already bytes
        assert_eq!(facts.ppid, None); // parent PID zero is unknown, as in sysinfo
        let unknown = Units {
            ticks: None,
            page: None,
            boot: Some(1000),
        };
        let facts = basic(1, Stat::parse(record).unwrap(), &unknown, true).unwrap();
        assert!(facts.cpu_counter.is_none());
        assert!(facts.start_time.is_none());
        assert!(facts.rss_bytes.is_none());
        assert!(facts.birth_before.is_some());
        let facts = basic(1, Stat::parse(record).unwrap(), &units, false).unwrap();
        assert!(facts.cpu_counter.is_none());
    }
}
