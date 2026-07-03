//! Output formatting: structured (JSON, CSV) and human-readable display.

use crate::data::{self, DEFAULT_TOP_N, SystemSnapshot};
use owo_colors::OwoColorize;
use std::borrow::Cow;
use std::io::{self, BufWriter, Write};

/// Truncates a string to at most `max_chars` characters, splitting on a valid UTF-8 boundary.
fn truncate_str(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Output format with format-specific state.
///
/// `Csv` carries a `BufWriter` for performance and a `header_written` flag
/// so the header is emitted only once across watch-mode iterations.
pub enum Output {
    /// Pretty-printed JSON to stdout.
    Json,
    /// RFC 4180 CSV with header.
    Csv {
        writer: BufWriter<io::Stdout>,
        header_written: bool,
    },
    /// Color-coded table for terminal display.
    Human {
        search: Option<String>,
        filter: Option<String>,
        sort_by: String,
        limit: usize,
        verbose: bool,
    },
}

impl Output {
    /// Creates a JSON output format.
    pub fn json() -> Self {
        Output::Json
    }

    /// Creates a CSV output format with a buffered stdout writer.
    pub fn csv() -> Self {
        Output::Csv {
            writer: BufWriter::new(io::stdout()),
            header_written: false,
        }
    }

    /// Creates a human-readable output format.
    ///
    /// The display parameters (`search`, `filter`, `sort_by`, `limit`) are shown
    /// in the header but do not affect the data — the caller has already applied
    /// the pipeline.
    pub fn human(
        search: Option<String>,
        filter: Option<String>,
        sort_by: Option<String>,
        limit: Option<usize>,
        verbose: bool,
    ) -> Self {
        Output::Human {
            search,
            filter,
            sort_by: sort_by.unwrap_or_else(|| "cpu".to_string()),
            limit: limit.unwrap_or(DEFAULT_TOP_N),
            verbose,
        }
    }

    /// Writes one snapshot in this format.
    pub fn write(&mut self, snapshot: &SystemSnapshot) -> io::Result<()> {
        match self {
            Output::Json => {
                writeln!(
                    io::stdout(),
                    "{}",
                    serde_json::to_string(snapshot).map_err(io::Error::other)?
                )?;
                io::stdout().flush()
            }
            Output::Csv {
                writer,
                header_written,
            } => {
                if !*header_written {
                    write_csv_header(writer)?;
                    *header_written = true;
                }
                write_csv_rows(writer, snapshot)?;
                writer.flush()
            }
            Output::Human {
                search,
                filter,
                sort_by,
                limit,
                verbose,
            } => write_human(snapshot, search, filter, sort_by, *limit, *verbose),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Converts a `BrokenPipe` error into `Ok(())` for graceful exit when output is piped.
pub fn ignore_broken_pipe(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other,
    }
}

/// Escapes a field for CSV output according to RFC 4180.
///
/// Returns a borrowed reference if no escaping is needed, avoiding allocations.
#[must_use]
pub fn escape_csv_field(field: &str) -> Cow<'_, str> {
    if field
        .bytes()
        .any(|b| matches!(b, b',' | b'"' | b'\n' | b'\r'))
    {
        Cow::Owned(format!("\"{}\"", field.replace('"', "\"\"")))
    } else {
        Cow::Borrowed(field)
    }
}

fn write_csv_header<W: Write>(writer: &mut W) -> io::Result<()> {
    writeln!(
        writer,
        "timestamp,cpu_usage,memory_total,memory_used,memory_percent,\
         pid,name,cpu_percent,memory_bytes,memory_percent_process,\
         user,command,thread_count,disk_read_bytes,disk_write_bytes,open_files"
    )?;
    writer.flush()
}

fn write_csv_rows<W: Write>(writer: &mut W, snapshot: &SystemSnapshot) -> io::Result<()> {
    for process in &snapshot.processes {
        let open_files_str = process
            .open_files
            .map(|n| n.to_string())
            .unwrap_or_default();
        writeln!(
            writer,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            escape_csv_field(&snapshot.timestamp),
            snapshot.system.cpu_usage,
            snapshot.system.memory_total,
            snapshot.system.memory_used,
            snapshot.system.memory_percent,
            process.pid,
            escape_csv_field(&process.name),
            process.cpu_percent,
            process.memory_bytes,
            process.memory_percent,
            escape_csv_field(&process.user),
            escape_csv_field(&process.command),
            process.thread_count,
            process.disk_read_bytes,
            process.disk_write_bytes,
            open_files_str
        )?;
    }
    writer.flush()
}

fn write_human(
    snapshot: &SystemSnapshot,
    search: &Option<String>,
    filter: &Option<String>,
    sort_by: &str,
    limit: usize,
    verbose: bool,
) -> io::Result<()> {
    let mut stdout = io::stdout();

    // Header
    writeln!(
        stdout,
        "{} {}",
        "stop".bold().cyan(),
        format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
    )?;
    writeln!(stdout)?;
    writeln!(stdout, "{}", "System:".bold())?;

    // CPU — color by usage
    let cpu_value = snapshot.system.cpu_usage;
    let cpu_display = if cpu_value > 80.0 {
        format!("{cpu_value:.1}%").red().to_string()
    } else if cpu_value > 50.0 {
        format!("{cpu_value:.1}%").yellow().to_string()
    } else {
        format!("{cpu_value:.1}%").green().to_string()
    };
    writeln!(stdout, "  CPU: {cpu_display}")?;

    // Memory — color by usage
    let mem_value = snapshot.system.memory_percent;
    let mem_str = {
        let (used_val, used_unit) = data::format_bytes_parts(snapshot.system.memory_used);
        let (total_val, total_unit) = data::format_bytes_parts(snapshot.system.memory_total);
        format!(
            "{:.1}% ({used_val} {used_unit} / {total_val} {total_unit})",
            mem_value,
        )
    };
    let mem_display = if mem_value > 80.0 {
        mem_str.red().to_string()
    } else if mem_value > 60.0 {
        mem_str.yellow().to_string()
    } else {
        mem_str.green().to_string()
    };
    writeln!(stdout, "  Memory: {mem_display}")?;
    writeln!(stdout)?;

    // Active filters
    if let Some(s) = search {
        writeln!(stdout, "{} {}", "Search:".bold(), s.cyan())?;
    }
    if let Some(f) = filter {
        writeln!(stdout, "{} {}", "Filter:".bold(), f.cyan())?;
    }
    writeln!(
        stdout,
        "{} {} | {} {} {}",
        "Sort:".bold(),
        sort_by.yellow(),
        "Showing:".bold(),
        snapshot.processes.len().min(limit).to_string().green(),
        "processes".dimmed()
    )?;
    writeln!(stdout)?;

    // Column headers
    if verbose {
        writeln!(
            stdout,
            "{:<8} {:<20} {:>8} {:>8} {:>7} {:>8} {:>8} {:>7}",
            "PID".bold(),
            "Name".bold(),
            "CPU%".bold(),
            "Mem%".bold(),
            "Threads".bold(),
            "Read".bold(),
            "Write".bold(),
            "Files".bold()
        )?;
        writeln!(stdout, "{}", "─".repeat(93).dimmed())?;
    } else {
        writeln!(
            stdout,
            "{:<8} {:<20} {:>8} {:>8} {:<10}",
            "PID".bold(),
            "Name".bold(),
            "CPU%".bold(),
            "Mem%".bold(),
            "User".bold()
        )?;
        writeln!(stdout, "{}", "─".repeat(70).dimmed())?;
    }

    // Process rows
    for process in &snapshot.processes {
        let cpu_str = format!("{:>7.1}%", process.cpu_percent);
        let cpu_display = if process.cpu_percent > 50.0 {
            cpu_str.red().to_string()
        } else if process.cpu_percent > 20.0 {
            cpu_str.yellow().to_string()
        } else {
            cpu_str.to_string()
        };

        let mem_str = format!("{:>7.1}%", process.memory_percent);
        let mem_display = if process.memory_percent > 5.0 {
            mem_str.red().to_string()
        } else if process.memory_percent > 2.0 {
            mem_str.yellow().to_string()
        } else {
            mem_str.to_string()
        };

        if verbose {
            let (read_val, read_unit) = data::format_bytes_parts(process.disk_read_bytes);
            let (write_val, write_unit) = data::format_bytes_parts(process.disk_write_bytes);
            let open_files_str = process
                .open_files
                .map(|f| f.to_string())
                .unwrap_or_else(|| "-".to_string());

            let read_formatted = format!("{:>6} {}", read_val, read_unit.dimmed());
            let write_formatted = format!("{:>6} {}", write_val, write_unit.dimmed());

            writeln!(
                stdout,
                "{:<8} {:<20} {} {} {:>7} {} {} {:>7}",
                process.pid.to_string().cyan(),
                truncate_str(&process.name, 20),
                cpu_display,
                mem_display,
                process.thread_count,
                read_formatted,
                write_formatted,
                open_files_str
            )?;
        } else {
            let user_str = truncate_str(&process.user, 10);
            let user_display = user_str.dimmed();
            writeln!(
                stdout,
                "{:<8} {:<20} {} {} {:<10}",
                process.pid.to_string().cyan(),
                truncate_str(&process.name, 20),
                cpu_display,
                mem_display,
                user_display
            )?;
        }
    }

    stdout.flush()
}
