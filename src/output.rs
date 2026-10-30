//! Plain terminal reports and explicit JSON. Writers propagate early pipe closure.

use serde::Serialize;
use std::io::{self, IsTerminal, Write};

use crate::cmd::tree::TreeNode;
use crate::error::StopError;
use crate::model::{
    CollectionMetadata, ProcessInfo, SamplePoint, Snapshot, SystemMetrics, format_bytes_parts,
};
use crate::ports::{PortReport, Visibility};
use crate::runtime::Cancellation;

mod diagnostic;
mod table;
mod text;
mod writer;

use writer::Destination;

pub use diagnostic::{print_help, report_error, usage_error};

pub fn print_json<T: Serialize>(
    value: &T,
    pretty: bool,
    cancel: &Cancellation,
) -> Result<(), StopError> {
    let mut encoded = Vec::new();
    json_to(&mut encoded, value, pretty)?;
    writer::write(&encoded, cancel, writer::Destination::Stdout)
}
fn json_to<T: Serialize>(out: &mut impl Write, value: &T, pretty: bool) -> io::Result<()> {
    let result = if pretty {
        serde_json::to_writer_pretty(&mut *out, value)
    } else {
        serde_json::to_writer(&mut *out, value)
    };
    result.map_err(|err| {
        io::Error::new(
            err.io_error_kind().unwrap_or(io::ErrorKind::InvalidData),
            err,
        )
    })?;
    writeln!(out)
}
fn write_stdout(value: &str, cancel: &Cancellation) -> Result<(), StopError> {
    writer::write(value.as_bytes(), cancel, writer::Destination::Stdout)
}
fn use_color(destination: Destination) -> bool {
    let terminal = match destination {
        Destination::Stdout => io::stdout().is_terminal(),
        Destination::Stderr => io::stderr().is_terminal(),
    };
    terminal && std::env::var_os("NO_COLOR").is_none()
}

/// Escape process-controlled text, including invisible direction overrides.
pub fn safe(value: &str) -> String {
    let mut result = String::new();
    for c in value.chars() {
        if c.is_control()
            || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            result.extend(c.escape_default());
        } else {
            result.push(c);
        }
    }
    result
}
fn bytes(value: u64) -> String {
    let (n, u) = format_bytes_parts(value);
    format!("{n} {u}")
}
fn terminal_width(destination: Destination) -> usize {
    let size = match destination {
        Destination::Stdout => terminal_size::terminal_size_of(io::stdout()),
        Destination::Stderr => terminal_size::terminal_size_of(io::stderr()),
    };
    size.map(|(width, _)| usize::from(width.0)).unwrap_or(100)
}

fn process_table(processes: &[ProcessInfo], cpu: bool) -> String {
    let mut headers = vec!["PID", "USER", "NAME"];
    if cpu {
        headers.push("CPU%");
    }
    headers.push("RSS");
    let rows = processes
        .iter()
        .map(|p| {
            let mut row = vec![
                p.pid.to_string(),
                safe(p.user.as_deref().or(p.uid.as_deref()).unwrap_or("-")),
                safe(&p.name),
            ];
            if cpu {
                row.push(
                    p.cpu_percent
                        .map(|v| format!("{v:.1}"))
                        .unwrap_or_else(|| "n/a".into()),
                );
            }
            row.push(p.rss_bytes.map(bytes).unwrap_or_else(|| "n/a".into()));
            row
        })
        .collect();
    table::render(
        &headers,
        rows,
        &[0, 3, 4],
        &[(2, usize::MAX, 8), (1, 16, 4)],
        terminal_width(Destination::Stdout),
        use_color(Destination::Stdout),
    )
}

pub fn print_snapshot(
    snapshot: &Snapshot,
    system: Option<&SystemMetrics>,
    cpu: bool,
    cancel: &Cancellation,
) -> Result<(), StopError> {
    write_stdout(
        &render_snapshot(snapshot, system, cpu, "No matching processes found"),
        cancel,
    )
}

fn render_snapshot(
    snapshot: &Snapshot,
    system: Option<&SystemMetrics>,
    cpu: bool,
    empty_message: &str,
) -> String {
    let mut out = String::new();
    if let Some(system) = system {
        out.push_str(&system_header(system, cpu));
        out.push('\n');
    }
    if snapshot.returned == 0 {
        out.push_str(empty_message);
        out.push('\n');
    } else {
        out.push_str(&process_table(&snapshot.processes, cpu));
    }
    if snapshot.truncated {
        out.push_str(&format!(
            "{} of {} processes",
            snapshot.returned, snapshot.matched
        ));
        if snapshot.collection.inaccessible_processes > 0 {
            out.push_str(&format!(
                " · incomplete data: {}",
                snapshot.collection.inaccessible_processes
            ));
        }
        out.push('\n');
    } else {
        append_visibility(&mut out, &snapshot.collection);
    }
    out
}

