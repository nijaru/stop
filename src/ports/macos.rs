use std::io;
use std::mem::{MaybeUninit, size_of, size_of_val};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::ptr;

use super::{PortScan, PortSocket, SocketRecord, same_birth_id};
use crate::error::StopError;
use crate::identity::birth_id;

const PROC_ALL_PIDS: u32 = 1;
const PROC_PIDFDSOCKETINFO: libc::c_int = 3;
const PROX_FDTYPE_SOCKET: u32 = libc::PROX_FDTYPE_SOCKET as u32;
const TCP_LISTEN: i32 = 1;
const SOCKINFO_IN: i32 = 1;
const SOCKINFO_TCP: i32 = 2;

// libc exposes the proc_info functions and proc_fdinfo, but not the private
// socket_fdinfo layout. Size, alignment and offsets mirror Apple's proc_info.h.
#[repr(C, align(8))]
struct SocketFdInfo {
    _file_info: [u8; 24],
    socket_info: [u8; 768],
}

pub(super) fn scan(port: u16) -> Result<PortScan, StopError> {
    let pids = list_pids()?;
    let mut records = Vec::new();
    let mut inaccessible_processes = 0;
    let mut unattributed_sockets = 0;

    for pid in pids {
        // list_pids filters non-positive signed PIDs, but keep conversion explicit.
        let Ok(record_pid) = u32::try_from(pid) else {
            inaccessible_processes += 1;
            continue;
        };
        let Some(before) = birth_id(record_pid) else {
            inaccessible_processes += 1;
            continue;
        };
        let Ok((sockets, process_inaccessible)) = scan_fds(pid, port) else {
            inaccessible_processes += 1;
            continue;
        };
        let after = birth_id(record_pid);
        if !same_birth_id(Some(&before), after.as_deref()) {
            inaccessible_processes += 1;
            unattributed_sockets += sockets.len();
            continue;
        }
        records.extend(sockets.into_iter().map(|socket| SocketRecord {
            pid: record_pid,
            birth_id: Some(before.clone()),
            socket,
        }));
        if process_inaccessible {
            inaccessible_processes += 1;
        }
    }

    Ok(PortScan {
        records,
        inaccessible_processes,
        unattributed_sockets,
    })
}

fn scan_fds(pid: libc::pid_t, port: u16) -> io::Result<(Vec<PortSocket>, bool)> {
    // SAFETY: a null buffer and zero size request only the required byte count.
    let fd_bytes = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, ptr::null_mut(), 0) };
    let fd_count = returned_count::<libc::proc_fdinfo>(fd_bytes, usize::MAX)?;
    let mut fds = vec![
        libc::proc_fdinfo {
            proc_fd: 0,
            proc_fdtype: 0,
        };
        fd_count + 64
    ];
    let capacity = buffer_size(&fds)?;
    // SAFETY: fds is aligned, initialized writable storage of capacity bytes.
    // proc_pidinfo performs a synchronous copy and retains no buffer pointer.
    let filled = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDLISTFDS,
            0,
            fds.as_mut_ptr().cast(),
            capacity,
        )
    };
    let filled_count = returned_count::<libc::proc_fdinfo>(filled, fds.len())?;
    // A full buffer may have omitted FDs added since the sizing query.
    let mut process_inaccessible = filled_count == fds.len();
    let mut sockets = Vec::new();
    let info_size = libc::c_int::try_from(size_of::<SocketFdInfo>())
        .map_err(|_| io::Error::other("socket info exceeds libproc buffer limit"))?;
    for fd in &fds[..filled_count] {
        if fd.proc_fdtype != PROX_FDTYPE_SOCKET {
            continue;
        }
        let mut info = MaybeUninit::<SocketFdInfo>::zeroed();
        // SAFETY: info is aligned writable storage of info_size bytes, matching
        // PROC_PIDFDSOCKETINFO's layout. No pointer is retained after the call.
        let info_bytes = unsafe {
            libc::proc_pidfdinfo(
                pid,
                fd.proc_fd,
                PROC_PIDFDSOCKETINFO,
                info.as_mut_ptr().cast(),
                info_size,
            )
        };
        if info_bytes != info_size {
            process_inaccessible = true;
            continue;
        }
        // SAFETY: storage was zeroed and the exact-size read succeeded; this
        // struct contains only byte arrays, valid for every bit pattern.
        let info = unsafe { info.assume_init() };
        if let Some(socket) = parse_socket(&info.socket_info, port) {
            sockets.push(socket);
        }
    }
    Ok((sockets, process_inaccessible))
}

