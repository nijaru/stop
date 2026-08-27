//! CLI argument definitions.

use std::time::Duration;

use clap::{Parser, Subcommand};
use sysinfo::Pid;

use crate::model::SortKey;

#[derive(Parser)]
#[command(
    name = "stop",
    about = "Structured process monitoring for AI agents and automation",
    long_about = "JSON-first process monitoring. `stop list` queries the live process table, \
                  `stop inspect` resolves a process or port owner, `stop top` ranks it, \
                  `stop tree` renders the parent/child hierarchy, and `stop sample` \
                  collects a bounded time series."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// List running processes with P0 facts (identity, lineage, resources).
    List(ListArgs),
    /// Resolve one process by PID/name, or inspect visible owners of a port.
    ///
    /// Numeric targets are treated as PIDs; otherwise the target matches
    /// process names (exact case-insensitive first, then substring). The
    /// `--port` form reports all visible TCP listeners and UDP bindings.
    /// Exit codes: 0 found, 2 not found, 3 ambiguous (candidates reported).
    Inspect(InspectArgs),
    /// Show system-wide metrics plus top processes ranked by CPU or memory.
    Top(TopArgs),
    /// Render the parent/child hierarchy as a tree.
    ///
    /// Without a target, renders the full process forest. With a target
    /// (PID or name, same selection rules as `inspect`), renders the
    /// subtree rooted at that process. Exit codes: 0 found, 2 not found,
    /// 3 ambiguous.
    Tree(TreeArgs),
    /// Collect a bounded time series of process and system samples.
    ///
    /// Samples are scheduled start-to-start. If collection takes longer than
    /// the requested interval, the next sample starts immediately; samples
    /// are never overlapped or backfilled. Use `--rate` instead of
    /// `--interval` to specify samples per second.
    Sample(SampleArgs),
}

/// Shared output-format flags across all commands.
#[derive(Parser, Debug)]
pub struct OutputArgs {
    /// Emit JSON instead of human-readable output.
    #[arg(long)]
    pub json: bool,

    /// Pretty-print JSON.
    #[arg(long)]
    pub pretty: bool,
}

/// Shared collection-mode flags across table-producing commands.
#[derive(Parser, Debug)]
pub struct CollectionArgs {
    /// Skip the 200 ms CPU warm-up: ~8x faster collection, but
    /// `cpu_percent` is null and `--sort cpu` ordering is meaningless.
    #[arg(long)]
    pub fast: bool,
}

#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Case-insensitive substring match on process name.
    #[arg(short, long)]
    pub name: Option<String>,

    /// Filter by owner username or raw UID.
    #[arg(short, long)]
    pub user: Option<String>,

    #[arg(short, long, value_enum, default_value_t = SortKey::Cpu)]
    pub sort: SortKey,

    /// Maximum number of processes to return after sorting.
    #[arg(short, long)]
    pub limit: Option<usize>,

    #[command(flatten)]
    pub collection: CollectionArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Parser, Debug)]
pub struct InspectArgs {
    /// Numeric targets resolve by PID; otherwise by name.
    #[arg(value_name = "PID_OR_NAME", required_unless_present = "port")]
    pub target: Option<String>,

    /// Find all visible TCP listeners and UDP bindings on this port.
    #[arg(
        long,
        value_name = "PORT",
        value_parser = clap::value_parser!(u16).range(1..=65535),
        conflicts_with = "target",
        required_unless_present = "target"
    )]
    pub port: Option<u16>,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Parser, Debug)]
pub struct TopArgs {
    #[arg(short, long, value_enum, default_value_t = SortKey::Cpu)]
    pub sort: SortKey,

    /// Number of processes to show.
    #[arg(short, long, default_value_t = 10)]
    pub limit: usize,

    #[command(flatten)]
    pub collection: CollectionArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Parser, Debug)]
pub struct TreeArgs {
    /// Optional root: PID or process name (same rules as inspect).
    /// Without a target, the full process forest is rendered.
    pub target: Option<String>,

    #[command(flatten)]
    pub collection: CollectionArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Parser, Debug)]
pub struct SampleArgs {
    /// Number of samples to collect (default: 1).
    #[arg(long, default_value_t = 1, value_parser = parse_sample_count)]
    pub count: usize,

    /// Target start-to-start period: e.g. 250ms, 1s, 1m, or 1h (default: 1s).
    #[arg(long, value_name = "DURATION", value_parser = parse_sample_interval, conflicts_with = "rate")]
    pub interval: Option<Duration>,

    /// Target samples per second; alternative to `--interval` (1..=1000).
    #[arg(long, value_name = "HZ", value_parser = parse_sample_rate, conflicts_with = "interval")]
    pub rate: Option<f64>,

    #[command(flatten)]
    pub collection: CollectionArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

impl SampleArgs {
    pub fn period(&self) -> Duration {
        self.interval
            .or_else(|| self.rate.map(|rate| Duration::from_secs_f64(1.0 / rate)))
            .unwrap_or_else(|| Duration::from_secs(1))
    }
}

fn parse_sample_count(value: &str) -> Result<usize, String> {
    let count = value
        .parse::<usize>()
        .map_err(|_| "count must be a positive integer".to_string())?;
    if !(1..=1000).contains(&count) {
        return Err("count must be between 1 and 1000".into());
    }
    Ok(count)
}

fn parse_sample_interval(value: &str) -> Result<Duration, String> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ms") {
        (number, 0.001)
    } else if let Some(number) = value.strip_suffix('s') {
        (number, 1.0)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60.0)
    } else if let Some(number) = value.strip_suffix('h') {
        (number, 3_600.0)
    } else {
        return Err("interval must use ms, s, m, or h (for example, 250ms or 1s)".into());
    };

    let seconds = number
        .parse::<f64>()
        .map_err(|_| "interval must be a positive duration".to_string())?
        * multiplier;
    if !seconds.is_finite() || !(0.001..=86_400.0).contains(&seconds) {
        return Err("interval must be between 1ms and 24h".into());
    }
    Ok(Duration::from_secs_f64(seconds))
}

fn parse_sample_rate(value: &str) -> Result<f64, String> {
    let rate = value
        .parse::<f64>()
        .map_err(|_| "rate must be a positive number".to_string())?;
    if !rate.is_finite() || !(1.0..=1000.0).contains(&rate) {
        return Err("rate must be between 1 and 1000 samples per second".into());
    }
    Ok(rate)
}

/// Parses an inspect target that is syntactically a PID.
///
/// Only canonical decimal PIDs parse — no leading zeros (ambiguous with
/// names), no signs or whitespace; everything else is treated as a name.
pub fn parse_pid(target: &str) -> Option<Pid> {
    if !target.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if target.len() > 1 && target.starts_with('0') {
        return None;
    }
    target.parse::<u32>().ok().map(Pid::from_u32)
}
