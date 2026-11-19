//! LiveKit: a room joined through a child process, and the room service as the source of truth.
//!
//! The client is the explorer workspace's `harness-util join`, run as a child process: the LiveKit
//! client SDK (and the WebRTC library it brings) stays out of this workspace. The child prints one
//! JSON object per line for everything it observes; [`LiveKitView`] folds those lines into the
//! state a scenario looks at. [`RoomService`] asks the server itself who is in a room, through
//! LiveKit's Twirp HTTP API.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use dcl_livekit_token::{AccessToken, join_grants, room_admin_token};
use parking_lot::Mutex;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};

use crate::engine::timeline::redact_text;
use crate::engine::{Ctx, expect_or};
use crate::stack::{Relay, Stack};

/// The environment variable that carries the connection string to the child. A command line is
/// readable by other processes; the environment of a child is not.
const CONNECTION_VARIABLE: &str = "CE2E_LIVEKIT_CONNECTION";

/// How long `start` waits for the child to report that it is connected.
const CONNECT_DEADLINE: Duration = Duration::from_secs(45);

/// How long after the child has exited its output is waited for before the exit is reported
/// anyway (a grandchild can keep the pipe open).
const OUTPUT_GRACE: Duration = Duration::from_secs(1);

/// A track as a joined client sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub identity: String,
    pub sid: String,
    pub kind: String,
    pub source: String,
}

/// What a joined client has observed so far.
#[derive(Debug, Clone, Default)]
pub struct LiveKitView {
    pub connected: bool,
    pub room: Option<String>,
    pub identity: Option<String>,
    pub sid: Option<String>,
    /// The other participants now in the room, by identity.
    pub participants: BTreeMap<String, String>,
    pub tracks: Vec<Track>,
    /// How LiveKit names the reason the server or the client ended the connection, such as
    /// `DUPLICATE_IDENTITY`.
    pub disconnect_reason: Option<String>,
    /// How many times the client reconnected after losing its connection.
    pub reconnects: u32,
    pub errors: Vec<String>,
    /// The last connection state the client reported.
    pub state: Option<String>,
    /// Every participant arrival and departure, in the order the client saw them.
    pub arrivals: Vec<Arrival>,
    /// The exit code once the child has exited and printed everything it was going to.
    pub exited: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrival {
    pub joined: bool,
    pub identity: String,
    pub sid: String,
}

impl LiveKitView {
    /// Folds one line of the child's output into the view and returns the event it carried. A
    /// line that is not JSON, or that names no event, is ignored.
    pub fn apply_line(&mut self, line: &str) -> Option<(String, Value)> {
        let value: Value = serde_json::from_str(line.trim()).ok()?;
        let event = value.get("event")?.as_str()?.to_owned();
        self.apply(&event, &value);
        Some((event, value))
    }

    fn apply(&mut self, event: &str, value: &Value) {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        match event {
            "connected" => {
                self.connected = true;
                self.room = text("room");
                self.identity = text("identity");
                self.sid = text("sid");
                self.disconnect_reason = None;
            }
            "participant_connected" => {
                if let (Some(identity), Some(sid)) = (text("identity"), text("sid")) {
                    self.participants.insert(identity.clone(), sid.clone());
                    self.arrivals.push(Arrival {
                        joined: true,
                        identity,
                        sid,
                    });
                }
            }
            "participant_disconnected" => {
                if let (Some(identity), Some(sid)) = (text("identity"), text("sid")) {
                    // A newer session of the same identity can be announced before the older one
                    // is reported gone: only the session that left is forgotten.
                    if self.participants.get(&identity) == Some(&sid) {
                        self.participants.remove(&identity);
                        self.tracks.retain(|track| track.identity != identity);
                    }
                    self.arrivals.push(Arrival {
                        joined: false,
                        identity,
                        sid,
                    });
                }
            }
            "track_published" => {
                if let (Some(identity), Some(sid)) = (text("identity"), text("sid"))
                    && !self.tracks.iter().any(|track| track.sid == sid)
                {
                    self.tracks.push(Track {
                        identity,
                        sid,
                        kind: text("kind").unwrap_or_default(),
                        source: text("source").unwrap_or_default(),
                    });
                }
            }
            "track_unpublished" => {
                if let Some(sid) = text("sid") {
                    self.tracks.retain(|track| track.sid != sid);
                }
            }
            "connection_state" => self.state = text("state"),
            "reconnecting" => self.state = Some("reconnecting".to_owned()),
            "reconnected" => {
                self.reconnects += 1;
                self.state = Some("connected".to_owned());
            }
            "disconnected" => {
                self.connected = false;
                self.state = Some("disconnected".to_owned());
                self.disconnect_reason =
                    text("reason").or_else(|| Some("UNKNOWN_REASON".to_owned()));
            }
            "error" => self
                .errors
                .push(text("message").unwrap_or_else(|| "an error".to_owned())),
            _ => {}
        }
    }

