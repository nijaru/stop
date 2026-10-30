//! Fresh BSD/task observations with one reusable, full-size procargs buffer.

use std::mem::{MaybeUninit, size_of};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::time::Instant;

use sysinfo::{ProcessStatus, Uid};

use super::{Acquisition, Facts, Request, UserNames};
use crate::{StopError, identity, libproc, process_args::MacArgs};

pub(super) fn acquire(request: &Request, users: &mut UserNames) -> Result<Acquisition, StopError> {
    let pids = if let Some(pid) = request.pid {
        vec![pid]
    } else {
        libproc::pids()?
            .into_iter()
            .filter_map(|pid| u32::try_from(pid).ok())
            .collect()
    };
    let mut args = MacArgs::new();
    let timebase = request.cpu.then(timebase_to_ms).flatten();
    let mut records = Vec::with_capacity(pids.len());
    let mut total_processes = 0;
    let mut inaccessible_processes = 0;
    for pid in pids {
        let before = identity::bsd_info(pid);
        // A missing BSD observation must not fabricate a native generation.
        // sysinfo still exposes a name-only row when proc_pidpath succeeds.
        let blob = before.as_ref().and_then(|_| args.as_mut()?.read(pid));
        let executable = blob.as_ref().and_then(|blob| blob.executable());
        let backup = executable.is_none().then(|| executable_path(pid)).flatten();
        let Some(executable) = executable.or(backup.as_deref()) else {
            if request.pid.is_none() || before.is_some() {
                inaccessible_processes += 1;
            }
            continue;
        };
        let name = executable
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let uid = before
            .as_ref()
            .and_then(|info| Uid::try_from(info.pbi_ruid as usize).ok());
        total_processes += 1;
        // Explicit PIDs must reach shared target binding before any selectors.
        if request.pid.is_none() {
            if !request.name_matches(&name) {
                continue;
            }
            if let Some(user) = &request.user {
                let matches = uid.as_ref().is_some_and(|uid| {
                    uid.to_string() == *user || users.name(uid) == Some(user.as_str())
                });
                if !matches {
                    continue;
                }
            }
        }
        let exe = request
            .details
            .then(|| executable.to_string_lossy().into_owned());
        let cmdline = if request.details {
            blob.as_ref()
                .zip(before.as_ref())
                .map(|(blob, info)| blob.argv(info.pbi_flags))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let task = before.as_ref().and_then(|_| task_info(pid));
        let cpu_at = Instant::now();
        let cpu_counter = task.as_ref().and_then(|task| {
            // Match sysinfo's ambiguous zero-memory treatment and its Mach
            // timebase conversion, including saturation and millisecond rounding.
            if task.pti_resident_size == 0 && task.pti_virtual_size == 0 {
                return None;
            }
            Some(
                (task.pti_total_user.saturating_add(task.pti_total_system) as f64 * timebase?)
                    as u64,
            )
        });
        let cwd = request.details.then(|| current_directory(pid)).flatten();
        let io = if request.details && before.is_some() {
            disk_io(pid)
        } else {
            None
        };
        let birth_before = before.as_ref().map(identity::bsd_birth_id);
        // All emitted facts are bracketed by native generation observations.
        let birth_after = identity::birth_id(pid);
        records.push(Facts {
            pid,
            birth_before,
            birth_after,
            name,
            ppid: before
                .as_ref()
                .map(|info| info.pbi_ppid)
                .filter(|&pid| pid != 0),
            uid,
            state: before
                .as_ref()
                .map(|info| ProcessStatus::from(info.pbi_status))
                .unwrap_or(ProcessStatus::Unknown(0)),
            start_time: before.as_ref().map(|info| info.pbi_start_tvsec),
            rss_bytes: task.as_ref().map(|task| task.pti_resident_size),
            virtual_bytes: task.as_ref().map(|task| task.pti_virtual_size),
            cpu_counter,
            cpu_at,
            exe,
            cwd,
            cmdline,
            threads: None,
            io_read_bytes: io.map(|io| io.0),
            io_written_bytes: io.map(|io| io.1),
        });
    }
    Ok(Acquisition {
        records,
        total_processes,
        inaccessible_processes,
    })
}

#[expect(
    deprecated,
    reason = "libc still exposes the SDK ABI; no extra Mach dependency is needed"
)]
fn timebase_to_ms() -> Option<f64> {
    let mut info = libc::mach_timebase_info { numer: 0, denom: 0 };
    // SAFETY: info is valid writable storage of the SDK-defined layout.
    let result = unsafe { libc::mach_timebase_info(&mut info) };
    (result == libc::KERN_SUCCESS && info.numer > 0 && info.denom > 0)
        .then(|| (info.numer as f64 / info.denom as f64) / 1_000_000.)
}

