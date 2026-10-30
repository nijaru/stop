//! Owned raw observations. Availability, identity validation, CPU deltas and
//! final selectors are applied once by Sampler, independently of the backend.

use sysinfo::{ProcessStatus, Uid};

use super::{Request, UserNames};
use crate::StopError;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod fallback;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

pub(super) struct Acquisition {
    pub records: Vec<Facts>,
    pub total_processes: usize,
    pub inaccessible_processes: usize,
}

pub(super) struct Facts {
    pub pid: u32,
    pub birth_before: Option<String>,
    pub birth_after: Option<String>,
    pub name: String,
    pub ppid: Option<u32>,
    pub uid: Option<Uid>,
    pub state: ProcessStatus,
    pub start_time: Option<u64>,
    pub rss_bytes: Option<u64>,
    pub virtual_bytes: Option<u64>,
    pub cpu_counter: Option<u64>,
    pub cpu_at: std::time::Instant,
    pub exe: Option<String>,
    pub cwd: Option<String>,
    pub cmdline: Vec<String>,
    pub threads: Option<u32>,
    pub io_read_bytes: Option<u64>,
    pub io_written_bytes: Option<u64>,
}

pub(super) fn acquire(request: &Request, users: &mut UserNames) -> Result<Acquisition, StopError> {
    #[cfg(target_os = "linux")]
    return linux::acquire(request, users);
    #[cfg(target_os = "macos")]
    return macos::acquire(request, users);
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fallback::acquire(request, users)
}
