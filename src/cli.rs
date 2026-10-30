//! Command-line requests. Collection policy is independent of output format.

use std::time::Duration;

use clap::builder::TypedValueParser;
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::model::SortKey;

pub const DEFAULT_CPU_WINDOW: Duration = Duration::from_millis(250);

#[derive(Parser, Debug)]
#[command(
    name = "stop",
    bin_name = "stop",
    version,
    about = "Show process CPU, memory, and details",
    after_help = "With no command, show system usage and the top 10 processes by CPU.\n\
    Process CPU uses one core as 100%; system CPU uses all cores as 100%."
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
    /// List processes (default order: PID).
    List(QueryArgs),
    /// Show details for a process or port.
    Inspect(InspectArgs),
    /// Show system usage and top processes (default: top 10 by CPU).
    Top(QueryArgs),
    /// Show a process tree.
    Tree(TreeArgs),
    /// Sample process usage (default: 5 reports, 1 second apart).
    Sample(SampleArgs),
}

#[derive(Args, Debug, Default)]
pub struct OutputArgs {
    /// Output JSON.
    #[arg(long, global = true, display_order = 90, conflicts_with = "jsonl")]
    pub json: bool,
    /// Pretty-print JSON (implies --json).
    #[arg(long, global = true, display_order = 91, conflicts_with = "jsonl")]
    pub pretty: bool,
    /// Output one JSON object per sample.
    #[arg(long, global = true, display_order = 92, conflicts_with_all = ["json", "pretty"])]
    pub jsonl: bool,
}

impl OutputArgs {
    pub fn document(&self) -> bool {
        self.json || self.pretty
    }
}

#[derive(Args, Debug, Default)]
pub struct CollectionArgs {
    /// Measure CPU (default: 250ms; use --cpu=500ms to change).
    #[arg(long, global = true, display_order = 80, num_args = 0..=1, require_equals = true,
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
    /// Filter by process ID.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pub pid: Option<u32>,
    /// Filter by part of a process name (case-insensitive).
    #[arg(short, long)]
    pub name: Option<String>,
    /// Filter by username or UID.
    #[arg(short, long)]
    pub user: Option<String>,
    /// Sort by CPU, memory (RSS), PID, or name.
    #[arg(short, long, value_enum)]
    pub sort: Option<SortKey>,
    /// Show at most N processes.
    #[arg(short, long, value_name = "N", value_parser = parse_limit)]
    pub limit: Option<usize>,
}

#[derive(Args, Debug)]
#[group(required = true, multiple = false)]
pub struct InspectArgs {
    /// Process ID or exact name (case-insensitive).
    pub target: Option<String>,
    /// Select by exact name, even if the name is a number.
    #[arg(long)]
    pub name: Option<String>,
    /// Find processes listening on a TCP port or bound to a UDP port.
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..=65535))]
    pub port: Option<u16>,
}

#[derive(Args, Debug)]
#[group(multiple = false)]
pub struct TreeArgs {
    /// Root process ID or exact name (default: all processes).
    pub target: Option<String>,
    /// Select the root by exact name, even if the name is a number.
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Args, Debug)]
pub struct SampleArgs {
    #[command(flatten)]
    pub query: QueryArgs,
    /// Number of reports (1-1000).
    #[arg(long, default_value_t = 5, value_parser = parse_sample_count)]
    pub count: usize,
    /// Time between samples (200ms-24h; default: 1s).
    #[arg(long, value_name = "DURATION", value_parser = parse_cpu_window, conflicts_with = "rate")]
    pub interval: Option<Duration>,
    /// Samples per second, instead of --interval (maximum: 5).
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
    pub fn parse_args(raw: Vec<std::ffi::OsString>) -> Result<Self, clap::Error> {
        let command = Self::command();
        let global = |id: &str| {
            command
                .get_arguments()
                .find(|arg| arg.get_id() == id)
                .expect("global option is defined on Cli")
                .clone()
        };
        let jsonl = global("jsonl");
        let cpu = global("cpu");
        // Local copies override help visibility while globals remain accepted
        // before or after a subcommand.
        let command = command
            .mut_arg("jsonl", |arg| arg.hide(true))
            .mut_subcommand("sample", |sub| sub.arg(jsonl).arg(cpu.hide(true)))
            .mut_subcommand("top", |sub| {
                sub.mut_arg("sort", |arg| {
                    arg.help("Sort by CPU or memory (RSS)").value_parser(
                        clap::builder::PossibleValuesParser::new(["cpu", "mem"]).map(|value| {
                            if value == "cpu" {
                                SortKey::Cpu
                            } else {
                                SortKey::Mem
                            }
                        }),
                    )
                })
            });
        let matches = command.try_get_matches_from(raw)?;
        Self::from_arg_matches(&matches)
    }

    pub fn validate(&self) -> Result<(), String> {
        // clap checks conflicts within one command's arguments, but global
        // options can arrive on opposite sides of a subcommand.
        if self.output.jsonl && self.output.document() {
            return Err("--jsonl cannot be combined with --json or --pretty".into());
        }
        if self.output.jsonl && !matches!(self.command, Some(Command::Sample(_))) {
            return Err("--jsonl is only supported by sample".into());
        }
        if matches!(self.command, Some(Command::Sample(_))) && self.collection.cpu.is_some() {
            return Err("sample measures CPU automatically; remove --cpu".into());
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
    value
        .parse::<usize>()
        .ok()
        .filter(|n| (1..=1000).contains(n))
        .ok_or_else(|| "count must be between 1 and 1000".into())
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
        return Err("duration must be between 200ms and 24h".into());
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
        return Err("rate must be between one sample per 24h and 5 samples per second".into());
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
