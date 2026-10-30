//! Help and diagnostics keep trusted line breaks, never terminal controls from input.

use std::io::{self, Write};

use clap::error::{ContextKind, ContextValue};
use serde::Serialize;

use super::{
    Destination, format_start_time, json_to, safe, table, terminal_width, use_color, write_stdout,
    writer,
};
use crate::error::StopError;
use crate::runtime::Cancellation;

pub fn usage_error(mut error: clap::Error) -> StopError {
    // Escape values before clap adds diagnostic layout. Usage comes from our
    // static command definition (including bin_name), not argv[0]. Suggestions
    // may embed rejected arguments, so their styled strings need escaping too.
    let context: Vec<_> = error
        .context()
        .map(|(kind, value)| (kind, value.clone()))
        .collect();
    for (kind, value) in context {
        if kind == ContextKind::Usage {
            continue;
        }
        let escaped = match value {
            ContextValue::String(value) => ContextValue::String(safe(&value)),
            ContextValue::Strings(values) => {
                ContextValue::Strings(values.iter().map(|v| safe(v)).collect())
            }
            ContextValue::StyledStr(value) => {
                ContextValue::StyledStr(safe(&value.to_string()).into())
            }
            ContextValue::StyledStrs(values) => ContextValue::StyledStrs(
                values.iter().map(|v| safe(&v.to_string()).into()).collect(),
            ),
            value => value,
        };
        error.insert(kind, escaped);
    }
    // Our value-parser error sources are static messages. Still escape any
    // remaining non-layout controls, without flattening clap's line breaks.
    StopError::Usage(
        error
            .to_string()
            .split('\n')
            .map(safe)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

pub fn print_help(help: &clap::Error, cancel: &Cancellation) -> Result<(), StopError> {
    let help = help.render();
    let text = if use_color(Destination::Stdout) {
        help.ansi().to_string()
    } else {
        help.to_string()
    };
    write_stdout(&text, cancel)
}

fn human_error(err: &StopError) -> String {
    if let StopError::Usage(message) = err {
        return message.clone();
    }
    let mut out = format!("stop: {}\n", safe(&err.to_string()));
    if let StopError::Resolve(err) = err {
        let rows = err
            .candidates
            .iter()
            .map(|candidate| {
                vec![
                    candidate.pid.to_string(),
                    safe(candidate.user.as_deref().unwrap_or("-")),
                    safe(&candidate.name),
                    format_start_time(candidate.start_time),
                    safe(
                        candidate
                            .exe
                            .as_deref()
                            .or(candidate.cwd.as_deref())
                            .unwrap_or("-"),
                    ),
                ]
            })
            .collect();
        out.push_str(&table::render(
            &["PID", "USER", "NAME", "STARTED", "PATH"],
            rows,
            &[0],
            &[(4, 60, 8), (2, usize::MAX, 8), (1, 16, 4)],
            terminal_width(Destination::Stderr),
            use_color(Destination::Stderr),
        ));
    }
    out
}

pub fn report_error(
    err: &StopError,
    machine: bool,
    cancel: Option<&Cancellation>,
) -> Result<(), StopError> {
    let mut out = Vec::new();
    if machine {
        if let StopError::Resolve(err) = err {
            json_to(&mut out, err, false)?;
        } else {
            #[derive(Serialize)]
            struct ErrorPayload<'a> {
                code: &'a str,
                message: String,
            }
            let code = match err {
                StopError::Usage(_) | StopError::InvalidRequest(_) => "usage",
                StopError::Interrupted => "interrupted",
                StopError::UnsupportedPlatform => "unsupported_platform",
                _ => "operational",
            };
            json_to(
                &mut out,
                &ErrorPayload {
                    code,
                    message: err.to_string(),
                },
                false,
            )?;
        }
    } else {
        out.extend_from_slice(human_error(err).as_bytes());
    }
    if let Some(cancel) = cancel {
        writer::write(&out, cancel, writer::Destination::Stderr)
    } else {
        // Handler installation failed. Leave descriptor flags alone so the
        // default signal action can still terminate a blocked write.
        io::stderr().lock().write_all(&out)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;

    #[test]
    fn usage_layout_is_preserved_but_untrusted_text_is_escaped() {
        for args in [
            vec!["stop", "list", "--bogus\n\x1b[31m\u{202e}"],
            vec!["stop", "list", "--sort", "cpu\n\x1b[31m\u{202e}"],
            vec!["stop", "inspect", "--port", "bad\n\x1b[31m\u{202e}"],
        ] {
            let err = Cli::parse_args(args.into_iter().map(Into::into).collect()).unwrap_err();
            let text = human_error(&usage_error(err));
            assert!(text.lines().count() > 1, "diagnostic layout lost: {text}");
            assert!(text.contains("\\n\\u{1b}[31m\\u{202e}"), "{text}");
            assert!(!text.contains('\x1b') && !text.contains('\u{202e}'));
        }
    }
}
