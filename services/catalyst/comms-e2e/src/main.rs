//! `comms-e2e`: list the scenarios, run them against a stack, or check that a stack is reachable.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use catalyst_comms_e2e::engine::{Ctx, RunOptions, Transport, run_scenarios};
use catalyst_comms_e2e::explorer::{Explorer, ExplorerOptions, Fronts};
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
        /// The transport Pulse bots use: enet or webtransport
        #[arg(long, value_name = "transport", default_value = "enet")]
        transport: String,
        /// Dial from the ordinary loopback address in every scenario. By default each scenario
        /// and repeat dials a loopback stack from its own address in 127.0.0.0/8, so the
        /// per-address budgets of the servers are not spent across scenarios
        #[arg(long)]
        one_source_address: bool,
    },
    /// Check that every address in the stack file answers
    Doctor {
        #[arg(long, value_name = "file")]
        stack_file: PathBuf,
    },
    /// Print the wallet address of a private key held in an environment variable: the address
    /// to configure in Pulse as the one scene listener it admits
    Address {
        /// The environment variable that holds the private key
        #[arg(long, value_name = "variable")]
        key_env: String,
    },
    /// Start the headless explorer behind the fault link and run console commands in it
    Console {
        #[arg(long, value_name = "file")]
        stack_file: PathBuf,
        /// The seed of the guest wallet
        #[arg(long, default_value_t = 1)]
        wallet_seed: u64,
        /// The parcel to start at, as x,y
        #[arg(long, default_value = "0,0", allow_hyphen_values = true)]
        position: String,
        /// Seconds to wait after the console opens before the first command
        #[arg(long, default_value_t = 0)]
        settle: u64,
        /// Seconds to keep the explorer running after the last command
        #[arg(long, default_value_t = 0)]
        hold: u64,
        /// Lines of the explorer log to print at the end
        #[arg(long, default_value_t = 0)]
        log_tail: usize,
        /// Console commands, each as one argument, for example "/world_state"
        commands: Vec<String>,
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
            transport,
            one_source_address,
        } => {
            let transport = match transport.as_str() {
                "enet" => Transport::Enet,
                "webtransport" => Transport::WebTransport,
                other => bail!("unknown transport {other}: enet or webtransport"),
            };
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
                transport,
                one_source_address,
                ..RunOptions::default()
            };
            if let Some(work_dir) = work_dir {
                options.work_dir = work_dir;
            }
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
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
        Command::Console {
            stack_file,
            wallet_seed,
            position,
            settle,
            hold,
            log_tail,
            commands,
        } => {
            let stack = Arc::new(Stack::load(&stack_file)?);
            let (x, y) = position
                .split_once(',')
                .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
                .ok_or_else(|| anyhow::anyhow!("--position takes x,y"))?;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async move {
                let work_dir = RunOptions::default().work_dir.join("console");
                let ctx = Ctx::new(stack, "console", "console", 0, 1, work_dir);
                let _fronts = Fronts::start(&ctx).await?;
                let explorer = Explorer::start(
                    &ctx,
                    "explorer",
                    ExplorerOptions {
                        wallet_seed,
                        position: (x, y),
                        ..ExplorerOptions::default()
                    },
                )
                .await?;
                println!("explorer log: {}", explorer.log_path().display());
                tokio::time::sleep(Duration::from_secs(settle)).await;
                for line in &commands {
                    match explorer.command(line, Duration::from_secs(15)).await {
                        Ok(answer) => println!("{line}\n  ok={} {}", answer.ok, answer.output),
                        Err(error) => println!("{line}\n  no answer: {error:#}"),
                    }
                }
                tokio::time::sleep(Duration::from_secs(hold)).await;
                if log_tail > 0 {
                    println!("--- log tail\n{}", explorer.log_tail(log_tail));
                }
                explorer.stop().await;
                anyhow::Ok(())
            })?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Address { key_env } => {
            let key = catalyst_comms_e2e::stack::secret_from_environment(&key_env)?;
            let identity =
                catalyst_comms_e2e::identity::Identity::from_wallet_key(&key, "address")?;
            println!("{}", identity.address());
            Ok(ExitCode::SUCCESS)
        }
        Command::Doctor { stack_file } => {
            let stack = Arc::new(Stack::load(&stack_file)?);
            println!("limits expected: {:?}", stack.limits);
            println!("grid expected: {:?}", stack.grid);
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            let checks = runtime.block_on(catalyst_comms_e2e::doctor::examine(stack));
            print!("{}", catalyst_comms_e2e::doctor::render(&checks));
            Ok(if catalyst_comms_e2e::doctor::usable(&checks) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}