    /// The tracks a participant has published with `source`.
    pub fn tracks_of(&self, identity: &str, source: &str) -> Vec<&Track> {
        self.tracks
            .iter()
            .filter(|track| track.identity == identity && track.source == source)
            .collect()
    }

    /// How many times `identity` was seen to join, and to leave.
    pub fn arrivals_of(&self, identity: &str) -> (usize, usize) {
        let joins = self
            .arrivals
            .iter()
            .filter(|a| a.identity == identity && a.joined)
            .count();
        let leaves = self
            .arrivals
            .iter()
            .filter(|a| a.identity == identity && !a.joined)
            .count();
        (joins, leaves)
    }
}

#[derive(Debug, Clone, Default)]
pub struct JoinOptions {
    /// Publish a silent track with the microphone source, so that track counts can be tested
    /// without a sound device.
    pub publish_microphone: bool,
    /// The client binary; `None` uses the stack file's `[livekit] client`.
    pub binary: Option<PathBuf>,
    /// Dial the address the connection string names, which is the fault link's front. Otherwise
    /// the front is rewritten to the real address.
    pub through_front: bool,
}

/// A connection string with the stack's front address for LiveKit replaced by the real one. A
/// string that names some other address is returned as it is.
pub fn to_real_address(connection: &str, relay: &Relay) -> String {
    let front = format!("//{}", relay.front);
    let real = format!("//{}", relay.real);
    match connection.split_once('?') {
        Some((address, query)) => format!("{}?{query}", address.replacen(&front, &real, 1)),
        None => connection.replacen(&front, &real, 1),
    }
}

/// The client's command line and environment: the connection string travels in the environment,
/// and none of the stack's secrets do.
fn join_command(
    stack: &Stack,
    binary: PathBuf,
    connection: &str,
    publish_microphone: bool,
) -> Command {
    let mut command = Command::new(binary);
    command
        .arg("join")
        .arg("--connection-string-env")
        .arg(CONNECTION_VARIABLE);
    if publish_microphone {
        command.arg("--publish-microphone");
    }
    command.env(CONNECTION_VARIABLE, connection);
    for variable in stack.secret_variables() {
        command.env_remove(variable);
    }
    command
}

struct Shared {
    name: String,
    ctx: Ctx,
    view: Mutex<LiveKitView>,
    stderr_tail: Mutex<Vec<String>>,
    /// The child's standard output has been read to its end.
    output_closed: AtomicBool,
    exit_seen: Mutex<Option<(Instant, i32)>>,
}

impl Shared {
    fn line(&self, line: &str) {
        let applied = self.view.lock().apply_line(line);
        if let Some((event, mut value)) = applied {
            if let Some(map) = value.as_object_mut() {
                map.remove("event");
            }
            self.ctx.record(&self.name, &event, value);
        }
    }
}

/// A client joined to a room, run as a child process.
pub struct LiveKitJoin {
    shared: Arc<Shared>,
    child: Mutex<Option<Child>>,
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
}

impl LiveKitJoin {
    /// Starts the client and waits until it reports that it is connected. A child that exits, or
    /// prints an error, before that fails the call with its message and leaves no process behind.
    pub async fn start(
        ctx: &Ctx,
        name: &str,
        connection: &str,
        options: JoinOptions,
    ) -> Result<LiveKitJoin> {
        let stack = ctx.stack();
        let binary = match options.binary.clone() {
            Some(binary) => binary,
            None => stack
                .livekit
                .as_ref()
                .and_then(|target| target.client.clone())
                .ok_or_else(|| {
                    anyhow!("no LiveKit client: set `client` under `[livekit]` in the stack file")
                })?,
        };
        let connection = match (&stack.explorer, options.through_front) {
            (Some(explorer), false) => match &explorer.livekit_signal {
                Some(relay) => to_real_address(connection, relay),
                None => connection.to_owned(),
            },
            _ => connection.to_owned(),
        };
        let command = join_command(stack, binary, &connection, options.publish_microphone);
        Self::start_command(ctx, name, command, CONNECT_DEADLINE).await
    }