fn count_label(count: usize, singular: &str, plural: &str) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
}

fn append_visibility(out: &mut String, metadata: &CollectionMetadata) {
    if metadata.inaccessible_processes > 0 {
        out.push_str(&format!(
            "Incomplete data for {}\n",
            count_label(metadata.inaccessible_processes, "process", "processes")
        ));
    }
}

pub fn print_sample_point(point: &SamplePoint, cancel: &Cancellation) -> Result<(), StopError> {
    let time = chrono::DateTime::parse_from_rfc3339(&point.snapshot.collection.collected_at)
        .expect("collection timestamps are RFC 3339")
        .with_timezone(&chrono::Local)
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string();
    let mut out = format!("Sample {}/{}  {time}\n", point.index, point.requested_count);
    let empty_message = if point.target_status.as_deref() == Some("no_longer_observed") {
        "Selected process no longer found"
    } else {
        "No matching processes found"
    };
    out.push_str(&render_snapshot(
        &point.snapshot,
        Some(&point.system),
        true,
        empty_message,
    ));
    if point.skipped_intervals > 0 {
        out.push_str(&format!(
            "Skipped {}\n",
            count_label(point.skipped_intervals, "sample", "samples")
        ));
    }
    out.push('\n');
    write_stdout(&out, cancel)
}

pub fn print_no_match(
    target: &str,
    metadata: &CollectionMetadata,
    cancel: &Cancellation,
) -> Result<(), StopError> {
    let mut out = format!("No process found for {}\n", safe(&format!("{target:?}")));
    append_visibility(&mut out, metadata);
    write_stdout(&out, cancel)
}

fn format_start_time(timestamp: Option<u64>) -> String {
    timestamp
        .and_then(|s| i64::try_from(s).ok())
        .and_then(|s| chrono::DateTime::from_timestamp(s, 0))
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S %:z")
                .to_string()
        })
        .unwrap_or_else(|| "n/a".into())
}

pub fn print_process_detail(
    p: &ProcessInfo,
    metadata: &CollectionMetadata,
    cpu: bool,
    cancel: &Cancellation,
) -> Result<(), StopError> {
    let mut out = format!(
        "{} · PID {}\nOwner {} · State {}\n\n",
        safe(&p.name),
        p.pid,
        safe(p.user.as_deref().or(p.uid.as_deref()).unwrap_or("n/a")),
        safe(&p.state)
    );
    let unavailable = |field: &str| match p.unavailable.get(field).map(String::as_str) {
        Some("unsupported") => "not supported".to_owned(),
        Some("not_requested") => "not measured".to_owned(),
        Some("missing_baseline") => "n/a (no previous measurement)".to_owned(),
        _ => "n/a".to_owned(),
    };
    let started = format_start_time(p.start_time);
    let width = terminal_width(Destination::Stdout);
    for (label, value) in [
        ("Started", started),
        (
            "Executable",
            p.exe
                .as_deref()
                .map(safe)
                .unwrap_or_else(|| unavailable("exe")),
        ),
        (
            "Directory",
            p.cwd
                .as_deref()
                .map(safe)
                .unwrap_or_else(|| unavailable("cwd")),
        ),
    ] {
        text::field(&mut out, label, &value, width);
    }
    if p.cmdline.is_empty() {
        text::field(&mut out, "Arguments", &unavailable("cmdline"), width);
    } else {
        text::arguments(&mut out, &p.cmdline, width);
    }
    for (label, value) in [
        (
            "Parent",
            p.ppid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "n/a".into()),
        ),
        (
            "RSS",
            p.rss_bytes
                .map(bytes)
                .unwrap_or_else(|| unavailable("rss_bytes")),
        ),
        (
            "Virtual",
            p.virtual_bytes
                .map(bytes)
                .unwrap_or_else(|| unavailable("virtual_bytes")),
        ),
        (
            "Threads",
            p.threads
                .map(|v| v.to_string())
                .unwrap_or_else(|| unavailable("threads")),
        ),
        (
            "Read total",
            p.io_read_bytes
                .map(bytes)
                .unwrap_or_else(|| unavailable("io_read_bytes")),
        ),
        (
            "Write total",
            p.io_written_bytes
                .map(bytes)
                .unwrap_or_else(|| unavailable("io_written_bytes")),
        ),
    ] {
        text::field(&mut out, label, &value, width);
    }
    if cpu {
        text::field(
            &mut out,
            "CPU",
            &p.cpu_percent
                .map(|v| format!("{v:.1}%"))
                .unwrap_or_else(|| unavailable("cpu_percent")),
            width,
        );
    }
    append_visibility(&mut out, metadata);
    write_stdout(&out, cancel)
}

