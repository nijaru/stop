//! `stop inspect` — resolve a process by PID/name or port ownership.

use crate::cli::InspectArgs;
use crate::cmd::{Outcome, resolve};
use crate::collector;
use crate::error::StopError;
use crate::output;
use crate::ports;

pub fn run(args: &InspectArgs) -> Result<Outcome, StopError> {
    let (_metrics, all) = collector::collect(true)?;

    if let Some(port) = args.port {
        let report = ports::inspect(port, &all)?;
        let no_match = report.owners.is_empty();
        if args.output.json {
            output::print_json(&report, args.output.pretty)?;
        } else {
            output::print_port_report(&report)?;
        }
        return Ok(if no_match {
            Outcome::NoMatch
        } else {
            Outcome::Success
        });
    }

    let target = args
        .target
        .as_deref()
        .expect("clap requires target when port is absent");
    match resolve::resolve(&all, target) {
        Ok(idx) => {
            if args.output.json {
                output::print_json(&all[idx], args.output.pretty)?;
            } else {
                output::print_process_detail(&all[idx])?;
            }
            Ok(Outcome::Success)
        }
        Err(err) => {
            resolve::report_error(&err, args.output.json)?;
            if err.code == "ambiguous" {
                Ok(Outcome::Ambiguous)
            } else {
                Ok(Outcome::NoMatch)
            }
        }
    }
}
