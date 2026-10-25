//! Command-line requests. Collection policy is independent of output format.

use std::time::Duration;

use clap::{Args, Parser, Subcommand};

use crate::model::SortKey;

pub const DEFAULT_CPU_WINDOW: Duration = Duration::from_millis(250);

#[derive(Parser, Debug)]
#[command(
    name = "stop",
    version,
    about = "Inspect processes without leaving the shell",
    long_about = "One-shot process inspection. With no command, show a system summary and the \
    top 10 processes by CPU. List inventories immediately; inspect resolves an exact name or PID; \
    tree shows current parent relationships; sample collects a finite series.\n\n\
    Process CPU is percent of one core (may exceed 100%); system CPU is normalized across cores."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub collection: CollectionArgs,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Inventory visible processes immediately (default order: PID).
    List(QueryArgs),
    /// Inspect one PID/exact name, or visible owners of a port.
    Inspect(InspectArgs),
    /// System summary and ranked processes (default: top 10 by CPU).
    Top(QueryArgs),
    /// Show observed parent relationships, optionally rooted at a PID/exact name.
    Tree(TreeArgs),
    /// Print a finite series as points arrive (default: five top-10 CPU reports).
    Sample(SampleArgs),
}

#[derive(Args, Debug, Default)]
pub struct OutputArgs {
    /// Emit one JSON document instead of human output.
    #[arg(long, global = true, conflicts_with = "jsonl")]
    pub json: bool,
    /// Pretty-print JSON (implies --json).
    #[arg(long, global = true, conflicts_with = "jsonl")]
    pub pretty: bool,
    /// Stream one JSON object per sample (sample only).
    #[arg(long, global = true, conflicts_with_all = ["json", "pretty"])]
    pub jsonl: bool,
}

impl OutputArgs {
    pub fn document(&self) -> bool {
        self.json || self.pretty
    }
    pub fn machine(&self) -> bool {
        self.document() || self.jsonl
    }
}

#[derive(Args, Debug, Default)]
pub struct CollectionArgs {
    /// Measure CPU; optionally choose a window with --cpu=500ms (default: 250ms).
    #[arg(long, global = true, num_args = 0..=1, require_equals = true,
        default_missing_value = "250ms", value_name = "DURATION", value_parser = parse_cpu_window)]
    pub cpu: Option<Duration>,
}

impl CollectionArgs {
    pub fn window(&self, sort: SortKey) -> Option<Duration> {
        self.cpu
            .or_else(|| (sort == SortKey::Cpu).then_some(DEFAULT_CPU_WINDOW))
    }
}

#[derive(Args, Debug, Default)]
pub struct QueryArgs {
    /// Select a PID (sample pins its first observed identity).
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pub pid: Option<u32>,
    /// Case-insensitive substring search on process names.
    #[arg(short, long)]
    pub name: Option<String>,
    /// Filter by username or raw UID.
    #[arg(short, long)]
    pub user: Option<String>,
    /// Rank by CPU/RSS or order by PID/name (top accepts only cpu/mem).
    #[arg(short, long, value_enum)]
    pub sort: Option<SortKey>,
    /// Positive maximum row count after filtering and sorting.
    #[arg(short, long, value_parser = parse_limit)]
    pub limit: Option<usize>,
}

#[derive(Args, Debug)]
#[group(required = true, multiple = false)]
pub struct InspectArgs {
    /// Canonical decimal PID or exact process name (case-insensitive).
    pub target: Option<String>,
    /// Exact process name, including numeric names (not substring search).
    #[arg(long)]
    pub name: Option<String>,
    /// Visible TCP listeners and UDP bindings on a port.
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..=65535))]
    pub port: Option<u16>,
}

#[derive(Args, Debug)]
#[group(multiple = false)]
pub struct TreeArgs {
    /// Optional root: canonical decimal PID or exact process name.
    pub target: Option<String>,
    /// Exact process name, including numeric names.
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Args, Debug)]
pub struct SampleArgs {
    #[command(flatten)]
    pub query: QueryArgs,
    /// Number of observations (1..=1000).
    #[arg(long, default_value_t = 5, value_parser = parse_sample_count)]
    pub count: usize,
    /// Start-to-start period, e.g. 250ms, 1s, 1m (default: 1s; minimum: 200ms).
    #[arg(long, value_name = "DURATION", value_parser = parse_cpu_window, conflicts_with = "rate")]
    pub interval: Option<Duration>,
    /// Samples per second; alternative to --interval (maximum: 5 Hz).
    #[arg(long, value_name = "HZ", value_parser = parse_sample_rate, conflicts_with = "interval")]
    pub rate: Option<f64>,
}

impl SampleArgs {
    pub fn period(&self) -> Duration {
        self.interval
            .or_else(|| self.rate.map(|r| Duration::from_secs_f64(1.0 / r)))
            .unwrap_or(Duration::from_secs(1))
    }
}

impl Cli {
    pub fn validate(&self) -> Result<(), String> {
        if self.output.jsonl && !matches!(self.command, Some(Command::Sample(_))) {
            return Err("--jsonl is only supported by sample".into());
        }
        if matches!(self.command, Some(Command::Sample(_))) && self.collection.cpu.is_some() {
            return Err("sample measures CPU over --interval; do not combine it with --cpu".into());
        }
        if let Some(Command::Top(q)) = &self.command
            && q.sort
                .is_some_and(|s| !matches!(s, SortKey::Cpu | SortKey::Mem))
        {
            return Err("top --sort accepts cpu or mem; use list for pid/name ordering".into());
        }
        Ok(())
    }
}

fn parse_limit(value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| "limit must be a positive integer".into())
}

fn parse_sample_count(value: &str) -> Result<usize, String> {
    parse_limit(value).and_then(|n| {
        if n <= 1000 {
            Ok(n)
        } else {
            Err("count must be between 1 and 1000".into())
        }
    })
}

fn parse_cpu_window(value: &str) -> Result<Duration, String> {
    let (number, multiplier) = if let Some(v) = value.strip_suffix("ms") {
        (v, 0.001)
    } else if let Some(v) = value.strip_suffix('s') {
        (v, 1.0)
    } else if let Some(v) = value.strip_suffix('m') {
        (v, 60.0)
    } else if let Some(v) = value.strip_suffix('h') {
        (v, 3600.0)
    } else {
        return Err("duration must use ms, s, m, or h".into());
    };
    let seconds = number.parse::<f64>().map_err(|_| "invalid duration")? * multiplier;
    if !seconds.is_finite()
        || seconds < sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.as_secs_f64()
        || seconds > 86400.0
    {
        return Err("CPU windows and sampling intervals must be between 200ms and 24h".into());
    }
    Ok(Duration::from_secs_f64(seconds))
}

fn parse_sample_rate(value: &str) -> Result<f64, String> {
    let rate = value
        .parse::<f64>()
        .map_err(|_| "rate must be a positive number")?;
    if !rate.is_finite()
        || rate < 1.0 / 86400.0
        || rate > 1.0 / sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.as_secs_f64()
    {
        return Err("rate must describe a period between 200ms and 24h (maximum 5 Hz)".into());
    }
    Ok(rate)
}

pub fn parse_pid(target: &str) -> Option<u32> {
    if target.is_empty()
        || !target.bytes().all(|c| c.is_ascii_digit())
        || (target.len() > 1 && target.starts_with('0'))
    {
        return None;
    }
    target.parse().ok()
}
