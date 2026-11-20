//! `comms-e2e`: list the scenarios, run them against a stack, or check that a stack is reachable.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use catalyst_comms_e2e::engine::{CrowdSize, Ctx, RunOptions, Transport, run_scenarios};
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
        /// Repeats per scenario, 1 to 100; absent, each scenario's own default
        #[arg(long, value_name = "n", value_parser = clap::value_parser!(u32).range(1..=100))]
        repeat: Option<u32>,
        /// The seed of the first repeat: replays a failure
        #[arg(long)]
        seed: Option<u64>,
        /// A skipped scenario fails the run
        #[arg(long)]
        strict: bool,
        /// A host that is not loopback which scenarios may reach. Without it a run reaches only
        /// loopback. It guards against accidents and is not access control
        #[arg(long = "allow-host", value_name = "host")]
        allowed_hosts: Vec<String>,
        /// A host named with --allow-host which hostile scenarios (floods, malformed frames,
        /// bans, restarts) may reach as well
        #[arg(long = "allow-hostile-host", value_name = "host")]
        allowed_hostile_hosts: Vec<String>,
        /// Name every host of the stack file in the report, not only the loopback ones
        #[arg(long)]
        report_hosts: bool,
        /// Write the full report, timelines included, as JSON
        #[arg(long, value_name = "file")]
        report: Option<PathBuf>,
        /// A directory in which the run makes a new directory of its own for its files
        /// (explorer logs); the system's temporary directory unless given
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
        /// A host that is not loopback which the explorer and the fault link may reach, as for
        /// `run`
        #[arg(long = "allow-host", value_name = "host")]
        allowed_hosts: Vec<String>,
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
            allowed_hostile_hosts,
            report_hosts,
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
                allowed_hostile_hosts,
                report_hosts,
                scenario_deadline: Duration::from_secs(deadline),
                transport,
                one_source_address,
                crowd: crowd_from_environment()?,
                ..RunOptions::default()
            };
            options.work_dir = match work_dir {
                Some(parent) => new_run_dir(&parent, "run")?,
                None => new_run_dir(&std::env::temp_dir(), "comms-e2e-run")?,
            };
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
            allowed_hosts,
            wallet_seed,
            position,
            settle,
            hold,
            log_tail,
            commands,
        } => {
            let stack = Arc::new(Stack::load(&stack_file)?);
            console_guard(&stack, &allowed_hosts)?;
            let (x, y) = position
                .split_once(',')
                .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
                .ok_or_else(|| anyhow::anyhow!("--position takes x,y"))?;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            let work_dir = new_run_dir(&std::env::temp_dir(), "comms-e2e-console")?;
            runtime.block_on(async move {
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
            println!("{}", address_of_key(&key)?);
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

/// The console starts an explorer and the fault links against the stack: the same host guard as
/// an ordinary scenario.
fn console_guard(stack: &Stack, allowed_hosts: &[String]) -> Result<()> {
    stack
        .ensure_safe_for(false, allowed_hosts, &[])
        .map_err(|refusal| anyhow::anyhow!("the console does not start: {refusal}"))
}

/// A directory made new for one run under `parent`, named `<prefix>-<random>`: never one that
/// was there before, so nothing in it was put there by anyone else, and readable by this user
/// alone where the platform has such modes.
fn new_run_dir(parent: &Path, prefix: &str) -> Result<PathBuf> {
    use std::hash::{BuildHasher, Hasher};
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder
    };
    #[cfg(not(unix))]
    let builder = std::fs::DirBuilder::new();
    for _ in 0..16 {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        );
        let path = parent.join(format!("{prefix}-{:016x}", hasher.finish()));
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("creating {}", path.display()));
            }
        }
    }
    bail!("no new directory could be made in {}", parent.display())
}

/// The crowd size the environment asks for: `CE2E_CROWD_PLAYERS` and `CE2E_CROWD_ROOM_PLAYERS`.
fn crowd_from_environment() -> Result<CrowdSize> {
    crowd_from(
        std::env::var(scenarios::crowd::PLAYERS_VARIABLE).ok(),
        std::env::var(scenarios::crowd_discovery::ROOM_PLAYERS_VARIABLE).ok(),
    )
}

/// The crowd size the texts of the two variables name.
fn crowd_from(players: Option<String>, room_players: Option<String>) -> Result<CrowdSize> {
    Ok(CrowdSize {
        players: players
            .map(|text| scenarios::crowd::players_from(Some(&text)))
            .transpose()?,
        room_players: room_players
            .map(|text| scenarios::crowd_discovery::room_players_from(&text))
            .transpose()?,
    })
}

