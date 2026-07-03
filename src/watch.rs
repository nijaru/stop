//! Continuous monitoring (watch mode).

use crate::error::StopError;
use crate::{Output, Pipeline, collect_snapshot};
use crossterm::{ExecutableCommand, cursor, terminal};
use std::io::{self, stdout};
use std::time::Duration;

/// Runs continuous monitoring, refreshing at the given interval.
///
/// JSON mode emits NDJSON. Human mode clears the screen each iteration.
/// Exits gracefully on broken pipe.
pub fn watch_mode(
    pipeline: &Pipeline,
    output: &mut Output,
    interval: f64,
) -> Result<(), StopError> {
    loop {
        let mut snapshot = collect_snapshot()?;
        pipeline.apply(&mut snapshot.processes);

        // Human mode: clear screen before drawing
        if matches!(output, Output::Human { .. }) {
            stdout()
                .execute(terminal::Clear(terminal::ClearType::All))?
                .execute(cursor::MoveTo(0, 0))?;
        }

        // Write one snapshot. Broken pipe means the consumer closed — exit cleanly.
        if let Err(e) = output.write(&snapshot) {
            if e.kind() == io::ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(e.into());
        }

        std::thread::sleep(Duration::from_secs_f64(interval));
    }
}
