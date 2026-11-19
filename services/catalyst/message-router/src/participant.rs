//! The router in the room, as upstream's `adapters/livekit.ts`: connect with a fresh token
//! (`connection::Retry`, upstream's five attempts one second apart), route every data packet
//! (`routing`, at most `max_in_flight` at once, `in_flight`), and after a final disconnect
//! connect again when `connection::reconnects` says so, or else exit non-zero for the supervisor
//! to start the process again. The SDK resumes a dropped signal connection by itself (`Reconnecting`, `Reconnected`);
//! only its `Disconnected` is final.
//!
//! The sender is the LiveKit-authenticated identity the SFU stamps on the packet: the remote
//! participant's, or, for data that arrives before the participant's announcement, the
//! `participant_identity` the patched SDK carries.

use std::sync::Arc;
use std::time::Duration;

use livekit::prelude::*;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::connection::{Reason, Retry, reconnects};
use crate::in_flight::InFlight;
use crate::membership::PgMembership;
use crate::metrics::{self, Connection};
use crate::routing::{self, Outcome, Refused};
use crate::shared::limits::{RateLimitDecision, RateLimiter};
use crate::token::TokenClient;

/// How long a shutdown lets the packets being routed finish before it aborts them: one
/// publish's bound.
const DRAIN: Duration = routing::PUBLISH_TIMEOUT;

pub struct Router {
    pub tokens: TokenClient,
    /// Dialed instead of the URL the gatekeeper hands out.
    pub livekit_url: Option<String>,
    pub retry: Retry,
    pub max_packet_size_bytes: usize,
    /// Packets routed at once (`config::Cli::max_in_flight`, the pool's size); an admitted
    /// packet beyond it is dropped and counted `overloaded`.
    pub max_in_flight: usize,
    pub members: Arc<PgMembership>,
    pub limiter: Arc<RateLimiter>,
    pub connection: Connection,
}
struct LiveRoom(Room);

#[async_trait::async_trait]
impl routing::Room for LiveRoom {
    fn connected(&self) -> bool {
        self.0.connection_state() == ConnectionState::Connected
    }

    async fn publish(
        &self,
        payload: Vec<u8>,
        topic: &str,
        destinations: Vec<String>,
    ) -> Result<(), String> {
        self.0
            .local_participant()
            .publish_data(DataPacket {
                payload,
                topic: Some(topic.to_owned()),
                reliable: true,
                destination_identities: destinations.into_iter().map(ParticipantIdentity).collect(),
            })
            .await
            .map_err(|e| e.to_string())
    }
}

impl Router {
    async fn connect(
        &self,
    ) -> anyhow::Result<(Arc<LiveRoom>, UnboundedReceiver<RoomEvent>, String)> {
        self.retry
            .run(|attempt| async move {
                let answer = self.tokens.fetch().await?;
                let url = self.livekit_url.clone().unwrap_or(answer.url);
                tracing::info!(
                    identity = %answer.identity,
                    room = %answer.room,
                    attempt,
                    attempts = self.retry.attempts,
                    "connecting to the LiveKit room"
                );
                let mut options = RoomOptions::default();
                // the router reads data packets only
                options.auto_subscribe = false;
                let (room, events) = Room::connect(&url, &answer.token, options).await.map_err(
                    |e| {
                        tracing::warn!(attempt, error = %e, "LiveKit connection attempt failed");
                        anyhow::anyhow!("connecting to {url}: {e}")
                    },
                )?;
                Ok::<_, anyhow::Error>((Arc::new(LiveRoom(room)), events, answer.identity))
            })
            .await
    }

