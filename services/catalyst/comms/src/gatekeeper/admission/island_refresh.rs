//! `POST /island-refresh`: a configured legacy owner renews its island credential.
//! Signed fetch over an empty body, `application/x-protobuf` both ways. The
//! caller must be the current owner session of its wallet's realm lane
//! (`social.archipelago_v4_assignments`, the fence `cluster::fence` reads):
//! the session is the chain's single ephemeral key, or the root wallet for a
//! chain without one. The credential is re-minted under the same fence
//! (owner epoch, revision, fencing token) for the island the row names,
//! and answered only if the row did not move while it was minted and the
//! wallet is still not platform-banned (the recorded device and the one the
//! request presents). Without the realm-lane fence
//! (`ARCHIPELAGO_CONTROL_V4_AUDIENCE`) it answers 404, as the old service did
//! by not mounting it; admission, so open while `SOCIAL_WRITES_ENABLED` is
//! off. Every refusal is a bare status.

use std::sync::LazyLock;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{OriginalUri, State};
use axum::http::header::{ACCEPT, CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use prost::Message;

use super::cluster::fence::{
    FenceError, RealmAssignmentReader, RealmAssignmentSnapshot, TokenFence,
};
use super::cluster::gateway::StateGateway;
use super::cluster::subscriber::{ClusterGateway, island_token_not_before};
use crate::Address;
use crate::auth::auth_chain::{AuthLink, AuthLinkType, parse_ephemeral_payload};
use crate::gatekeeper::SharedState;
use crate::gatekeeper::auth::{device_identifier, verify_signed_fetch};
use crate::shared::signed_fetch::SignedPath;

pub const ISLAND_REFRESH_PATH: &str = "/island-refresh";
pub const ISLAND_REFRESH_TTL_SECONDS: u64 = super::cluster::MAX_ISLAND_TOKEN_TTL_SECONDS;
pub const ISLAND_REFRESH_CONTENT_TYPE: &str = "application/x-protobuf";
const ISLAND_REFRESH_DEADLINE: Duration = Duration::from_secs(2);
const MAX_CONCURRENT_ISLAND_REFRESHES: usize = 128;
static ISLAND_REFRESH_PERMITS: LazyLock<tokio::sync::Semaphore> =
    LazyLock::new(|| tokio::sync::Semaphore::new(MAX_CONCURRENT_ISLAND_REFRESHES));

#[derive(Clone, PartialEq, Message)]
pub struct IslandRefreshResponse {
    #[prost(string, tag = "1")]
    pub adapter: String,
    #[prost(uint32, tag = "2")]
    pub expires_in_seconds: u32,
}

pub struct IslandRefreshError(StatusCode);

impl IntoResponse for IslandRefreshError {
    fn into_response(self) -> Response {
        Response::builder()
            .status(self.0)
            .header(CONTENT_TYPE, ISLAND_REFRESH_CONTENT_TYPE)
            .header(CACHE_CONTROL, "no-store")
            .body(Body::empty())
            .expect("static island refresh response")
    }
}

pub async fn refresh(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, IslandRefreshError> {
    require_protobuf_headers(&headers)?;
    if !body.is_empty() {
        return Err(IslandRefreshError(StatusCode::BAD_REQUEST));
    }
    let Some(reader) = state.assignment_reader() else {
        return Err(IslandRefreshError(StatusCode::NOT_FOUND));
    };
    let _permit = ISLAND_REFRESH_PERMITS
        .try_acquire()
        .map_err(|_| IslandRefreshError(StatusCode::SERVICE_UNAVAILABLE))?;
    tokio::time::timeout(
        ISLAND_REFRESH_DEADLINE,
        refresh_bounded(
            &state,
            &*reader,
            &headers,
            &SignedPath::at(&uri, ISLAND_REFRESH_PATH),
        ),
    )
    .await
    .map_err(|_| IslandRefreshError(StatusCode::SERVICE_UNAVAILABLE))?
}

async fn refresh_bounded(
    state: &SharedState,
    assignments: &dyn RealmAssignmentReader,
    headers: &HeaderMap,
    path: &SignedPath,
) -> Result<Response, IslandRefreshError> {
    let chain = crate::shared::signed_fetch::extract_auth_chain(headers)
        .map_err(|_| IslandRefreshError(StatusCode::BAD_REQUEST))?;
    let signed = verify_signed_fetch(headers, "post", path, &[], state.eip1654())
        .await
        .map_err(|error| {
            IslandRefreshError(
                StatusCode::from_u16(error.status).unwrap_or(StatusCode::BAD_REQUEST),
            )
        })?;
    let session = effective_session(&chain, &signed.signer)
        .map_err(|_| IslandRefreshError(StatusCode::FORBIDDEN))?;
    let wallet = signed.signer.as_str();
    let device_id = device_identifier(&signed.metadata);
    ensure_not_banned(state, &signed.signer, device_id.as_deref()).await?;

    let before = read_current(assignments, wallet, &session).await?;
    let stamp = before
        .pulse_admission
        .as_ref()
        .ok_or(IslandRefreshError(StatusCode::SERVICE_UNAVAILABLE))?;
    // Preserve the stored credential's structural/time/SQL readiness.
    // The gateway and post-mint checks supply the two fresh Pulse observations.
    if !state.island_stamp_ready_sql(stamp).await {
        return Err(IslandRefreshError(StatusCode::SERVICE_UNAVAILABLE));
    }
    let plan = stamp.snapshot();
    let fence = TokenFence {
        audience: before.audience.clone(),
        lane: before.lane.clone(),
        owner_session: before.owner_session.clone(),
        owner_epoch: before.owner_epoch,
        assignment_revision: before.assignment_revision,
        fencing_token: before.fencing_token,
    };
    let not_before = island_token_not_before(
        stamp.token_not_before,
        chrono::Utc::now().timestamp_millis().max(0) as u64,
    )
    .max(stamp.safety_token_floor);
    let mut candidate =
        super::cluster::fence::PulseAdmissionStamp::from_snapshot(&plan, not_before)
            .ok_or(IslandRefreshError(StatusCode::SERVICE_UNAVAILABLE))?;
    candidate.safety_generation = stamp.safety_generation;
    candidate.safety_token_floor = stamp.safety_token_floor;
    let adapter = StateGateway::new(state.clone())
        .mint_fenced_connection_string(
            &candidate,
            &before.wallet,
            &before.island_id,
            ISLAND_REFRESH_TTL_SECONDS,
            &fence,
            Some(not_before),
        )
        .await
        .map_err(|_| IslandRefreshError(StatusCode::SERVICE_UNAVAILABLE))?;

    if !state.verify_island_stamp(&candidate).await {
        return Err(IslandRefreshError(StatusCode::FORBIDDEN));
    }
    // Pulse validation can await broker replies. Recheck the independent local
    // authority afterwards, then retain synchronous time/receipt safety across
    // these final SQL awaits without opening another broker wait.
    ensure_not_banned(state, &signed.signer, device_id.as_deref()).await?;
    let after = read_current(assignments, wallet, &session).await?;
    if after != before
        || !state
            .island_fenced_stamp_ready_sql(&candidate, &fence, device_id.as_deref())
            .await
    {
        return Err(IslandRefreshError(StatusCode::FORBIDDEN));
    }
    let response = IslandRefreshResponse {
        adapter,
        expires_in_seconds: ISLAND_REFRESH_TTL_SECONDS as u32,
    };
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, ISLAND_REFRESH_CONTENT_TYPE)
        .header(CACHE_CONTROL, "no-store")
        .body(Body::from(response.encode_to_vec()))
        .expect("static island refresh response"))
}

