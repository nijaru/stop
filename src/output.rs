//! Plain terminal reports and explicit JSON. Writers propagate early pipe closure.

use owo_colors::OwoColorize;
use serde::Serialize;
use std::io::{self, IsTerminal, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::cmd::tree::TreeNode;
use crate::error::StopError;
use crate::model::{
    CollectionMetadata, ProcessInfo, SamplePoint, Snapshot, SystemMetrics, format_bytes_parts,
};
use crate::ports::{PortReport, Visibility};
use crate::runtime::Cancellation;

mod writer;

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
fn use_color() -> bool {
    io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
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
fn truncate(value: &str, width: usize) -> String {
    if value.width() <= width {
        return value.into();
    }
    let mut result = String::new();
    let mut used = 0;
    for c in value.chars() {
        let next = c.width().unwrap_or(0);
        if used + next > width.saturating_sub(1) {
            break;
        }
        result.push(c);
        used += next;
    }
    result.push('…');
    result
}
fn pad(value: &str, width: usize, right: bool) -> String {
    let space = " ".repeat(width.saturating_sub(value.width()));
    if right {
        format!("{space}{value}")
    } else {
        format!("{value}{space}")
    }
}
fn bytes(value: u64) -> String {
    let (n, u) = format_bytes_parts(value);
    format!("{n} {u}")
}
fn terminal_width() -> usize {
    if io::stdout().is_terminal() {
        terminal_size::terminal_size()
            .map(|(w, _)| usize::from(w.0))
            .unwrap_or(100)
    } else {
        100
    }
}

fn table(processes: &[ProcessInfo], cpu: bool) -> String {
    let mut headers = vec!["PID", "USER", "NAME"];
    if cpu {
        headers.push("CPU%");
    }
    headers.push("RSS");
    let mut rows: Vec<Vec<String>> = processes
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
    let mut widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            rows.iter()
                .map(|r| r[i].width())
                .max()
                .unwrap_or(0)
                .max(h.width())
        })
        .collect();
    // Only names/owners are shortened; numbers stay readable in narrow terminals.
    widths[1] = widths[1].min(16);
    let fixed = widths
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 2)
        .map(|(_, w)| w)
        .sum::<usize>()
        + 2 * (widths.len() - 1);
    widths[2] = widths[2].min(terminal_width().saturating_sub(fixed).max(8));
    for row in &mut rows {
        row[1] = truncate(&row[1], widths[1]);
        row[2] = truncate(&row[2], widths[2]);
    }
    let render = |row: &[String]| {
        row.iter()
            .enumerate()
            .map(|(i, s)| pad(s, widths[i], i == 0 || i >= 3))
            .collect::<Vec<_>>()
            .join("  ")
    };
    let heading = render(&headers.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    let mut result = if use_color() {
        format!("{}\n", heading.bold())
    } else {
        format!("{heading}\n")
    };
    for row in rows {
        result.push_str(&render(&row));
        result.push('\n');
    }
    result
}

pub fn print_snapshot(
    snapshot: &Snapshot,
    system: Option<&SystemMetrics>,
    cpu: bool,
    cancel: &Cancellation,
) -> Result<(), StopError> {
    let mut out = String::new();
    if let Some(system) = system {
        out.push_str(&system_header(system, cpu));
        out.push('\n');
    }
    if let Some(ms) = snapshot.collection.cpu_interval_ms {
        out.push_str(&format!("CPU measured over {ms} ms\n"));
    }
    if snapshot.returned == 0 {
        out.push_str("No visible processes matched.\n");
    } else {
        out.push_str(&table(&snapshot.processes, cpu));
    }
    if snapshot.truncated {
        out.push_str(&format!(
            "Showing {} of {} matching processes ({} observed).\n",
            snapshot.returned, snapshot.matched, snapshot.total_processes
        ));
    }
    append_visibility(&mut out, &snapshot.collection);
    write_stdout(&out, cancel)
}
fn append_visibility(out: &mut String, metadata: &CollectionMetadata) {
    if metadata.inaccessible_processes > 0 {
        out.push_str(&format!("Visibility is partial: {} processes had limited facts or were lost during collection.\n",metadata.inaccessible_processes));
    }
}

