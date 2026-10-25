//! Parse, dispatch, and map outcomes once. Human output is the default.

use clap::Parser;
use stop::cli::{Cli, Command, QueryArgs};
use stop::cmd::{self, Outcome};
use stop::error::StopError;
use stop::runtime::Cancellation;

fn main() -> std::process::ExitCode {
    let raw: Vec<_> = std::env::args_os().collect();
    // Recognize explicit formats even when clap rejects the rest of the request.
    let machine = raw
        .iter()
        .skip(1)
        .take_while(|s| s.as_os_str() != "--")
        .any(|s| matches!(s.to_str(), Some("--json" | "--pretty" | "--jsonl")));
    // Parse diagnostics and help can block too. Install before any output so
    // interruption always unwinds and restores inherited descriptor flags.
    let cancel = match Cancellation::install() {
        Ok(cancel) => cancel,
        Err(err) => {
            let _ = stop::output::report_error(
                &StopError::Operational(format!("cannot install interrupt handler: {err}")),
                machine,
                None,
            );
            return std::process::ExitCode::from(1);
        }
    };
    let code = match run(raw, &cancel) {
        Ok(outcome) => outcome.exit_code(),
        Err(StopError::Io(err)) if err.kind() == std::io::ErrorKind::BrokenPipe => 0,
        Err(err) => {
            let code = match &err {
                StopError::Interrupted => 130,
                StopError::Resolve(e) if e.code == "ambiguous" => 3,
                _ => 1,
            };
            let _ = stop::output::report_error(&err, machine, Some(&cancel));
            if cancel.interrupted() { 130 } else { code }
        }
    };
    std::process::ExitCode::from(code as u8)
}

fn run(raw: Vec<std::ffi::OsString>, cancel: &Cancellation) -> Result<Outcome, StopError> {
    let cli = match Cli::try_parse_from(raw) {
        Ok(cli) => cli,
        Err(err) if err.use_stderr() => return Err(StopError::InvalidRequest(err.to_string())),
        Err(help) => {
            stop::output::print_help(&help, cancel)?;
            return Ok(Outcome::Success);
        }
    };
    cli.validate().map_err(StopError::InvalidRequest)?;
    let defaults = QueryArgs::default();
    match &cli.command {
        None => cmd::query::run(&defaults, true, &cli.collection, &cli.output, cancel),
        Some(Command::List(args)) => {
            cmd::query::run(args, false, &cli.collection, &cli.output, cancel)
        }
        Some(Command::Top(args)) => {
            cmd::query::run(args, true, &cli.collection, &cli.output, cancel)
        }
        Some(Command::Inspect(args)) => {
            cmd::inspect::run(args, &cli.collection, &cli.output, cancel)
        }
        Some(Command::Tree(args)) => cmd::tree::run(args, &cli.collection, &cli.output, cancel),
        Some(Command::Sample(args)) => cmd::sample::run(args, &cli.output, cancel),
    }
}
