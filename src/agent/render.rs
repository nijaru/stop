use super::model::{ProcessRecord, ProcessResult};
use std::io::{self, Write};

pub fn write_json(result: &ProcessResult) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    serde_json::to_writer(&mut out, result).map_err(io::Error::other)?;
    writeln!(out)
}

pub fn write_list_text(result: &ProcessResult) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    writeln!(out, "{:<8} {:<24} {:>8} {:>12}  CWD", "PID", "PROCESS", "CPU%", "RSS")?;

    for process in &result.processes {
        writeln!(
            out,
            "{:<8} {:<24} {:>7.1}% {:>12}  {}",
            process.id.pid,
            truncate(&process.name, 24),
            process.cpu_percent,
            format_bytes(process.rss_bytes),
            process.cwd.as_deref().unwrap_or("-")
        )?;
    }

    if result.meta.truncated {
        writeln!(
            out,
            "\nshowing {} of {} matches",
            result.meta.returned, result.meta.matched
        )?;
    }

    Ok(())
}

pub fn write_inspect_text(process: &ProcessRecord) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    writeln!(
        out,
        "{}  pid {}  {}",
        process.name, process.id.pid, process.state
    )?;
    writeln!(out, "id       {}", process.id.stable_id())?;
    writeln!(
        out,
        "parent   {}",
        process
            .ppid
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "-".to_string())
    )?;
    writeln!(out, "cwd      {}", process.cwd.as_deref().unwrap_or("-"))?;
    writeln!(out, "exe      {}", process.exe.as_deref().unwrap_or("-"))?;
    writeln!(out, "cpu      {:.1}%", process.cpu_percent)?;
    writeln!(out, "rss      {}", format_bytes(process.rss_bytes))?;
    writeln!(
        out,
        "threads  {}",
        process
            .threads
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
    )?;
    writeln!(out, "read     {}", format_bytes(process.io.read_bytes))?;
    writeln!(out, "written  {}", format_bytes(process.io.write_bytes))?;

    if !process.argv.is_empty() {
        writeln!(out, "command  {}", process.argv.join(" "))?;
    }

    Ok(())
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let prefix: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", prefix.chars().take(max_chars.saturating_sub(1)).collect::<String>())
    } else {
        prefix
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    let value = bytes as f64;
    if value >= GIB {
        format!("{:.1} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}
