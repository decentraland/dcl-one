//! A Pulse bot: it connects, signs the handshake, sends whatever a scenario tells it to (valid or
//! not) and keeps a view of what the server told it. It has no policy of its own: no reconnect,
//! no pacing, no backoff. Those belong to the explorer and are tested through the explorer.

pub mod enet;
pub mod messages;
pub mod view;
pub mod webtransport;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use catalyst_comms::pulse::decentraland::pulse::{ClientMessage, PlayerInitialState};
use parking_lot::Mutex;
use prost::Message as _;

use crate::engine::{Ctx, Timeline, expect_or};
use crate::identity::{Identity, now_ms};
use crate::stack::{Grid, WebTransportTarget};
use view::Session;
pub use view::{PulseView, Subject};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Reliable,
    UnreliableSequenced,
    UnreliableUnsequenced,
}

impl Channel {
    pub fn id(self) -> u8 {
        match self {
            Channel::Reliable => 0,
            Channel::UnreliableSequenced => 1,
            Channel::UnreliableUnsequenced => 2,
        }
    }
}

pub(crate) enum Command {
    Send { channel: Channel, bytes: Vec<u8> },
    Disconnect { code: u32 },
    Stop,
}

/// What the transport thread and the bot share.
pub(crate) struct Shared {
    pub name: String,
    session: Mutex<Session>,
    timeline: Timeline,
    /// Received frames are recorded on the timeline only while this is on: a soak would drown it.
    record_frames: bool,
}

impl Shared {
    pub(crate) fn record(&self, kind: &str, detail: impl serde::Serialize) {
        self.timeline.record(&self.name, kind, detail);
    }

    pub(crate) fn connected(&self) {
        self.session.lock().view.connected = true;
        self.record("transport connected", ());
    }

    pub(crate) fn disconnected(&self, code: u32) {
        let mut session = self.session.lock();
        session.view.connected = false;
        session.view.disconnect = Some(code);
        drop(session);
        self.record("disconnected", messages::reason(code));
    }

    pub(crate) fn closed_locally(&self) {
        let mut session = self.session.lock();
        session.view.connected = false;
        drop(session);
        self.record("closed", ());
    }

    pub(crate) fn transport_failed(&self, why: String) {
        let mut session = self.session.lock();
        session.view.connected = false;
        session.view.disconnect.get_or_insert(0);
        drop(session);
        self.record("transport failed", why);
    }

    pub(crate) fn socket_error(&self, error: &std::io::Error) {
        self.record("socket error", error.to_string());
    }

    /// Takes one received frame; returns what to send back at once.
    pub(crate) fn received(&self, bytes: &[u8]) -> Vec<(Channel, Vec<u8>)> {
        let seen = self.session.lock().on_frame(bytes);
        if self.record_frames || !matches!(seen.kind, "delta" | "full state") {
            self.record(seen.kind, &seen.detail);
        }
        seen.replies
            .into_iter()
            .map(|(channel, message)| {
                self.record("resync requested", ());
                (channel, message.encode_to_vec())
            })
            .collect()
    }
}

const DEFAULT_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default)]
pub struct ConnectOptions {
    /// The local address to dial from. The servers count their per-address limits by it.
    pub bind: Option<IpAddr>,
    /// Do not answer a sequence gap with a resync request.
    pub no_auto_resync: bool,
    /// Record every delta and full state on the timeline.
    pub record_frames: bool,
    /// How long the transport may take to connect. Absent: 10 s.
    pub deadline: Option<Duration>,
    /// Dial this address instead of the one the stack file names: a fault link in front of Pulse.
    pub via: Option<SocketAddr>,
}

