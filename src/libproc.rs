//! Shared macOS libproc array boundaries and process discovery.

use std::io;
use std::mem::{size_of, size_of_val};
use std::ptr;

pub(crate) fn pids() -> io::Result<Vec<libc::pid_t>> {
    const PROC_ALL_PIDS: u32 = 1;
    // SAFETY: null buffer and size zero request a byte count, not a copy.
    let required = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, ptr::null_mut(), 0) };
    let count = returned_count::<libc::pid_t>(required, usize::MAX)?;
    let mut pids = vec![0; count + 64];
    let capacity = buffer_size(&pids)?;
    // SAFETY: pids is aligned initialized storage of capacity bytes. libproc
    // writes synchronously and does not retain the buffer pointer.
    let filled =
        unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, pids.as_mut_ptr().cast(), capacity) };
    let filled_count = returned_count::<libc::pid_t>(filled, pids.len())?;
    if filled_count == pids.len() {
        return Err(io::Error::other(
            "process list grew beyond buffer; retry lookup",
        ));
    }
    pids.truncate(filled_count);
    pids.retain(|pid| *pid > 0);
    Ok(pids)
}

pub(crate) fn buffer_size<T>(buffer: &[T]) -> io::Result<libc::c_int> {
    libc::c_int::try_from(size_of_val(buffer))
        .map_err(|_| io::Error::other("process info exceeds libproc buffer limit"))
}

pub(crate) fn returned_count<T>(bytes: libc::c_int, capacity: usize) -> io::Result<usize> {
    // libproc maps syscall failure to zero, not -1. A zero FD result can also
    // mean an empty table; conservatively keep that ambiguity partial.
    if bytes <= 0 {
        return Err(io::Error::last_os_error());
    }
    let bytes =
        usize::try_from(bytes).map_err(|_| io::Error::other("invalid libproc byte count"))?;
    let count = bytes / size_of::<T>();
    if !bytes.is_multiple_of(size_of::<T>()) || count > capacity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "libproc returned an invalid array length",
        ));
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_ffi_array_lengths_before_slicing() {
        assert_eq!(returned_count::<libc::pid_t>(8, 2).unwrap(), 2);
        for bytes in [-1, 0, 3, 12] {
            assert!(returned_count::<libc::pid_t>(bytes, 2).is_err());
        }
    }
}