    /// [`Self::start`] for a command already built.
    pub(crate) async fn start_command(
        ctx: &Ctx,
        name: &str,
        mut command: Command,
        deadline: Duration,
    ) -> Result<LiveKitJoin> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().context("starting the LiveKit client")?;
        let stdout = child
            .stdout
            .take()
            .context("the client has no standard output")?;
        let stderr = child
            .stderr
            .take()
            .context("the client has no standard error")?;
        let stdin = child.stdin.take();
        let shared = Arc::new(Shared {
            name: name.to_owned(),
            ctx: ctx.clone(),
            view: Mutex::new(LiveKitView::default()),
            stderr_tail: Mutex::new(Vec::new()),
            output_closed: AtomicBool::new(false),
            exit_seen: Mutex::new(None),
        });

        let reader = shared.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                reader.line(&line);
            }
            reader.output_closed.store(true, Ordering::SeqCst);
        });
        let stderr_reader = shared.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut tail = stderr_reader.stderr_tail.lock();
                tail.push(redact_text(&line));
                if tail.len() > 20 {
                    tail.remove(0);
                }
            }
        });

        let join = LiveKitJoin {
            shared,
            child: Mutex::new(Some(child)),
            stdin: tokio::sync::Mutex::new(stdin),
        };

        let outcome = expect_or(
            ctx,
            &format!("{name} is connected to the room"),
            deadline,
            || {
                {
                    let view = join.shared.view.lock();
                    if view.connected {
                        return Ok(());
                    }
                    if let Some(message) = view.errors.first() {
                        return Err(format!("fatal: {message}"));
                    }
                }
                match join.poll_exit() {
                    Some(code) => {
                        // an error line printed just before the exit has been read by now
                        if let Some(message) = join.shared.view.lock().errors.first() {
                            return Err(format!("fatal: {message}"));
                        }
                        Err(format!(
                            "fatal: the client exited with code {code} before it connected{}",
                            join.stderr_summary()
                        ))
                    }
                    None => Err("not connected yet".to_owned()),
                }
            },
        )
        .await;
        match outcome {
            Ok(()) => Ok(join),
            Err(error) => {
                join.kill();
                Err(error)
            }
        }
    }

    fn stderr_summary(&self) -> String {
        let tail = self.shared.stderr_tail.lock();
        if tail.is_empty() {
            String::new()
        } else {
            let last: Vec<String> = tail.iter().rev().take(3).rev().cloned().collect();
            format!(": {}", last.join(" | "))
        }
    }

    /// The exit code once the child has exited and its output has been read.
    fn poll_exit(&self) -> Option<i32> {
        if let Some(code) = self.shared.view.lock().exited {
            return Some(code);
        }
        let mut seen = self.shared.exit_seen.lock();
        if seen.is_none() {
            let status = self.child.lock().as_mut()?.try_wait().ok().flatten()?;
            *seen = Some((Instant::now(), status.code().unwrap_or(-1)));
        }
        let (at, code) = (*seen)?;
        if self.shared.output_closed.load(Ordering::SeqCst) || at.elapsed() >= OUTPUT_GRACE {
            drop(seen);
            self.shared.view.lock().exited = Some(code);
            self.shared
                .ctx
                .record(&self.shared.name, "process exited", code);
            return Some(code);
        }
        None
    }

    pub fn name(&self) -> &str {
        &self.shared.name
    }

    pub fn view(&self) -> LiveKitView {
        self.poll_exit();
        self.shared.view.lock().clone()
    }

    /// For a probe: ok while the client is connected. Its being disconnected is a fatal reason
    /// naming how LiveKit ended it, so the wait that asked stops at once.
    pub fn alive(&self) -> Result<(), String> {
        let view = self.view();
        if view.connected {
            return Ok(());
        }
        match (&view.disconnect_reason, view.exited) {
            (Some(reason), _) => Err(format!("fatal: the client was disconnected: {reason}")),
            (None, Some(code)) => Err(format!("fatal: the client exited with code {code}")),
            (None, None) => Err("fatal: the client is not connected".to_owned()),
        }
    }

    /// Asks the child to leave the room cleanly and waits for it to exit.
    pub async fn leave(&self) -> Result<()> {
        {
            let mut stdin = self.stdin.lock().await;
            if let Some(input) = stdin.as_mut() {
                let _ = input.write_all(b"leave\n").await;
                let _ = input.flush().await;
            }
            // closing the input leaves as well
            *stdin = None;
        }
        let outcome = expect_or(
            &self.shared.ctx,
            &format!("{} has left the room", self.shared.name),
            Duration::from_secs(15),
            || {
                self.poll_exit()
                    .map(|_| ())
                    .ok_or_else(|| "still running".to_owned())
            },
        )
        .await;
        if outcome.is_err() {
            self.kill();
        }
        outcome
    }

    /// Writes a line command to the child (`status` asks it to print the room as it sees it).
    pub async fn command(&self, line: &str) -> Result<()> {
        let mut stdin = self.stdin.lock().await;
        let input = stdin
            .as_mut()
            .ok_or_else(|| anyhow!("the client's input is closed"))?;
        input.write_all(format!("{line}\n").as_bytes()).await?;
        input.flush().await?;
        Ok(())
    }

    fn kill(&self) {
        if let Some(child) = self.child.lock().as_mut() {
            let _ = child.start_kill();
        }
    }
}

