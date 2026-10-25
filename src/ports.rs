//! Port ownership discovery. This module owns platform-specific socket APIs.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::error::StopError;
use crate::model::ProcessInfo;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct PortSocket {
    pub protocol: String,
    pub local_address: String,
    pub local_port: u16,
    pub state: String,
}

#[derive(Serialize)]
pub struct PortOwner {
    pub process: ProcessInfo,
    pub sockets: Vec<PortSocket>,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Complete,
    Partial,
}

#[derive(Serialize)]
pub struct PortReport {
    pub port: u16,
    pub visibility: Visibility,
    pub inaccessible_processes: usize,
    pub unattributed_sockets: usize,
    pub owners: Vec<PortOwner>,
}

pub(crate) struct PortScan {
    pub records: Vec<SocketRecord>,
    pub inaccessible_processes: usize,
    pub unattributed_sockets: usize,
}

pub(crate) struct SocketRecord {
    pub pid: u32,
    /// Present only after equal native identities bracket the PID's FD scan.
    pub birth_id: Option<String>,
    pub socket: PortSocket,
}

fn same_birth_id(before: Option<&str>, after: Option<&str>) -> bool {
    matches!((before, after), (Some(before), Some(after)) if before == after)
}

pub fn inspect(port: u16, processes: &[ProcessInfo]) -> Result<PortReport, StopError> {
    Ok(associate(port, processes, platform_scan(port)?))
}

fn associate(port: u16, processes: &[ProcessInfo], scan: PortScan) -> PortReport {
    let by_pid: std::collections::HashMap<u32, &ProcessInfo> =
        processes.iter().map(|p| (p.pid, p)).collect();
    let mut sockets_by_pid: BTreeMap<u32, Vec<PortSocket>> = BTreeMap::new();
    let mut unattributed_sockets = scan.unattributed_sockets;

    for record in scan.records {
        if by_pid.get(&record.pid).is_some_and(|process| {
            same_birth_id(process.birth_id.as_deref(), record.birth_id.as_deref())
        }) {
            sockets_by_pid
                .entry(record.pid)
                .or_default()
                .push(record.socket);
        } else {
            unattributed_sockets += 1;
        }
    }

    let owners = sockets_by_pid
        .into_iter()
        .filter_map(|(pid, mut sockets)| {
            let process = (*by_pid.get(&pid)?).clone();
            sockets.sort_by(|a, b| {
                (&a.protocol, &a.local_address, &a.state).cmp(&(
                    &b.protocol,
                    &b.local_address,
                    &b.state,
                ))
            });
            sockets.dedup();
            Some(PortOwner { process, sockets })
        })
        .collect();

    let visibility = if scan.inaccessible_processes > 0 || unattributed_sockets > 0 {
        Visibility::Partial
    } else {
        Visibility::Complete
    };

    PortReport {
        port,
        visibility,
        inaccessible_processes: scan.inaccessible_processes,
        unattributed_sockets,
        owners,
    }
}

#[cfg(target_os = "linux")]
fn platform_scan(port: u16) -> Result<PortScan, StopError> {
    linux::scan(port)
}

#[cfg(target_os = "macos")]
fn platform_scan(port: u16) -> Result<PortScan, StopError> {
    macos::scan(port)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn platform_scan(port: u16) -> Result<PortScan, StopError> {
    unsupported::scan(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, birth_id: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid,
            birth_id: birth_id.map(str::to_owned),
            unavailable: BTreeMap::new(),
            start_time: Some(1_700_000_000),
            ppid: None,
            name: "test".into(),
            exe: None,
            cmdline: Vec::new(),
            cwd: None,
            state: "run".into(),
            user: None,
            uid: None,
            cpu_percent: None,
            rss_bytes: Some(0),
            virtual_bytes: Some(0),
            threads: None,
            io_read_bytes: Some(0),
            io_written_bytes: Some(0),
        }
    }

    #[test]
    fn associates_only_equal_present_birth_ids_and_counts_unattributed() {
        let processes = [process(1, Some("original")), process(2, None)];
        let records = [
            (1, Some("original")),
            (1, Some("replacement")),
            (1, None),
            (2, Some("original")),
            (2, None),
            (3, Some("original")),
        ]
        .into_iter()
        .map(|(pid, birth)| SocketRecord {
            pid,
            birth_id: birth.map(str::to_owned),
            socket: PortSocket {
                protocol: "tcp".into(),
                local_address: "127.0.0.1".into(),
                local_port: 3000,
                state: "listen".into(),
            },
        })
        .collect();
        let report = associate(
            3000,
            &processes,
            PortScan {
                records,
                inaccessible_processes: 0,
                unattributed_sockets: 1,
            },
        );
        assert_eq!(report.owners.len(), 1);
        assert_eq!(report.owners[0].process.pid, 1);
        assert_eq!(report.owners[0].sockets.len(), 1);
        assert_eq!(report.unattributed_sockets, 6);
        assert_eq!(report.visibility, Visibility::Partial);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn live_scan_attributes_tcp_listener_and_udp_binding_to_current_generation() {
        let tcp = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let udp = std::net::UdpSocket::bind(("127.0.0.1", 0)).unwrap();
        let pid = std::process::id();
        let birth = crate::identity::birth_id(pid).expect("current PID identity is readable");
        let processes = [process(pid, Some(&birth))];
        for (protocol, port) in [
            ("tcp", tcp.local_addr().unwrap().port()),
            ("udp", udp.local_addr().unwrap().port()),
        ] {
            let report = inspect(port, &processes).unwrap();
            assert!(report.owners.iter().any(|owner| {
                owner.process.pid == pid
                    && owner.sockets.iter().any(|socket| {
                        socket.protocol == protocol
                            && socket.local_port == port
                            && socket.local_address == "127.0.0.1"
                    })
            }));
        }
    }
}