/// The chain's single ephemeral key (lowercased), or the verified root
/// wallet for a chain without one; two ephemeral delegations are ambiguous.
fn effective_session(chain: &[AuthLink], signer: &Address) -> Result<String, ()> {
    let mut session = None;
    for link in chain {
        if matches!(
            link.link_type,
            AuthLinkType::EcdsaEphemeral | AuthLinkType::EcdsaEip1654Ephemeral
        ) {
            if session.is_some() {
                return Err(());
            }
            let (_, address, _) = parse_ephemeral_payload(&link.payload).map_err(|_| ())?;
            session = Some(address.to_lowercase());
        }
    }
    Ok(session.unwrap_or_else(|| signer.as_str().to_string()))
}

async fn read_current(
    assignments: &dyn RealmAssignmentReader,
    wallet: &str,
    session: &str,
) -> Result<RealmAssignmentSnapshot, IslandRefreshError> {
    assignments
        .current_realm_assignment(wallet, session)
        .await
        .map_err(|error| match error {
            FenceError::Unavailable => IslandRefreshError(StatusCode::SERVICE_UNAVAILABLE),
            FenceError::Conflict | FenceError::Unowned | FenceError::Invalid => {
                IslandRefreshError(StatusCode::FORBIDDEN)
            }
        })
}

/// The device recorded for the wallet and the one the request presents are
/// both checked, so a supplied device cannot hide a recorded device ban.
async fn ensure_not_banned(
    state: &SharedState,
    wallet: &Address,
    presented_device_id: Option<&str>,
) -> Result<(), IslandRefreshError> {
    let unavailable = |_| IslandRefreshError(StatusCode::SERVICE_UNAVAILABLE);
    let recorded = crate::gatekeeper::moderation::admission_ban(
        state.policy.as_ref(),
        state.bans.as_ref(),
        wallet,
        None,
        None,
        chrono::Utc::now().naive_utc(),
    )
    .await
    .map_err(unavailable)?;
    let presented = match presented_device_id {
        Some(device_id) => crate::gatekeeper::moderation::admission_ban(
            state.policy.as_ref(),
            state.bans.as_ref(),
            wallet,
            None,
            Some(device_id),
            chrono::Utc::now().naive_utc(),
        )
        .await
        .map_err(unavailable)?,
        None => None,
    };
    if recorded.is_some() || presented.is_some() {
        return Err(IslandRefreshError(StatusCode::FORBIDDEN));
    }
    Ok(())
}

