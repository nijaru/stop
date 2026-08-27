//! Entry point: parse args, dispatch commands, map outcomes to exit codes.
//!
//! Exit codes: 0 success, 1 operational/usage error, 2 no-match,
//! 3 ambiguous inspect target. Code 2 is reserved for query results;
//! argument parsing failures report as 1.

use std::io::Write;

use clap::Parser as _;
use serde::Serialize;

use stop::cli::{Cli, Command};
use stop::cmd::{self};
use stop::error::StopError;

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            // clap's default exit code (2) would collide with no-match.
            let code = if err.use_stderr() { 1 } else { 0 };
            let _ = err.print();
            std::process::exit(code);
        }
    };

    let json = match &cli.command {
        Command::List(a) => a.output.json,
        Command::Inspect(a) => a.output.json,
        Command::Top(a) => a.output.json,
        Command::Tree(a) => a.output.json,
        Command::Sample(a) => a.output.json,
    };

    let result = match &cli.command {
        Command::List(args) => cmd::list::run(args),
        Command::Inspect(args) => cmd::inspect::run(args),
        Command::Top(args) => cmd::top::run(args),
        Command::Tree(args) => cmd::tree::run(args),
        Command::Sample(args) => cmd::sample::run(args),
    };

    let exit_code = match result {
        Ok(outcome) => outcome.exit_code(),
        Err(err) => {
            report_error(&err, json);
            1
        }
    };

    std::process::exit(exit_code);
}

#[derive(Serialize)]
struct ErrorPayload<'a> {
    code: &'a str,
    message: String,
}

fn report_error(err: &StopError, json: bool) {
    let mut stderr = std::io::stderr().lock();
    if json {
        let payload = ErrorPayload {
            code: "internal",
            message: err.to_string(),
        };
        if let Ok(line) = serde_json::to_string(&payload) {
            let _ = writeln!(stderr, "{line}");
            return;
        }
    }
    let _ = writeln!(stderr, "stop: {err}");
}
