//! The Pulse application relay's room authority: `POST
//! /internal/pulse/room-authority/v1`, the protobuf endpoint `catalyst
//! pulse` (`comms::pulse::application_relay::http::HttpAuthority`) asks
//! before it admits a peer to a LiveKit-scoped relay room, on every lease
//! renewal (at most two seconds apart) and at release. The request is
//! authenticated with a dedicated HMAC service key (`PULSE_ROOM_AUTHORITY_KEY`,
//! never the LiveKit secret) over the method, path, timestamp, nonce, canonical
//! source UUID and body hash, replay-protected by nonce (`shared::signed_request`, shared with
//! the message router's token route); the join carries the claim
//! (`header.payload` of the LiveKit token comms minted) and the possession
//! proof, verified exclusively here. Shared frames, path and limits live in
//! `comms::protocol::room_authority`; proof verification and claims live here. A lease admits the
//! exact room for `LEASE_TTL`; the policy ([`policy::CurrentRoomPolicy`]) is
//! asked on the join and on every renewal, so a revoked permission expires
//! the running room's scope within one lease. Denials, capacity and store
//! faults are told apart on the wire (`DenialReason`).

pub mod policy;

pub mod proof;
use crate::protocol::room_authority::denied::Reason as DenialReason;
use proof::{ProofVerifier, VerifiedRoom};

use crate::protocol::room_authority::{
    self as wire, AUTHORITY_PATH, MAX_BODY, MAX_CLAIMS_BYTES, MAX_LEASES,
};
use crate::shared::routing::any;
use crate::shared::signed_request::{self, Replay, ReplayCache, one_header};
use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Method, Request, StatusCode, header},
    response::{IntoResponse, Response},
};
use prost::Message;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{sync::Semaphore, time::Instant};
use uuid::Uuid;

const CONTENT_TYPE: &str = "application/x-protobuf";
#[allow(clippy::cast_possible_truncation, reason = "REPLAY_TTL is 11 seconds")]
const MAX_REPLAYS: usize = MAX_LEASES * (REPLAY_TTL.as_secs() as usize + 5);
const MAX_CHECKS: usize = 128;
const CHECK_TIMEOUT: Duration = Duration::from_millis(650);
const LEASE_TTL: Duration = Duration::from_secs(2);
const REPLAY_TTL: Duration = Duration::from_secs(11);

#[async_trait]
pub trait RelayPolicy: Send + Sync {
    async fn authorize(&self, room: &VerifiedRoom) -> Result<(), DenialReason>;
}

pub struct RelayAuthority {
    generation: Option<(sqlx::PgPool, String)>,
    policy: Arc<dyn RelayPolicy>,
    verifier: ProofVerifier,
    service_key: [u8; 32],
    incarnation: [u8; 16],
    requests: Semaphore,
    checks: Semaphore,
    leases: Mutex<LeaseRegistry>,
    replays: ReplayCache,
}

struct LeaseRegistry {
    entries: HashMap<[u8; 16], StoredLease>,
    next_sweep: Instant,
}

struct StoredLease {
    instance: [u8; 16],
    generation: [u8; 16],
    room: Arc<VerifiedRoom>,
    expires: Instant,
    checking: bool,
}

struct Binding {
    instance: [u8; 16],
    generation: [u8; 16],
    id: [u8; 16],
}

