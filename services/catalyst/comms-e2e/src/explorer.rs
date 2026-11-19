//! The headless explorer as an actor: a child process driven through its loopback console.
//!
//! The explorer dials the fault link's fronts, never the stack itself, so a scenario can disturb
//! everything the explorer sends and receives. [`Fronts`] holds those links.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use tokio::process::{Child, Command};

use crate::engine::{Ctx, Timeline};
use crate::fault::{Direction, Fault, FaultLink};
use crate::stack::{ExplorerTarget, Stack};

/// The fault links in front of everything the explorer dials.
pub struct Fronts {
    /// HTTP and the archipelago socket.
    pub api: FaultLink,
    /// Pulse over ENet.
    pub pulse: FaultLink,
    /// LiveKit signalling, when the stack file names it.
    pub livekit_signal: Option<FaultLink>,
    /// LiveKit media, when the stack file names it.
    pub livekit_media: Option<FaultLink>,
}

impl Fronts {
    /// Starts relaying every front the stack file names to the address behind it.
    pub async fn start(ctx: &Ctx) -> Result<Fronts> {
        let target = explorer_target(ctx)?;
        let api = ctx
            .stack()
            .api
            .as_ref()
            .ok_or_else(|| anyhow!("the stack file has no API address (`api`)"))?;
        let api_real = socket_address(api)?;
        let livekit_signal = match target.livekit_signal {
            Some(relay) => {
                Some(FaultLink::tcp(ctx, "front livekit signal", relay.front, relay.real).await?)
            }
            None => None,
        };
        let livekit_media = match target.livekit_media {
            Some(relay) => {
                Some(FaultLink::udp(ctx, "front livekit media", relay.front, relay.real).await?)
            }
            None => None,
        };
        Ok(Fronts {
            api: FaultLink::tcp(ctx, "front api", target.api_front, api_real).await?,
            pulse: FaultLink::udp(
                ctx,
                "front pulse",
                target.pulse_front,
                ctx.stack().pulse_enet,
            )
            .await?,
            livekit_signal,
            livekit_media,
        })
    }

    fn all(&self) -> impl Iterator<Item = &FaultLink> {
        [
            Some(&self.api),
            Some(&self.pulse),
            self.livekit_signal.as_ref(),
            self.livekit_media.as_ref(),
        ]
        .into_iter()
        .flatten()
    }

    /// The same fault on every front.
    pub fn set_all(&self, direction: Direction, fault: Fault) {
        self.all().for_each(|link| link.set(direction, fault));
    }

    /// Every flow that exists goes silent for good, on every front: what a machine waking from
    /// sleep leaves behind.
    pub fn half_open_all(&self) {
        self.all().for_each(FaultLink::half_open);
    }

    pub fn restore_all(&self) {
        self.all().for_each(FaultLink::restore);
    }
}

/// The explorer's command line and environment. The console token travels in the environment,
/// where other processes cannot read it; the stack's secrets (the moderator token, the LiveKit key
/// and secret, the scene listener key) are removed from what the child inherits.
fn explorer_command(
    stack: &Stack,
    target: &ExplorerTarget,
    options: &ExplorerOptions,
    port: u16,
    token: &str,
) -> Command {
    let front = format!("http://{}", target.api_front);
    let mut command = Command::new(&target.binary);
    command
        .arg("--realm")
        .arg(&front)
        .arg("--position")
        .arg(format!("{},{}", options.position.0, options.position.1))
        .arg("--catalyst")
        .arg(&front)
        .arg("--comms-gatekeeper")
        .arg(format!("{front}/comms-gatekeeper"))
        .arg("--pulse-server")
        .arg(target.pulse_front.to_string())
        .arg("--wallet-seed")
        .arg(options.wallet_seed.to_string())
        .arg("--automation-port")
        .arg(port.to_string())
        .args(&target.arguments)
        .args(&options.extra);
    for variable in stack.secret_variables() {
        command.env_remove(variable);
    }
    command.env("DCL_AUTOMATION_TOKEN", token);
    command
}

fn explorer_target(ctx: &Ctx) -> Result<&ExplorerTarget> {
    ctx.stack()
        .explorer
        .as_ref()
        .ok_or_else(|| anyhow!("the stack file has no headless explorer (`explorer`)"))
}

fn socket_address(url: &url::Url) -> Result<SocketAddr> {
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
    let port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("{url} has no port"))?;
    format!("{host}:{port}")
        .parse()
        .or_else(|_| format!("[{host}]:{port}").parse())
        .with_context(|| format!("{url} must name an IP address for the fault link to relay to"))
}