impl Drop for LiveKitJoin {
    fn drop(&mut self) {
        self.kill();
    }
}

/// A room as the server lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub name: String,
    pub sid: String,
    pub num_participants: u32,
}

/// A participant as the server lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    pub identity: String,
    pub sid: String,
    pub state: String,
    pub tracks: Vec<ParticipantTrack>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantTrack {
    pub sid: String,
    pub kind: String,
    pub source: String,
    pub muted: bool,
}

/// Where a LiveKit client dials for signalling, given the room service base: the same host and
/// port, over TLS when the base is.
fn signal_url(base: &url::Url) -> String {
    let scheme = if base.scheme() == "https" {
        "wss"
    } else {
        "ws"
    };
    // an IPv6 host already comes in its brackets
    let host = base.host_str().unwrap_or("127.0.0.1");
    let port = base.port_or_known_default().unwrap_or(80);
    format!("{scheme}://{host}:{port}")
}

/// The LiveKit server's own word on who is in a room.
pub struct RoomService {
    http: reqwest::Client,
    base: String,
    signal_url: String,
    key: String,
    secret: String,
}

impl RoomService {
    pub fn new(stack: &Stack) -> Result<RoomService> {
        let target = stack
            .livekit
            .as_ref()
            .ok_or_else(|| anyhow!("the stack file has no LiveKit (`livekit`)"))?;
        let key = stack.secret(&target.key_env)?;
        let secret = stack.secret(&target.secret_env)?;
        let base = target.url.as_str().trim_end_matches('/').to_owned();
        Ok(RoomService {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?,
            base,
            signal_url: signal_url(&target.url),
            key,
            secret,
        })
    }

    /// A connection string that joins `room` as `identity` at the real signalling address,
    /// without going through the gatekeeper.
    pub fn mint_join(&self, room: &str, identity: &str) -> Result<String> {
        let token = AccessToken::new(&self.key, &self.secret, identity, join_grants(room))
            .with_ttl(Duration::from_secs(600))
            .to_jwt()
            .map_err(|error| anyhow!("minting a join token: {error}"))?;
        Ok(format!("livekit:{}?access_token={token}", self.signal_url))
    }

    async fn call(&self, method: &str, room: &str, body: Value) -> Result<Value> {
        let token = room_admin_token(&self.key, &self.secret, room)
            .map_err(|error| anyhow!("minting a room service token: {error}"))?;
        let response = self
            .http
            .post(format!("{}/twirp/livekit.RoomService/{method}", self.base))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("calling {method}"))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .with_context(|| format!("reading the {method} answer"))?;
        if !status.is_success() {
            bail!(
                "{method} answered {status}: {}",
                text.chars().take(200).collect::<String>()
            );
        }
        serde_json::from_str(&text)
            .with_context(|| format!("{method} answered something that is not JSON"))
    }

    pub async fn rooms(&self) -> Result<Vec<Room>> {
        parse_rooms(&self.call("ListRooms", "", serde_json::json!({})).await?)
    }

    pub async fn participants(&self, room: &str) -> Result<Vec<Participant>> {
        parse_participants(
            &self
                .call(
                    "ListParticipants",
                    room,
                    serde_json::json!({ "room": room }),
                )
                .await?,
        )
    }
}

