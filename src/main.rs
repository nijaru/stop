//! CLI entry point for stop — structured process monitoring.

use clap::Parser;
use std::io::{self, Write};
use stop::{
    collect_snapshot, ignore_broken_pipe, watch, FilterExpr, Output, Pipeline, StopError,
};

/// Command-line arguments for the stop tool.
#[derive(Parser, Debug)]
#[command(name = "stop")]
#[command(about = "Modern process monitoring with structured output")]
#[command(long_about = "Modern process monitoring with structured output

EXAMPLES:
    stop                              # Human-readable table
    stop --json                       # JSON output
    stop -s chrome                    # Search for chrome processes
    stop --filter \"cpu > 10\"          # Filter processes
    stop -s postgres --filter \"mem > 5\" # Combine search and filter
    stop --watch                      # Live monitoring")]
#[command(version)]
struct Args {
    #[arg(long, help = "Output as JSON")]
    json: bool,

    #[arg(long, help = "Output as CSV")]
    csv: bool,

    #[arg(
        short,
        long,
        value_name = "TEXT",
        help = "Search processes by name or command"
    )]
    search: Option<String>,

    #[arg(
        long,
        value_name = "EXPR",
        help = "Filter processes (e.g., 'cpu > 10')",
        long_help = "Filter processes by expression

Fields:    cpu, mem, pid, name, user
Operators: >, >=, <, <=, ==, !=
Note:      name == uses case-insensitive substring match
           user == uses case-sensitive exact match
Logic:     and, or

Examples:
  cpu > 50
  cpu > 10 and mem > 5
  name == chrome or name == firefox"
    )]
    filter: Option<String>,

    #[arg(long, value_name = "FIELD", help = "Sort by: cpu, mem, pid, name")]
    sort_by: Option<String>,

    #[arg(long, value_name = "N", help = "Show top N processes")]
    top_n: Option<usize>,

    #[arg(long, help = "Continuous monitoring (watch mode)")]
    watch: bool,

    #[arg(
        long,
        value_name = "SECS",
        help = "Update interval",
        default_value_t = 2.0
    )]
    interval: f64,

    #[arg(short, long, help = "Show threads, disk I/O, and open files")]
    verbose: bool,
}

// ---------------------------------------------------------------------------
// Binary-layer helpers (not in the library)
// ---------------------------------------------------------------------------

/// Parses a filter expression, printing a user-friendly error and exiting on failure.
///
/// In JSON mode, errors are emitted as a structured JSON object for AI agent consumption.
fn parse_filter_or_exit(expr: &str, json_mode: bool) -> Option<FilterExpr> {
    match FilterExpr::parse(expr) {
        Ok(f) => Some(f),
        Err(e) => {
            if json_mode {
                let error_json = serde_json::json!({
                    "error": "FilterError",
                    "message": e.to_string(),
                    "expression": expr,
                });
                let _ = writeln!(
                    io::stdout(),
                    "{}",
                    serde_json::to_string_pretty(&error_json).unwrap_or_default()
                );
            } else {
                eprintln!("Error: {e}");
                eprintln!("Expression: {expr}");
            }
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<(), StopError> {
    let args = Args::parse();

    // Validate interval
    if !args.interval.is_finite() || args.interval < 0.0 {
        return Err(StopError::config("Interval must be a finite positive number"));
    }
    if args.interval < 0.2 {
        eprintln!("Warning: Interval below 0.2s may cause high CPU usage");
    }

    // Build the pipeline
    let filter = args
        .filter
        .as_deref()
        .and_then(|f| parse_filter_or_exit(f, args.json));
    let pipeline = Pipeline::new(args.search.clone(), filter, args.sort_by.clone(), args.top_n);

    // Build the output format
    let mut output = if args.json {
        Output::json()
    } else if args.csv {
        Output::csv()
    } else {
        Output::human(
            args.search.clone(),
            args.filter.clone(),
            args.sort_by.clone(),
            args.top_n,
            args.verbose,
        )
    };

    // Dispatch
    if args.watch {
        return watch::watch_mode(&pipeline, &mut output, args.interval);
    }

    let mut snapshot = collect_snapshot()?;
    let had_filter = pipeline.has_active_filter();
    pipeline.apply(&mut snapshot.processes);

    // Exit 2 when a filter/search was active but nothing matched.
    // Still write output (valid JSON/CSV/table with empty process list)
    // so consumers can inspect both exit code and data.
    if had_filter && snapshot.processes.is_empty() {
        let _ = output.write(&snapshot);
        std::process::exit(2);
    }

    ignore_broken_pipe(output.write(&snapshot))?;
    Ok(())
}
