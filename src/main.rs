mod filter;
mod watch;

use clap::Parser;
use filter::FilterExpr;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::error::Error;
use sysinfo::System;

/// Minimum interval for CPU usage calculation (milliseconds).
/// Required by sysinfo to get accurate CPU percentage.
const CPU_SAMPLE_INTERVAL_MS: u64 = 200;

/// Default number of processes to show when --top-n is not specified.
const DEFAULT_TOP_N: usize = 20;

/// Command-line arguments for the stop tool.
#[derive(Parser, Debug)]
#[command(name = "stop")]
#[command(about = "Structured process and system monitoring with JSON output")]
#[command(version)]
pub struct Args {
    #[arg(long, help = "Output as JSON")]
    pub json: bool,

    #[arg(long, help = "Output as CSV")]
    pub csv: bool,

    #[arg(long, help = "Filter processes (e.g., 'cpu > 10')")]
    pub filter: Option<String>,

    #[arg(long, help = "Sort by metric (cpu, mem, pid, name)")]
    pub sort_by: Option<String>,

    #[arg(long, help = "Show top N processes")]
    pub top_n: Option<usize>,

    #[arg(long, help = "Watch mode (continuous updates)")]
    pub watch: bool,

    #[arg(long, help = "Update interval in seconds for watch mode", default_value_t = 2.0)]
    pub interval: f64,
}

/// A snapshot of system and process metrics at a point in time.
#[derive(Serialize, Deserialize, Debug)]
pub struct SystemSnapshot {
    /// ISO 8601 timestamp (RFC3339)
    pub timestamp: String,
    /// System-wide metrics
    pub system: SystemMetrics,
    /// List of process information
    pub processes: Vec<ProcessInfo>,
}

/// System-wide metrics (CPU, memory).
#[derive(Serialize, Deserialize, Debug)]
pub struct SystemMetrics {
    /// Global CPU usage percentage (0-100)
    pub cpu_usage: f32,
    /// Total system memory in bytes
    pub memory_total: u64,
    /// Used system memory in bytes
    pub memory_used: u64,
    /// Memory usage percentage (0-100)
    pub memory_percent: f32,
}

/// Information about a single process.
#[derive(Serialize, Deserialize, Debug)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// CPU usage percentage (0-100+)
    pub cpu_percent: f32,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// Memory usage percentage (0-100)
    pub memory_percent: f32,
    /// User ID (may be numeric string like "501")
    pub user: String,
    /// Full command line
    pub command: String,
    /// Number of threads
    pub thread_count: usize,
    /// Total bytes read from disk
    pub disk_read_bytes: u64,
    /// Total bytes written to disk
    pub disk_write_bytes: u64,
    /// Number of open file descriptors (None if unavailable)
    pub open_files: Option<usize>,
}

/// Collects a snapshot of system and process metrics.
///
/// Sleeps for 200ms to allow accurate CPU usage calculation as required by sysinfo.
///
/// # Errors
///
/// Returns error if system information collection fails.
pub fn collect_snapshot() -> Result<SystemSnapshot, Box<dyn Error>> {
    let mut sys = System::new_all();

    std::thread::sleep(std::time::Duration::from_millis(CPU_SAMPLE_INTERVAL_MS));
    sys.refresh_all();

    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let memory_percent = (used_memory as f64 / total_memory as f64 * 100.0) as f32;

    let global_cpu_usage = sys.global_cpu_usage();

    let processes: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .map(|(pid, process)| {
            let cmd_vec: Vec<String> = process
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect();

            let disk_usage = process.disk_usage();
            let (disk_read, disk_write) = (disk_usage.total_read_bytes, disk_usage.total_written_bytes);

            ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().into_owned(),
                cpu_percent: process.cpu_usage(),
                memory_bytes: process.memory(),
                memory_percent: (process.memory() as f64 / total_memory as f64 * 100.0) as f32,
                user: process
                    .user_id()
                    .map(|uid| uid.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                command: cmd_vec.join(" "),
                thread_count: process.tasks().map(|t| t.len()).unwrap_or(1),
                disk_read_bytes: disk_read,
                disk_write_bytes: disk_write,
                open_files: process.open_files(),
            }
        })
        .collect();

    Ok(SystemSnapshot {
        timestamp: chrono::Utc::now().to_rfc3339(),
        system: SystemMetrics {
            cpu_usage: global_cpu_usage,
            memory_total: total_memory,
            memory_used: used_memory,
            memory_percent,
        },
        processes,
    })
}