#[derive(Debug, Clone)]
pub struct ExplorerOptions {
    /// The seed of the deterministic guest wallet.
    pub wallet_seed: u64,
    /// The parcel to start at.
    pub position: (i32, i32),
    /// The loopback port of the console. Absent: one the operating system says is free.
    pub automation_port: Option<u16>,
    /// Arguments after the ones the actor sets.
    pub extra: Vec<String>,
    /// How long the explorer may take to open its console.
    pub ready_deadline: Duration,
}

impl Default for ExplorerOptions {
    fn default() -> Self {
        Self {
            wallet_seed: 1,
            position: (0, 0),
            automation_port: None,
            extra: Vec::new(),
            ready_deadline: Duration::from_secs(120),
        }
    }
}

/// The answer to one console command.
#[derive(Debug, Clone)]
pub struct Answer {
    pub ok: bool,
    pub output: String,
}

impl Answer {
    /// The output as JSON, when it is JSON.
    pub fn json(&self) -> Option<Value> {
        serde_json::from_str(self.output.trim()).ok()
    }
}

pub struct Explorer {
    name: String,
    /// Behind a lock because `try_wait` needs `&mut` while console calls take `&self` and may run
    /// concurrently; the lock is never held across an await.
    child: parking_lot::Mutex<Child>,
    port: u16,
    token: String,
    http: reqwest::Client,
    log: PathBuf,
    timeline: Timeline,
    request: AtomicU64,
}

impl Explorer {
    /// Starts the headless explorer behind the fronts and waits for its console to answer.
    pub async fn start(ctx: &Ctx, name: &str, options: ExplorerOptions) -> Result<Explorer> {
        let target = explorer_target(ctx)?;
        let work_dir = ctx.work_dir();
        std::fs::create_dir_all(&work_dir)
            .with_context(|| format!("creating {}", work_dir.display()))?;
        let log = work_dir.join(format!("{name}.log"));
        let token = console_token();
        let port = console_port(&options)?;
        let mut command = explorer_command(ctx.stack(), target, &options, port, &token);
        command
            .stdin(Stdio::null())
            .stdout(std::fs::File::create(&log)?)
            .stderr(std::fs::OpenOptions::new().append(true).open(&log)?)
            .current_dir(&work_dir)
            .kill_on_drop(true);
        ctx.record(
            name,
            "explorer starting",
            json!({ "binary": target.binary.display().to_string(), "port": port, "wallet_seed": options.wallet_seed, "log": log.display().to_string() }),
        );
        let child = command
            .spawn()
            .with_context(|| format!("starting the explorer {}", target.binary.display()))?;

        let explorer = Explorer {
            name: name.to_owned(),
            child: parking_lot::Mutex::new(child),
            port,
            token,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()?,
            log,
            timeline: ctx.timeline().clone(),
            request: AtomicU64::new(1),
        };
        explorer.wait_for_console(options.ready_deadline).await?;
        ctx.record(name, "explorer console ready", ());
        Ok(explorer)
    }