/// A field under either protobuf JSON spelling.
fn field<'a>(value: &'a Value, snake: &str, camel: &str) -> Option<&'a Value> {
    value.get(snake).or_else(|| value.get(camel))
}

fn string_field(value: &Value, name: &str) -> String {
    value
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// A protobuf JSON number, which a 64-bit integer carries as a string.
fn number_field(value: &Value, snake: &str, camel: &str) -> u64 {
    match field(value, snake, camel) {
        Some(Value::Number(number)) => number.as_u64().unwrap_or(0),
        Some(Value::String(text)) => text.parse().unwrap_or(0),
        _ => 0,
    }
}

/// An enum as protobuf JSON writes it: its name, or its number when names are not used.
fn enum_field(value: &Value, key: &str, names: &[&str], default: &str) -> String {
    match value.get(key) {
        Some(Value::String(text)) => text.to_ascii_lowercase(),
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|index| names.get(index as usize))
            .map(|text| text.to_ascii_lowercase())
            .unwrap_or_else(|| default.to_owned()),
        _ => default.to_owned(),
    }
}

pub fn parse_rooms(body: &Value) -> Result<Vec<Room>> {
    let Some(rooms) = body.get("rooms") else {
        // an empty list is omitted from the answer
        return Ok(Vec::new());
    };
    rooms
        .as_array()
        .ok_or_else(|| anyhow!("`rooms` is not a list"))?
        .iter()
        .map(|room| {
            let name = string_field(room, "name");
            if name.is_empty() {
                bail!("a room without a name");
            }
            Ok(Room {
                name,
                sid: string_field(room, "sid"),
                num_participants: number_field(room, "num_participants", "numParticipants") as u32,
            })
        })
        .collect()
}