impl RelayAuthority {
    fn new_inner(
        policy: Arc<dyn RelayPolicy>,
        livekit_api_key: String,
        livekit_secret: Vec<u8>,
        service_key: [u8; 32],
    ) -> Result<Self, &'static str> {
        if livekit_secret.as_slice() == service_key {
            return Err("relay authority requires a dedicated service key");
        }
        Ok(Self {
            generation: None,
            policy,
            verifier: ProofVerifier::new(livekit_api_key, livekit_secret)?,
            service_key,
            incarnation: *Uuid::new_v4().as_bytes(),
            requests: Semaphore::new(MAX_CHECKS),
            checks: Semaphore::new(MAX_CHECKS),
            leases: Mutex::new(LeaseRegistry {
                entries: HashMap::new(),
                next_sweep: Instant::now(),
            }),
            replays: ReplayCache::new(REPLAY_TTL, MAX_REPLAYS),
        })
    }

    pub fn new_with_generation(
        policy: Arc<dyn RelayPolicy>,
        livekit_api_key: String,
        livekit_secret: Vec<u8>,
        service_key: [u8; 32],
        pool: sqlx::PgPool,
        audience: String,
    ) -> Result<Self, &'static str> {
        if audience.is_empty() || audience.len() > 256 {
            return Err("room authority requires a deployment audience");
        }
        let mut authority = Self::new_inner(policy, livekit_api_key, livekit_secret, service_key)?;
        authority.generation = Some((pool, audience));
        Ok(authority)
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn unfenced_test(
        policy: Arc<dyn RelayPolicy>,
        livekit_api_key: String,
        livekit_secret: Vec<u8>,
        service_key: [u8; 32],
    ) -> Result<Self, &'static str> {
        Self::new_inner(policy, livekit_api_key, livekit_secret, service_key)
    }
    async fn generation_guard(
        &self,
        source: Option<&str>,
        started: Instant,
    ) -> Result<Option<sqlx::Transaction<'static, sqlx::Postgres>>, DenialReason> {
        let Some((pool, audience)) = &self.generation else {
            return Ok(None);
        };
        let source = source.ok_or(DenialReason::NotAuthorized)?;
        let deadline = started + CHECK_TIMEOUT;
        tokio::time::timeout_at(deadline, async {
            let mut tx = pool.begin().await.map_err(|_| DenialReason::Unavailable)?;
            sqlx::query("SET LOCAL lock_timeout='500ms'")
                .execute(&mut *tx)
                .await
                .map_err(|_| DenialReason::Unavailable)?;
            sqlx::query("SET LOCAL statement_timeout='500ms'")
                .execute(&mut *tx)
                .await
                .map_err(|_| DenialReason::Unavailable)?;
            super::cluster::generation::lock(&mut tx, audience, source)
                .await
                .map_err(|_| DenialReason::NotAuthorized)?;
            Ok(Some(tx))
        })
        .await
        .map_err(|_| DenialReason::Unavailable)?
    }

    async fn handle(&self, request: Request<Body>) -> Response {
        let started = Instant::now();
        let _request_permit = match self.requests.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                return error_response(StatusCode::SERVICE_UNAVAILABLE, DenialReason::Capacity);
            }
        };
        let wall = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(time) => time,
            Err(_) => {
                return error_response(StatusCode::SERVICE_UNAVAILABLE, DenialReason::Unavailable);
            }
        };
        if request.method() != Method::POST {
            return error_response(StatusCode::METHOD_NOT_ALLOWED, DenialReason::Invalid);
        }
        if request.uri().path() != AUTHORITY_PATH || request.uri().query().is_some() {
            return error_response(StatusCode::NOT_FOUND, DenialReason::Invalid);
        }
        if one_header(request.headers(), header::CONTENT_TYPE.as_str()) != Some(CONTENT_TYPE)
            || one_header(request.headers(), header::ACCEPT.as_str()) != Some(CONTENT_TYPE)
        {
            return error_response(StatusCode::UNSUPPORTED_MEDIA_TYPE, DenialReason::Invalid);
        }
        let (parts, body) = request.into_parts();
        let bytes = match tokio::time::timeout_at(started + CHECK_TIMEOUT, to_bytes(body, MAX_BODY))
            .await
        {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(_)) => {
                return error_response(StatusCode::PAYLOAD_TOO_LARGE, DenialReason::Invalid);
            }
            Err(_) => {
                return error_response(StatusCode::REQUEST_TIMEOUT, DenialReason::Unavailable);
            }
        };
        if let Err(reason) = self.authenticate(&parts.headers, &bytes, wall.as_secs(), started) {
            let status = if reason == DenialReason::Capacity || reason == DenialReason::Unavailable
            {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::UNAUTHORIZED
            };
            return error_response(status, reason);
        }
        let source = one_header(&parts.headers, "x-pulse-authority-source");
        if self.generation_guard(source, started).await.is_err() {
            return error_response(StatusCode::SERVICE_UNAVAILABLE, DenialReason::Unavailable);
        }
        let request = match wire::AuthorityRequest::decode(bytes) {
            Ok(request) => request,
            Err(_) => return error_response(StatusCode::BAD_REQUEST, DenialReason::Invalid),
        };
        let result = self
            .execute_with_source(request, wall, started, source)
            .await;
        protobuf_response(StatusCode::OK, result)
    }

    fn authenticate(
        &self,
        headers: &HeaderMap,
        body: &[u8],
        now_secs: u64,
        now: Instant,
    ) -> Result<(), DenialReason> {
        let timestamp =
            one_header(headers, "x-pulse-authority-timestamp").ok_or(DenialReason::Invalid)?;
        if !signed_request::fresh_timestamp(timestamp, now_secs, 5) {
            return Err(DenialReason::Invalid);
        }
        let nonce_text =
            one_header(headers, "x-pulse-authority-nonce").ok_or(DenialReason::Invalid)?;
        let nonce = decode_fixed::<16>(nonce_text)?;
        let signature =
            one_header(headers, "x-pulse-authority-signature").ok_or(DenialReason::Invalid)?;
        decode_fixed::<32>(signature)?;
        let source = if self.generation.is_some() {
            let source =
                one_header(headers, "x-pulse-authority-source").ok_or(DenialReason::Invalid)?;
            if !Uuid::parse_str(source).is_ok_and(|id| id.to_string() == source) {
                return Err(DenialReason::Invalid);
            }
            Some(source)
        } else {
            None
        };
        let preimage =
            signed_request::preimage(AUTHORITY_PATH, timestamp, nonce_text, source, body);
        if !signed_request::verify(&self.service_key, &preimage, signature) {
            return Err(DenialReason::NotAuthorized);
        }
        self.replays
            .admit(nonce, now)
            .map_err(|replay| match replay {
                Replay::Seen => DenialReason::NotAuthorized,
                Replay::Full => DenialReason::Capacity,
            })?;
        Ok(())
    }

    #[cfg(test)]
    async fn execute(
        &self,
        request: wire::AuthorityRequest,
        wall: Duration,
        started: Instant,
    ) -> wire::AuthorityResponse {
        self.execute_with_source(request, wall, started, None).await
    }
    async fn execute_with_source(
        &self,
        request: wire::AuthorityRequest,
        wall: Duration,
        started: Instant,
        source: Option<&str>,
    ) -> wire::AuthorityResponse {
        use wire::authority_request::Operation;
        let result = match request.operation {
            Some(Operation::Join(join)) => self.join(join, wall, started, source).await,
            Some(Operation::Renew(renew)) => self.renew(renew, started, source).await,
            Some(Operation::Release(release)) => self.release(release),
            None => Err(DenialReason::Invalid),
        };
        wire::AuthorityResponse {
            result: Some(result.unwrap_or_else(denied)),
        }
    }

    async fn join(
        &self,
        join: wire::Join,
        wall: Duration,
        started: Instant,
        source: Option<&str>,
    ) -> Result<wire::authority_response::Result, DenialReason> {
        let instance = fixed::<16>(&join.pulse_instance)?;
        let generation = fixed::<16>(&join.connection_generation)?;
        let wallet = fixed::<20>(&join.wallet)?;
        let session = fixed::<20>(&join.session)?;
        let nonce = fixed::<32>(&join.nonce)?;
        fixed::<32>(&join.proof)?;
        if join.header_payload.len() > MAX_CLAIMS_BYTES {
            return Err(DenialReason::Invalid);
        }
        let room = self
            .verifier
            .verify(
                &join.header_payload,
                &join.proof,
                &nonce,
                &format!("0x{}", hex::encode(wallet)),
                &format!("0x{}", hex::encode(session)),
                wall.saturating_add(started.elapsed()).as_secs(),
            )
            .map_err(|_| DenialReason::NotAuthorized)?;
        {
            let mut leases = self.leases.lock().map_err(|_| DenialReason::Unavailable)?;
            leases.sweep(Instant::now());
            if leases.entries.len() >= MAX_LEASES {
                return Err(DenialReason::Capacity);
            }
        }
        self.check_policy(&room, started).await?;
        let _generation = self.generation_guard(source, started).await?;
        let mut leases = self.leases.lock().map_err(|_| DenialReason::Unavailable)?;
        let now = Instant::now();
        let expires = started + LEASE_TTL;
        if expires <= now || room.claims.exp <= wall.saturating_add(started.elapsed()).as_secs() {
            return Err(DenialReason::Expired);
        }
        leases.sweep(now);
        if leases.entries.len() >= MAX_LEASES {
            return Err(DenialReason::Capacity);
        }
        let id = *Uuid::new_v4().as_bytes();
        if leases.entries.contains_key(&id) {
            return Err(DenialReason::Unavailable);
        }
        leases.entries.insert(
            id,
            StoredLease {
                instance,
                generation,
                room: Arc::new(room.clone()),
                expires,
                checking: false,
            },
        );
        Ok(self.lease_response(id, &room))
    }

    async fn renew(
        &self,
        renew: wire::Context,
        started: Instant,
        source: Option<&str>,
    ) -> Result<wire::authority_response::Result, DenialReason> {
        let binding = self.binding(
            &renew.pulse_instance,
            &renew.connection_generation,
            &renew.lease_id,
            &renew.authority_incarnation,
        )?;
        let room = {
            let mut leases = self.leases.lock().map_err(|_| DenialReason::Unavailable)?;
            let entry = leases
                .entries
                .get(&binding.id)
                .ok_or(DenialReason::Expired)?;
            if !binding.matches(entry) {
                return Err(DenialReason::Invalid);
            }
            if entry.expires <= Instant::now() {
                leases.entries.remove(&binding.id);
                return Err(DenialReason::Expired);
            }
            if entry.checking {
                leases.entries.remove(&binding.id);
                return Err(DenialReason::Capacity);
            }
            let entry = leases
                .entries
                .get_mut(&binding.id)
                .ok_or(DenialReason::Expired)?;
            entry.checking = true;
            Arc::clone(&entry.room)
        };
        let authorized = self.check_policy(&room, started).await;
        let generation = self.generation_guard(source, started).await;
        let mut leases = self.leases.lock().map_err(|_| DenialReason::Unavailable)?;
        let entry = leases
            .entries
            .get(&binding.id)
            .ok_or(DenialReason::Expired)?;
        if !binding.matches(entry) {
            return Err(DenialReason::Invalid);
        }
        if let Err(reason) = authorized {
            leases.entries.remove(&binding.id);
            return Err(reason);
        }
        let expires = entry.expires;
        let _generation = match generation {
            Ok(guard) => guard,
            Err(reason) => {
                // The exact lease still exists and matches; the failed check grants
                // nothing and leaves its original expiry available for a later retry.
                leases
                    .entries
                    .get_mut(&binding.id)
                    .ok_or(DenialReason::Expired)?
                    .checking = false;
                return Err(reason);
            }
        };
        if expires <= Instant::now() || started + LEASE_TTL <= Instant::now() {
            leases.entries.remove(&binding.id);
            return Err(DenialReason::Expired);
        }
        let entry = leases
            .entries
            .get_mut(&binding.id)
            .ok_or(DenialReason::Expired)?;
        entry.expires = started + LEASE_TTL;
        entry.checking = false;
        Ok(self.lease_response(binding.id, &room))
    }

    fn release(
        &self,
        release: wire::Context,
    ) -> Result<wire::authority_response::Result, DenialReason> {
        let binding = self.binding(
            &release.pulse_instance,
            &release.connection_generation,
            &release.lease_id,
            &release.authority_incarnation,
        )?;
        let mut leases = self.leases.lock().map_err(|_| DenialReason::Unavailable)?;
        let entry = leases
            .entries
            .get(&binding.id)
            .ok_or(DenialReason::Expired)?;
        if !binding.matches(entry) {
            return Err(DenialReason::Invalid);
        }
        leases.entries.remove(&binding.id);
        Ok(wire::authority_response::Result::Released(
            wire::Released {},
        ))
    }

    async fn check_policy(
        &self,
        room: &VerifiedRoom,
        started: Instant,
    ) -> Result<(), DenialReason> {
        if started.elapsed() >= CHECK_TIMEOUT {
            return Err(DenialReason::Unavailable);
        }
        let _permit = self
            .checks
            .try_acquire()
            .map_err(|_| DenialReason::Capacity)?;
        let result =
            tokio::time::timeout_at(started + CHECK_TIMEOUT, self.policy.authorize(room)).await;
        if started.elapsed() >= CHECK_TIMEOUT {
            return Err(DenialReason::Unavailable);
        }
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(DenialReason::Unspecified)) | Err(_) => Err(DenialReason::Unavailable),
            Ok(Err(reason)) => Err(reason),
        }
    }

    fn binding(
        &self,
        instance: &[u8],
        generation: &[u8],
        id: &[u8],
        incarnation: &[u8],
    ) -> Result<Binding, DenialReason> {
        if fixed::<16>(incarnation)? != self.incarnation {
            return Err(DenialReason::Expired);
        }
        Ok(Binding {
            instance: fixed(instance)?,
            generation: fixed(generation)?,
            id: fixed(id)?,
        })
    }

    fn lease_response(
        &self,
        id: [u8; 16],
        room: &VerifiedRoom,
    ) -> wire::authority_response::Result {
        wire::authority_response::Result::Lease(wire::Lease {
            lease_id: id.to_vec(),
            authority_incarnation: self.incarnation.to_vec(),
            ttl_ms: u32::try_from(LEASE_TTL.as_millis()).unwrap_or(u32::MAX),
            admitted: Some(wire::AdmittedRoom {
                room: room.claims.video.room.clone(),
                identity: room.claims.sub.clone(),
                is_guest: Some(room.is_guest),
            }),
        })
    }
}

