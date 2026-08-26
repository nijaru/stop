//! CLI argument definitions.

use clap::{Parser, Subcommand};
use sysinfo::Pid;

use crate::model::SortKey;

#[derive(Parser)]
#[command(
    name = "stop",
    about = "Structured process monitoring for AI agents and automation",
    long_about = "JSON-first process monitoring. `stop list` queries the live process table, \
                  `stop inspect` resolves a single process, `stop top` ranks it."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// List running processes with P0 facts (identity, lineage, resources).
    List(ListArgs),
    /// Resolve exactly one process by PID or name.
    ///
    /// Numeric targets are treated as PIDs; otherwise the target matches
    /// process names (exact case-insensitive first, then substring). Exit
    /// codes: 0 found, 2 not found, 3 ambiguous (candidates are reported).
    Inspect(InspectArgs),
    /// Show system-wide metrics plus top processes ranked by CPU or memory.
    Top(TopArgs),
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
    pub target: String,

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