    /// Routes the room's packets until the room's final disconnect; its reason, or `None` when
    /// the events end without one. It never waits for a routing slot, so it reads every event
    /// as it comes.
    async fn serve(
        &self,
        room: Arc<LiveRoom>,
        mut events: UnboundedReceiver<RoomEvent>,
        identity: &str,
        in_flight: &mut InFlight,
    ) -> Option<Reason> {
        loop {
            let event = tokio::select! {
                () = in_flight.reap() => continue,
                event = events.recv() => event?,
            };
            match event {
                RoomEvent::DataReceived {
                    payload,
                    topic,
                    participant,
                    participant_identity,
                    ..
                } => {
                    let sender = participant
                        .map(|p| p.identity().0)
                        .or(participant_identity.map(|i| i.0));
                    let admitted = routing::admit(
                        sender.as_deref(),
                        identity,
                        topic.as_deref(),
                        payload.len(),
                        self.max_packet_size_bytes,
                        |sender| self.limiter.check(sender) == RateLimitDecision::Allow,
                    );
                    let (sender, topic) = match admitted {
                        Ok((sender, topic)) => (sender.to_owned(), topic.to_owned()),
                        Err(Refused::Counted(outcome)) => {
                            tracing::warn!(
                                ?sender,
                                outcome = outcome.label(),
                                size = payload.len(),
                                "dropping a data packet"
                            );
                            metrics::outcome(outcome);
                            continue;
                        }
                        Err(refused) => {
                            tracing::debug!(?sender, ?refused, "ignoring a data packet");
                            continue;
                        }
                    };
                    let room = room.clone();
                    let members = self.members.clone();
                    let (from, on) = (sender.clone(), topic.clone());
                    let routed = in_flight.try_spawn(async move {
                        let started = std::time::Instant::now();
                        let outcome = routing::route(
                            room.as_ref(),
                            members.as_ref(),
                            &payload,
                            &sender,
                            &topic,
                        )
                        .await;
                        metrics::latency(started.elapsed());
                        metrics::outcome(outcome);
                    });
                    if !routed {
                        tracing::warn!(
                            sender = %from,
                            topic = %on,
                            limit = self.max_in_flight,
                            "every routing slot is taken; dropping a community message"
                        );
                        metrics::outcome(Outcome::Overloaded);
                    }
                }
                RoomEvent::Reconnecting => {
                    tracing::warn!("reconnecting to the LiveKit room");
                    self.connection.set(false);
                }
                RoomEvent::Reconnected => {
                    tracing::info!("reconnected to the LiveKit room");
                    self.connection.set(true);
                }
                RoomEvent::Disconnected { reason } => {
                    return Some(Reason::from_code(reason as i32));
                }
                _ => {}
            }
        }
    }

    /// Runs until `shutdown`. Fails, so the process exits non-zero and its supervisor starts it
    /// again, when a connection exhausts its attempts and when the room ends this participant
    /// for a reason that does not reconnect (another participant took its identity, the room
    /// was deleted).
    pub async fn run(
        self: Arc<Self>,
        shutdown: impl std::future::Future<Output = ()>,
    ) -> anyhow::Result<()> {
        tokio::pin!(shutdown);
        let mut in_flight = InFlight::new(self.max_in_flight);
        loop {
            let (room, events, identity) = tokio::select! {
                connected = self.connect() => connected.map_err(|e| {
                    anyhow::anyhow!("failed to connect to the LiveKit room after all attempts: {e}")
                })?,
                _ = &mut shutdown => return Ok(()),
            };
            tracing::info!(%identity, "connected to the LiveKit room");
            self.connection.set(true);
            let ended = tokio::select! {
                reason = self.serve(room.clone(), events, &identity, &mut in_flight) => Some(reason),
                _ = &mut shutdown => None,
            };
            self.connection.set(false);
            let Some(reason) = ended else {
                // the messages being routed finish on the room before it closes
                in_flight.drain(DRAIN).await;
                let _ = room.0.close().await;
                return Ok(());
            };
            // the room is gone: what still routes for it stops before the next connection routes
            in_flight.abort().await;
            match reason {
                Some(reason) if !reconnects(reason) => {
                    anyhow::bail!(
                        "disconnected from the LiveKit room for good ({reason:?}); exiting so the \
                         supervisor starts the router again"
                    );
                }
                reason => {
                    tracing::warn!(?reason, "disconnected from the LiveKit room; reconnecting");
                }
            }
        }
    }
}
