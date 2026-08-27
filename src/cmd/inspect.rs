//! `stop inspect` — resolve exactly one process by PID or name.

use crate::cli::InspectArgs;
use crate::cmd::{Outcome, resolve};
use crate::collector;
use crate::error::StopError;
use crate::output;

pub fn run(args: &InspectArgs) -> Result<Outcome, StopError> {
    let (_metrics, all) = collector::collect(true)?;

    match resolve::resolve(&all, &args.target) {
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
