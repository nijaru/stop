use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use stop::agent::{ProcessQuery, ProcessSelector, SortKey, collect_observation, render};

const DEFAULT_TOP_LIMIT: usize = 20;
const EXIT_NO_MATCH_OR_AMBIGUOUS: i32 = 3;

#[derive(Parser, Debug)]
#[command(name = "stop-next")]
#[command(about = "Experimental agent-first process observation CLI")]
#[command(version)]
struct Cli {
    #[arg(long, global = true, help = "Emit the stable machine-readable JSON result")]
    json: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List matching processes. Exhaustive unless --limit is supplied.
    List(QueryArgs),

    /// Inspect one process selected explicitly.
    Inspect(InspectArgs),

    /// Show ranked resource consumers. Limited to 20 by default.
    Top(TopArgs),
}

#[derive(ClapArgs, Clone, Debug, Default)]
struct Selectors {
    #[arg(long)]
    pid: Option<u32>,

    #[arg(long)]
    name: Option<String>,

    #[arg(long)]
    user: Option<String>,

    #[arg(long)]
    cwd: Option<String>,

    #[arg(long)]
    parent: Option<u32>,

    #[arg(long, value_name = "PERCENT")]
    min_cpu: Option<f32>,
}

impl Selectors {
    fn is_empty(&self) -> bool {
        self.pid.is_none()
            && self.name.is_none()
            && self.user.is_none()
            && self.cwd.is_none()
            && self.parent.is_none()
            && self.min_cpu.is_none()
    }
}

#[derive(ClapArgs, Debug)]
struct QueryArgs {
    #[command(flatten)]
    selectors: Selectors,

    #[arg(long, value_enum, default_value_t = SortArg::Cpu)]
    sort: SortArg,

    #[arg(short = 'n', long)]
    limit: Option<usize>,
}

#[derive(ClapArgs, Debug)]
struct InspectArgs {
    #[command(flatten)]
    selectors: Selectors,
}

#[derive(ClapArgs, Debug)]
struct TopArgs {
    #[command(flatten)]
    selectors: Selectors,

    #[arg(long, value_enum, default_value_t = SortArg::Cpu)]
    sort: SortArg,

    #[arg(short = 'n', long, default_value_t = DEFAULT_TOP_LIMIT)]
    limit: usize,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum SortArg {
    #[default]
    Cpu,
    Memory,
    Pid,
    Name,
}

impl From<SortArg> for SortKey {
    fn from(value: SortArg) -> Self {
        match value {
            SortArg::Cpu => SortKey::Cpu,
            SortArg::Memory => SortKey::Memory,
            SortArg::Pid => SortKey::Pid,
            SortArg::Name => SortKey::Name,
        }
    }
}

impl From<Selectors> for ProcessSelector {
    fn from(value: Selectors) -> Self {
        ProcessSelector {
            pid: value.pid,
            name: value.name,
            user: value.user,
            cwd: value.cwd,
            parent: value.parent,
            min_cpu: value.min_cpu,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        None => run_list(
            ProcessQuery {
                selector: ProcessSelector::default(),
                sort: SortKey::Cpu,
                limit: Some(DEFAULT_TOP_LIMIT),
            },
            cli.json,
        ),
        Some(Command::List(args)) => run_list(
            ProcessQuery {
                selector: checked_selector(args.selectors, false)?,
                sort: args.sort.into(),
                limit: args.limit,
            },
            cli.json,
        ),
        Some(Command::Top(args)) => run_list(
            ProcessQuery {
                selector: checked_selector(args.selectors, false)?,
                sort: args.sort.into(),
                limit: Some(args.limit),
            },
            cli.json,
        ),
        Some(Command::Inspect(args)) => {
            let selector = checked_selector(args.selectors, true)?;
            let result = ProcessQuery {
                selector,
                sort: SortKey::Pid,
                limit: None,
            }
            .execute(collect_observation()?);

            if cli.json {
                render::write_json(&result)?;
            } else if result.processes.len() == 1 {
                render::write_inspect_text(&result.processes[0])?;
            } else {
                render::write_list_text(&result)?;
            }

            match result.processes.len() {
                1 => Ok(()),
                0 => {
                    eprintln!("no process matched the selector");
                    std::process::exit(EXIT_NO_MATCH_OR_AMBIGUOUS);
                }
                count => {
                    eprintln!("selector is ambiguous: {count} processes matched");
                    std::process::exit(EXIT_NO_MATCH_OR_AMBIGUOUS);
                }
            }
        }
    }
}

fn run_list(query: ProcessQuery, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let result = query.execute(collect_observation()?);
    if json {
        render::write_json(&result)?;
    } else {
        render::write_list_text(&result)?;
    }
    Ok(())
}

fn checked_selector(
    selectors: Selectors,
    require_selector: bool,
) -> Result<ProcessSelector, Box<dyn std::error::Error>> {
    if require_selector && selectors.is_empty() {
        return Err("inspect requires an explicit selector such as --pid or --name".into());
    }

    if let Some(cpu) = selectors.min_cpu {
        if !cpu.is_finite() || cpu < 0.0 {
            return Err("--min-cpu must be a finite non-negative percentage".into());
        }
    }

    Ok(selectors.into())
}
