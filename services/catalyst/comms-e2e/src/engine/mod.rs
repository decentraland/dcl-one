//! The scenario engine: what a scenario is, the context it runs in, and the runner that repeats
//! it and turns what happened into a report.

pub mod concurrency;
pub mod expect;
pub mod report;
pub mod timeline;

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use futures::FutureExt;
use parking_lot::Mutex;

use crate::identity::Identity;
use crate::stack::Stack;
pub use concurrency::{Seeded, together};
pub use expect::{expect_async, expect_or, expect_within, holds_for};
pub use report::{Note, Outcome, Report, ScenarioResult};
pub use timeline::{Entry, Timeline};

/// How the Pulse bots of a scenario reach Pulse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub enum Transport {
    #[default]
    Enet,
    WebTransport,
}

pub type ScenarioFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

/// One scripted situation and what must hold in it.
pub struct Scenario {
    pub name: &'static str,
    pub tags: &'static [&'static str],
    /// The recorded problems this scenario guards: pull requests and commits.
    pub guards: &'static [&'static str],
    pub summary: &'static str,
    pub needs: Needs,
    /// Floods, sends malformed frames or creates bans: refused against a public host.
    pub hostile: bool,
    /// How many times a run repeats it unless told otherwise. Races repeat.
    pub default_repeat: u32,
    pub run: fn(Ctx) -> ScenarioFuture,
}

/// What a scenario needs from the stack beyond Pulse over ENet.
#[derive(Debug, Clone, Copy, Default)]
pub struct Needs {
    pub api: bool,
    pub webtransport: bool,
    pub pulse_metrics: bool,
    pub livekit: bool,
    pub moderator: bool,
    pub explorer: bool,
    /// The stack file names a command that restarts the gatekeeper's process.
    pub restart: bool,
}

impl Needs {
    /// What `stack` lacks for this scenario, if anything.
    pub fn missing(&self, stack: &Stack) -> Option<String> {
        let mut missing = Vec::new();
        if self.api && stack.api.is_none() {
            missing.push("an API address (`api`)");
        }
        if self.webtransport && stack.pulse_webtransport.is_none() {
            missing.push("Pulse over WebTransport (`pulse_webtransport`)");
        }
        if self.pulse_metrics && stack.pulse_metrics.is_none() {
            missing.push("the Pulse metrics address (`pulse_metrics`)");
        }
        if self.livekit && stack.livekit.is_none() {
            missing.push("LiveKit (`livekit`)");
        }
        if self.moderator && stack.moderator_token_env.is_none() {
            missing.push("the moderator token (`moderator_token_env`)");
        }
        if self.explorer && stack.explorer.is_none() {
            missing.push("the headless explorer (`explorer`)");
        }
        if self.restart && stack.restart_command.is_none() {
            missing.push("a restart command (`restart_command`)");
        }
        (!missing.is_empty()).then(|| format!("the stack file has no {}", missing.join(", no ")))
    }
}

struct CtxInner {
    stack: Arc<Stack>,
    run_id: String,
    scenario: String,
    repeat: u32,
    seed: u64,
    transport: Transport,
    timeline: Timeline,
    seeded: Mutex<Seeded>,
    notes: Mutex<Vec<Note>>,
    work_dir: PathBuf,
}

/// What a running scenario holds: the stack, its own names and wallets, its seed and its timeline.
#[derive(Clone)]
pub struct Ctx {
    inner: Arc<CtxInner>,
}

impl Ctx {
    pub fn new(
        stack: Arc<Stack>,
        run_id: &str,
        scenario: &str,
        repeat: u32,
        seed: u64,
        work_dir: PathBuf,
    ) -> Ctx {
        Ctx::new_on(stack, run_id, scenario, repeat, seed, work_dir, Transport::Enet)
    }

    /// [`Ctx::new`] with the transport its Pulse bots use.
    pub fn new_on(
        stack: Arc<Stack>,
        run_id: &str,
        scenario: &str,
        repeat: u32,
        seed: u64,
        work_dir: PathBuf,
        transport: Transport,
    ) -> Ctx {
        Ctx {
            inner: Arc::new(CtxInner {
                stack,
                run_id: run_id.to_owned(),
                scenario: scenario.to_owned(),
                repeat,
                seed,
                transport,
                timeline: Timeline::new(),
                seeded: Mutex::new(Seeded::new(seed)),
                notes: Mutex::new(Vec::new()),
                work_dir,
            }),
        }
    }

    pub fn stack(&self) -> &Stack {
        &self.inner.stack
    }

    pub fn timeline(&self) -> &Timeline {
        &self.inner.timeline
    }

    pub fn seed(&self) -> u64 {
        self.inner.seed
    }

    /// The transport the Pulse bots of this run use.
    pub fn transport(&self) -> Transport {
        self.inner.transport
    }

    pub fn repeat(&self) -> u32 {
        self.inner.repeat
    }

    pub fn run_id(&self) -> &str {
        &self.inner.run_id
    }

    /// A directory for this scenario's files (explorer logs).
    pub fn work_dir(&self) -> PathBuf {
        self.inner
            .work_dir
            .join(format!("{}-{}", self.inner.scenario, self.inner.repeat))
    }

    /// A realm name no other run, scenario or repeat uses, so players of one never meet players
    /// of another.
    pub fn realm(&self, label: &str) -> String {
        format!(
            "e2e-{}-{}-{}-{label}",
            self.inner.run_id, self.inner.scenario, self.inner.repeat
        )
    }

