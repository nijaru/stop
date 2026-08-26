//! Entry point: parse args, dispatch commands, map outcomes to exit codes.

use std::io::Write;

use clap::Parser;

use stop::cli::{Cli, Command};
use stop::cmd;
use stop::error::StopError;

fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Command::List(args) => cmd::list::run(args),
        Command::Inspect(args) => cmd::inspect::run(args),
        Command::Top(args) => cmd::top::run(args),
    };

    let exit_code = match result {
        Ok(outcome) => outcome.exit_code(),
        Err(err) => {
            report_error(&err);
            1
        }
    };

    std::process::exit(exit_code);
}

fn report_error(err: &StopError) {
    let _ = writeln!(std::io::stderr().lock(), "stop: {err}");
}