fn require_protobuf_headers(headers: &HeaderMap) -> Result<(), IslandRefreshError> {
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if content_type != Some(ISLAND_REFRESH_CONTENT_TYPE) {
        return Err(IslandRefreshError(StatusCode::UNSUPPORTED_MEDIA_TYPE));
    }
    let accepts_protobuf = headers
        .get_all(ACCEPT)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|value| value.split(';').next())
        .map(str::trim)
        .any(|value| value == ISLAND_REFRESH_CONTENT_TYPE);
    accepts_protobuf
        .then_some(())
        .ok_or(IslandRefreshError(StatusCode::NOT_ACCEPTABLE))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy::signers::Signer as _;
    use alloy::signers::local::PrivateKeySigner;
    use axum::http::{HeaderName, HeaderValue, Request};
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;
    use crate::gatekeeper::admission::cluster::ClusterConfig;
    use crate::gatekeeper::moderation::store::{CreateBan, UpsertPlayerConnection};
    use crate::gatekeeper::{CommsState, Settings, router};
    use crate::http::Outbound;
    use crate::interfaces::test_support::{StubPolicy, StubScenes};
    use crate::shared::signed_fetch::{
        AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
    };

    const SESSION_A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn effective_session_is_the_ephemeral_authority() {
        let signer = Address::parse("0x1111111111111111111111111111111111111111").unwrap();
        let link = |link_type, payload: &str| AuthLink {
            link_type,
            payload: payload.into(),
            signature: Some("unused".into()),
        };
        let chain = vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: signer.as_str().into(),
                signature: None,
            },
            link(
                AuthLinkType::EcdsaEphemeral,
                "Decentraland Login\nEphemeral address: 0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\nExpiration: 2099-01-01T00:00:00Z",
            ),
            link(AuthLinkType::EcdsaSignedEntity, "signed fetch payload"),
        ];
        assert_eq!(effective_session(&chain, &signer).as_deref(), Ok(SESSION_A));
        assert_eq!(
            effective_session(&chain[..1], &signer).as_deref(),
            Ok(signer.as_str()),
            "a wallet-only chain is its own session"
        );
        let doubled = vec![chain[0].clone(), chain[1].clone(), chain[1].clone()];
        assert_eq!(
            effective_session(&doubled, &signer),
            Err(()),
            "two delegations are ambiguous"
        );
    }

    const AUDIENCE: &str = "island-refresh-test";

    struct Delegate {
        root: PrivateKeySigner,
        session: PrivateKeySigner,
    }

    impl Delegate {
        fn new() -> Self {
            Self {
                root: PrivateKeySigner::random(),
                session: PrivateKeySigner::random(),
            }
        }
        fn wallet(&self) -> String {
            format!("{:#x}", self.root.address())
        }
        fn session(&self) -> String {
            format!("{:#x}", self.session.address())
        }
        async fn headers(&self, metadata: &str) -> HeaderMap {
            let timestamp = chrono::Utc::now().timestamp_millis().to_string();
            let payload = build_payload_v6("post", ISLAND_REFRESH_PATH, &timestamp, metadata);
            let expiration = (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
            let ephemeral = format!(
                "Decentraland Login\nEphemeral address: {:#x}\nExpiration: {expiration}",
                self.session.address()
            );
            let links = [
                json!({ "type": "SIGNER", "payload": self.wallet(), "signature": "" }),
                json!({ "type": "ECDSA_EPHEMERAL", "payload": ephemeral, "signature": self.root.sign_message(ephemeral.as_bytes()).await.unwrap().to_string() }),
                json!({ "type": "ECDSA_SIGNED_ENTITY", "payload": payload, "signature": self.session.sign_message(payload.as_bytes()).await.unwrap().to_string() }),
            ];
            let mut headers = HeaderMap::new();
            headers.insert(
                AUTH_TIMESTAMP_HEADER,
                HeaderValue::from_str(&timestamp).unwrap(),
            );
            headers.insert(
                AUTH_METADATA_HEADER,
                HeaderValue::from_str(metadata).unwrap(),
            );
            for (i, link) in links.iter().enumerate() {
                headers.insert(
                    HeaderName::from_bytes(format!("{AUTH_CHAIN_HEADER_PREFIX}{i}").as_bytes())
                        .unwrap(),
                    HeaderValue::from_str(&link.to_string()).unwrap(),
                );
            }
            headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static(ISLAND_REFRESH_CONTENT_TYPE),
            );
            headers.insert(
                ACCEPT,
                HeaderValue::from_static(ISLAND_REFRESH_CONTENT_TYPE),
            );
            headers
        }
    }

    async fn seed(pool: &sqlx::PgPool, wallet: &str, session: &str, island: &str, lapsed: bool) {
        let safety = super::super::cluster::safety::SafetyStore::new(pool.clone())
            .snapshot(wallet)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO social.archipelago_v4_assignments \
                (deployment_audience, owner_address, lane_key, authority_incarnation, assignment_revision, owner_session, owner_epoch, fencing_token, assignment_json, updated_at) \
             VALUES ($1, $2, 'realm', 'authority-refresh-test', 3, $3, 7, 11, $4, timezone('UTC', now()) - CASE WHEN $5 THEN interval '5 minutes' ELSE interval '0' END)",
        )
        .bind(AUDIENCE)
        .bind(wallet)
        .bind(session)
        .bind(json!({ "islandId": island, "connectionString": "livekit:expired", "tokenExpiresAtUnix": 1, "pulseAdmission": { "wallet": wallet, "session": session, "clusterId": island.strip_prefix("island-").unwrap_or(island), "realm": "realm", "sourceIncarnation": "refresh-test", "assignmentVersion": 1, "tokenNotBefore": 0, "credentialNotBefore": 0, "safetyGeneration":safety.generation,"safetyTokenFloor":safety.token_floor } }))
        .bind(lapsed)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn app(pool: sqlx::PgPool) -> axum::Router {
        app_with_plan(pool, 0).await
    }

    async fn app_with_plan(pool: sqlx::PgPool, mode: usize) -> axum::Router {
        app_with_lookup_hook(pool, mode, None).await
    }

    async fn app_with_lookup_hook(
        pool: sqlx::PgPool,
        mode: usize,
        hook: Option<Arc<dyn Fn(usize) + Send + Sync>>,
    ) -> axum::Router {
        router(state_with_lookup_hook(pool, mode, hook).await)
    }

    async fn state_with_lookup_hook(
        pool: sqlx::PgPool,
        mode: usize,
        hook: Option<Arc<dyn Fn(usize) + Send + Sync>>,
    ) -> SharedState {
        let config = ClusterConfig {
            control_v4_audience: Some(AUDIENCE.into()),
            nats_url: Some("nats://127.0.0.1:1".into()),
            ..ClusterConfig::default()
        };
        let state = CommsState::new(
            pool,
            Outbound::for_tests(),
            Settings::for_tests(),
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(crate::interfaces::test_support::StubDirectory::default()),
        )
        .with_cluster(config)
        .unwrap();
        use crate::gatekeeper::bus::{InProcessBus, NatsBus};
        use crate::protocol::pulse::decentraland::pulse::{PeerClusterSnapshot, PendingTakeover};
        let bus = Arc::new(InProcessBus::new());
        let calls = std::sync::atomic::AtomicUsize::new(0);
        bus.respond_to_requests(Arc::new(move |subject, payload| {
            let lookup = crate::protocol::bus::cluster::ClusterLookup::decode(subject, payload)
                .map_err(|_| crate::gatekeeper::bus::RequestError::Invalid)?;
            let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if let Some(hook) = &hook {
                hook(n);
            }
            let snapshot = PeerClusterSnapshot {
                wallet: lookup.wallet,
                session: lookup.session,
                cluster_id: "C7".into(),
                realm: "realm".into(),
                pass: 1,
                source_incarnation: "refresh-test".into(),
                assignment_version: if (mode == 2 && n >= 1) || mode == 3 {
                    2
                } else {
                    1
                },
                token_not_before: Some(if mode == 4 { 1 } else { 0 }),
                pending_takeovers: if mode == 1 {
                    vec![PendingTakeover {
                        takeover_id: "pending".into(),
                        displaced_session: SESSION_A.into(),
                        displaced_cluster_id: "C7".into(),
                        displaced_realm: "realm".into(),
                    }]
                } else {
                    vec![]
                },
            };
            Ok(snapshot.encode_to_vec())
        }));
        bus.connect();
        let state = Arc::new(state.with_bus(bus.clone()));
        assert!(
            state.island_subscriber().upgrade().is_none(),
            "replica fixture starts no cleanup subscriber"
        );
        state
    }

    async fn call(
        app: &axum::Router,
        headers: HeaderMap,
        body: &'static [u8],
    ) -> (StatusCode, Bytes) {
        let mut req = Request::post(ISLAND_REFRESH_PATH);
        for (k, v) in headers.iter() {
            req = req.header(k, v);
        }
        let resp = app
            .clone()
            .oneshot(req.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        (
            status,
            axum::body::to_bytes(resp.into_body(), 1 << 20)
                .await
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn refresh_rejects_pending_cleanup_and_authority_change_during_mint() {
        let pool = crate::db::scratch::pool(4)
            .await
            .expect("mandatory disposable PostgreSQL fixture");
        for mode in [1, 2, 3] {
            let owner = Delegate::new();
            seed(&pool, &owner.wallet(), &owner.session(), "island-C7", false).await;
            let app = app_with_plan(pool.clone(), mode).await;
            assert_eq!(
                call(&app, owner.headers("{}").await, b"").await.0,
                if mode != 2 {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::FORBIDDEN
                }
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn refresh_final_pulse_wait_preserves_sql_fence_and_ban_authority() {
        let pool = crate::db::scratch::pool(4)
            .await
            .expect("mandatory disposable PostgreSQL");
        let mut statuses = Vec::new();
        for change in [
            "unchanged",
            "owner",
            "revision",
            "ban",
            "generation",
            "floor",
            "hazard",
        ] {
            let owner = Delegate::new();
            seed(&pool, &owner.wallet(), &owner.session(), "island-C7", false).await;
            let (entered, waiting) = std::sync::mpsc::channel();
            let release = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
            let gate = release.clone();
            let hook = Arc::new(move |n| {
                if n == 1 {
                    // Single final post-mint authority read; Pulse identity stays unchanged
                    entered.send(()).unwrap();
                    tokio::task::block_in_place(|| {
                        let (lock, ready) = &*gate;
                        let (guard, timeout) = ready
                            .wait_timeout_while(
                                lock.lock().unwrap(),
                                Duration::from_secs(5),
                                |released| !*released,
                            )
                            .unwrap();
                        assert!(
                            *guard && !timeout.timed_out(),
                            "test must release paused lookup"
                        );
                    });
                }
            });
            let app = app_with_lookup_hook(pool.clone(), 0, Some(hook)).await;
            let headers = owner.headers("{}").await;
            let work = tokio::spawn(async move { call(&app, headers, b"").await });
            tokio::task::spawn_blocking(move || {
                waiting.recv_timeout(Duration::from_secs(5)).unwrap()
            })
            .await
            .unwrap();
            match change {
                "owner" => {
                    sqlx::query("UPDATE social.archipelago_v4_assignments SET owner_session=$2, owner_epoch=owner_epoch+1, fencing_token=fencing_token+1 WHERE owner_address=$1").bind(owner.wallet()).bind(SESSION_A).execute(&pool).await.unwrap();
                }
                "revision" => {
                    sqlx::query("UPDATE social.archipelago_v4_assignments SET assignment_revision=assignment_revision+1, fencing_token=fencing_token+1 WHERE owner_address=$1").bind(owner.wallet()).execute(&pool).await.unwrap();
                }
                "ban" => {
                    sqlx::query("INSERT INTO social.user_bans (banned_address,banned_by,reason,banned_at) VALUES ($1,'test','test',timezone('UTC',now()))").bind(owner.wallet()).execute(&pool).await.unwrap();
                }
                "generation" => {
                    sqlx::query("UPDATE social.island_admission_fences SET generation=generation+1 WHERE wallet=$1").bind(owner.wallet()).execute(&pool).await.unwrap();
                }
                "floor" => {
                    sqlx::query("UPDATE social.island_admission_fences SET token_floor=floor(extract(epoch FROM clock_timestamp()))::bigint+10 WHERE wallet=$1").bind(owner.wallet()).execute(&pool).await.unwrap();
                }
                "hazard" => {
                    let store = super::super::cluster::safety::SafetyStore::new(pool.clone());
                    let operation = crate::protocol::pulse::decentraland::pulse::PendingTakeover {
                        takeover_id: "final-wait".into(),
                        displaced_session: SESSION_A.into(),
                        displaced_cluster_id: "C7".into(),
                        displaced_realm: "realm".into(),
                    };
                    store
                        .prepare_takeover(
                            &owner.wallet(),
                            "independent-source",
                            &owner.session(),
                            &operation,
                        )
                        .await
                        .unwrap();
                    store
                        .begin_takeover(&owner.wallet(), "independent-source", &operation, 0)
                        .await
                        .unwrap()
                        .unwrap();
                }
                _ => {}
            }
            *release.0.lock().unwrap() = true;
            release.1.notify_one();
            let (status, body) = work.await.unwrap();
            if status != StatusCode::OK {
                assert!(body.is_empty(), "rejection returns no credential");
            }
            statuses.push((change, status));
        }
        assert_eq!(
            statuses,
            vec![
                ("unchanged", StatusCode::OK),
                ("owner", StatusCode::FORBIDDEN),
                ("revision", StatusCode::FORBIDDEN),
                ("ban", StatusCode::FORBIDDEN),
                ("generation", StatusCode::FORBIDDEN),
                ("floor", StatusCode::FORBIDDEN),
                ("hazard", StatusCode::FORBIDDEN)
            ]
        );
    }

    #[tokio::test]
    async fn refresh_replica_uses_two_authoritative_reads_and_rejects_final_pulse_only_change() {
        let pool = crate::db::scratch::pool(4).await.expect("real PostgreSQL");
        for mutate in [false, true] {
            let owner = Delegate::new();
            seed(&pool, &owner.wallet(), &owner.session(), "island-C7", false).await;
            let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let count = calls.clone();
            let hook = Arc::new(move |n| {
                assert_eq!(count.fetch_add(1, std::sync::atomic::Ordering::SeqCst), n);
            });
            let app =
                app_with_lookup_hook(pool.clone(), if mutate { 2 } else { 0 }, Some(hook)).await;
            let (status, body) = call(&app, owner.headers("{}").await, b"").await;
            assert_eq!(
                calls.load(std::sync::atomic::Ordering::SeqCst),
                2,
                "gateway and post-mint Pulse checks only"
            );
            assert_eq!(
                status,
                if mutate {
                    StatusCode::FORBIDDEN
                } else {
                    StatusCode::OK
                }
            );
            if mutate {
                assert!(body.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn refresh_preserves_original_credential_structure_and_time_readiness() {
        let pool = crate::db::scratch::pool(4).await.expect("real PostgreSQL");
        for mode in [
            "old-valid",
            "future",
            "beyond-consumer-bound",
            "below-safety-floor",
            "below-pulse-floor",
        ] {
            let owner = Delegate::new();
            seed(&pool, &owner.wallet(), &owner.session(), "island-C7", false).await;
            let original = match mode {
                "future" => chrono::Utc::now().timestamp() + 30,
                "beyond-consumer-bound" => {
                    crate::protocol::bus::cluster::MAX_CONSUMER_SECONDS as i64 + 1
                }
                _ => 0,
            };
            sqlx::query("UPDATE social.archipelago_v4_assignments SET assignment_json=jsonb_set(assignment_json,'{pulseAdmission,credentialNotBefore}',to_jsonb($2::bigint)) WHERE owner_address=$1").bind(owner.wallet()).bind(original).execute(&pool).await.unwrap();
            if mode == "below-safety-floor" {
                sqlx::query(
                    "UPDATE social.island_admission_fences SET token_floor=1 WHERE wallet=$1",
                )
                .bind(owner.wallet())
                .execute(&pool)
                .await
                .unwrap();
                sqlx::query("UPDATE social.archipelago_v4_assignments SET assignment_json=jsonb_set(assignment_json,'{pulseAdmission,safetyTokenFloor}','1') WHERE owner_address=$1").bind(owner.wallet()).execute(&pool).await.unwrap();
            }
            if mode == "below-pulse-floor" {
                sqlx::query("UPDATE social.archipelago_v4_assignments SET assignment_json=jsonb_set(assignment_json,'{pulseAdmission,tokenNotBefore}','1') WHERE owner_address=$1").bind(owner.wallet()).execute(&pool).await.unwrap();
            }
            let app = app_with_plan(
                pool.clone(),
                if mode == "below-pulse-floor" { 4 } else { 0 },
            )
            .await;
            let (status, body) = call(&app, owner.headers("{}").await, b"").await;
            assert_eq!(
                status,
                match mode {
                    "old-valid" => StatusCode::OK,
                    "future" => StatusCode::SERVICE_UNAVAILABLE,
                    _ => StatusCode::FORBIDDEN, // Existing reader rejects malformed stored stamps.
                },
                "{mode}"
            );
            if mode != "old-valid" {
                assert!(
                    body.is_empty(),
                    "{mode}: no normalized replacement credential"
                );
            }
        }
    }

    struct FinalReadBarrier {
        inner: Arc<dyn RealmAssignmentReader>,
        reads: std::sync::atomic::AtomicUsize,
        entered: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }
    #[async_trait::async_trait]
    impl RealmAssignmentReader for FinalReadBarrier {
        async fn current_realm_assignment(
            &self,
            wallet: &str,
            session: &str,
        ) -> Result<RealmAssignmentSnapshot, FenceError> {
            let snapshot = self.inner.current_realm_assignment(wallet, session).await?;
            if self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Ok(snapshot)
        }
    }
    async fn wait_for_final_sql_lock(pool: &sqlx::PgPool, label: &str, fragment: &str) {
        tokio::time::timeout(Duration::from_secs(1),async {
            loop {
                let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock' AND query LIKE $2)")
                    .bind(label).bind(format!("%{fragment}%")).fetch_one(pool).await.unwrap();
                if waiting{break;}tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.expect("final verifier actually awaited the intended SQL lock");
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn refresh_final_owner_lock_wait_rechecks_wallet_recorded_and_presented_device_bans() {
        if std::env::var("CATALYST_TEST_DATABASE_URL")
            .ok()
            .is_none_or(|value| value.trim().is_empty())
        {
            assert_ne!(
                std::env::var("GITHUB_ACTIONS").as_deref(),
                Ok("true"),
                "mandatory PostgreSQL in CI"
            );
            eprintln!(
                "local skip: durable final-boundary regression requires CATALYST_TEST_DATABASE_URL"
            );
            return;
        }

        let pool = crate::db::scratch::pool(8)
            .await
            .expect("mandatory disposable PostgreSQL final-policy fixture");
        for change in [
            "unchanged",
            "wallet",
            "recorded-device",
            "presented-device",
            "owner-expiry",
            "short-wait",
        ] {
            let owner = Delegate::new();
            seed(&pool, &owner.wallet(), &owner.session(), "island-C7", false).await;
            let label = format!("final-ban-{}", uuid::Uuid::new_v4());
            let worker_pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(3)
                .connect_with(
                    pool.connect_options()
                        .as_ref()
                        .clone()
                        .application_name(&label),
                )
                .await
                .unwrap();
            let device = format!("device-{}", uuid::Uuid::new_v4());
            if change == "recorded-device" {
                sqlx::query(
                    "INSERT INTO social.player_connections(address,device_id) VALUES ($1,$2)",
                )
                .bind(owner.wallet())
                .bind(&device)
                .execute(&pool)
                .await
                .unwrap();
            }
            let state = state_with_lookup_hook(worker_pool.clone(), 0, None).await;
            let reader = Arc::new(FinalReadBarrier {
                inner: state.assignment_reader().unwrap(),
                reads: 0.into(),
                entered: Default::default(),
                release: Default::default(),
            });
            let metadata = if change == "presented-device" {
                json!({"deviceIdentifier":device}).to_string()
            } else {
                "{}".into()
            };
            let headers = owner.headers(&metadata).await;
            if matches!(change, "owner-expiry" | "short-wait") {
                sqlx::query("UPDATE social.archipelago_v4_assignments SET updated_at=clock_timestamp()-interval '89 seconds' WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'").bind(AUDIENCE).bind(owner.wallet()).execute(&pool).await.unwrap();
            }
            let gate = reader.clone();
            let task = tokio::spawn(async move {
                let uri = ISLAND_REFRESH_PATH.parse().unwrap();
                tokio::time::timeout(
                    ISLAND_REFRESH_DEADLINE,
                    refresh_bounded(
                        &state,
                        &*gate,
                        &headers,
                        &SignedPath::at(&uri, ISLAND_REFRESH_PATH),
                    ),
                )
                .await
                .expect("hard2s refresh budget")
                .map(IntoResponse::into_response)
                .unwrap_or_else(IntoResponse::into_response)
            });
            tokio::time::timeout(ISLAND_REFRESH_DEADLINE, reader.entered.notified())
                .await
                .expect("refresh prelude reached the final owner read");
            let mut held = pool.begin().await.unwrap();
            sqlx::query("SELECT 1 FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm' FOR UPDATE").bind(AUDIENCE).bind(owner.wallet()).fetch_one(&mut *held).await.unwrap();
            reader.release.notify_one();
            wait_for_final_sql_lock(&pool, &label, "FROM social.archipelago_v4_assignments").await;
            if matches!(change, "owner-expiry" | "short-wait") {
                let fresh_before:bool=sqlx::query_scalar("SELECT updated_at>=clock_timestamp()-interval '90 seconds' FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'").bind(AUDIENCE).bind(owner.wallet()).fetch_one(&pool).await.unwrap();
                assert!(
                    fresh_before,
                    "{change}: owner is fresh when the final row wait is observed"
                );
            }
            if change == "owner-expiry" {
                // No row mutation while blocked: let the existing lease expire
                // naturally after the SELECT's pre-lock projection. Keep both
                // the production1.5s lock and2s refresh budgets intact.
                tokio::time::sleep(Duration::from_millis(1050)).await;
            } else if change == "short-wait" {
                tokio::time::sleep(Duration::from_millis(100)).await;
            } else if change != "unchanged" {
                let banned = if change == "wallet" {
                    owner.wallet()
                } else {
                    format!("{:#x}", PrivateKeySigner::random().address())
                };
                sqlx::query("INSERT INTO social.user_bans(banned_address,banned_by,reason,banned_device_id) VALUES ($1,'test','final wait',$2)").bind(banned).bind((change!="wallet").then_some(&device)).execute(&pool).await.unwrap();
            }
            if matches!(change, "owner-expiry" | "short-wait") {
                let fresh_after:bool=sqlx::query_scalar("SELECT updated_at>=clock_timestamp()-interval '90 seconds' FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'").bind(AUDIENCE).bind(owner.wallet()).fetch_one(&pool).await.unwrap();
                assert_eq!(
                    fresh_after,
                    change == "short-wait",
                    "{change}: database expiry immediately before releasing owner lock"
                );
                let still_waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock' AND query LIKE '%FROM social.archipelago_v4_assignments%')").bind(&label).fetch_one(&pool).await.unwrap();
                assert!(
                    still_waiting,
                    "{change}: final authorizer is still waiting, not timed out"
                );
                eprintln!(
                    "{change}: database fresh before=true, before release={fresh_after}; final authorizer still waiting"
                );
            }
            held.commit().await.unwrap();
            let response = task.await.unwrap();
            assert_eq!(
                response.status(),
                if matches!(change, "unchanged" | "short-wait") {
                    StatusCode::OK
                } else {
                    StatusCode::FORBIDDEN
                },
                "{change}"
            );
            if !matches!(change, "unchanged" | "short-wait") {
                assert!(
                    axum::body::to_bytes(response.into_body(), 1024)
                        .await
                        .unwrap()
                        .is_empty()
                );
            }
            worker_pool.close().await;
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn refresh_final_wallet_wait_preserves_exact_committed_fence() {
        if std::env::var("CATALYST_TEST_DATABASE_URL")
            .ok()
            .is_none_or(|value| value.trim().is_empty())
        {
            assert_ne!(
                std::env::var("GITHUB_ACTIONS").as_deref(),
                Ok("true"),
                "mandatory PostgreSQL in CI"
            );
            eprintln!(
                "local skip: durable final-boundary regression requires CATALYST_TEST_DATABASE_URL"
            );
            return;
        }

        let pool = crate::db::scratch::pool(8)
            .await
            .expect("mandatory disposable PostgreSQL final-fence fixture");
        for change in ["unchanged", "revision", "epoch", "token"] {
            let owner = Delegate::new();
            seed(&pool, &owner.wallet(), &owner.session(), "island-C7", false).await;
            let label = format!("final-fence-{}", uuid::Uuid::new_v4());
            let worker_pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(3)
                .connect_with(
                    pool.connect_options()
                        .as_ref()
                        .clone()
                        .application_name(&label),
                )
                .await
                .unwrap();
            let state = state_with_lookup_hook(worker_pool.clone(), 0, None).await;
            let reader = Arc::new(FinalReadBarrier {
                inner: state.assignment_reader().unwrap(),
                reads: 0.into(),
                entered: Default::default(),
                release: Default::default(),
            });
            let headers = owner.headers("{}").await;
            let gate = reader.clone();
            let task = tokio::spawn(async move {
                let uri = ISLAND_REFRESH_PATH.parse().unwrap();
                tokio::time::timeout(
                    ISLAND_REFRESH_DEADLINE,
                    refresh_bounded(
                        &state,
                        &*gate,
                        &headers,
                        &SignedPath::at(&uri, ISLAND_REFRESH_PATH),
                    ),
                )
                .await
                .expect("hard2s refresh budget")
                .map(IntoResponse::into_response)
                .unwrap_or_else(IntoResponse::into_response)
            });
            tokio::time::timeout(ISLAND_REFRESH_DEADLINE, reader.entered.notified())
                .await
                .expect("refresh prelude reached the final fence read");
            let store = super::super::cluster::safety::SafetyStore::new(pool.clone());
            let mut held = store.lock(&owner.wallet()).await.unwrap();
            reader.release.notify_one();
            wait_for_final_sql_lock(&pool, &label, "social.island_admission_fences").await;
            match change {
                "revision" => {
                    sqlx::query("UPDATE social.archipelago_v4_assignments SET assignment_revision=assignment_revision+1 WHERE owner_address=$1").bind(owner.wallet()).execute(&mut *held.tx).await.unwrap();
                }
                "epoch" => {
                    sqlx::query("UPDATE social.archipelago_v4_assignments SET owner_epoch=owner_epoch+1 WHERE owner_address=$1").bind(owner.wallet()).execute(&mut *held.tx).await.unwrap();
                }
                "token" => {
                    sqlx::query("UPDATE social.archipelago_v4_assignments SET fencing_token=fencing_token+1 WHERE owner_address=$1").bind(owner.wallet()).execute(&mut *held.tx).await.unwrap();
                }
                _ => {}
            }
            held.commit().await.unwrap();
            let response = task.await.unwrap();
            assert_eq!(
                response.status(),
                if change == "unchanged" {
                    StatusCode::OK
                } else {
                    StatusCode::FORBIDDEN
                },
                "{change}"
            );
            if change != "unchanged" {
                assert!(
                    axum::body::to_bytes(response.into_body(), 1024)
                        .await
                        .unwrap()
                        .is_empty()
                );
            }
            worker_pool.close().await;
        }
    }

    /// The current delegated owner gets a fenced credential for the island
    /// its row names without the row moving; a lapsed or a different session
    /// is refused; a device ban recorded for the wallet refuses even when the
    /// request presents another device; the headers and the empty body are
    /// mandatory.
    #[tokio::test]
    async fn the_current_owner_refreshes_under_the_same_fence() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let app = app(pool.clone()).await;
        let owner = Delegate::new();
        seed(&pool, &owner.wallet(), &owner.session(), "island-C7", false).await;
        let (status, body) = call(&app, owner.headers("{}").await, b"").await;
        assert_eq!(status, StatusCode::OK);
        let response = IslandRefreshResponse::decode(body.as_ref()).unwrap();
        assert_eq!(
            response.expires_in_seconds,
            ISLAND_REFRESH_TTL_SECONDS as u32
        );
        let token = response.adapter.split_once("access_token=").unwrap().1;
        let claims: serde_json::Value = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(token.split('.').nth(1).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(claims["video"]["room"], "island-C7");
        let owner_meta: serde_json::Value =
            serde_json::from_str(claims["metadata"].as_str().unwrap()).unwrap();
        assert_eq!(
            owner_meta["catalyrstIsland"],
            json!({ "version": 2, "wallet": owner.wallet(), "session": owner.session(), "audience": AUDIENCE, "lane": "realm", "ownerEpoch": 7, "assignmentRevision": 3, "fencingToken": 11 })
        );
        let revision: i64 = sqlx::query_scalar("SELECT assignment_revision FROM social.archipelago_v4_assignments WHERE owner_address = $1").bind(owner.wallet()).fetch_one(&pool).await.unwrap();
        assert_eq!(revision, 3, "a refresh never mutates the row");

        let (status, _) = call(&app, owner.headers("{}").await, b"x").await;
        assert_eq!(
            status,
            StatusCode::PAYLOAD_TOO_LARGE,
            "the body must be empty (the route's zero body limit)"
        );
        let mut plain = owner.headers("{}").await;
        plain.insert(ACCEPT, HeaderValue::from_static("application/json"));
        assert_eq!(call(&app, plain, b"").await.0, StatusCode::NOT_ACCEPTABLE);
        let mut json_body = owner.headers("{}").await;
        json_body.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        assert_eq!(
            call(&app, json_body, b"").await.0,
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );

        let other_session = Delegate {
            root: owner.root.clone(),
            session: PrivateKeySigner::random(),
        };
        assert_eq!(
            call(&app, other_session.headers("{}").await, b"").await.0,
            StatusCode::FORBIDDEN,
            "another session of the wallet is not the owner"
        );
        let lapsed = Delegate::new();
        seed(
            &pool,
            &lapsed.wallet(),
            &lapsed.session(),
            "island-C8",
            true,
        )
        .await;
        assert_eq!(
            call(&app, lapsed.headers("{}").await, b"").await.0,
            StatusCode::FORBIDDEN,
            "a lapsed owner cannot refresh old credentials"
        );

        let banned = Delegate::new();
        seed(
            &pool,
            &banned.wallet(),
            &banned.session(),
            "island-C9",
            false,
        )
        .await;
        let state = CommsState::new(
            pool.clone(),
            Outbound::for_tests(),
            Settings::for_tests(),
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(crate::interfaces::test_support::StubDirectory::default()),
        );
        let wallet = Address::parse(&banned.wallet()).unwrap();
        let device = format!("device-{}", uuid::Uuid::new_v4().simple());
        state
            .moderation
            .upsert_connection(UpsertPlayerConnection {
                address: wallet.clone(),
                ip_address: None,
                device_id: Some(device.clone()),
            })
            .await
            .unwrap();
        state
            .moderation
            .create_ban(CreateBan {
                banned_address: Address::parse(&format!(
                    "{:#x}",
                    PrivateKeySigner::random().address()
                ))
                .unwrap(),
                banned_by: "m".into(),
                reason: "r".into(),
                custom_message: None,
                banned_device_id: Some(device),
                duration_ms: None,
            })
            .await
            .unwrap();
        let (status, _) = call(
            &app,
            banned
                .headers(r#"{"deviceIdentifier":"another-device"}"#)
                .await,
            b"",
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "a presented device cannot hide the recorded device ban"
        );
    }
}

#[cfg(test)]
mod moderation_boundary_tests {
    use super::*;
    use crate::gatekeeper::moderation::test_support::{StubBanStatus, state};
    use crate::interfaces::test_support::StubPolicy;
    use std::sync::Arc;
    #[tokio::test]
    async fn operator_block_prevents_island_refresh_before_any_mint_or_database_call() {
        let state = Arc::new(state(
            StubPolicy {
                blocked: Ok(true),
                ..Default::default()
            },
            StubBanStatus(Err(crate::AuthorityNotEstablished::store_unavailable(
                "unexpected raw read",
                &sqlx::Error::PoolClosed,
            ))),
        ));
        let wallet = Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        assert_eq!(
            ensure_not_banned(&state, &wallet, Some("different-device"))
                .await
                .unwrap_err()
                .0,
            StatusCode::FORBIDDEN
        );
    }
}