impl LeaseRegistry {
    fn sweep(&mut self, now: Instant) {
        if now >= self.next_sweep || self.entries.len() >= MAX_LEASES {
            self.entries.retain(|_, entry| entry.expires > now);
            self.next_sweep = now + Duration::from_millis(100);
        }
    }
}

impl Binding {
    fn matches(&self, entry: &StoredLease) -> bool {
        self.instance == entry.instance && self.generation == entry.generation
    }
}

pub fn router<S: Clone + Send + Sync + 'static>(authority: Arc<RelayAuthority>) -> Router<S> {
    Router::new().route(
        AUTHORITY_PATH,
        any(move |request: Request<Body>| {
            let authority = Arc::clone(&authority);
            async move { authority.handle(request).await }
        }),
    )
}

fn fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N], DenialReason> {
    bytes.try_into().map_err(|_| DenialReason::Invalid)
}

fn decode_fixed<const N: usize>(value: &str) -> Result<[u8; N], DenialReason> {
    signed_request::decode_fixed(value).ok_or(DenialReason::Invalid)
}

fn denied(reason: DenialReason) -> wire::authority_response::Result {
    wire::authority_response::Result::Denied(wire::Denied {
        reason: reason as i32,
    })
}

fn error_response(status: StatusCode, reason: DenialReason) -> Response {
    protobuf_response(
        status,
        wire::AuthorityResponse {
            result: Some(denied(reason)),
        },
    )
}

fn protobuf_response(status: StatusCode, body: wire::AuthorityResponse) -> Response {
    (
        status,
        [
            (header::CONTENT_TYPE, CONTENT_TYPE),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body.encode_to_vec(),
    )
        .into_response()
}

/// `PULSE_ROOM_AUTHORITY_KEY`: 32 base64url bytes without padding, distinct
/// from the LiveKit secret (the old comms config's rule).
pub fn parse_service_key(encoded: &str, livekit_secret: &[u8]) -> anyhow::Result<[u8; 32]> {
    Ok(signed_request::parse_key(
        "PULSE_ROOM_AUTHORITY_KEY",
        encoded,
        Some(livekit_secret),
    )?)
}

#[cfg(test)]
mod tests;
