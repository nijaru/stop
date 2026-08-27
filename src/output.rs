//! Output rendering: JSON (primary) and human tables (secondary).

use std::io::{self, IsTerminal, Write};

use owo_colors::OwoColorize;
use serde::Serialize;

use crate::cmd::tree::TreeNode;
use crate::model::{ProcessInfo, SystemMetrics};
use crate::ports::{PortReport, Visibility};

/// Renders a value as compact or pretty JSON to stdout.
///
/// Broken pipes are treated as success (standard CLI behavior for piped
/// consumers like `head`).
pub fn print_json<T: Serialize>(value: &T, pretty: bool) -> io::Result<()> {
    let out = if pretty {
        serde_json::to_string_pretty(value)?
    } else {
        serde_json::to_string(value)?
    };
    write_stdout(out)
}

fn write_stdout(s: String) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    writeln!(lock, "{s}").or_else(ignore_broken_pipe)
}

fn ignore_broken_pipe(err: io::Error) -> io::Result<()> {
    if err.kind() == io::ErrorKind::BrokenPipe {
        Ok(())
    } else {
        Err(err)
    }
}

/// Truncates on char boundaries with an ellipsis.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
}

const NAME_WIDTH: usize = 32;

/// Colors only for terminals, and never when NO_COLOR is set
/// (https://no-color.org/).
fn use_color() -> bool {
    io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Human table shared by `list` and `top`.
pub fn print_process_table(processes: &[ProcessInfo], header: Option<&str>) -> io::Result<()> {
    let color = use_color();

    let rows: Vec<(String, String, String, String, String, String)> = processes
        .iter()
        .map(|p| {
            let (rss_val, rss_unit) = crate::model::format_bytes_parts(p.rss_bytes);
            (
                p.pid.to_string(),
                p.user.clone().unwrap_or_else(|| "-".into()),
                truncate_chars(&p.name, NAME_WIDTH),
                p.cpu_percent
                    .map(|c| format!("{c:.1}"))
                    .unwrap_or_else(|| "-".into()),
                format!("{rss_val}{rss_unit}"),
                p.threads
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "-".into()),
            )
        })
        .collect();

    let headers = ("PID", "USER", "NAME", "CPU%", "MEM", "THR");
    let widths = [
        headers
            .0
            .len()
            .max(rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(0)),
        headers
            .1
            .len()
            .max(rows.iter().map(|r| r.1.chars().count()).max().unwrap_or(0)),
        headers
            .2
            .len()
            .max(rows.iter().map(|r| r.2.chars().count()).max().unwrap_or(0)),
        headers.3.len(),
        headers.4.len(),
        headers
            .5
            .len()
            .max(rows.iter().map(|r| r.5.chars().count()).max().unwrap_or(0)),
    ];

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let res: io::Result<()> = (|| {
        if let Some(h) = header {
            writeln!(out, "{h}")?;
        }
        let head = format!(
            "{:<pid_w$} {:<usr_w$} {:<name_w$} {:>cpu_w$} {:>mem_w$} {:>thr_w$}",
            headers.0,
            headers.1,
            headers.2,
            headers.3,
            headers.4,
            headers.5,
            pid_w = widths[0],
            usr_w = widths[1],
            name_w = widths[2],
            cpu_w = widths[3],
            mem_w = widths[4],
            thr_w = widths[5]
        );
        if color {
            writeln!(out, "{}", head.bold())?;
        } else {
            writeln!(out, "{head}")?;
        }
        for r in &rows {
            writeln!(
                out,
                "{:<pid_w$} {:<usr_w$} {:<name_w$} {:>cpu_w$} {:>mem_w$} {:>thr_w$}",
                r.0,
                r.1,
                r.2,
                r.3,
                r.4,
                r.5,
                pid_w = widths[0],
                usr_w = widths[1],
                name_w = widths[2],
                cpu_w = widths[3],
                mem_w = widths[4],
                thr_w = widths[5]
            )?;
        }
        Ok(())
    })();
    res.or_else(ignore_broken_pipe)
}

/// Indented process tree for `tree`: PID and NAME per node, box-drawing
/// connectors, children nested under their parent.
pub fn print_process_tree(roots: &[TreeNode]) -> io::Result<()> {
    write_stdout(render_process_tree(roots, use_color()))
}

/// Renders the forest as one line per node. Root nodes carry no
/// connector; every descendant is prefixed with its connector chain.
pub fn render_process_tree(roots: &[TreeNode], color: bool) -> String {
    let mut out = String::new();
    for root in roots {
        out.push_str(&node_label(root));
        out.push('\n');
        for (i, child) in root.children.iter().enumerate() {
            write_node(&mut out, child, "", i + 1 == root.children.len(), color);
        }
    }
    out
}

fn node_label(node: &TreeNode) -> String {
    format!(
        "{:<6} {}",
        node.process.pid,
        truncate_chars(&node.process.name, NAME_WIDTH)
    )
}