pub fn parse_participants(body: &Value) -> Result<Vec<Participant>> {
    let Some(participants) = body.get("participants") else {
        return Ok(Vec::new());
    };
    participants
        .as_array()
        .ok_or_else(|| anyhow!("`participants` is not a list"))?
        .iter()
        .map(|participant| {
            let identity = string_field(participant, "identity");
            if identity.is_empty() {
                bail!("a participant without an identity");
            }
            let tracks = participant
                .get("tracks")
                .and_then(Value::as_array)
                .map(|tracks| {
                    tracks
                        .iter()
                        .map(|track| ParticipantTrack {
                            sid: string_field(track, "sid"),
                            kind: enum_field(track, "type", &["AUDIO", "VIDEO", "DATA"], "unknown"),
                            source: enum_field(
                                track,
                                "source",
                                &[
                                    "UNKNOWN",
                                    "CAMERA",
                                    "MICROPHONE",
                                    "SCREEN_SHARE",
                                    "SCREEN_SHARE_AUDIO",
                                ],
                                "unknown",
                            ),
                            muted: track.get("muted").and_then(Value::as_bool).unwrap_or(false),
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(Participant {
                identity,
                sid: string_field(participant, "sid"),
                state: enum_field(
                    participant,
                    "state",
                    &["JOINING", "JOINED", "ACTIVE", "DISCONNECTED"],
                    "joining",
                ),
                tracks,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn ctx() -> Ctx {
        Ctx::new(
            Arc::new(Stack::pulse_only("127.0.0.1:9".parse().unwrap())),
            "unit",
            "livekit",
            0,
            1,
            std::env::temp_dir().join("comms-e2e-unit"),
        )
    }

    #[test]
    fn the_signalling_address_follows_the_room_service_base() {
        let signal = |base: &str| signal_url(&url::Url::parse(base).unwrap());
        assert_eq!(signal("http://127.0.0.1:7880"), "ws://127.0.0.1:7880");
        assert_eq!(signal("http://livekit.example/"), "ws://livekit.example:80");
        // an IPv6 host keeps its one pair of brackets
        assert_eq!(signal("http://[::1]:7880/"), "ws://[::1]:7880");
        // a base over TLS is dialled over TLS
        assert_eq!(
            signal("https://livekit.example"),
            "wss://livekit.example:443"
        );
    }

    #[test]
    fn the_client_is_started_without_the_stacks_secrets() {
        let stack = Stack::parse(
            "api = \"http://127.0.0.1:1\"\npulse_enet = \"127.0.0.1:9\"\nmoderator_token_env = \"E2E_MODERATOR\"\nscene_listener_key_env = \"E2E_SCENE\"\n[livekit]\nurl = \"http://127.0.0.1:2\"\nkey_env = \"E2E_KEY\"\nsecret_env = \"E2E_SECRET\"\n",
        )
        .unwrap();
        let command = join_command(&stack, PathBuf::from("client"), "wss://x/rtc", false);
        let environment: BTreeMap<String, Option<String>> = command
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
            environment.get(CONNECTION_VARIABLE),
            Some(&Some("wss://x/rtc".to_owned()))
        );
    }

    fn view_of(lines: &[&str]) -> LiveKitView {
        let mut view = LiveKitView::default();
        for line in lines {
            view.apply_line(line);
        }
        view
    }

    #[test]
    fn a_join_and_a_track_fill_the_view() {
        let view = view_of(&[
            r#"{"event":"connected","room":"r","identity":"me","sid":"PA_me"}"#,
            r#"{"event":"participant_connected","identity":"you","sid":"PA_you"}"#,
            r#"{"event":"track_published","identity":"you","sid":"TR_1","kind":"audio","source":"microphone"}"#,
            r#"{"event":"track_subscribed","identity":"you","sid":"TR_1","kind":"audio","source":"microphone"}"#,
            "not json at all",
            r#"{"no_event":true}"#,
        ]);
        assert!(view.connected);
        assert_eq!(view.sid.as_deref(), Some("PA_me"));
        assert_eq!(
            view.participants.get("you").map(String::as_str),
            Some("PA_you")
        );
        assert_eq!(view.tracks_of("you", "microphone").len(), 1);
        assert_eq!(view.tracks.len(), 1, "a subscription is not a second track");
    }

    #[test]
    fn a_duplicate_identity_disconnect_is_told_apart() {
        let view = view_of(&[
            r#"{"event":"connected","room":"r","identity":"me","sid":"PA_1"}"#,
            r#"{"event":"disconnected","reason":"DUPLICATE_IDENTITY"}"#,
        ]);
        assert!(!view.connected);
        assert_eq!(
            view.disconnect_reason.as_deref(),
            Some("DUPLICATE_IDENTITY")
        );
    }

    #[test]
    fn a_reconnect_is_counted_and_the_client_stays_connected() {
        let view = view_of(&[
            r#"{"event":"connected","room":"r","identity":"me","sid":"PA_1"}"#,
            r#"{"event":"connection_state","state":"reconnecting"}"#,
            r#"{"event":"reconnecting"}"#,
            r#"{"event":"reconnected"}"#,
            r#"{"event":"connection_state","state":"connected"}"#,
        ]);
        assert!(view.connected);
        assert_eq!(view.reconnects, 1);
        assert_eq!(view.state.as_deref(), Some("connected"));
        assert_eq!(view.disconnect_reason, None);
    }

    #[test]
    fn a_replacement_session_survives_the_old_one_being_reported_gone_after_it() {
        // the new session is announced before the old one leaves
        let view = view_of(&[
            r#"{"event":"participant_connected","identity":"you","sid":"PA_old"}"#,
            r#"{"event":"participant_connected","identity":"you","sid":"PA_new"}"#,
            r#"{"event":"participant_disconnected","identity":"you","sid":"PA_old"}"#,
        ]);
        assert_eq!(
            view.participants.get("you").map(String::as_str),
            Some("PA_new")
        );
        assert_eq!(view.arrivals_of("you"), (2, 1));
        // and the other order
        let view = view_of(&[
            r#"{"event":"participant_connected","identity":"you","sid":"PA_old"}"#,
            r#"{"event":"participant_disconnected","identity":"you","sid":"PA_old"}"#,
            r#"{"event":"participant_connected","identity":"you","sid":"PA_new"}"#,
        ]);
        assert_eq!(
            view.participants.get("you").map(String::as_str),
            Some("PA_new")
        );
        assert_eq!(view.participants.len(), 1);
        assert_eq!(view.arrivals_of("you"), (2, 1));
    }

    #[test]
    fn a_departure_takes_the_tracks_with_it() {
        let view = view_of(&[
            r#"{"event":"participant_connected","identity":"you","sid":"PA_1"}"#,
            r#"{"event":"track_published","identity":"you","sid":"TR_1","kind":"audio","source":"microphone"}"#,
            r#"{"event":"participant_disconnected","identity":"you","sid":"PA_1"}"#,
        ]);
        assert!(view.participants.is_empty());
        assert!(view.tracks.is_empty());
        let view = view_of(&[
            r#"{"event":"track_published","identity":"you","sid":"TR_1","kind":"audio","source":"microphone"}"#,
            r#"{"event":"track_unpublished","identity":"you","sid":"TR_1","kind":"audio","source":"microphone"}"#,
        ]);
        assert!(view.tracks.is_empty());
    }

    #[test]
    fn an_error_line_is_kept() {
        let view = view_of(&[r#"{"event":"error","message":"connect failed: boom"}"#]);
        assert_eq!(view.errors, vec!["connect failed: boom".to_owned()]);
        assert!(!view.connected);
    }

    #[test]
    fn the_front_address_is_rewritten_to_the_real_one() {
        let relay = Relay {
            front: "127.0.0.1:6214".parse::<SocketAddr>().unwrap(),
            real: "127.0.0.1:5214".parse::<SocketAddr>().unwrap(),
        };
        assert_eq!(
            to_real_address("livekit:ws://127.0.0.1:6214?access_token=a.b.c", &relay),
            "livekit:ws://127.0.0.1:5214?access_token=a.b.c"
        );
        assert_eq!(
            to_real_address("livekit:ws://127.0.0.1:6214/rtc?access_token=a.b.c", &relay),
            "livekit:ws://127.0.0.1:5214/rtc?access_token=a.b.c"
        );
        // another address is left alone, and so is a token that happens to contain the text
        assert_eq!(
            to_real_address(
                "livekit:ws://10.0.0.1:6214?access_token=//127.0.0.1:6214",
                &relay
            ),
            "livekit:ws://10.0.0.1:6214?access_token=//127.0.0.1:6214"
        );
    }

    #[test]
    fn a_room_list_is_parsed_under_either_spelling() {
        let rooms = parse_rooms(&serde_json::json!({"rooms":[
            {"sid":"RM_a","name":"alpha","empty_timeout":300,"num_participants":2,"creation_time":"1700000000"},
            {"sid":"RM_b","name":"beta","numParticipants":0}
        ]}))
        .unwrap();
        assert_eq!(
            rooms,
            vec![
                Room {
                    name: "alpha".into(),
                    sid: "RM_a".into(),
                    num_participants: 2
                },
                Room {
                    name: "beta".into(),
                    sid: "RM_b".into(),
                    num_participants: 0
                },
            ]
        );
        assert!(parse_rooms(&serde_json::json!({})).unwrap().is_empty());
        assert!(parse_rooms(&serde_json::json!({"rooms":[{"sid":"RM_a"}]})).is_err());
        assert!(parse_rooms(&serde_json::json!({"rooms":"x"})).is_err());
    }

    #[test]
    fn participants_and_their_tracks_are_parsed() {
        let body = serde_json::json!({"participants":[
            {"sid":"PA_1","identity":"0xabc","state":"ACTIVE","joined_at":"1700000000",
             "tracks":[
                {"sid":"TR_1","type":"AUDIO","source":"MICROPHONE","muted":false,"name":"microphone"},
                {"sid":"TR_2","type":"VIDEO","source":"CAMERA","muted":true}
             ]},
            {"sid":"PA_2","identity":"observer","state":1,"tracks":[]},
            {"sid":"PA_3","identity":"joining"}
        ]});
        let participants = parse_participants(&body).unwrap();
        assert_eq!(participants.len(), 3);
        assert_eq!(participants[0].identity, "0xabc");
        assert_eq!(participants[0].state, "active");
        assert_eq!(
            participants[0].tracks,
            vec![
                ParticipantTrack {
                    sid: "TR_1".into(),
                    kind: "audio".into(),
                    source: "microphone".into(),
                    muted: false
                },
                ParticipantTrack {
                    sid: "TR_2".into(),
                    kind: "video".into(),
                    source: "camera".into(),
                    muted: true
                },
            ]
        );
        assert_eq!(participants[1].state, "joined");
        assert_eq!(participants[2].state, "joining");
        assert!(participants[2].tracks.is_empty());
        assert!(
            parse_participants(&serde_json::json!({}))
                .unwrap()
                .is_empty()
        );
        assert!(parse_participants(&serde_json::json!({"participants":[{"sid":"x"}]})).is_err());
    }

    /// A stand-in client: a shell running `script`.
    fn stand_in(script: &str) -> Command {
        if cfg!(windows) {
            let mut command = Command::new("cmd");
            #[cfg(windows)]
            command.as_std_mut().raw_arg(format!("/C \"{script}\""));
            command
        } else {
            let mut command = Command::new("sh");
            command.arg("-c").arg(script);
            command
        }
    }

    #[cfg(windows)]
    use std::os::windows::process::CommandExt as _;

    const CONNECTED: &str = r#"{"event":"connected","room":"r","identity":"me","sid":"PA_1"}"#;

    /// Prints `lines` then waits for a long time.
    fn printing_then_idle(lines: &[&str]) -> Command {
        if cfg!(windows) {
            let echoes: Vec<String> = lines.iter().map(|line| format!("echo {line}")).collect();
            stand_in(&format!(
                "{} & ping -n 6 127.0.0.1 > nul",
                echoes.join(" & ")
            ))
        } else {
            let echoes: Vec<String> = lines.iter().map(|line| format!("echo '{line}'")).collect();
            stand_in(&format!("{}; sleep 6", echoes.join("; ")))
        }
    }

    #[tokio::test]
    async fn a_child_that_exits_at_once_fails_the_start() {
        let command = if cfg!(windows) {
            stand_in("echo broken 1>&2 & exit 3")
        } else {
            stand_in("echo broken >&2; exit 3")
        };
        let error = match LiveKitJoin::start_command(
            &ctx(),
            "gone",
            command,
            Duration::from_secs(10),
        )
        .await
        {
            Ok(_) => panic!("a child that exited cannot be joined"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("exited with code 3"), "{error}");
        assert!(
            error.contains("broken"),
            "the child's own words are kept: {error}"
        );
    }

    #[tokio::test]
    async fn a_child_that_prints_an_error_fails_the_start_with_its_message() {
        // the stand-in stays alive after the error line: the start must not wait for it to exit
        let command =
            printing_then_idle(&[r#"{"event":"error","message":"connect failed: refused"}"#]);
        let started = Instant::now();
        let error =
            match LiveKitJoin::start_command(&ctx(), "refused", command, Duration::from_secs(10))
                .await
            {
                Ok(_) => panic!("an error line cannot be a join"),
                Err(error) => error.to_string(),
            };
        assert!(error.contains("connect failed: refused"), "{error}");
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "the wait ended on the error line"
        );
    }

    #[tokio::test]
    async fn a_connected_child_is_viewed_and_leaves_with_its_input() {
        let ctx = ctx();
        let command = if cfg!(windows) {
            stand_in(&format!(
                "echo {CONNECTED} & echo {{\"event\":\"participant_connected\",\"identity\":\"you\",\"sid\":\"PA_2\"}} & more > nul"
            ))
        } else {
            stand_in(&format!(
                "echo '{CONNECTED}'; echo '{{\"event\":\"participant_connected\",\"identity\":\"you\",\"sid\":\"PA_2\"}}'; cat >/dev/null"
            ))
        };
        let join = LiveKitJoin::start_command(&ctx, "kept", command, Duration::from_secs(10))
            .await
            .unwrap();
        join.alive().unwrap();
        expect_or(
            &ctx,
            "the other participant is seen",
            Duration::from_secs(5),
            || {
                join.view()
                    .participants
                    .get("you")
                    .map(|_| ())
                    .ok_or_else(String::new)
            },
        )
        .await
        .unwrap();
        assert_eq!(ctx.timeline().count("kept", "connected"), 1);
        join.leave().await.unwrap();
        assert!(join.view().exited.is_some());
    }

    #[tokio::test]
    async fn dropping_the_join_kills_the_child() {
        let ctx = ctx();
        let command = printing_then_idle(&[CONNECTED]);
        let join = LiveKitJoin::start_command(&ctx, "doomed", command, Duration::from_secs(10))
            .await
            .unwrap();
        let pid = join
            .child
            .lock()
            .as_ref()
            .and_then(Child::id)
            .expect("a running child has an id");
        assert!(process_exists(pid));
        drop(join);
        expect_or(&ctx, "the child is gone", Duration::from_secs(10), || {
            if process_exists(pid) {
                Err("still running".to_owned())
            } else {
                Ok(())
            }
        })
        .await
        .unwrap();
    }

    fn process_exists(pid: u32) -> bool {
        if cfg!(windows) {
            let output = std::process::Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/NH"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&output.stdout).contains(&format!(" {pid} "))
        } else {
            std::path::Path::new(&format!("/proc/{pid}")).exists()
        }
    }
}