    async fn wait_for_console(&self, deadline: Duration) -> Result<()> {
        let started = Instant::now();
        loop {
            if let Some(status) = self.child.lock().try_wait()? {
                bail!(
                    "{}: the explorer exited ({status}) before its console opened; the end of its log:\n{}",
                    self.name,
                    self.log_tail(25)
                );
            }
            // any answer, even a refusal of the call id, means the console is up
            if self
                .rpc("dcl/console_result", json!({ "id": 0 }))
                .await
                .is_ok()
            {
                return Ok(());
            }
            if started.elapsed() >= deadline {
                // the lock is not held across the wait: the child is killed first
                let _ = self.child.lock().start_kill();
                self.wait_for_exit().await;
                bail!(
                    "{}: the explorer did not open its console within {} s; the end of its log:\n{}",
                    self.name,
                    deadline.as_secs(),
                    self.log_tail(25)
                );
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// One JSON-RPC exchange with the console server: the whole response object. A console that
    /// cannot be reached because the process has exited is a `fatal:` error carrying the exit
    /// status and the end of the log, which ends the waits of `engine::expect` at once.
    async fn rpc(&self, method: &str, mut params: Value) -> Result<Value> {
        params["token"] = Value::String(self.token.clone());
        let id = self.request.fetch_add(1, Ordering::Relaxed);
        let exchange = async {
            self.http
                .post(format!("http://127.0.0.1:{}/", self.port))
                .json(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
                .send()
                .await?
                .json()
                .await
        };
        match exchange.await {
            Ok(response) => Ok(response),
            Err(error) => {
                if let Some(status) = self.exit_after(&error).await {
                    bail!(
                        "fatal: {}: the explorer exited ({status}); the end of its log:
{}",
                        self.name,
                        self.log_tail(25)
                    );
                }
                Err(error.into())
            }
        }
    }

    /// The exit status of the process after a console call failed with `error`, if it has exited.
    /// A refused connection can arrive a moment before the operating system reports the exit, so
    /// that case looks a few times; a call that failed some other way looks once.
    async fn exit_after(&self, error: &reqwest::Error) -> Option<std::process::ExitStatus> {
        let looks = if error.is_connect() { 25 } else { 1 };
        for look in 0..looks {
            if let Some(status) = self.exit_status() {
                return Some(status);
            }
            if look + 1 < looks {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
        None
    }

    /// One call whose result is wanted: a protocol error is an `Err`.
    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let response = self.rpc(method, params).await?;
        if let Some(error) = response.get("error") {
            bail!("the console refused {method}: {error}");
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Runs one console command and waits for its answer. A command that failed is an `Ok`
    /// answer with `ok: false`; an `Err` means the console could not be reached or did not
    /// answer in time.
    pub async fn command(&self, line: &str, deadline: Duration) -> Result<Answer> {
        self.run(line, deadline, true).await
    }

    async fn run(&self, line: &str, deadline: Duration, record: bool) -> Result<Answer> {
        let started = Instant::now();
        let call = self.call("dcl/console", json!({ "command": line })).await?;
        let id = call
            .get("id")
            .cloned()
            .ok_or_else(|| anyhow!("{}: the console gave no call id for `{line}`", self.name))?;
        loop {
            let result = self.call("dcl/console_result", json!({ "id": id })).await?;
            if result.get("done").and_then(Value::as_bool) == Some(true) {
                let answer = Answer {
                    ok: result.get("ok").and_then(Value::as_bool).unwrap_or(false),
                    output: result
                        .get("output")
                        .map(|output| match output {
                            Value::String(text) => text.clone(),
                            other => other.to_string(),
                        })
                        .unwrap_or_default(),
                };
                if record {
                    self.timeline.record(
                    &self.name,
                    "console",
                    json!({ "command": line, "ok": answer.ok, "output": answer.output.chars().take(400).collect::<String>() }),
                    );
                }
                return Ok(answer);
            }
            if started.elapsed() >= deadline {
                bail!(
                    "{}: `{line}` did not answer within {} ms",
                    self.name,
                    deadline.as_millis()
                );
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Sends one console command without waiting for its answer: for a burst of commands that
    /// must reach the explorer faster than one answer takes.
    pub async fn fire(&self, line: &str) -> Result<()> {
        self.call("dcl/console", json!({ "command": line })).await?;
        Ok(())
    }

    /// The log lines that contain `needle`, without colour codes or access tokens, newest last.
    pub fn log_lines(&self, needle: &str) -> Vec<String> {
        let text = std::fs::read(&self.log)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        text.lines()
            .filter(|line| line.contains(needle))
            .map(|line| {
                let mut plain = String::with_capacity(line.len());
                let mut characters = line.chars();
                while let Some(character) = characters.next() {
                    if character == '\u{1b}' {
                        // an escape sequence ends at its first letter
                        for skipped in characters.by_ref() {
                            if skipped.is_ascii_alphabetic() {
                                break;
                            }
                        }
                    } else {
                        plain.push(character);
                    }
                }
                crate::engine::timeline::redact_text(plain.trim())
            })
            .collect()
    }

    /// Runs a command whose output is JSON. Fails if the command failed or the output is not JSON.
    pub async fn query(&self, line: &str) -> Result<Value> {
        let answer = self.command(line, Duration::from_secs(10)).await?;
        if !answer.ok {
            bail!("{}: `{line}` failed: {}", self.name, answer.output);
        }
        answer.json().ok_or_else(|| {
            anyhow!(
                "{}: `{line}` did not answer with JSON: {}",
                self.name,
                answer.output
            )
        })
    }

    /// [`Self::query`] for a probe: the reason it is not answering yet, as text.
    pub async fn probe(&self, line: &str) -> Result<Value, String> {
        self.query(line).await.map_err(|error| format!("{error:#}"))
    }

    /// [`Self::probe`] that leaves no entry on the timeline: for a state polled many times,
    /// where the caller records the changes.
    pub async fn poll(&self, line: &str) -> Result<Value, String> {
        let answer = self
            .run(line, Duration::from_secs(10), false)
            .await
            .map_err(|error| format!("{error:#}"))?;
        if !answer.ok {
            return Err(format!("{}: `{line}` failed: {}", self.name, answer.output));
        }
        answer.json().ok_or_else(|| {
            format!(
                "{}: `{line}` did not answer with JSON: {}",
                self.name, answer.output
            )
        })
    }

    /// Whether the process is still running.
    pub fn running(&self) -> bool {
        matches!(self.child.lock().try_wait(), Ok(None))
    }

    /// The exit status, once the process has exited.
    fn exit_status(&self) -> Option<std::process::ExitStatus> {
        self.child.lock().try_wait().ok().flatten()
    }

    /// Waits for the killed process to be reaped, without holding the lock while it waits.
    async fn wait_for_exit(&self) {
        while self.running() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// The last `lines` lines of the log, without access tokens.
    pub fn log_tail(&self, lines: usize) -> String {
        let text = std::fs::read(&self.log)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        let all: Vec<&str> = text.lines().collect();
        let tail = all[all.len().saturating_sub(lines)..].join("\n");
        crate::engine::timeline::redact_text(&tail)
    }

    /// How many log lines contain `needle`.
    pub fn log_count(&self, needle: &str) -> usize {
        std::fs::read(&self.log)
            .map(|bytes| {
                String::from_utf8_lossy(&bytes)
                    .lines()
                    .filter(|line| line.contains(needle))
                    .count()
            })
            .unwrap_or(0)
    }

    pub fn log_path(&self) -> &PathBuf {
        &self.log
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Ends the explorer and waits for the process to go.
    pub async fn stop(mut self) {
        let _ = self.command("/exit", Duration::from_secs(2)).await;
        let child = self.child.get_mut();
        if tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .is_err()
        {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        self.timeline.record(&self.name, "explorer stopped", ());
    }
}

/// The loopback port an explorer's console listens on: the one the scenario names, or one the
/// operating system says nothing listens on. An explorer that cannot bind its console port keeps
/// running without it, so a port chosen blind would fail the start only at its deadline.
fn console_port(options: &ExplorerOptions) -> Result<u16> {
    if let Some(port) = options.automation_port {
        return Ok(port);
    }
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .context("asking the operating system for a free loopback port")?;
    Ok(listener.local_addr()?.port())
}

/// A token for one explorer: random, from the operating system.
fn console_token() -> String {
    let key = dcl_auth::Wallet::random()
        .expect("the operating system gives entropy")
        .private_key_hex();
    key.trim_start_matches("0x")[..32].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explorer_is_not_given_a_console_port_something_listens_on() {
        let options = ExplorerOptions::default();
        let first = console_port(&options).unwrap();
        // something else on the machine takes that port before the explorer starts
        let _held = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, first)).unwrap();
        let second = console_port(&options).unwrap();
        assert_ne!(first, second, "the port in use was chosen again");
        assert!(
            std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, second)).is_ok(),
            "port {second} is in use"
        );
        // a port the scenario names is taken as it is
        let named = ExplorerOptions {
            automation_port: Some(first),
            ..ExplorerOptions::default()
        };
        assert_eq!(console_port(&named).unwrap(), first);
    }

    #[test]
    fn the_explorer_is_started_without_the_stacks_secrets() {
        let stack = Stack::parse(
            "api = \"http://127.0.0.1:1\"
pulse_enet = \"127.0.0.1:9\"
moderator_token_env = \"E2E_MODERATOR\"
scene_listener_key_env = \"E2E_SCENE\"
[livekit]
url = \"http://127.0.0.1:2\"
key_env = \"E2E_KEY\"
secret_env = \"E2E_SECRET\"
[explorer]
binary = \"explorer\"
api_front = \"127.0.0.1:3\"
pulse_front = \"127.0.0.1:4\"
",
        )
        .unwrap();
        let target = stack.explorer.as_ref().unwrap();
        let command = explorer_command(&stack, target, &ExplorerOptions::default(), 1234, "tok");
        let environment: std::collections::BTreeMap<String, Option<String>> = command
            .as_std()
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect();
        // `None` is an explicit removal from the inherited environment
        for variable in ["E2E_MODERATOR", "E2E_SCENE", "E2E_KEY", "E2E_SECRET"] {
            assert_eq!(environment.get(variable), Some(&None), "{variable}");
        }
        assert_eq!(
            environment.get("DCL_AUTOMATION_TOKEN"),
            Some(&Some("tok".to_owned()))
        );
    }
}