/// Human table for port ownership results.
pub fn print_port_report(report: &PortReport) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let res: io::Result<()> = (|| {
        if report.owners.is_empty() {
            let visibility = visibility_name(report.visibility);
            if report.visibility == Visibility::Partial {
                writeln!(
                    out,
                    "no visible process owns port {} (visibility: {visibility}; {} inaccessible processes, {} unattributed sockets)",
                    report.port, report.inaccessible_processes, report.unattributed_sockets
                )?;
            } else {
                writeln!(
                    out,
                    "no process owns port {} (visibility: {visibility})",
                    report.port
                )?;
            }
            return Ok(());
        }

        writeln!(
            out,
            "PORT {}  VISIBILITY {}",
            report.port,
            visibility_name(report.visibility)
        )?;
        writeln!(
            out,
            "PID    USER NAME                             PROTO LOCAL STATE"
        )?;
        for owner in &report.owners {
            for socket in &owner.sockets {
                writeln!(
                    out,
                    "{:<6} {:<4} {:<32} {:<5} {:<22} {}",
                    owner.process.pid,
                    owner.process.user.as_deref().unwrap_or("-"),
                    truncate_chars(&owner.process.name, NAME_WIDTH),
                    socket.protocol,
                    format!("{}:{}", socket.local_address, socket.local_port),
                    socket.state
                )?;
            }
        }
        Ok(())
    })();
    res.or_else(ignore_broken_pipe)
}

fn visibility_name(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Complete => "complete",
        Visibility::Partial => "partial",
    }
}

fn write_node(out: &mut String, node: &TreeNode, prefix: &str, is_last: bool, color: bool) {
    let connector = if is_last { "└─ " } else { "├─ " };
    let label = node_label(node);
    if color {
        out.push_str(&format!("{}{}{}\n", prefix, connector.dimmed(), label));
    } else {
        out.push_str(&format!("{prefix}{connector}{label}\n"));
    }

    let child_prefix = format!("{prefix}{}", if is_last { "   " } else { "│  " });
    for (i, child) in node.children.iter().enumerate() {
        write_node(
            out,
            child,
            &child_prefix,
            i + 1 == node.children.len(),
            color,
        );
    }
}

/// Human key/value detail for `inspect`.
pub fn print_process_detail(p: &ProcessInfo) -> io::Result<()> {
    let kv: Vec<(&str, String)> = vec![
        ("pid", p.pid.to_string()),
        ("start_time", unix_to_rfc3339(p.start_time)),
        ("ppid", opt_u32(&p.ppid)),
        ("name", p.name.clone()),
        ("exe", opt_str(&p.exe)),
        ("cmdline", p.cmdline.join(" ")),
        ("cwd", opt_str(&p.cwd)),
        ("state", p.state.clone()),
        ("user", opt_str(&p.user)),
        ("uid", opt_str(&p.uid)),
        ("cpu_percent", opt_f32(p.cpu_percent)),
        ("rss", human_bytes(p.rss_bytes)),
        ("virtual", human_bytes(p.virtual_bytes)),
        ("threads", opt_u32(&p.threads)),
        ("io_read_total", human_bytes(p.io_read_bytes)),
        ("io_written_total", human_bytes(p.io_written_bytes)),
    ];

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let res: io::Result<()> = (|| {
        for (k, v) in kv {
            if use_color() {
                writeln!(out, "{}: {}", k.bold(), v)?;
            } else {
                writeln!(out, "{k}: {v}")?;
            }
        }
        Ok(())
    })();
    res.or_else(ignore_broken_pipe)
}

/// Human header line summarizing system metrics.
pub fn system_header(metrics: &SystemMetrics) -> String {
    let (val, unit) = crate::model::format_bytes_parts(metrics.memory_used_bytes);
    let (tval, tunit) = crate::model::format_bytes_parts(metrics.memory_total_bytes);
    let cpu = metrics
        .cpu_percent
        .map(|c| format!("{c:.1}%"))
        .unwrap_or_else(|| "-".into());
    format!(
        "CPU {cpu}  MEM {val}{unit}/{tval}{tunit} ({:.1}%)",
        metrics.memory_used_percent
    )
}

fn human_bytes(bytes: u64) -> String {
    let (v, u) = crate::model::format_bytes_parts(bytes);
    format!("{v} {u}")
}

fn opt_str(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "-".into())
}

fn opt_u32(v: &Option<u32>) -> String {
    match v {
        Some(n) => n.to_string(),
        None => "-".into(),
    }
}

fn opt_f32(v: Option<f32>) -> String {
    v.map(|c| format!("{c:.2}")).unwrap_or_else(|| "-".into())
}

fn unix_to_rfc3339(secs: u64) -> String {
    use chrono::TimeZone;
    chrono::Utc
        .timestamp_opt(secs as i64, 0)
        .single()
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(|| secs.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ProcessInfo;

    fn node(pid: u32, name: &str, children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            process: ProcessInfo {
                pid,
                start_time: 0,
                ppid: None,
                name: name.to_string(),
                exe: None,
                cmdline: vec![],
                cwd: None,
                state: "run".to_string(),
                user: None,
                uid: None,
                cpu_percent: None,
                rss_bytes: 0,
                virtual_bytes: 0,
                threads: None,
                io_read_bytes: 0,
                io_written_bytes: 0,
            },
            children,
        }
    }

    #[test]
    fn render_nests_children_with_connectors() {
        // 1 -> [2 -> [3], 4]
        let roots = vec![node(
            1,
            "a",
            vec![
                node(2, "b", vec![node(3, "c", vec![])]),
                node(4, "d", vec![]),
            ],
        )];
        let rendered = render_process_tree(&roots, false);
        assert_eq!(
            rendered,
            "1      a\n".to_string() + "├─ 2      b\n" + "│  └─ 3      c\n" + "└─ 4      d\n"
        );
    }

    #[test]
    fn render_multiple_roots_have_no_connectors() {
        let roots = vec![node(1, "a", vec![]), node(2, "b", vec![])];
        let rendered = render_process_tree(&roots, false);
        assert_eq!(rendered, "1      a\n2      b\n");
    }
}