pub fn system_header(metrics: &SystemMetrics, cpu: bool) -> String {
    let memory = format!(
        "Memory {} / {}",
        bytes(metrics.memory_used_bytes),
        bytes(metrics.memory_total_bytes)
    );
    if cpu {
        format!(
            "CPU {} · {memory}",
            metrics
                .cpu_percent
                .map(|v| format!("{v:.1}%"))
                .unwrap_or_else(|| "n/a".into())
        )
    } else {
        memory
    }
}

pub fn print_process_tree(
    roots: &[TreeNode],
    metadata: &CollectionMetadata,
    cpu: bool,
    cancel: &Cancellation,
) -> Result<(), StopError> {
    write_stdout(&render_process_tree(roots, metadata, cpu), cancel)
}
pub fn render_process_tree(roots: &[TreeNode], metadata: &CollectionMetadata, cpu: bool) -> String {
    fn label(node: &TreeNode, cpu: bool) -> String {
        let p = &node.process;
        format!(
            "{}  {}  {}{}",
            p.pid,
            safe(&p.name),
            p.rss_bytes.map(bytes).unwrap_or_else(|| "n/a".into()),
            if cpu {
                format!(
                    "  CPU {}",
                    p.cpu_percent
                        .map(|v| format!("{v:.1}%"))
                        .unwrap_or_else(|| "n/a".into())
                )
            } else {
                String::new()
            }
        )
    }
    fn walk(node: &TreeNode, prefix: &str, last: bool, cpu: bool, out: &mut String) {
        out.push_str(&format!(
            "{prefix}{}{}\n",
            if last { "└─ " } else { "├─ " },
            label(node, cpu)
        ));
        let next = format!("{prefix}{}", if last { "   " } else { "│  " });
        for (i, child) in node.children.iter().enumerate() {
            walk(child, &next, i + 1 == node.children.len(), cpu, out);
        }
    }
    let mut out = String::new();
    if roots.is_empty() {
        out.push_str("No processes found\n");
    }
    for root in roots {
        out.push_str(&label(root, cpu));
        out.push('\n');
        for (i, child) in root.children.iter().enumerate() {
            walk(child, "", i + 1 == root.children.len(), cpu, &mut out);
        }
    }
    append_visibility(&mut out, metadata);
    out
}

pub fn print_port_report(
    report: &PortReport,
    cpu: bool,
    cancel: &Cancellation,
) -> Result<(), StopError> {
    write_stdout(&render_port_report(report, cpu), cancel)
}

