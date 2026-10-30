//! Native process generations. Birth tokens are opaque and meaningful only
//! together with a PID and the observation scope; human start times are not tokens.

/// Read the native birth token without rounding its precision. Unavailable or
/// unsupported identity is `None`, never a synthesized timestamp.
pub fn birth_id(pid: u32) -> Option<String> {
    platform::birth_id(pid)
}

/// The BSD record also supplies the target ABI needed to decode macOS argv.
#[cfg(target_os = "macos")]
pub(crate) fn bsd_info(pid: u32) -> Option<libc::proc_bsdinfo> {
    platform::bsd_info(pid)
}

/// Identify the boot and PID-numbering scope used by native process reads.
/// Failure to establish that scope is explicit rather than machine-global trust.
pub fn scope_id() -> Option<String> {
    platform::scope_id()
}

#[cfg(target_os = "linux")]
pub(crate) mod proc_stat;

#[cfg(target_os = "linux")]
mod platform {
    use std::fs;
    use std::os::unix::fs::MetadataExt;

    pub(super) fn birth_id(pid: u32) -> Option<String> {
        super::proc_stat::read(pid, |stat| {
            stat.number::<u64>(22).map(super::proc_stat::birth_id)
        })
    }

    pub(super) fn scope_id() -> Option<String> {
        let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
        let boot = boot.trim();
        if boot.is_empty() {
            return None;
        }
        // PID 1 belongs to the namespace represented by this procfs mount.
        // /proc/self/ns/pid can instead name a nested namespace when procfs was
        // inherited from an ancestor, whose PID numbering we are still reading.
        let namespace = fs::metadata("/proc/1/ns/pid").ok()?;
        Some(format!(
            "linux:{boot}:pidns:{}:{}",
            namespace.dev(),
            namespace.ino()
        ))
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::CStr;
    use std::mem::{MaybeUninit, size_of};
    use std::ptr;

    pub(super) fn birth_id(pid: u32) -> Option<String> {
        let info = bsd_info(pid)?;
        Some(format!(
            "macos:{}:{:06}",
            info.pbi_start_tvsec, info.pbi_start_tvusec
        ))
    }

    pub(super) fn bsd_info(pid: u32) -> Option<libc::proc_bsdinfo> {
        let native_pid = libc::pid_t::try_from(pid).ok()?;
        if native_pid <= 0 {
            return None;
        }
        let mut info = MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        let size = libc::c_int::try_from(size_of::<libc::proc_bsdinfo>()).ok()?;
        // SAFETY: info is aligned writable storage of exactly size bytes for
        // the requested libc layout. The API retains no pointer to this buffer.
        let filled = unsafe {
            libc::proc_pidinfo(
                native_pid,
                libc::PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast(),
                size,
            )
        };
        if filled != size {
            return None;
        }
        // SAFETY: a successful, exact-size PROC_PIDTBSDINFO read initialized
        // this struct, whose fields are integer values and byte arrays.
        let info = unsafe { info.assume_init() };
        if info.pbi_pid != pid || info.pbi_start_tvusec >= 1_000_000 {
            return None;
        }
        Some(info)
    }

    pub(super) fn scope_id() -> Option<String> {
        let mut bytes = [0u8; 128];
        let mut len = bytes.len();
        // SAFETY: the name is NUL-terminated; bytes and len are writable for
        // their declared lengths. Null newp with length zero requests no write.
        let result = unsafe {
            libc::sysctlbyname(
                c"kern.bootsessionuuid".as_ptr(),
                bytes.as_mut_ptr().cast(),
                &mut len,
                ptr::null_mut(),
                0,
            )
        };
        if result != 0 {
            return None;
        }
        let boot = CStr::from_bytes_with_nul(bytes.get(..len)?).ok()?;
        let boot = boot.to_str().ok()?;
        if boot.is_empty() {
            return None;
        }
        Some(format!("macos:{boot}"))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod platform {
    pub(super) fn birth_id(_pid: u32) -> Option<String> {
        None
    }

    pub(super) fn scope_id() -> Option<String> {
        None
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn current_process_has_repeatable_native_identity() {
        let first = birth_id(std::process::id()).expect("current PID identity is readable");
        assert_eq!(birth_id(std::process::id()).as_ref(), Some(&first));
        assert!(birth_id(u32::MAX).is_none());
    }
}