pub fn print_sample_point(point: &SamplePoint, cancel: &Cancellation) -> Result<(), StopError> {
    write_stdout(
        &format!(
            "SAMPLE {}/{}  {}\n",
            point.index, point.requested_count, point.snapshot.collection.collected_at
        ),
        cancel,
    )?;
    print_snapshot(&point.snapshot, Some(&point.system), true, cancel)?;
    if point.skipped_intervals > 0 {
        write_stdout(
            &format!(
                "Skipped {} elapsed schedule slots.\n",
                point.skipped_intervals
            ),
            cancel,
        )?;
    }
    if point.target_status.as_deref() == Some("no_longer_observed") {
        write_stdout(
            "The selected process identity is no longer observed.\n",
            cancel,
        )?;
    }
    write_stdout("\n", cancel)
}

pub fn print_no_match(target: &str, cancel: &Cancellation) -> Result<(), StopError> {
    write_stdout(
        &format!(
            "No visible process matching {}. Search names with stop list --name <name>.\n",
            safe(&format!("{target:?}"))
        ),
        cancel,
    )
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
        safe(
            p.user
                .as_deref()
                .or(p.uid.as_deref())
                .unwrap_or("unavailable")
        ),
        safe(&p.state)
    );
    let unavailable = |field: &str| {
        format!(
            "unavailable ({})",
            p.unavailable
                .get(field)
                .map(String::as_str)
                .unwrap_or("unknown")
        )
    };
    let start = p
        .start_time
        .and_then(|s| i64::try_from(s).ok())
        .and_then(|s| chrono::DateTime::from_timestamp(s, 0));
    let started = start
        .map(|s| s.to_rfc3339())
        .unwrap_or_else(|| "unavailable".into());
    let argv = p
        .cmdline
        .iter()
        .map(|s| safe(&format!("{s:?}")))
        .collect::<Vec<_>>()
        .join(" ");
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
        (
            "Arguments",
            if argv.is_empty() {
                unavailable("cmdline")
            } else {
                argv
            },
        ),
        (
            "Parent",
            p.ppid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "unavailable".into()),
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
        out.push_str(&format!("{label:<12} {value}\n"));
    }
    if cpu {
        out.push_str(&format!(
            "{:<12} {}\n",
            "CPU",
            p.cpu_percent
                .map(|v| format!("{v:.1}%"))
                .unwrap_or_else(|| unavailable("cpu_percent"))
        ));
    }
    if let Some(ms) = metadata.cpu_interval_ms {
        out.push_str(&format!("CPU measured over {ms} ms\n"));
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
                .unwrap_or_else(|| "unavailable".into())
        )
    } else {
        memory
    }
}

pub fn print_process_tree(
    roots: &[TreeNode],
    cpu: bool,
    cancel: &Cancellation,
) -> Result<(), StopError> {
    write_stdout(&render_process_tree(roots, cpu), cancel)
}
pub fn render_process_tree(roots: &[TreeNode], cpu: bool) -> String {
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
    for root in roots {
        out.push_str(&label(root, cpu));
        out.push('\n');
        for (i, child) in root.children.iter().enumerate() {
            walk(child, "", i + 1 == root.children.len(), cpu, &mut out);
        }
    }
    out
}