#[derive(Debug, Clone, Default)]
pub struct HandshakeOptions {
    pub initial_state: Option<PlayerInitialState>,
    /// The timestamp to sign. Absent: now. A scenario sets it to replay or to age a handshake.
    pub timestamp_ms: Option<i64>,
    /// How long the server may take to answer. Absent: 10 s.
    pub deadline: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeOutcome {
    Accepted,
    /// The server answered with `success: false`.
    Refused {
        error: Option<String>,
    },
    /// The server ended the connection instead of answering.
    Disconnected {
        code: u32,
    },
}

pub struct PulseBot {
    shared: Arc<Shared>,
    commands: Sender<Command>,
    grid: Grid,
    local: SocketAddr,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PulseBot {
    /// Dials Pulse over ENet and waits for the transport to connect.
    pub async fn connect(
        ctx: &Ctx,
        name: &str,
        target: SocketAddr,
        options: ConnectOptions,
    ) -> Result<PulseBot> {
        let (shared, receiver, commands) = PulseBot::prepare(ctx, name, &options);
        shared.record("dialing", target.to_string());
        let (local, thread) = enet::spawn(target, options.bind, shared.clone(), receiver)?;
        let bot = PulseBot::assemble(ctx, shared, commands, local, thread);
        bot.connected(
            ctx,
            &format!("{name} connects to Pulse at {target}"),
            &options,
        )
        .await?;
        Ok(bot)
    }

    /// Dials Pulse over WebTransport and waits for the session to open. The URL's host and port
    /// are replaced by `options.via` when a fault link is in front; the certificate hash stays.
    /// `options.bind` is the local address the QUIC endpoint binds to; it needs the certificate
    /// hash of the target.
    pub async fn connect_webtransport(
        ctx: &Ctx,
        name: &str,
        target: &WebTransportTarget,
        options: ConnectOptions,
    ) -> Result<PulseBot> {
        if let (Some(bind), None) = (options.bind, &target.certificate_hash) {
            bail!(
                "{name}: the WebTransport client binds {bind} only with a certificate hash in the stack file"
            );
        }
        let mut url = target.url.clone();
        if let Some(via) = options.via {
            url.set_ip_host(via.ip())
                .map_err(|()| anyhow!("{name}: {via} cannot be the host of {url}"))?;
            url.set_port(Some(via.port()))
                .map_err(|()| anyhow!("{name}: {via} cannot be the port of {url}"))?;
        }
        let certificate_hash = target
            .certificate_hash
            .as_deref()
            .map(hex::decode)
            .transpose()
            .map_err(|error| anyhow!("{name}: the certificate hash is not hex: {error}"))?;
        let (shared, receiver, commands) = PulseBot::prepare(ctx, name, &options);
        shared.record("dialing", url.to_string());
        let deadline = options.deadline.unwrap_or(DEFAULT_DEADLINE);
        let thread = webtransport::spawn(
            webtransport::Dial {
                url: url.to_string(),
                certificate_hash,
                local: options.bind,
                deadline,
            },
            shared.clone(),
            receiver,
        )?;
        // No local address to report: the unspecified one, as for a socket not yet bound.
        let local = SocketAddr::from(([0, 0, 0, 0], 0));
        let bot = PulseBot::assemble(ctx, shared, commands, local, thread);
        bot.connected(ctx, &format!("{name} connects to Pulse at {url}"), &options)
            .await?;
        Ok(bot)
    }

    fn prepare(
        ctx: &Ctx,
        name: &str,
        options: &ConnectOptions,
    ) -> (
        Arc<Shared>,
        std::sync::mpsc::Receiver<Command>,
        Sender<Command>,
    ) {
        let shared = Arc::new(Shared {
            name: name.to_owned(),
            session: Mutex::new(Session::new(ctx.stack().grid, !options.no_auto_resync)),
            timeline: ctx.timeline().clone(),
            record_frames: options.record_frames,
        });
        let (commands, receiver) = std::sync::mpsc::channel();
        (shared, receiver, commands)
    }

    fn assemble(
        ctx: &Ctx,
        shared: Arc<Shared>,
        commands: Sender<Command>,
        local: SocketAddr,
        thread: std::thread::JoinHandle<()>,
    ) -> PulseBot {
        PulseBot {
            shared,
            commands,
            grid: ctx.stack().grid,
            local,
            thread: Some(thread),
        }
    }

    async fn connected(&self, ctx: &Ctx, what: &str, options: &ConnectOptions) -> Result<()> {
        let deadline = options.deadline.unwrap_or(DEFAULT_DEADLINE);
        expect_or(ctx, what, deadline, || {
            let view = self.view();
            match (view.connected, view.disconnect) {
                (true, _) => Ok(()),
                (false, Some(code)) => Err(format!(
                    "fatal: the connection ended with {}",
                    messages::reason(code)
                )),
                (false, None) => Err("still connecting".to_owned()),
            }
        })
        .await
    }

    /// Dials Pulse on the transport of the run, at the address the stack file names or at
    /// `options.via` (a fault link in front of it), and waits for the transport to connect.
    /// Scenarios use this, so one scenario runs on either transport.
    pub async fn dial(ctx: &Ctx, name: &str, mut options: ConnectOptions) -> Result<PulseBot> {
        // Unless the scenario chose an address, dial from the one this scenario was given. On
        // WebTransport a chosen address needs the pinned certificate, so without one the bot
        // dials from the ordinary address.
        if options.bind.is_none() {
            let pinned = ctx
                .stack()
                .pulse_webtransport
                .as_ref()
                .is_some_and(|target| target.certificate_hash.is_some());
            if ctx.transport() == crate::engine::Transport::Enet || pinned {
                options.bind = ctx.source_address();
            }
        }
        match ctx.transport() {
            crate::engine::Transport::Enet => {
                let target = options.via.unwrap_or(ctx.stack().pulse_enet);
                PulseBot::connect(ctx, name, target, options).await
            }
            crate::engine::Transport::WebTransport => {
                let target = ctx.stack().pulse_webtransport.as_ref().ok_or_else(|| {
                    anyhow!("{name}: the stack file names no Pulse WebTransport endpoint")
                })?;
                PulseBot::connect_webtransport(ctx, name, target, options).await
            }
        }
    }

    /// [`Self::dial`], then the handshake; fails unless the server accepts.
    pub async fn enter(ctx: &Ctx, name: &str, identity: &Identity) -> Result<PulseBot> {
        let bot = PulseBot::dial(ctx, name, ConnectOptions::default()).await?;
        bot.accepted(ctx, identity).await?;
        Ok(bot)
    }

    /// Handshakes and fails unless the server accepts.
    pub async fn accepted(&self, ctx: &Ctx, identity: &Identity) -> Result<()> {
        let name = self.name();
        match self
            .handshake(ctx, identity, HandshakeOptions::default())
            .await?
        {
            HandshakeOutcome::Accepted => Ok(()),
            HandshakeOutcome::Refused { error } => {
                bail!("{name}: the server refused the handshake: {error:?}")
            }
            HandshakeOutcome::Disconnected { code } => {
                bail!(
                    "{name}: the server ended the connection at the handshake with {}",
                    messages::reason(code)
                )
            }
        }
    }

    pub fn name(&self) -> &str {
        &self.shared.name
    }

    /// The local address the bot dials from. Over WebTransport the client picks its own socket
    /// and this is the unspecified address, `0.0.0.0:0`.
    pub fn local_address(&self) -> SocketAddr {
        self.local
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Sends the signed handshake and waits for the server's answer.
    pub async fn handshake(
        &self,
        ctx: &Ctx,
        identity: &Identity,
        options: HandshakeOptions,
    ) -> Result<HandshakeOutcome> {
        self.send_handshake(identity, &options);
        self.handshake_outcome(ctx, options.deadline).await
    }

    /// Sends the signed handshake without waiting: for scenarios that race several.
    pub fn send_handshake(&self, identity: &Identity, options: &HandshakeOptions) {
        let timestamp = options.timestamp_ms.unwrap_or_else(now_ms);
        {
            let mut session = self.shared.session.lock();
            session.own_wallet = Some(identity.address());
            session.view.handshake = None;
        }
        self.shared.record(
            "handshake sent",
            serde_json::json!({ "wallet": identity.address(), "session": identity.session(), "timestamp": timestamp }),
        );
        self.send(
            identity.pulse_handshake(timestamp, options.initial_state.clone()),
            Channel::Reliable,
        );
    }

    /// Sends a scene listener's signed handshake without waiting; [`Self::handshake_outcome`]
    /// reads the answer.
    pub fn send_listener_handshake(
        &self,
        identity: &Identity,
        area_of_interest: Vec<catalyst_comms::pulse::decentraland::pulse::SceneListenerAoi>,
    ) {
        {
            let mut session = self.shared.session.lock();
            session.own_wallet = Some(identity.address());
            session.view.handshake = None;
        }
        self.shared.record(
            "listener handshake sent",
            serde_json::json!({ "wallet": identity.address(), "areas": area_of_interest.len() }),
        );
        self.send(
            identity.pulse_listener_handshake(now_ms(), area_of_interest),
            Channel::Reliable,
        );
    }

    /// Waits for the answer to a handshake already sent.
    pub async fn handshake_outcome(
        &self,
        ctx: &Ctx,
        deadline: Option<Duration>,
    ) -> Result<HandshakeOutcome> {
        let name = self.name().to_owned();
        expect_or(
            ctx,
            &format!("{name} gets an answer to its handshake"),
            deadline.unwrap_or(Duration::from_secs(10)),
            || {
                let view = self.view();
                match (view.handshake, view.disconnect) {
                    (Some(true), _) => Ok(HandshakeOutcome::Accepted),
                    (Some(false), _) => Ok(HandshakeOutcome::Refused {
                        error: view.handshake_error,
                    }),
                    (None, Some(code)) => Ok(HandshakeOutcome::Disconnected { code }),
                    (None, None) => Err("no answer yet".to_owned()),
                }
            },
        )
        .await
    }

    /// Connects, handshakes and fails unless the server accepts.
    pub async fn join(
        ctx: &Ctx,
        name: &str,
        target: SocketAddr,
        identity: &Identity,
    ) -> Result<PulseBot> {
        let bot = PulseBot::connect(ctx, name, target, ConnectOptions::default()).await?;
        match bot
            .handshake(ctx, identity, HandshakeOptions::default())
            .await?
        {
            HandshakeOutcome::Accepted => Ok(bot),
            HandshakeOutcome::Refused { error } => {
                bail!("{name}: the server refused the handshake: {error:?}")
            }
            HandshakeOutcome::Disconnected { code } => {
                bail!(
                    "{name}: the server ended the connection at the handshake with {}",
                    messages::reason(code)
                )
            }
        }
    }

    pub fn send(&self, message: ClientMessage, channel: Channel) {
        self.send_raw(message.encode_to_vec(), channel);
    }

    /// Sends bytes as they are: the way to send a frame that is not a message at all.
    pub fn send_raw(&self, bytes: Vec<u8>, channel: Channel) {
        let _ = self.commands.send(Command::Send { channel, bytes });
    }

    /// Announces the realm and the position, as a client does on entering a realm.
    pub fn teleport(&self, realm: &str, world: [f32; 3]) -> Result<()> {
        let message = messages::teleport(&self.grid, realm, world)
            .ok_or_else(|| anyhow!("{world:?} is outside the parcel grid"))?;
        self.shared.record(
            "teleport sent",
            serde_json::json!({ "realm": realm, "world": world }),
        );
        self.send(message, Channel::Reliable);
        Ok(())
    }

    /// One movement input at `world`.
    pub fn move_to(&self, world: [f32; 3]) -> Result<()> {
        let message = messages::input_at(&self.grid, world)
            .ok_or_else(|| anyhow!("{world:?} is outside the parcel grid"))?;
        self.send(message, Channel::UnreliableSequenced);
        Ok(())
    }

    pub fn view(&self) -> PulseView {
        self.shared.session.lock().view.clone()
    }

    /// Fails at once, naming the code, if the server has ended the connection.
    pub fn expect_alive(&self) -> Result<()> {
        self.alive()
            .map_err(|reason| anyhow!("{}: {}", self.name(), reason.trim_start_matches("fatal: ")))
    }

    /// [`Self::expect_alive`] for a probe: a dead connection is a fatal reason, so the wait that
    /// asked ends at once instead of at its deadline.
    pub fn alive(&self) -> Result<(), String> {
        match self.view().disconnect {
            Some(code) => Err(format!(
                "fatal: the server ended the connection with {}",
                messages::reason(code)
            )),
            None => Ok(()),
        }
    }

    /// Waits for the connection to end and returns the code.
    pub async fn expect_disconnect(&self, ctx: &Ctx, deadline: Duration) -> Result<u32> {
        let name = self.name().to_owned();
        expect_or(
            ctx,
            &format!("{name} is disconnected by the server"),
            deadline,
            || {
                self.view()
                    .disconnect
                    .ok_or_else(|| "still connected".to_owned())
            },
        )
        .await
    }

    /// Waits for the connection to end with exactly `expected`.
    pub async fn expect_disconnect_with(
        &self,
        ctx: &Ctx,
        expected: catalyst_comms::pulse::hardening::DisconnectReason,
        deadline: Duration,
    ) -> Result<()> {
        let code = self.expect_disconnect(ctx, deadline).await?;
        if code != expected.code() {
            bail!(
                "{}: expected a disconnect with {}, got {}",
                self.name(),
                messages::reason(expected.code()),
                messages::reason(code)
            );
        }
        Ok(())
    }

    /// Asks the server to end the connection with `code` and returns without waiting.
    pub fn disconnect(&self, code: u32) {
        self.shared
            .record("disconnect sent", messages::reason(code));
        let _ = self.commands.send(Command::Disconnect { code });
    }

    /// Ends the connection now and waits for the transport thread to finish.
    pub fn close(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for PulseBot {
    fn drop(&mut self) {
        self.stop();
    }
}
