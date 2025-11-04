mod filter;
mod watch;

use clap::Parser;
use filter::FilterExpr;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::error::Error;
use sysinfo::System;

#[derive(Parser, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemSnapshot {
    pub timestamp: String,
    pub system: SystemMetrics,
    pub processes: Vec<ProcessInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub memory_percent: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub memory_percent: f32,
    pub user: String,
    pub command: String,
}

pub fn collect_snapshot() -> Result<SystemSnapshot, Box<dyn Error>> {
    let mut sys = System::new_all();

    std::thread::sleep(std::time::Duration::from_millis(200));
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
                .map(|s| s.to_string_lossy().to_string())
                .collect();

            ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().to_string(),
                cpu_percent: process.cpu_usage(),
                memory_bytes: process.memory(),
                memory_percent: (process.memory() as f64 / total_memory as f64 * 100.0) as f32,
                user: process
                    .user_id()
                    .map(|uid| uid.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                command: cmd_vec.join(" "),
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

pub fn escape_csv_field(field: &str) -> String {
    // RFC 4180: If field contains comma, quote, or newline, wrap in quotes and escape quotes
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

pub fn output_csv_header() {
    println!("timestamp,cpu_usage,memory_total,memory_used,memory_percent,pid,name,cpu_percent,memory_bytes,memory_percent_process,user,command");
}

pub fn output_csv_rows(snapshot: &SystemSnapshot) {
    for process in &snapshot.processes {
        println!(
            "{},{},{},{},{},{},{},{},{},{},{},{}",
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
            escape_csv_field(&process.command)
        );
    }
}

fn output_csv(snapshot: &SystemSnapshot) {
    output_csv_header();
    output_csv_rows(snapshot);
}

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
        "name" => processes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        _ => {
            eprintln!(
                "Warning: Unknown sort field '{}', using 'cpu'. Valid: cpu, mem, pid, name",
                sort_by
            );
            processes.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }
}

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
        format!("{:.1}%", cpu_value).red().to_string()
    } else if cpu_value > 50.0 {
        format!("{:.1}%", cpu_value).yellow().to_string()
    } else {
        format!("{:.1}%", cpu_value).green().to_string()
    };
    println!("  CPU: {}", cpu_display);

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
    println!("  Memory: {}", mem_display);
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
                    eprintln!("Error: {}", e);
                    eprintln!("Expression: {}", filter_expr_str);
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
    let limit = args.top_n.unwrap_or(20);
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
