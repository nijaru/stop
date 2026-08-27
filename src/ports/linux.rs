use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

use super::{PortScan, PortSocket, SocketRecord};
use crate::error::StopError;

const TCP_LISTEN: &str = "0A";

pub(super) fn scan(port: u16) -> Result<PortScan, StopError> {
    let mut sockets_by_inode: HashMap<u64, PortSocket> = HashMap::new();
    for (path, protocol, tcp_only) in [
        ("/proc/net/tcp", "tcp", true),
        ("/proc/net/tcp6", "tcp", true),
        ("/proc/net/udp", "udp", false),
        ("/proc/net/udp6", "udp", false),
    ] {
        for (inode, socket) in read_socket_table(path, protocol, tcp_only, port)? {
            sockets_by_inode.insert(inode, socket);
        }
    }

    if sockets_by_inode.is_empty() {
        return Ok(PortScan {
            records: Vec::new(),
            inaccessible_processes: 0,
            unattributed_sockets: 0,
        });
    }

    let mut records = Vec::new();
    let mut matched_inodes = HashSet::new();
    let mut inaccessible_processes = 0;
    for entry in fs::read_dir("/proc")? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let pid = match entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        {
            Some(pid) => pid,
            None => continue,
        };
        let fd_dir = entry.path().join("fd");
        let fds = match fs::read_dir(&fd_dir) {
            Ok(fds) => fds,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => {
                inaccessible_processes += 1;
                continue;
            }
        };
        let mut process_inaccessible = false;
        for fd in fds {
            let fd = match fd {
                Ok(fd) => fd,
                Err(_) => {
                    process_inaccessible = true;
                    continue;
                }
            };
            let target = match fs::read_link(fd.path()) {
                Ok(target) => target,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    process_inaccessible = true;
                    continue;
                }
                Err(_) => continue,
            };
            let inode = match socket_inode(&target) {
                Some(inode) if sockets_by_inode.contains_key(&inode) => inode,
                _ => continue,
            };
            matched_inodes.insert(inode);
            records.push(SocketRecord {
                pid,
                socket: sockets_by_inode[&inode].clone(),
            });
        }
        if process_inaccessible {
            inaccessible_processes += 1;
        }
    }

    Ok(PortScan {
        records,
        inaccessible_processes,
        unattributed_sockets: sockets_by_inode
            .keys()
            .filter(|inode| !matched_inodes.contains(inode))
            .count(),
    })
}

fn read_socket_table(
    path: &str,
    protocol: &str,
    tcp_only: bool,
    port: u16,
) -> Result<Vec<(u64, PortSocket)>, StopError> {
    let contents = fs::read_to_string(path)?;
    Ok(contents
        .lines()
        .skip(1)
        .filter_map(|line| parse_socket_line(line, protocol, tcp_only, port))
        .collect())
}

fn parse_socket_line(
    line: &str,
    protocol: &str,
    tcp_only: bool,
    port: u16,
) -> Option<(u64, PortSocket)> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let local = fields.get(1)?;
    let state = fields.get(3)?;
    let inode = fields.get(9)?.parse::<u64>().ok()?;
    let (address_hex, port_hex) = local.split_once(':')?;
    let local_port = u16::from_str_radix(port_hex, 16).ok()?;
    if local_port != port || (tcp_only && *state != TCP_LISTEN) {
        return None;
    }

    Some((
        inode,
        PortSocket {
            protocol: protocol.to_string(),
            local_address: decode_address(address_hex)?,
            local_port,
            state: if tcp_only { "listen" } else { "bound" }.to_string(),
        },
    ))
}

fn decode_address(address: &str) -> Option<String> {
    match address.len() {
        8 => {
            let value = u32::from_str_radix(address, 16).ok()?;
            Some(IpAddr::V4(Ipv4Addr::from(value.to_le_bytes())).to_string())
        }
        32 => {
            let mut bytes = [0u8; 16];
            let (chunks, remainder) = address.as_bytes().as_chunks::<8>();
            let (outputs, output_remainder) = bytes.as_chunks_mut::<4>();
            if !remainder.is_empty() || !output_remainder.is_empty() {
                return None;
            }
            for (chunk, output) in chunks.iter().zip(outputs.iter_mut()) {
                let value = u32::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
                output.copy_from_slice(&value.to_le_bytes());
            }
            Some(IpAddr::V6(Ipv6Addr::from(bytes)).to_string())
        }
        _ => None,
    }
}

fn socket_inode(target: &Path) -> Option<u64> {
    let target = target.to_str()?;
    let inode = target.strip_prefix("socket:[")?.strip_suffix(']')?;
    inode.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_listening_tcp_line() {
        let line = "  0: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000   1000        0 12345 1 0000000000000000 100 0 0 10 0";
        let (_, socket) = parse_socket_line(line, "tcp", true, 3000).expect("line matches");
        assert_eq!(socket.protocol, "tcp");
        assert_eq!(socket.local_address, "127.0.0.1");
        assert_eq!(socket.local_port, 3000);
        assert_eq!(socket.state, "listen");
    }

    #[test]
    fn ignores_non_listening_tcp_line() {
        let line = "  0: 0100007F:0BB8 00000000:0000 01 00000000:00000000 00:00000000 00000000   1000        0 12345 1 0000000000000000 100 0 0 10 0";
        assert!(parse_socket_line(line, "tcp", true, 3000).is_none());
    }

    #[test]
    fn parses_bound_udp_line() {
        let line = "  0: 00000000:0BB8 00000000:0000 07 00000000:00000000 00:00000000 00000000   1000        0 12345 1 0000000000000000 100 0 0 10 0";
        let (_, socket) = parse_socket_line(line, "udp", false, 3000).expect("line matches");
        assert_eq!(socket.local_address, "0.0.0.0");
        assert_eq!(socket.state, "bound");
    }

    #[test]
    fn decodes_proc_ipv6_words() {
        assert_eq!(
            decode_address("00000000000000000000000001000000"),
            Some("::1".to_string())
        );
    }

    #[test]
    fn parses_socket_inode_symlink() {
        assert_eq!(socket_inode(Path::new("socket:[12345]")), Some(12345));
        assert_eq!(socket_inode(Path::new("pipe:[12345]")), None);
    }
}
