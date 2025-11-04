use clap::Parser;
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
struct ProcessInfo {
    pid: u32,
    name: String,
    cpu_percent: f32,
    memory_bytes: u64,
    memory_percent: f32,
    user: String,
    command: String,
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

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let snapshot = collect_snapshot()?;

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
        println!(
            "{:<8} {:<20} {:>8} {:>8} {:<10}",
            "PID", "NAME", "CPU%", "MEM%", "USER"
        );
        println!("{}", "-".repeat(70));

        let mut processes = snapshot.processes;
        processes.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap());

        let limit = args.top_n.unwrap_or(20);
        for process in processes.iter().take(limit) {
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
