pub mod http;
pub mod state;

use crate::protocol::pulse::decentraland::pulse::{ApplicationErrorCode, ApplicationJoin};
use crate::protocol::room_authority::{AdmittedRoom, MAX_CLAIMS_BYTES};
use sha2::{Digest, Sha256};
use state::{Delivery, RelayState, failure};
use std::{
    collections::{BTreeSet, HashMap},
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::task::JoinSet;

pub const CAPABILITY: &str = "application_relay";
const MAX_AUTHORITY_JOBS: usize = 128;
pub const MAX_AUTHORITY_LEASE: Duration = Duration::from_secs(2);
const AUTHORITY_TIMEOUT: Duration = Duration::from_millis(750);

/// Admission and renewal consult live room policy, platform/scene bans and assignment
/// fencing. Signature verification alone is insufficient. Authority failure expires closed.
/// Revocation has a bounded lease delay, not atomic equivalence with the SFU kick.
#[derive(Clone)]
pub struct PendingJoin {
    pub header_payload: String,
    pub proof: Vec<u8>,
    pub wallet: String,
    pub session: String,
    pub nonce: [u8; 32],
}
impl PendingJoin {
    fn key(&self) -> ([u8; 32], [u8; 32]) {
        (
            self.nonce,
            Sha256::digest(self.header_payload.as_bytes()).into(),
        )
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct AdmittedScope {
    pub admitted: AdmittedRoom,
    pub nonce: [u8; 32],
    pub fingerprint: [u8; 32],
}
#[derive(Clone)]
pub enum AuthorityInput {
    Join(PendingJoin),
    Renew(AdmittedScope),
}
impl AuthorityInput {
    fn key(&self) -> ([u8; 32], [u8; 32]) {
        match self {
            Self::Join(p) => p.key(),
            Self::Renew(s) => (s.nonce, s.fingerprint),
        }
    }
}
#[derive(Debug, PartialEq)]
pub struct Admission {
    pub scope: AdmittedScope,
    pub ttl: Duration,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AuthorityError {
    NotAuthorized,
    Unavailable,
}
pub trait RoomAuthority: Send + Sync {
    fn authorize(
        &self,
        input: AuthorityInput,
    ) -> Pin<Box<dyn Future<Output = Result<Admission, AuthorityError>> + Send + '_>>;
    fn release(&self, _scope: AdmittedScope) {}
    fn revoked_connections(&self) -> Vec<[u8; 32]> {
        Vec::new()
    }
}

struct Connection {
    nonce: [u8; 32],
    wallet: String,
    session: String,
    join_budget: (Instant, u8),
}
struct Completion {
    peer: u32,
    nonce: [u8; 32],
    request_id: u32,
    room_id: Option<u32>,
    admitted: Option<AdmittedScope>,
    error: AuthorityError,
    deadline: Option<Instant>,
    renew_at: Option<Instant>,
}

pub struct ApplicationRelay {
    authority: Arc<dyn RoomAuthority>,
    connections: HashMap<u32, Connection>,
    pending: HashMap<(u32, u32), String>,
    jobs: JoinSet<Completion>,
    renewal_times: BTreeSet<(Instant, u32, u32)>,
    renewal_by_scope: HashMap<(u32, u32), Instant>,
    pub state: RelayState,
}

impl ApplicationRelay {
    pub fn new(authority: Arc<dyn RoomAuthority>) -> Self {
        Self {
            authority,
            connections: HashMap::new(),
            pending: HashMap::new(),
            jobs: JoinSet::new(),
            renewal_times: BTreeSet::new(),
            renewal_by_scope: HashMap::new(),
            state: RelayState::default(),
        }
    }
    pub fn authenticated(&mut self, peer: u32, nonce: [u8; 32], wallet: String, session: String) {
        self.connections.entry(peer).or_insert(Connection {
            nonce,
            wallet,
            session,
            join_budget: (Instant::now(), 0),
        });
    }
    pub fn has_jobs(&self) -> bool {
        !self.jobs.is_empty()
    }
    pub fn nonce(&self, peer: u32) -> Option<&[u8; 32]> {
        self.connections.get(&peer).map(|c| &c.nonce)
    }

    pub fn join(&mut self, peer: u32, join: ApplicationJoin, _now_secs: u64) -> Vec<Delivery> {
        self.schedule_renewals(Instant::now());
        let denied = |error| vec![failure(peer, join.request_id, error)];
        let Some(connection) = self.connections.get_mut(&peer) else {
            return denied(ApplicationErrorCode::ApplicationUnavailable);
        };
        let now = Instant::now();
        if now.duration_since(connection.join_budget.0) >= Duration::from_secs(1) {
            connection.join_budget = (now, 0);
        }
        if connection.join_budget.1 >= 16 {
            return denied(ApplicationErrorCode::ApplicationLimit);
        }
        connection.join_budget.1 += 1;
        if join.request_id == 0 {
            return denied(ApplicationErrorCode::ApplicationInvalid);
        }
        if let Some(pending) = self.pending.get(&(peer, join.request_id)) {
            return if pending == &join.jwt_header_payload {
                vec![]
            } else {
                denied(ApplicationErrorCode::ApplicationInvalid)
            };
        }
        if self.jobs.len() >= MAX_AUTHORITY_JOBS
            || self.pending.keys().filter(|(p, _)| *p == peer).count() >= state::MAX_PEER_ROOMS
        {
            return denied(ApplicationErrorCode::ApplicationLimit);
        }
        if join.jwt_header_payload.len() > MAX_CLAIMS_BYTES || join.proof.len() != 32 {
            return denied(ApplicationErrorCode::ApplicationNotAuthorized);
        }
        let fingerprint: [u8; 32] = Sha256::digest(join.jwt_header_payload.as_bytes()).into();
        if self.state.memberships.iter().any(|((p, _), m)| {
            *p == peer && m.request_id == join.request_id && m.scope.fingerprint != fingerprint
        }) {
            return denied(ApplicationErrorCode::ApplicationInvalid);
        }
        let pending = PendingJoin {
            header_payload: join.jwt_header_payload.clone(),
            proof: join.proof,
            wallet: connection.wallet.clone(),
            session: connection.session.clone(),
            nonce: connection.nonce,
        };
        let nonce = connection.nonce;
        self.pending
            .insert((peer, join.request_id), join.jwt_header_payload);
        self.spawn(
            peer,
            nonce,
            join.request_id,
            None,
            AuthorityInput::Join(pending),
        );
        vec![]
    }

    fn spawn(
        &mut self,
        peer: u32,
        nonce: [u8; 32],
        request_id: u32,
        room_id: Option<u32>,
        input: AuthorityInput,
    ) {
        let authority = self.authority.clone();
        self.jobs.spawn(async move {
            let started = Instant::now();
            let result = tokio::time::timeout(AUTHORITY_TIMEOUT, authority.authorize(input)).await;
            let (admitted, lease, error) = match result {
                Ok(Ok(admission))
                    if !admission.ttl.is_zero()
                        && admission.ttl <= MAX_AUTHORITY_LEASE
                        && admission.scope.admitted.is_valid() =>
                {
                    (
                        Some(admission.scope),
                        Some(admission.ttl),
                        AuthorityError::Unavailable,
                    )
                }
                Ok(Err(error)) => (None, None, error),
                _ => (None, None, AuthorityError::Unavailable),
            };
            let deadline = lease.map(|lease| started + lease);
            let renew_at = lease.map(|lease| started + lease / 2);
            Completion {
                peer,
                nonce,
                request_id,
                room_id,
                admitted,
                error,
                deadline,
                renew_at,
            }
        });
    }

    pub async fn completed(&mut self) -> (Vec<Delivery>, Vec<u32>) {
        let Some(Ok(done)) = self.jobs.join_next().await else {
            return (vec![], vec![]);
        };
        let result = self.finish(done);
        self.schedule_renewals(Instant::now());
        result
    }

    fn finish(&mut self, done: Completion) -> (Vec<Delivery>, Vec<u32>) {
        if !self
            .connections
            .get(&done.peer)
            .is_some_and(|c| c.nonce == done.nonce)
        {
            return (vec![], vec![]);
        }
        let now = Instant::now();
        if let Some(room_id) = done.room_id {
            let Some(member) = self.state.memberships.get_mut(&(done.peer, room_id)) else {
                return (vec![], vec![]);
            };
            member.renewing = false;
            if let Some(deadline) = done.deadline.filter(|d| {
                *d > now
                    && member.lease_deadline > now
                    && done.admitted.as_ref() == Some(&member.scope)
            }) {
                member.lease_deadline = deadline;
                self.set_renewal(
                    done.peer,
                    room_id,
                    done.renew_at.expect("valid lease has renewal time"),
                );
                return (vec![], vec![]);
            }
            return (self.leave(done.peer, room_id), vec![]);
        }
        self.pending.remove(&(done.peer, done.request_id));
        match done.deadline.filter(|d| *d > now) {
            Some(deadline) => {
                let admitted = done.admitted.expect("successful admission has projection");
                let room_name = admitted.admitted.room.clone();
                let out = self
                    .state
                    .join(done.peer, done.request_id, admitted, deadline, now);
                if let Some(member) = self.state.membership(done.peer, &room_name)
                    && !self
                        .renewal_by_scope
                        .contains_key(&(done.peer, member.room_id))
                    && !member.renewing
                {
                    self.set_renewal(
                        done.peer,
                        member.room_id,
                        done.renew_at.expect("valid lease has renewal time"),
                    );
                }
                (out, vec![])
            }
            None => (
                vec![failure(
                    done.peer,
                    done.request_id,
                    match done.error {
                        AuthorityError::NotAuthorized => {
                            ApplicationErrorCode::ApplicationNotAuthorized
                        }
                        AuthorityError::Unavailable => ApplicationErrorCode::ApplicationUnavailable,
                    },
                )],
                vec![],
            ),
        }
    }

    pub fn maintenance(&mut self, now: Instant) -> Vec<Delivery> {
        let revoked = self
            .authority
            .revoked_connections()
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let expired = self
            .state
            .memberships
            .iter()
            .filter_map(|((peer, room), member)| {
                (member.lease_deadline <= now || revoked.contains(&member.scope.nonce))
                    .then_some((*peer, *room))
            })
            .collect::<Vec<_>>();
        let mut out = Vec::new();
        for (peer, room) in expired {
            out.extend(self.leave(peer, room));
        }
        self.schedule_renewals(now);
        out
    }

    fn set_renewal(&mut self, peer: u32, room: u32, at: Instant) {
        if let Some(previous) = self.renewal_by_scope.insert((peer, room), at) {
            self.renewal_times.remove(&(previous, peer, room));
        }
        self.renewal_times.insert((at, peer, room));
    }

    fn remove_renewal(&mut self, peer: u32, room: u32) {
        if let Some(previous) = self.renewal_by_scope.remove(&(peer, room)) {
            self.renewal_times.remove(&(previous, peer, room));
        }
    }

    pub fn next_renewal(&self) -> Option<Instant> {
        (self.jobs.len() < MAX_AUTHORITY_JOBS)
            .then(|| self.renewal_times.first().map(|(at, _, _)| *at))
            .flatten()
    }

    pub fn schedule_renewals(&mut self, now: Instant) {
        while self.jobs.len() < MAX_AUTHORITY_JOBS {
            let Some(&(at, peer, room)) = self.renewal_times.first() else {
                break;
            };
            if at > now {
                break;
            }
            self.renewal_times.pop_first();
            self.renewal_by_scope.remove(&(peer, room));
            let Some(member) = self.state.memberships.get_mut(&(peer, room)) else {
                continue;
            };
            if member.renewing || member.lease_deadline <= now {
                continue;
            }
            let Some(connection) = self.connections.get(&peer) else {
                continue;
            };
            member.renewing = true;
            let (nonce, request_id, verified) =
                (connection.nonce, member.request_id, member.scope.clone());
            self.spawn(
                peer,
                nonce,
                request_id,
                Some(room),
                AuthorityInput::Renew(verified),
            );
        }
    }

    pub fn disconnect(&mut self, peer: u32) -> Vec<Delivery> {
        for room in self.state.room_ids_for_peer(peer) {
            self.remove_renewal(peer, room);
        }
        for credential in self.state.credentials_for_peer(peer) {
            self.authority.release(credential);
        }
        self.connections.remove(&peer);
        self.pending.retain(|(p, _), _| *p != peer);
        self.state.disconnect(peer)
    }

    pub fn send(
        &mut self,
        peer: u32,
        message: crate::protocol::pulse::decentraland::pulse::ApplicationSend,
        now: Instant,
    ) -> (Vec<Delivery>, bool) {
        let room = message.room_id;
        let reliable = !message.unreliable;
        match self.state.send(peer, message, now) {
            Ok(out) => (out, false),
            Err(state::RelaySendError::UnknownScope) => (vec![], false),
            Err(state::RelaySendError::ExpiredScope) => (self.leave(peer, room), false),
            Err(state::RelaySendError::RateLimited) if reliable => (self.leave(peer, room), false),
            Err(state::RelaySendError::RateLimited) => (vec![], false),
            Err(state::RelaySendError::InvalidPayload) => (vec![], reliable),
        }
    }

    pub fn retire_scopes(&mut self, peer: u32) -> Vec<Delivery> {
        self.state
            .room_ids_for_peer(peer)
            .into_iter()
            .flat_map(|room| self.leave(peer, room))
            .collect()
    }

    pub fn leave(&mut self, peer: u32, room_id: u32) -> Vec<Delivery> {
        self.remove_renewal(peer, room_id);
        let Some(member) = self.state.memberships.get(&(peer, room_id)) else {
            return vec![];
        };
        self.authority.release(member.scope.clone());
        let self_left = Delivery {
            target: peer,
            unreliable: false,
            message: crate::protocol::pulse::decentraland::pulse::ServerMessage {
                message: Some(
                    crate::protocol::pulse::decentraland::pulse::server_message::Message::ApplicationPeerLeft(
                        crate::protocol::pulse::decentraland::pulse::ApplicationPeerLeft {
                            room_id,
                            peer_id: member.actor_id,
                            roster_version: self.state.leave_version(peer, room_id),
                        },
                    ),
                ),
            },
        };
        let mut out = self.state.leave(peer, room_id);
        out.push(self_left);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::pulse::decentraland::pulse::server_message;
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicBool, Ordering};
    const WALLET: &str = "0x1111111111111111111111111111111111111111";
    const SESSION: &str = "0x2222222222222222222222222222222222222222";
    struct Authority {
        allowed: AtomicBool,
    }
    impl RoomAuthority for Authority {
        fn authorize(
            &self,
            input: AuthorityInput,
        ) -> Pin<Box<dyn Future<Output = Result<Admission, AuthorityError>> + Send + '_>> {
            Box::pin(async move {
                if !self.allowed.load(Ordering::Relaxed) {
                    return Err(AuthorityError::Unavailable);
                }
                let scope = match input {
                    AuthorityInput::Join(p) => AdmittedScope {
                        admitted: AdmittedRoom {
                            room: "scene".into(),
                            identity: WALLET.into(),
                            is_guest: Some(true),
                        },
                        nonce: p.nonce,
                        fingerprint: p.key().1,
                    },
                    AuthorityInput::Renew(scope) => scope,
                };
                Ok(Admission {
                    scope,
                    ttl: Duration::from_millis(100),
                })
            })
        }
    }
    fn join(nonce: [u8; 32]) -> ApplicationJoin {
        let claims = serde_json::json!({"iss":"key","sub":WALLET,"exp":100,"video":{"room":"scene","roomJoin":true,"canPublishData":true}});
        let hp = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256"}"#),
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let mut signature = Hmac::<Sha256>::new_from_slice(&[7; 32]).unwrap();
        signature.update(hp.as_bytes());
        let mut proof = Hmac::<Sha256>::new_from_slice(&signature.finalize().into_bytes()).unwrap();
        proof.update(crate::protocol::room_authority::PROOF_DOMAIN);
        proof.update(&nonce);
        proof.update(&crate::protocol::room_authority::address_bytes(WALLET).unwrap());
        proof.update(&crate::protocol::room_authority::address_bytes(SESSION).unwrap());
        proof.update(&Sha256::digest(hp.as_bytes()));
        ApplicationJoin {
            request_id: 1,
            jwt_header_payload: hp,
            proof: proof.finalize().into_bytes().to_vec(),
        }
    }
    fn relay() -> (ApplicationRelay, Arc<Authority>) {
        let authority = Arc::new(Authority {
            allowed: AtomicBool::new(true),
        });
        let mut relay = ApplicationRelay::new(authority.clone());
        relay.authenticated(1, [3; 32], WALLET.into(), SESSION.into());
        (relay, authority)
    }
    #[tokio::test]
    async fn more_than_one_job_batch_refills_immediately_without_a_simulation_tick() {
        let (mut relay, _) = relay();
        let proof = join([3; 32]);
        let base = AdmittedScope {
            admitted: AdmittedRoom {
                room: "scene".into(),
                identity: WALLET.into(),
                is_guest: Some(true),
            },
            nonce: [3; 32],
            fingerprint: Sha256::digest(proof.jwt_header_payload.as_bytes()).into(),
        };
        let now = Instant::now();
        let original_deadline = now + Duration::from_secs(2);
        for peer in 1u32..=129 {
            let mut nonce = [0; 32];
            nonce[..4].copy_from_slice(&peer.to_le_bytes());
            relay.authenticated(peer, nonce, WALLET.into(), SESSION.into());
            let mut room = base.clone();
            room.nonce = nonce;
            room.admitted.room = format!("room-{peer}");
            let name = room.admitted.room.clone();
            relay.state.join(peer, peer, room, original_deadline, now);
            let room_id = relay.state.membership(peer, &name).unwrap().room_id;
            relay.set_renewal(peer, room_id, now);
        }
        relay.schedule_renewals(now);
        assert_eq!(relay.jobs.len(), MAX_AUTHORITY_JOBS);
        assert_eq!(relay.renewal_times.len(), 1);
        relay.completed().await;
        assert_eq!(relay.jobs.len(), MAX_AUTHORITY_JOBS);
        assert!(relay.state.membership(129, "room-129").unwrap().renewing);
        for _ in 1..129 {
            relay.completed().await;
        }
        assert_eq!(relay.state.memberships.len(), 129);
        assert!(
            relay
                .state
                .memberships
                .values()
                .all(|m| m.lease_deadline < original_deadline)
        );
        assert!(relay.renewal_times.len() <= relay.state.memberships.len());
    }

    #[tokio::test]
    async fn reliable_data_racing_scope_revocation_does_not_disconnect_typed_pulse() {
        let (mut relay, _) = relay();
        relay.join(1, join([3; 32]), 99);
        relay.completed().await;
        let room_id = relay.state.membership(1, "scene").unwrap().room_id;
        let send = crate::protocol::pulse::decentraland::pulse::ApplicationSend {
            room_id,
            payload: vec![1],
            ..Default::default()
        };
        relay
            .state
            .memberships
            .get_mut(&(1, room_id))
            .unwrap()
            .lease_deadline = Instant::now() - Duration::from_millis(1);
        let (out, close) = relay.send(1, send.clone(), Instant::now());
        assert!(!close);
        assert!(matches!(
            out[0].message.message,
            Some(server_message::Message::ApplicationPeerLeft(_))
        ));
        let (out, close) = relay.send(1, send, Instant::now());
        assert!(!close);
        assert!(out.is_empty());
        assert!(relay.connections.contains_key(&1));
    }

    const CLIENT_BURST_PACKETS: u32 = 16;
    const CLIENT_PACKETS_PER_SECOND: u32 = 60;

    fn hold_lease(relay: &mut ApplicationRelay, room_id: u32) -> Instant {
        let now = Instant::now();
        relay
            .state
            .memberships
            .get_mut(&(1, room_id))
            .unwrap()
            .lease_deadline = now + Duration::from_secs(5);
        now
    }

    #[tokio::test]
    async fn a_compliant_burst_compressed_into_one_window_retires_the_scope_not_the_session() {
        let (mut relay, _) = relay();
        relay.join(1, join([3; 32]), 99);
        relay.completed().await;
        let room_id = relay.state.membership(1, "scene").unwrap().room_id;
        let send = |unreliable| crate::protocol::pulse::decentraland::pulse::ApplicationSend {
            room_id,
            payload: vec![1],
            unreliable,
            ..Default::default()
        };
        let stalled_for_two_seconds = CLIENT_BURST_PACKETS + 2 * CLIENT_PACKETS_PER_SECOND;
        let now = hold_lease(&mut relay, room_id);
        let mut left = vec![];
        for packet in 1..=stalled_for_two_seconds {
            let (out, close) = relay.send(1, send(false), now);
            assert!(
                !close,
                "compliant reliable packet {packet} of {stalled_for_two_seconds} in one server window disconnects the sender"
            );
            left.extend(out);
        }
        assert_eq!(
            left.iter()
                .map(|delivery| (delivery.target, delivery.message.message.clone()))
                .collect::<Vec<_>>(),
            vec![(
                1,
                Some(server_message::Message::ApplicationPeerLeft(
                    crate::protocol::pulse::decentraland::pulse::ApplicationPeerLeft {
                        room_id,
                        peer_id: room_id,
                        roster_version: 2,
                    }
                ))
            )]
        );
        assert!(relay.state.membership(1, "scene").is_none());
        assert!(relay.connections.contains_key(&1));
    }

    #[tokio::test]
    async fn an_over_limit_unreliable_packet_is_dropped_and_the_scope_stays() {
        let (mut relay, _) = relay();
        relay.join(1, join([3; 32]), 99);
        relay.completed().await;
        let room_id = relay.state.membership(1, "scene").unwrap().room_id;
        let now = hold_lease(&mut relay, room_id);
        for _ in 0..200 {
            let send = crate::protocol::pulse::decentraland::pulse::ApplicationSend {
                room_id,
                payload: vec![1],
                unreliable: true,
                ..Default::default()
            };
            assert_eq!(relay.send(1, send, now), (vec![], false));
        }
        assert!(relay.state.membership(1, "scene").is_some());
    }

    #[tokio::test]
    async fn a_reliable_payload_outside_the_advertised_limits_still_disconnects() {
        let (mut relay, _) = relay();
        relay.join(1, join([3; 32]), 99);
        relay.completed().await;
        let room_id = relay.state.membership(1, "scene").unwrap().room_id;
        let now = hold_lease(&mut relay, room_id);
        for payload in [vec![], vec![0; state::MAX_RELIABLE_PAYLOAD + 1]] {
            let send = crate::protocol::pulse::decentraland::pulse::ApplicationSend {
                room_id,
                payload,
                ..Default::default()
            };
            assert_eq!(relay.send(1, send, now), (vec![], true));
        }
        assert!(relay.state.membership(1, "scene").is_some());
    }

    #[tokio::test]
    async fn scope_leave_and_connection_replacement_remove_their_unique_timers() {
        let (mut relay, _) = relay();
        relay.join(1, join([3; 32]), 99);
        relay.completed().await;
        let room_id = relay.state.membership(1, "scene").unwrap().room_id;
        assert_eq!(relay.renewal_times.len(), 1);
        relay.set_renewal(1, room_id, Instant::now());
        assert_eq!(relay.renewal_times.len(), 1);
        relay.leave(1, room_id);
        assert!(relay.renewal_times.is_empty());
        assert!(relay.renewal_by_scope.is_empty());
        relay.join(1, join([3; 32]), 99);
        relay.completed().await;
        assert!(relay.state.membership(1, "scene").unwrap().room_id > room_id);
        relay.disconnect(1);
        assert!(relay.renewal_times.is_empty());
        assert!(relay.renewal_by_scope.is_empty());
        relay.authenticated(1, [4; 32], WALLET.into(), SESSION.into());
        relay.schedule_renewals(Instant::now() + Duration::from_secs(10));
        assert!(!relay.has_jobs());
    }

    #[tokio::test]
    async fn authority_revocation_retires_peer_and_connection_reuse_discards_old_results() {
        let (mut relay, authority) = relay();
        assert!(relay.join(1, join([3; 32]), 99).is_empty());
        let (out, retire) = relay.completed().await;
        assert!(retire.is_empty());
        assert!(
            matches!(out[0].message.message,Some(server_message::Message::ApplicationJoinResult(ref r)) if r.error==0)
        );
        authority.allowed.store(false, Ordering::Relaxed);
        assert!(
            relay
                .maintenance(Instant::now() + Duration::from_millis(60))
                .is_empty()
        );
        let (out, retire) = relay.completed().await;
        assert!(retire.is_empty());
        assert!(matches!(
            out[0].message.message,
            Some(server_message::Message::ApplicationPeerLeft(_))
        ));
        relay.disconnect(1);
        assert!(relay.state.memberships.is_empty());
        authority.allowed.store(true, Ordering::Relaxed);
        relay.authenticated(1, [3; 32], WALLET.into(), SESSION.into());
        relay.join(1, join([3; 32]), 99);
        relay.disconnect(1);
        relay.authenticated(1, [4; 32], WALLET.into(), SESSION.into());
        assert_eq!(relay.completed().await, (vec![], vec![]));
        assert!(relay.state.memberships.is_empty());
    }
    #[tokio::test]
    async fn renewal_cannot_resurrect_a_membership_after_its_old_lease_expires() {
        let (mut relay, _) = relay();
        relay.join(1, join([3; 32]), 99);
        relay.completed().await;
        relay.maintenance(Instant::now() + Duration::from_millis(60));
        let member = relay.state.memberships.values_mut().next().unwrap();
        member.lease_deadline = Instant::now() - Duration::from_millis(1);
        let (out, retire) = relay.completed().await;
        assert!(retire.is_empty());
        assert!(matches!(
            out[0].message.message,
            Some(server_message::Message::ApplicationPeerLeft(_))
        ));
        assert!(
            relay
                .state
                .memberships
                .values()
                .all(|member| member.lease_deadline < Instant::now())
        );
    }

    #[tokio::test]
    async fn unavailable_authority_never_returns_ready_and_join_budget_is_bounded() {
        let (mut relay, authority) = relay();
        authority.allowed.store(false, Ordering::Relaxed);
        relay.join(1, join([3; 32]), 99);
        let (out, _) = relay.completed().await;
        assert!(
            matches!(out[0].message.message,Some(server_message::Message::ApplicationJoinResult(ref r)) if r.error==ApplicationErrorCode::ApplicationUnavailable as i32)
        );
        assert!(relay.state.memberships.is_empty());
        for _ in 0..16 {
            relay.join(1, join([3; 32]), 99);
        }
        let out = relay.join(1, join([3; 32]), 99);
        assert!(
            matches!(out[0].message.message,Some(server_message::Message::ApplicationJoinResult(ref r)) if r.error==ApplicationErrorCode::ApplicationLimit as i32)
        );
        assert_eq!(relay.jobs.len(), 1);
    }
}
