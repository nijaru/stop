use std::mem::{MaybeUninit, size_of};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::ptr;

use super::{PortScan, PortSocket, SocketRecord};
use crate::error::StopError;

const PROC_ALL_PIDS: u32 = 1;
const PROC_PIDFDSOCKETINFO: libc::c_int = 3;
const PROX_FDTYPE_SOCKET: u32 = libc::PROX_FDTYPE_SOCKET as u32;
const AF_INET: i32 = 2;
const AF_INET6: i32 = 30;
const IPPROTO_TCP: i32 = 6;
const IPPROTO_UDP: i32 = 17;
const TCP_LISTEN: i32 = 1;

// libc exposes the proc_info functions and proc_fdinfo, but not the private
// socket_fdinfo layout. These sizes and offsets mirror Apple's proc_info.h.
#[repr(C)]
struct SocketFdInfo {
    _file_info: [u8; 24],
    socket_info: [u8; 768],
}

pub(super) fn scan(port: u16) -> Result<PortScan, StopError> {
    let pids = list_pids()?;
    let mut records = Vec::new();
    let mut inaccessible_processes = 0;

    for pid in pids {
        let fd_bytes =
            unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, ptr::null_mut(), 0) };
        if fd_bytes < 0 {
            inaccessible_processes += 1;
            continue;
        }
        if fd_bytes == 0 {
            continue;
        }

        let fd_count = fd_bytes as usize / size_of::<libc::proc_fdinfo>();
        let mut fds = vec![
            libc::proc_fdinfo {
                proc_fd: 0,
                proc_fdtype: 0,
            };
            fd_count
        ];
        let filled = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDLISTFDS,
                0,
                fds.as_mut_ptr().cast(),
                fd_bytes,
            )
        };
        if filled < 0 {
            inaccessible_processes += 1;
            continue;
        }
        let filled_count = filled as usize / size_of::<libc::proc_fdinfo>();
        if filled_count > fds.len() {
            inaccessible_processes += 1;
            continue;
        }

        let mut process_inaccessible = false;
        for fd in &fds[..filled_count] {
            if fd.proc_fdtype != PROX_FDTYPE_SOCKET {
                continue;
            }
            let mut info = MaybeUninit::<SocketFdInfo>::zeroed();
            let info_bytes = unsafe {
                libc::proc_pidfdinfo(
                    pid,
                    fd.proc_fd,
                    PROC_PIDFDSOCKETINFO,
                    info.as_mut_ptr().cast(),
                    size_of::<SocketFdInfo>() as libc::c_int,
                )
            };
            if info_bytes != size_of::<SocketFdInfo>() as libc::c_int {
                process_inaccessible = true;
                continue;
            }
            let info = unsafe { info.assume_init() };
            let Some(socket) = parse_socket(&info.socket_info, port) else {
                continue;
            };
            records.push(SocketRecord {
                pid: pid as u32,
                socket,
            });
        }
        if process_inaccessible {
            inaccessible_processes += 1;
        }
    }

    Ok(PortScan {
        records,
        inaccessible_processes,
        unattributed_sockets: 0,
    })
}

fn list_pids() -> Result<Vec<libc::c_int>, StopError> {
    let required = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, ptr::null_mut(), 0) };
    if required < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if required == 0 {
        return Ok(Vec::new());
    }

    let mut pids = vec![0; required as usize / size_of::<libc::c_int>() + 64];
    let filled = unsafe {
        libc::proc_listpids(
            PROC_ALL_PIDS,
            0,
            pids.as_mut_ptr().cast(),
            (pids.len() * size_of::<libc::c_int>()) as libc::c_int,
        )
    };
    if filled < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    pids.truncate(filled as usize / size_of::<libc::c_int>());
    Ok(pids.into_iter().filter(|pid| *pid > 0).collect())
}

fn parse_socket(info: &[u8; 768], port: u16) -> Option<PortSocket> {
    let family = read_i32(info, 160)?;
    let protocol = read_i32(info, 156)?;
    let raw_port = read_i32(info, 244)?;
    if raw_port < 0 {
        return None;
    }
    let local_port = u16::from_be(raw_port as u16);
    if local_port != port {
        return None;
    }

    let (protocol_name, state) = match protocol {
        IPPROTO_TCP if read_i32(info, 320)? == TCP_LISTEN => ("tcp", "listen"),
        IPPROTO_UDP => ("udp", "bound"),
        _ => return None,
    };
    let local_address = match family {
        AF_INET => {
            let bytes: [u8; 4] = info.get(300..304)?.try_into().ok()?;
            Ipv4Addr::from(bytes).to_string()
        }
        AF_INET6 => {
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
    }

    #[test]
    fn parses_tcp_listener() {
        let mut info = [0u8; 768];
        info[156..160].copy_from_slice(&IPPROTO_TCP.to_ne_bytes());
        info[160..164].copy_from_slice(&AF_INET.to_ne_bytes());
        let raw_port = i32::from(u16::to_be(3000));
        info[244..248].copy_from_slice(&raw_port.to_ne_bytes());
        info[300..304].copy_from_slice(&[127, 0, 0, 1]);
        info[320..324].copy_from_slice(&TCP_LISTEN.to_ne_bytes());

        let socket = parse_socket(&info, 3000).expect("listener matches");
        assert_eq!(socket.protocol, "tcp");
        assert_eq!(socket.local_address, "127.0.0.1");
        assert_eq!(socket.state, "listen");
    }

    #[test]
    fn ignores_established_tcp_socket() {
        let mut info = [0u8; 768];
        info[156..160].copy_from_slice(&IPPROTO_TCP.to_ne_bytes());
        info[160..164].copy_from_slice(&AF_INET.to_ne_bytes());
        let raw_port = i32::from(u16::to_be(3000));
        info[244..248].copy_from_slice(&raw_port.to_ne_bytes());
        info[320..324].copy_from_slice(&4i32.to_ne_bytes());
        assert!(parse_socket(&info, 3000).is_none());
    }
}
