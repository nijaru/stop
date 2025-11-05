use crate::{
    Args, DEFAULT_TOP_N, collect_snapshot, filter::FilterExpr, output_csv_header, output_csv_rows,
    output_human_readable, sort_processes,
};
use crossterm::{ExecutableCommand, cursor, terminal};
use std::error::Error;
use std::io::{Write, stdout};
use std::time::Duration;

/// Runs continuous monitoring mode, refreshing data at the specified interval.
///
/// Outputs in NDJSON format for JSON mode, or clears screen for human-readable.
/// Gracefully exits on broken pipe (e.g., when piping to `head`).
///
/// # Errors
///
/// Returns error if data collection or output fails.
pub fn watch_mode(args: &Args) -> Result<(), Box<dyn Error>> {
    // Parse filter once before loop
    let filter = if let Some(filter_expr_str) = &args.filter {
        match FilterExpr::parse(filter_expr_str) {
            Ok(f) => Some(f),
            Err(e) => {
                if args.json {
                    let error_json = serde_json::json!({
                        "error": "FilterError",
                        "message": e.to_string(),
                        "expression": filter_expr_str,
                    });
                    println!("{}", serde_json::to_string(&error_json)?);
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

    let mut first_iteration = true;

    loop {
        let mut snapshot = collect_snapshot()?;

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

        // Output based on mode
        if args.json {
            // NDJSON: one JSON object per line
            println!("{}", serde_json::to_string(&snapshot)?);
            if let Err(e) = stdout().flush() {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    return Ok(()); // Graceful exit when output is closed
                }
                return Err(e.into());
            }
        } else if args.csv {
            // CSV: header once, then rows
            if first_iteration {
                if let Err(e) = output_csv_header() {
                    if e.kind() == std::io::ErrorKind::BrokenPipe {
                        return Ok(()); // Graceful exit when output is closed
                    }
                    return Err(e.into());
                }
                first_iteration = false;
            }
            if let Err(e) = output_csv_rows(&snapshot) {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    return Ok(()); // Graceful exit when output is closed
                }
                return Err(e.into());
            }
        } else {
            // Human-readable: clear screen and redraw
            stdout()
                .execute(terminal::Clear(terminal::ClearType::All))?
                .execute(cursor::MoveTo(0, 0))?;
            if let Err(e) = output_human_readable(&snapshot, args.filter.as_ref(), sort_by, limit) {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    return Ok(()); // Graceful exit when output is closed
                }
                return Err(e.into());
            }
        }

        // Sleep before next iteration
        std::thread::sleep(Duration::from_secs_f64(args.interval));
    }
}
