//! `comms-e2e`: list the scenarios, run them against a stack, or check that a stack is reachable.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use catalyst_comms_e2e::engine::{RunOptions, run_scenarios};
use catalyst_comms_e2e::scenarios;
use catalyst_comms_e2e::stack::Stack;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "comms-e2e",
    about = "End-to-end scenarios against a running comms stack"
)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the scenarios and the problems each one guards
    List {
        /// Only scenarios carrying this tag
        #[arg(long)]
        tag: Option<String>,
    },
    /// Run scenarios against the stack a stack file describes
    Run {
        /// One scenario by name
        scenario: Option<String>,
        /// Every scenario carrying this tag
        #[arg(long)]
        tag: Option<String>,
        /// Every scenario
        #[arg(long)]
        all: bool,
        /// The stack file
        #[arg(long, value_name = "file")]
        stack_file: PathBuf,
        /// Repeats per scenario; absent, each scenario's own default
        #[arg(long, value_name = "n")]
        repeat: Option<u32>,
        /// The seed of the first repeat: replays a failure
        #[arg(long)]
        seed: Option<u64>,
        /// A skipped scenario fails the run
        #[arg(long)]
        strict: bool,
        /// A host that hostile scenarios may target although it is not local
        #[arg(long = "allow-host", value_name = "host")]
        allowed_hosts: Vec<String>,
        /// Write the full report, timelines included, as JSON
        #[arg(long, value_name = "file")]
        report: Option<PathBuf>,
        /// Where scenarios keep their files (explorer logs)
        #[arg(long, value_name = "directory")]
        work_dir: Option<PathBuf>,
        /// The longest one repeat may take, in seconds
        #[arg(long, value_name = "seconds", default_value_t = 300)]
        deadline: u64,
        /// Timeline entries shown for a failure
        #[arg(long, value_name = "n", default_value_t = 40)]
        timeline: usize,
    },
    /// Check that every address in the stack file answers
    Doctor {
        #[arg(long, value_name = "file")]
        stack_file: PathBuf,
    },
}

fn main() -> ExitCode {
    match run(Arguments::parse()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("comms-e2e: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn run(arguments: Arguments) -> Result<ExitCode> {
    match arguments.command {
        Command::List { tag } => {
            for scenario in scenarios::select(None, tag.as_deref(), tag.is_none()) {
                println!("{}", scenario.name);
                println!("    {}", scenario.summary);
                println!("    tags: {}", scenario.tags.join(", "));
                if !scenario.guards.is_empty() {
                    println!("    guards: {}", scenario.guards.join(", "));
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Run {
            scenario,
            tag,
            all,
            stack_file,
            repeat,
            seed,
            strict,
            allowed_hosts,
            report,
            work_dir,
            deadline,
            timeline,
        } => {
            let selection = scenarios::select(scenario.as_deref(), tag.as_deref(), all);
            if selection.is_empty() {
                bail!("nothing selected: name a scenario, a --tag or --all (see `comms-e2e list`)");
            }
            let stack = Arc::new(Stack::load(&stack_file)?);
            let mut options = RunOptions {
                repeat,
                seed,
                strict,
                allowed_hosts,
                scenario_deadline: Duration::from_secs(deadline),
                ..RunOptions::default()
            };
            if let Some(work_dir) = work_dir {
                options.work_dir = work_dir;
            }
            let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
            let outcome = runtime.block_on(run_scenarios(stack, &selection, &options));
            print!("{}", outcome.render(timeline));
            if let Some(path) = report {
                outcome.write_json(&path)?;
            }
            Ok(if outcome.failed(strict) {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        Command::Doctor { stack_file } => {
            let stack = Stack::load(&stack_file)?;
            println!("hosts: {}", stack.hosts().join(", "));
            println!("limits in use: {:?}", stack.limits);
            println!("grid in use: {:?}", stack.grid);
            Ok(ExitCode::SUCCESS)
        }
    }
}