/// Escapes a field for CSV output according to RFC 4180.
///
/// Wraps field in quotes and escapes internal quotes if the field contains
/// commas, quotes, or newlines.
pub fn escape_csv_field(field: &str) -> String {
    // RFC 4180: If field contains comma, quote, or newline, wrap in quotes and escape quotes
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Outputs the CSV header row with all column names.
pub fn output_csv_header() {
    println!("timestamp,cpu_usage,memory_total,memory_used,memory_percent,pid,name,cpu_percent,memory_bytes,memory_percent_process,user,command,thread_count,disk_read_bytes,disk_write_bytes,open_files");
}

/// Outputs CSV rows for all processes in the snapshot.
pub fn output_csv_rows(snapshot: &SystemSnapshot) {
    for process in &snapshot.processes {
        let open_files_str = process.open_files.map(|n| n.to_string()).unwrap_or_default();
        println!(
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
        );
    }
}

fn output_csv(snapshot: &SystemSnapshot) {
    output_csv_header();
    output_csv_rows(snapshot);
}

/// Sorts processes in-place by the specified metric.
///
/// # Arguments
///
/// * `processes` - Mutable slice of processes to sort
/// * `sort_by` - Sort key: "cpu", "mem"/"memory", "pid", or "name" (case-insensitive)
///
/// Defaults to CPU descending if an unknown sort key is provided.
pub fn sort_processes(processes: &mut [ProcessInfo], sort_by: &str) {
    match sort_by.to_lowercase().as_str() {
        "cpu" => processes.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "mem" | "memory" => processes.sort_by(|a, b| {
            b.memory_percent
                .partial_cmp(&a.memory_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "pid" => processes.sort_by_key(|p| p.pid),
        "name" => processes.sort_by_cached_key(|p| p.name.to_lowercase()),
        _ => {
            eprintln!(
                "Warning: Unknown sort field '{sort_by}', using 'cpu'. Valid: cpu, mem, pid, name"
            );
            processes.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }
}

/// Outputs snapshot in human-readable format with colors and formatting.
///
/// Displays system metrics, filter info, and a table of processes with
/// color-coded CPU and memory usage.
pub fn output_human_readable(
    snapshot: &SystemSnapshot,
    filter_expr: Option<&String>,
    sort_by: &str,
    limit: usize,
) {
    println!(
        "{} {}",
        "stop".bold().cyan(),
        format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
    );
    println!();
    println!("{}", "System:".bold());

    // Color code CPU based on usage
    let cpu_value = snapshot.system.cpu_usage;
    let cpu_display = if cpu_value > 80.0 {
        format!("{cpu_value:.1}%").red().to_string()
    } else if cpu_value > 50.0 {
        format!("{cpu_value:.1}%").yellow().to_string()
    } else {
        format!("{cpu_value:.1}%").green().to_string()
    };
    println!("  CPU: {cpu_display}");

    // Color code memory based on usage
    let mem_value = snapshot.system.memory_percent;
    let mem_str = format!(
        "{:.1}% ({} / {} MB)",
        mem_value,
        snapshot.system.memory_used / 1024 / 1024,
        snapshot.system.memory_total / 1024 / 1024
    );
    let mem_display = if mem_value > 80.0 {
        mem_str.red().to_string()
    } else if mem_value > 60.0 {
        mem_str.yellow().to_string()
    } else {
        mem_str.green().to_string()
    };
    println!("  Memory: {mem_display}");
    println!();

    if let Some(filter) = filter_expr {
        println!("{} {}", "Filter:".bold(), filter.cyan());
    }
    println!(
        "{} {} | {} {} {}",
        "Sort:".bold(),
        sort_by.yellow(),
        "Showing:".bold(),
        snapshot.processes.len().min(limit).to_string().green(),
        "processes".dimmed()
    );
    println!();

    println!(
        "{:<8} {:<20} {:>8} {:>8} {:<10}",
        "PID".bold(),
        "NAME".bold(),
        "CPU%".bold(),
        "MEM%".bold(),
        "USER".bold()
    );
    println!("{}", "─".repeat(70).dimmed());

    for process in &snapshot.processes {
        // Color code CPU usage
        let cpu_str = format!("{:>7.1}%", process.cpu_percent);
        let cpu_display = if process.cpu_percent > 50.0 {
            cpu_str.red().to_string()
        } else if process.cpu_percent > 20.0 {
            cpu_str.yellow().to_string()
        } else {
            cpu_str.to_string()
        };

        // Color code memory usage
        let mem_str = format!("{:>7.1}%", process.memory_percent);
        let mem_display = if process.memory_percent > 5.0 {
            mem_str.red().to_string()
        } else if process.memory_percent > 2.0 {
            mem_str.yellow().to_string()
        } else {
            mem_str.to_string()
        };

        let user_str = &process.user[..process.user.len().min(10)];
        let user_display = user_str.dimmed();
        println!(
            "{:<8} {:<20} {} {} {:<10}",
            process.pid.to_string().cyan(),
            &process.name[..process.name.len().min(20)],
            cpu_display,
            mem_display,
            user_display
        );
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Validate interval
    if args.interval < 0.2 {
        eprintln!("Warning: Interval below 0.2s may cause high CPU usage");
    }

    // Watch mode
    if args.watch {
        return watch::watch_mode(&args);
    }

    // Single snapshot mode
    let mut snapshot = collect_snapshot()?;

    // Parse filter if provided
    let filter = if let Some(filter_expr_str) = &args.filter {
        match FilterExpr::parse(filter_expr_str) {
            Ok(f) => Some(f),
            Err(e) => {
                if args.json {
                    // Output error as JSON for AI agents
                    let error_json = serde_json::json!({
                        "error": "FilterError",
                        "message": e.to_string(),
                        "expression": filter_expr_str,
                    });
                    println!("{}", serde_json::to_string_pretty(&error_json)?);
                } else {
                    eprintln!("Error: {e}");
                    eprintln!("Expression: {filter_expr_str}");
                }
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    // Apply filter
    if let Some(ref f) = filter {
        snapshot.processes.retain(|p| f.matches(p));
    }

    // Apply sorting
    let sort_by = args.sort_by.as_deref().unwrap_or("cpu");
    sort_processes(&mut snapshot.processes, sort_by);

    // Apply top-n limit
    let limit = args.top_n.unwrap_or(DEFAULT_TOP_N);
    snapshot.processes.truncate(limit);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
    } else if args.csv {
        output_csv(&snapshot);
    } else {
        output_human_readable(&snapshot, args.filter.as_ref(), sort_by, limit);
    }

    Ok(())
}
