//! The router in the room, as upstream's `adapters/livekit.ts`: connect with a fresh token
//! (`connection::Retry`, upstream's five attempts one second apart), route every data packet
//! (`routing`), and after a final disconnect connect again when `connection::reconnects` says
//! so. The SDK resumes a dropped signal connection by itself (`Reconnecting`, `Reconnected`);
//! only its `Disconnected` is final.
//!
//! The sender is the LiveKit-authenticated identity the SFU stamps on the packet: the remote
//! participant's, or, for data that arrives before the participant's announcement, the
//! `participant_identity` the patched SDK carries.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use livekit::prelude::*;
use tokio::sync::Semaphore;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::connection::{Reason, Retry, reconnects};
use crate::membership::PgMembership;
use crate::metrics::{self, Connection};
use crate::rate_limiter::RateLimiter;
use crate::routing::{self, Refused};
use crate::token::TokenClient;

/// Packets routed at once; the event loop waits for a slot beyond that.
const MAX_IN_FLIGHT: usize = 64;

pub struct Router {
    pub tokens: TokenClient,
    /// Dialed instead of the URL the gatekeeper hands out.
    pub livekit_url: Option<String>,
    pub retry: Retry,
    pub max_packet_size_bytes: usize,
    pub members: Arc<PgMembership>,
    pub limiter: Arc<Mutex<RateLimiter>>,
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
    /// the events end without one.
    async fn serve(
        &self,
        room: Arc<LiveRoom>,
        mut events: UnboundedReceiver<RoomEvent>,
        identity: &str,
    ) -> Option<Reason> {
        let slots = Arc::new(Semaphore::new(MAX_IN_FLIGHT));
        while let Some(event) = events.recv().await {
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
                        |sender| {
                            self.limiter
                                .lock()
                                .expect("rate limiter")
                                .allow(sender, Instant::now())
                        },
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
                    let Ok(slot) = slots.clone().acquire_owned().await else {
                        continue;
                    };
                    let room = room.clone();
                    let members = self.members.clone();
                    tokio::spawn(async move {
                        let started = Instant::now();
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
                        drop(slot);
                    });
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
        None
    }

    /// Runs until `shutdown`. Fails when a connection exhausts its attempts, so the process
    /// exits and its supervisor starts it again.
    pub async fn run(
        self: Arc<Self>,
        shutdown: impl std::future::Future<Output = ()>,
    ) -> anyhow::Result<()> {
        tokio::pin!(shutdown);
        loop {
            let (room, events, identity) = tokio::select! {
                connected = self.connect() => connected.map_err(|e| {
                    anyhow::anyhow!("failed to connect to the LiveKit room after all attempts: {e}")
                })?,
                _ = &mut shutdown => return Ok(()),
            };
            tracing::info!(%identity, "connected to the LiveKit room");
            self.connection.set(true);
            let reason = tokio::select! {
                reason = self.serve(room.clone(), events, &identity) => reason,
                _ = &mut shutdown => {
                    let _ = room.0.close().await;
                    self.connection.set(false);
                    return Ok(());
                }
            };
            self.connection.set(false);
            match reason {
                Some(reason) if !reconnects(reason) => {
                    tracing::error!(
                        ?reason,
                        "disconnected from the LiveKit room for good; not reconnecting"
                    );
                    shutdown.await;
                    return Ok(());
                }
                reason => {
                    tracing::warn!(?reason, "disconnected from the LiveKit room; reconnecting");
                }
            }
        }
    }
}