fn render_port_report(report: &PortReport, cpu: bool) -> String {
    let mut out = String::new();
    if report.owners.is_empty() {
        out.push_str(&format!("Port {}: no owner found\n", report.port));
    } else {
        out.push_str(&format!("Port {}\n", report.port));
        let mut headers = vec!["PID", "USER", "NAME", "PROTO", "LOCAL"];
        if cpu {
            headers.push("CPU%");
        }
        let rows = report
            .owners
            .iter()
            .flat_map(|owner| {
                owner.sockets.iter().map(|socket| {
                    let addr = if socket.local_address.contains(':') {
                        format!("[{}]:{}", socket.local_address, socket.local_port)
                    } else {
                        format!("{}:{}", socket.local_address, socket.local_port)
                    };
                    let mut row = vec![
                        owner.process.pid.to_string(),
                        safe(
                            owner
                                .process
                                .user
                                .as_deref()
                                .or(owner.process.uid.as_deref())
                                .unwrap_or("-"),
                        ),
                        safe(&owner.process.name),
                        safe(&socket.protocol),
                        safe(&addr),
                    ];
                    if cpu {
                        row.push(
                            owner
                                .process
                                .cpu_percent
                                .map(|p| format!("{p:.1}"))
                                .unwrap_or_else(|| "n/a".into()),
                        );
                    }
                    row
                })
            })
            .collect();
        out.push_str(&table::render(
            &headers,
            rows,
            &[0, 5],
            &[(2, usize::MAX, 8), (1, 16, 4)],
            terminal_width(Destination::Stdout),
            use_color(Destination::Stdout),
        ));
    }
    if report.visibility == Visibility::Partial {
        let mut reasons = Vec::new();
        if report.inaccessible_processes > 0 {
            reasons.push(format!(
                "{} inaccessible",
                count_label(report.inaccessible_processes, "process", "processes")
            ));
        }
        if report.unattributed_sockets > 0 {
            reasons.push(format!(
                "{} without an owner",
                count_label(report.unattributed_sockets, "socket", "sockets")
            ));
        }
        out.push_str(&format!("Search incomplete: {}\n", reasons.join("; ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{PortOwner, PortSocket};
    use unicode_width::UnicodeWidthStr;

    fn process() -> ProcessInfo {
        ProcessInfo {
            pid: 12345,
            start_time: Some(1_700_000_000),
            birth_id: Some("test".into()),
            unavailable: Default::default(),
            ppid: Some(1),
            name: "日本語❤️".into(),
            exe: None,
            cmdline: vec![],
            cwd: None,
            state: "run".into(),
            user: None,
            uid: Some("501".into()),
            cpu_percent: Some(12.3),
            rss_bytes: Some(1024),
            virtual_bytes: None,
            threads: None,
            io_read_bytes: None,
            io_written_bytes: None,
        }
    }

    fn metadata() -> CollectionMetadata {
        CollectionMetadata {
            schema_version: crate::model::SCHEMA_VERSION,
            collection_started_at: "2026-01-01T00:00:00Z".into(),
            collected_at: "2026-01-01T00:00:00Z".into(),
            collection_duration_ms: 0,
            cpu_interval_ms: None,
            scope_id: Some("test".into()),
            scope_pid: None,
            inaccessible_processes: 1,
        }
    }

    #[test]
    fn untrusted_text_cannot_control_the_terminal() {
        assert_eq!(
            safe("node\n\x1b[31m\t\u{202e}"),
            "node\\n\\u{1b}[31m\\t\\u{202e}"
        );
    }

    #[test]
    fn trees_report_data_gaps_even_when_empty() {
        for roots in [
            vec![],
            vec![TreeNode {
                process: process(),
                children: vec![],
            }],
        ] {
            let text = render_process_tree(&roots, &metadata(), false);
            assert!(text.contains("Incomplete data for 1 process\n"), "{text}");
            if roots.is_empty() {
                assert!(text.starts_with("No processes found\n"));
            } else {
                assert!(text.starts_with("12345"));
            }
        }
    }

    #[test]
    fn truncated_snapshots_combine_counts_and_data_gaps_without_hiding_either() {
        let mut second = process();
        second.pid += 1;
        let snapshot = Snapshot::finish(metadata(), 2, vec![process(), second], Some(1));
        let text = render_snapshot(&snapshot, None, false, "No matching processes found");
        assert!(
            text.ends_with("1 of 2 processes · incomplete data: 1\n"),
            "{text}"
        );
        assert_eq!(text.matches("incomplete data").count(), 1);
    }

    #[test]
    fn port_columns_align_with_unicode_names_and_uid_fallback() {
        let report = PortReport {
            port: 3000,
            visibility: Visibility::Partial,
            inaccessible_processes: 2,
            unattributed_sockets: 1,
            owners: vec![PortOwner {
                process: process(),
                sockets: vec![PortSocket {
                    protocol: "tcp".into(),
                    local_address: "::1".into(),
                    local_port: 3000,
                    state: "listen".into(),
                }],
            }],
        };
        let text = render_port_report(&report, true);
        let lines: Vec<_> = text.lines().collect();
        let header = lines[1];
        let row = lines[2];
        for (label, value) in [("USER", "501"), ("PROTO", "tcp"), ("LOCAL", "[::1]:3000")] {
            assert_eq!(
                header[..header.find(label).unwrap()].width(),
                row[..row.find(value).unwrap()].width(),
                "{text}"
            );
        }
        assert!(row.trim_end().ends_with("12.3"), "{text}");
        assert_eq!(text.matches("Search incomplete:").count(), 1);
        assert!(
            text.contains("2 processes inaccessible; 1 socket without an owner"),
            "{text}"
        );
    }

    #[test]
    fn empty_port_reports_do_not_claim_the_port_is_free() {
        let mut report = PortReport {
            port: 3000,
            visibility: Visibility::Complete,
            inaccessible_processes: 0,
            unattributed_sockets: 0,
            owners: vec![],
        };
        assert_eq!(
            render_port_report(&report, false),
            "Port 3000: no owner found\n"
        );
        report.visibility = Visibility::Partial;
        report.inaccessible_processes = 1;
        assert_eq!(
            render_port_report(&report, false),
            "Port 3000: no owner found\nSearch incomplete: 1 process inaccessible\n"
        );
    }
}