fn task_info(pid: u32) -> Option<libc::proc_taskinfo> {
    let pid = libc::pid_t::try_from(pid).ok()?;
    let mut info = MaybeUninit::<libc::proc_taskinfo>::zeroed();
    let size = size_of::<libc::proc_taskinfo>() as libc::c_int;
    // SAFETY: the flavor matches the SDK/libc layout, writable for size bytes.
    let filled = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if filled != size {
        return None;
    }
    // SAFETY: the exact-size read initialized this integer-only structure.
    Some(unsafe { info.assume_init() })
}

fn executable_path(pid: u32) -> Option<std::path::PathBuf> {
    let pid = libc::pid_t::try_from(pid).ok()?;
    let mut bytes = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: bytes is initialized writable storage of the declared size.
    let filled = unsafe { libc::proc_pidpath(pid, bytes.as_mut_ptr().cast(), bytes.len() as u32) };
    let len = usize::try_from(filled)
        .ok()
        .filter(|&len| len > 0 && len < bytes.len())?;
    let path = bytes[..len].strip_suffix(&[0]).unwrap_or(&bytes[..len]);
    if path.contains(&0) {
        return None;
    }
    Some(Path::new(std::ffi::OsStr::from_bytes(path)).to_owned())
}

fn current_directory(pid: u32) -> Option<String> {
    let pid = libc::pid_t::try_from(pid).ok()?;
    let mut info = MaybeUninit::<libc::proc_vnodepathinfo>::zeroed();
    let size = size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
    // SAFETY: the flavor matches this writable SDK/libc layout and size.
    let filled = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if filled != size {
        return None;
    }
    // SAFETY: the exact-size read initialized this integer/byte-array layout.
    let info = unsafe { info.assume_init() };
    if info.pvi_cdir.vip_vi.vi_stat.vst_dev == 0 {
        return None;
    }
    let path = info.pvi_cdir.vip_path.as_flattened();
    path.iter().position(|&b| b == 0)?;
    // SAFETY: the initialized SDK path array contains a NUL within its bounds.
    let path = unsafe { std::ffi::CStr::from_ptr(path.as_ptr()) };
    Some(path.to_string_lossy().into_owned())
}

fn disk_io(pid: u32) -> Option<(u64, u64)> {
    let pid = libc::pid_t::try_from(pid).ok()?;
    let mut info = MaybeUninit::<libc::rusage_info_v2>::zeroed();
    // SAFETY: RUSAGE_INFO_V2 expects writable storage for rusage_info_v2,
    // passed through the API's opaque rusage_info_t pointer. Nothing is retained.
    let result =
        unsafe { libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V2, info.as_mut_ptr().cast()) };
    if result != 0 {
        return None;
    }
    // SAFETY: success initializes the requested SDK/libc structure.
    let info = unsafe { info.assume_init() };
    Some((info.ri_diskio_bytesread, info.ri_diskio_byteswritten))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_prefix_preserves_long_alias_instead_of_resolved_path_or_argv0() {
        use std::os::unix::process::CommandExt;

        let name = format!(
            "stop-native-alias-longer-than-bsd-name-{}",
            std::process::id()
        );
        let path = std::env::temp_dir().join(&name);
        std::os::unix::fs::symlink("/bin/sleep", &path).unwrap();
        let mut child = std::process::Command::new(&path)
            .arg0("")
            .arg("60")
            .spawn()
            .unwrap();
        let observed = acquire(
            &Request {
                pid: Some(child.id()),
                name: Some("does-not-match".into()),
                details: true,
                ..Request::default()
            },
            &mut UserNames::new(),
        );
        let _ = child.kill();
        child.wait().unwrap();
        std::fs::remove_file(&path).unwrap();
        let observed = observed.unwrap();
        let p = &observed.records[0];
        assert_eq!(p.name, name);
        assert_eq!(p.exe.as_deref(), path.to_str());
        assert_eq!(p.cmdline, ["", "60"]);
        assert_eq!(p.birth_before, p.birth_after);
    }

    #[test]
    fn native_inventory_and_explicit_pid_share_birth_and_basic_facts() {
        let pid = std::process::id();
        let mut users = UserNames::new();
        let all = acquire(&Request::default(), &mut users).unwrap();
        let current = all.records.iter().find(|p| p.pid == pid).unwrap();
        let explicit = acquire(
            &Request {
                pid: Some(pid),
                details: true,
                ..Request::default()
            },
            &mut users,
        )
        .unwrap();
        let p = &explicit.records[0];
        assert_eq!(p.birth_before, current.birth_before);
        assert_eq!(p.birth_after, current.birth_after);
        assert_eq!(p.name, current.name);
        assert_eq!(p.uid, current.uid);
        assert_eq!(p.ppid, current.ppid);
        assert!(p.rss_bytes.is_some_and(|rss| rss > 0));
        assert!(p.exe.is_some());
        assert!(!p.cmdline.is_empty());
        assert!(p.cwd.is_some());
        assert!(all.total_processes >= all.records.len());
        assert!(all.records.iter().all(|p| p.pid > 0));
    }
}
