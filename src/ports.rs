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
    pub socket: PortSocket,
}

pub fn inspect(port: u16, processes: &[ProcessInfo]) -> Result<PortReport, StopError> {
    let scan = platform_scan(port)?;
    let by_pid: std::collections::HashMap<u32, &ProcessInfo> =
        processes.iter().map(|p| (p.pid, p)).collect();
    let mut sockets_by_pid: BTreeMap<u32, Vec<PortSocket>> = BTreeMap::new();
    let mut unattributed_sockets = scan.unattributed_sockets;

    for record in scan.records {
        if by_pid.contains_key(&record.pid) {
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

    Ok(PortReport {
        port,
        visibility,
        inaccessible_processes: scan.inaccessible_processes,
        unattributed_sockets,
        owners,
    })
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