pub fn print_port_report(
    report: &PortReport,
    metadata: &CollectionMetadata,
    cpu: bool,
    cancel: &Cancellation,
) -> Result<(), StopError> {
    let partial = report.visibility == Visibility::Partial;
    let mut out = format!(
        "PORT {} · Visibility {}\n",
        report.port,
        if partial {
            "partial"
        } else {
            "complete within the observed scope"
        }
    );
    if report.owners.is_empty() {
        out.push_str("No visible owner found.\n");
    } else {
        out.push_str(&format!(
            "PID     USER          NAME                  PROTO  LOCAL{}\n",
            if cpu { "  CPU%" } else { "" }
        ));
        for owner in &report.owners {
            for socket in &owner.sockets {
                let addr = if socket.local_address.contains(':') {
                    format!("[{}]:{}", socket.local_address, socket.local_port)
                } else {
                    format!("{}:{}", socket.local_address, socket.local_port)
                };
                out.push_str(&format!(
                    "{:<7} {}  {}  {:<5}  {}{}\n",
                    owner.process.pid,
                    pad(
                        &truncate(&safe(owner.process.user.as_deref().unwrap_or("-")), 12),
                        12,
                        false
                    ),
                    pad(&truncate(&safe(&owner.process.name), 20), 20, false),
                    safe(&socket.protocol),
                    safe(&addr),
                    if cpu {
                        format!(
                            "  {}",
                            owner
                                .process
                                .cpu_percent
                                .map(|p| format!("{p:.1}"))
                                .unwrap_or_else(|| "n/a".into())
                        )
                    } else {
                        String::new()
                    }
                ));
            }
        }
    }
    if let Some(ms) = metadata.cpu_interval_ms {
        out.push_str(&format!("CPU measured over {ms} ms\n"));
    }
    append_visibility(&mut out, metadata);
    if partial {
        out.push_str(&format!("{} inaccessible processes; {} unattributed sockets. This does not establish that the port is free.\n",report.inaccessible_processes,report.unattributed_sockets));
    }
    write_stdout(&out, cancel)
}

pub fn print_help(help: &clap::Error, cancel: &Cancellation) -> Result<(), StopError> {
    let help = help.render();
    let text = if use_color() {
        help.ansi().to_string()
    } else {
        help.to_string()
    };
    write_stdout(&text, cancel)
}

pub fn report_error(
    err: &StopError,
    machine: bool,
    cancel: Option<&Cancellation>,
) -> Result<(), StopError> {
    let mut out = Vec::new();
    if machine {
        if let StopError::Resolve(err) = err {
            json_to(&mut out, err, false)?;
        } else {
            #[derive(Serialize)]
            struct ErrorPayload<'a> {
                code: &'a str,
                message: String,
            }
            let code = match err {
                StopError::InvalidRequest(_) => "usage",
                StopError::Interrupted => "interrupted",
                StopError::UnsupportedPlatform => "unsupported_platform",
                _ => "operational",
            };
            json_to(
                &mut out,
                &ErrorPayload {
                    code,
                    message: err.to_string(),
                },
                false,
            )?;
        }
    } else {
        writeln!(out, "stop: {}", safe(&err.to_string()))?;
        if let StopError::Resolve(err) = err {
            for c in &err.candidates {
                writeln!(
                    out,
                    "  PID {}  {}  {}  started {}  {}",
                    c.pid,
                    safe(&c.name),
                    safe(c.user.as_deref().unwrap_or("-")),
                    c.start_time
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "unavailable".into()),
                    safe(c.exe.as_deref().or(c.cwd.as_deref()).unwrap_or(""))
                )?;
            }
        }
    }
    if let Some(cancel) = cancel {
        writer::write(&out, cancel, writer::Destination::Stderr)
    } else {
        // Handler installation failed. Leave descriptor flags alone: the
        // default signal action must remain able to terminate a blocked write.
        io::stderr().lock().write_all(&out)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn untrusted_text_cannot_control_the_terminal() {
        assert_eq!(
            safe("node\n\x1b[31m\t\u{202e}"),
            "node\\n\\u{1b}[31m\\t\\u{202e}"
        );
    }
    #[test]
    fn truncation_and_alignment_use_display_cells() {
        assert_eq!(truncate("日本語abcd", 6), "日本…");
        assert_eq!(pad("日本", 6, false), "日本  ");
        assert_eq!(pad("日本", 6, true), "  日本");
    }
}