/// The wallet address of a private key. A key that does not parse is an error that does not
/// repeat it.
fn address_of_key(key: &str) -> Result<String> {
    Ok(catalyst_comms_e2e::identity::Identity::from_wallet_key(key, "address")?.address())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_command_does_not_repeat_a_key_that_does_not_parse() {
        let error = format!(
            "{:#}",
            address_of_key("not-a-real-key-fake-0123").unwrap_err()
        );
        assert!(error.contains("not a valid private key"), "{error}");
        assert!(!error.contains("not-a-real-key-fake-0123"), "{error}");
        // an obviously made-up key that parses
        assert!(address_of_key(&"11".repeat(32)).unwrap().starts_with("0x"));
    }

    #[test]
    fn the_crowd_size_comes_from_the_texts_of_its_variables() {
        assert_eq!(crowd_from(None, None).unwrap(), CrowdSize::default());
        assert_eq!(
            crowd_from(Some(" 12 ".into()), Some("0".into())).unwrap(),
            CrowdSize {
                players: Some(12),
                room_players: Some(0),
            }
        );
        assert!(crowd_from(Some("1".into()), None).is_err());
        assert!(crowd_from(None, Some("some".into())).is_err());
    }

    #[test]
    fn the_console_passes_the_host_guard_and_takes_allow_host() {
        let public =
            Stack::parse("api = \"http://203.0.113.5:5210\"\npulse_enet = \"127.0.0.1:9\"\n")
                .unwrap();
        let refusal = format!("{:#}", console_guard(&public, &[]).unwrap_err());
        assert!(refusal.contains("--allow-host"), "{refusal}");
        console_guard(&public, &["203.0.113.5".to_owned()]).unwrap();
        let local = Stack::pulse_only("127.0.0.1:9".parse().unwrap());
        console_guard(&local, &[]).unwrap();

        let Command::Console { allowed_hosts, .. } = Arguments::try_parse_from([
            "comms-e2e",
            "console",
            "--stack-file",
            "stack.toml",
            "--allow-host",
            "stack.example",
            "/world_state",
        ])
        .unwrap()
        .command
        else {
            panic!("a console");
        };
        assert_eq!(allowed_hosts, vec!["stack.example".to_owned()]);
    }

    #[test]
    fn each_run_gets_a_directory_made_new_for_it() {
        let parent = std::env::temp_dir().join(format!("comms-e2e-dirs-{}", std::process::id()));
        let first = new_run_dir(&parent, "run").unwrap();
        let second = new_run_dir(&parent, "run").unwrap();
        assert_ne!(first, second);
        for made in [&first, &second] {
            assert!(
                made.is_dir() && made.starts_with(&parent),
                "{}",
                made.display()
            );
            assert_eq!(std::fs::read_dir(made).unwrap().count(), 0);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(made).unwrap().permissions().mode();
                assert_eq!(mode & 0o777, 0o700, "{mode:o}");
            }
        }
        std::fs::remove_dir_all(&parent).unwrap();
    }

    fn run_with_repeat(repeat: &str) -> Result<Arguments, clap::Error> {
        Arguments::try_parse_from([
            "comms-e2e",
            "run",
            "pulse-smoke",
            "--stack-file",
            "stack.toml",
            "--repeat",
            repeat,
        ])
    }

    #[test]
    fn repeat_runs_from_1_to_100() {
        for accepted in ["1", "100"] {
            assert!(run_with_repeat(accepted).is_ok(), "{accepted}");
        }
        for refused in ["0", "101", "100000"] {
            assert!(run_with_repeat(refused).is_err(), "{refused}");
        }
    }

    #[test]
    fn hostile_hosts_are_a_flag_of_their_own() {
        let Command::Run {
            allowed_hosts,
            allowed_hostile_hosts,
            report_hosts,
            ..
        } = Arguments::try_parse_from([
            "comms-e2e",
            "run",
            "--all",
            "--stack-file",
            "stack.toml",
            "--allow-host",
            "stack.example",
            "--allow-hostile-host",
            "stack.example",
            "--report-hosts",
        ])
        .unwrap()
        .command
        else {
            panic!("a run");
        };
        assert_eq!(allowed_hosts, vec!["stack.example".to_owned()]);
        assert_eq!(allowed_hostile_hosts, vec!["stack.example".to_owned()]);
        assert!(report_hosts);
    }
}
