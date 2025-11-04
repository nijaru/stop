mod filter;

use clap::Parser;
use filter::Filter;
use serde::{Deserialize, Serialize};
use std::error::Error;
use sysinfo::System;

#[derive(Parser, Debug)]
#[command(name = "stop")]
#[command(about = "Structured process and system monitoring with JSON output")]
#[command(version)]
struct Args {
    #[arg(long, help = "Output as JSON")]
    json: bool,

    #[arg(long, help = "Filter processes (e.g., 'cpu > 10')")]
    filter: Option<String>,

    #[arg(long, help = "Sort by metric (cpu, mem, pid, name)")]
    sort_by: Option<String>,

    #[arg(long, help = "Show top N processes")]
    top_n: Option<usize>,

    #[arg(long, help = "Watch mode (continuous updates)")]
    watch: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct SystemSnapshot {
    timestamp: String,
    system: SystemMetrics,
    processes: Vec<ProcessInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
struct SystemMetrics {
    cpu_usage: f32,
    memory_total: u64,
    memory_used: u64,
    memory_percent: f32,
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

fn collect_snapshot() -> Result<SystemSnapshot, Box<dyn Error>> {
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

fn sort_processes(processes: &mut [ProcessInfo], sort_by: &str) {
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

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut snapshot = collect_snapshot()?;

    // Parse filter if provided
    let filter = if let Some(filter_expr) = &args.filter {
        match Filter::parse(filter_expr) {
            Ok(f) => Some(f),
            Err(e) => {
                if args.json {
                    // Output error as JSON for AI agents
                    let error_json = serde_json::json!({
                        "error": "FilterError",
                        "message": e.to_string(),
                        "expression": filter_expr,
                    });
                    println!("{}", serde_json::to_string_pretty(&error_json)?);
                } else {
                    eprintln!("Error: {}", e);
                    eprintln!("Expression: {}", filter_expr);
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
    } else {
        println!("stop v{}", env!("CARGO_PKG_VERSION"));
        println!();
        println!("System:");
        println!("  CPU: {:.1}%", snapshot.system.cpu_usage);
        println!(
            "  Memory: {:.1}% ({} / {} MB)",
            snapshot.system.memory_percent,
            snapshot.system.memory_used / 1024 / 1024,
            snapshot.system.memory_total / 1024 / 1024
        );
        println!();

        if let Some(filter_expr) = &args.filter {
            println!("Filter: {}", filter_expr);
        }
        println!(
            "Sort: {} | Showing: {} processes",
            sort_by,
            snapshot.processes.len().min(limit)
        );
        println!();

        println!(
            "{:<8} {:<20} {:>8} {:>8} {:<10}",
            "PID", "NAME", "CPU%", "MEM%", "USER"
        );
        println!("{}", "-".repeat(70));

        for process in &snapshot.processes {
            println!(
                "{:<8} {:<20} {:>7.1}% {:>7.1}% {:<10}",
                process.pid,
                &process.name[..process.name.len().min(20)],
                process.cpu_percent,
                process.memory_percent,
                &process.user[..process.user.len().min(10)]
            );
        }
    }

    Ok(())
}
