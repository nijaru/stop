//! Output rendering: JSON (primary) and human tables (secondary).

use std::io::{self, IsTerminal, Write};

use owo_colors::OwoColorize;
use serde::Serialize;

use crate::model::{ProcessInfo, SystemMetrics};

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

/// Human table shared by `list` and `top`.
pub fn print_process_table(processes: &[ProcessInfo], header: Option<&str>) -> io::Result<()> {
    let color = io::stdout().is_terminal();

    let rows: Vec<(String, String, String, String, String, String)> = processes
        .iter()
        .map(|p| {
            let (rss_val, rss_unit) = crate::model::format_bytes_parts(p.rss_bytes);
            (
                p.pid.to_string(),
                p.user.clone().unwrap_or_else(|| "-".into()),
                truncate_chars(&p.name, NAME_WIDTH),
                format!("{:.1}", p.cpu_percent),
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
            .max(rows.iter().map(|r| r.0.len()).max().unwrap_or(0)),
        headers
            .1
            .len()
            .max(rows.iter().map(|r| r.1.len()).max().unwrap_or(0)),
        headers.2.len(),
        headers.3.len(),
        headers.4.len(),
        headers
            .5
            .len()
            .max(rows.iter().map(|r| r.5.len()).max().unwrap_or(0)),
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
        ("cpu_percent", format!("{:.2}", p.cpu_percent)),
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
            if io::stdout().is_terminal() {
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
    format!(
        "CPU {:.1}%  MEM {val}{unit}/{tval}{tunit} ({:.1}%)",
        metrics.cpu_percent, metrics.memory_used_percent
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

fn unix_to_rfc3339(secs: u64) -> String {
    use chrono::TimeZone;
    chrono::Utc
        .timestamp_opt(secs as i64, 0)
        .single()
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(|| secs.to_string())
}