    /// The identity of actor `index`; no other run, scenario or repeat derives the same wallet.
    pub fn identity(&self, index: u32) -> Identity {
        Identity::derive(
            &format!("{}/{}/{}", self.inner.run_id, self.inner.scenario, self.inner.repeat),
            index,
        )
    }

    pub fn record(&self, actor: &str, kind: &str, detail: impl serde::Serialize) {
        self.inner.timeline.record(actor, kind, detail);
    }

    /// A measurement worth keeping whether the scenario passes or fails.
    pub fn note(&self, key: &str, value: impl serde::Serialize) {
        let value = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
        self.record("note", key, &value);
        self.inner.notes.lock().push(Note {
            repeat: self.inner.repeat,
            key: key.to_owned(),
            value,
        });
    }

    pub(crate) fn take_notes(&self) -> Vec<Note> {
        std::mem::take(&mut self.inner.notes.lock())
    }

    /// A seeded gap inside `window`: the arrival order of a race, replayable from the seed.
    pub fn offset_within(&self, window: Duration) -> Duration {
        self.inner.seeded.lock().gap(window)
    }

    /// A seeded value in `0..bound`.
    pub fn random_below(&self, bound: u64) -> u64 {
        self.inner.seeded.lock().below(bound)
    }

    /// Which of two actors goes first in this repeat: both orders are covered across repeats.
    pub fn first_of_two(&self) -> usize {
        (self.inner.repeat % 2) as usize
    }
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Repeats per scenario; `None` uses each scenario's own default.
    pub repeat: Option<u32>,
    /// The seed of the first repeat; `None` draws one from the clock.
    pub seed: Option<u64>,
    /// A skipped scenario fails the run.
    pub strict: bool,
    pub allowed_hosts: Vec<String>,
    pub work_dir: PathBuf,
    /// The longest one repeat may take.
    pub scenario_deadline: Duration,
    /// The transport Pulse bots use.
    pub transport: Transport,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            repeat: None,
            seed: None,
            strict: false,
            allowed_hosts: Vec::new(),
            work_dir: std::env::temp_dir().join("comms-e2e"),
            scenario_deadline: Duration::from_secs(300),
            transport: Transport::Enet,
        }
    }
}

fn fresh_run_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    // short, unique enough per machine, and safe inside a realm name
    format!("{:x}{:03x}", now.as_secs(), now.subsec_millis())
}

/// Runs each scenario its number of repeats, stopping a scenario at its first failure, and
/// reports what happened. Each repeat `r` runs with seed `first_seed + r`.
pub async fn run_scenarios(
    stack: Arc<Stack>,
    selection: &[&Scenario],
    options: &RunOptions,
) -> Report {
    let run_id = fresh_run_id();
    let first_seed = options.seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
            & 0xffff_ffff
    });
    let mut report = Report::new(&run_id, first_seed, stack.hosts());

    for scenario in selection {
        let started = Instant::now();
        let mut result = ScenarioResult {
            name: scenario.name.to_owned(),
            guards: scenario.guards.iter().map(|guard| (*guard).to_owned()).collect(),
            outcome: Outcome::Passed,
            repeats_run: 0,
            duration_ms: 0,
            notes: Vec::new(),
        };

        if let Some(reason) = scenario.needs.missing(&stack) {
            result.outcome = Outcome::Skipped { reason };
        } else if options.transport == Transport::WebTransport && stack.pulse_webtransport.is_none() {
            result.outcome = Outcome::Skipped {
                reason: "the stack file has no Pulse over WebTransport (`pulse_webtransport`)".to_owned(),
            };
        } else if let Err(refusal) = stack.ensure_safe_for(scenario.hostile, &options.allowed_hosts) {
            result.outcome = Outcome::Failed {
                expectation: format!("refused before the first packet: {refusal}"),
                seed: first_seed,
                repeat: 0,
                timeline: Vec::new(),
            };
        } else {
            let repeats = options.repeat.unwrap_or(scenario.default_repeat).max(1);
            for repeat in 0..repeats {
                let seed = first_seed.wrapping_add(repeat as u64);
                let ctx = Ctx::new_on(
                    stack.clone(),
                    &run_id,
                    scenario.name,
                    repeat,
                    seed,
                    options.work_dir.join(&run_id),
                    options.transport,
                );
                ctx.record("runner", "start", serde_json::json!({ "seed": seed, "repeat": repeat }));
                let outcome = run_once(scenario, ctx.clone(), options.scenario_deadline).await;
                result.repeats_run = repeat + 1;
                result.notes.extend(ctx.take_notes());
                if let Err(expectation) = outcome {
                    result.outcome = Outcome::Failed {
                        expectation,
                        seed,
                        repeat,
                        timeline: ctx.timeline().snapshot(),
                    };
                    break;
                }
            }
        }
        result.duration_ms = started.elapsed().as_millis() as u64;
        report.results.push(result);
    }
    report
}

/// One repeat: its error, a panic or the deadline all end it as a failure with a reason.
async fn run_once(scenario: &Scenario, ctx: Ctx, deadline: Duration) -> Result<(), String> {
    let work = std::panic::AssertUnwindSafe((scenario.run)(ctx)).catch_unwind();
    match tokio::time::timeout(deadline, work).await {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(error))) => Err(format!("{error:#}")),
        Ok(Err(panic)) => {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
                .unwrap_or_else(|| "no message".to_owned());
            Err(format!("the scenario panicked: {message}"))
        }
        Err(_) => Err(format!(
            "the scenario did not finish within {} s",
            deadline.as_secs()
        )),
    }
}