fn list_pids() -> Result<Vec<libc::pid_t>, StopError> {
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
        return Err(io::Error::other("process list grew beyond buffer; retry port lookup").into());
    }
    pids.truncate(filled_count);
    pids.retain(|pid| *pid > 0);
    Ok(pids)
}

fn buffer_size<T>(buffer: &[T]) -> io::Result<libc::c_int> {
    libc::c_int::try_from(size_of_val(buffer))
        .map_err(|_| io::Error::other("process info exceeds libproc buffer limit"))
}

fn returned_count<T>(bytes: libc::c_int, capacity: usize) -> io::Result<usize> {
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

fn parse_socket(info: &[u8; 768], port: u16) -> Option<PortSocket> {
    let family = read_i32(info, 160)?;
    let protocol = read_i32(info, 156)?;
    let kind = read_i32(info, 232)?;
    let raw_port = u16::try_from(read_i32(info, 244)?).ok()?;
    let local_port = u16::from_be(raw_port);
    if local_port != port {
        return None;
    }

    let (protocol_name, state) = match (protocol, kind) {
        (libc::IPPROTO_TCP, SOCKINFO_TCP) if read_i32(info, 320)? == TCP_LISTEN => {
            ("tcp", "listen")
        }
        (libc::IPPROTO_UDP, SOCKINFO_IN) => ("udp", "bound"),
        _ => return None,
    };
    let local_address = match family {
        libc::AF_INET => {
            let bytes: [u8; 4] = info.get(300..304)?.try_into().ok()?;
            Ipv4Addr::from(bytes).to_string()
        }
        libc::AF_INET6 => {
            let bytes: [u8; 16] = info.get(288..304)?.try_into().ok()?;
            Ipv6Addr::from(bytes).to_string()
        }
        _ => return None,
    };

    Some(PortSocket {
        protocol: protocol_name.to_string(),
        local_address,
        local_port: port,
        state: state.to_string(),
    })
}

fn read_i32(bytes: &[u8], offset: usize) -> Option<i32> {
    let value = bytes.get(offset..offset + size_of::<i32>())?;
    Some(i32::from_ne_bytes(value.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_info_layout_matches_macos_header() {
        assert_eq!(size_of::<SocketFdInfo>(), 792);
        assert_eq!(std::mem::align_of::<SocketFdInfo>(), 8);
    }

    fn socket_info(protocol: i32, kind: i32) -> [u8; 768] {
        let mut info = [0u8; 768];
        info[156..160].copy_from_slice(&protocol.to_ne_bytes());
        info[160..164].copy_from_slice(&libc::AF_INET.to_ne_bytes());
        info[232..236].copy_from_slice(&kind.to_ne_bytes());
        let raw_port = i32::from(u16::to_be(3000));
        info[244..248].copy_from_slice(&raw_port.to_ne_bytes());
        info[300..304].copy_from_slice(&[127, 0, 0, 1]);
        info
    }

    #[test]
    fn parses_tcp_listener_and_udp_binding_but_not_established_tcp() {
        let mut info = socket_info(libc::IPPROTO_TCP, SOCKINFO_TCP);
        info[320..324].copy_from_slice(&TCP_LISTEN.to_ne_bytes());
        let socket = parse_socket(&info, 3000).expect("listener matches");
        assert_eq!(socket.protocol, "tcp");
        assert_eq!(socket.local_address, "127.0.0.1");
        assert_eq!(socket.state, "listen");

        info[320..324].copy_from_slice(&4i32.to_ne_bytes());
        assert!(parse_socket(&info, 3000).is_none());

        let info = socket_info(libc::IPPROTO_UDP, SOCKINFO_IN);
        let socket = parse_socket(&info, 3000).expect("UDP binding matches");
        assert_eq!(socket.protocol, "udp");
        assert_eq!(socket.state, "bound");
    }

    #[test]
    fn rejects_wrong_union_kind_and_out_of_range_port() {
        let info = socket_info(libc::IPPROTO_UDP, SOCKINFO_TCP);
        assert!(parse_socket(&info, 3000).is_none());
        let mut info = socket_info(libc::IPPROTO_UDP, SOCKINFO_IN);
        let bad_port = 0x10000 | i32::from(u16::to_be(3000));
        info[244..248].copy_from_slice(&bad_port.to_ne_bytes());
        assert!(parse_socket(&info, 3000).is_none());
    }

    #[test]
    fn validates_ffi_array_lengths_before_slicing() {
        assert_eq!(returned_count::<libc::pid_t>(8, 2).unwrap(), 2);
        for bytes in [-1, 0, 3, 12] {
            assert!(returned_count::<libc::pid_t>(bytes, 2).is_err());
        }
    }
}
