// from: catalyrst/crates/catalyrst-archipelago/tests/ban_check.rs
//! The two admission denylists. The platform ban is now the local
//! `worlds::policy::Policy::is_banned` over `worlds.blocked_wallets` (the old HTTP hop to the
//! gatekeeper's `/users/{address}/bans` is gone), so its cases run on a scratch database and
//! skip without `CATALYST_TEST_DATABASE_URL`; the external denylist document still comes from
//! an HTTP stub.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};

use catalyst::comms::archipelago::config::ClusterConfig;
use catalyst::comms::archipelago::peers::PeerDirectory;
use catalyst::comms::archipelago::registry::{PeersRegistry, SocketEvent};
use catalyst::comms::deny_list::DenyList;
use catalyst::http::Outbound;
use catalyst::worlds::policy::{PersonalWorldsPolicy, Policy};

#[path = "support/archipelago.rs"]
mod archipelago;
#[path = "support/db.rs"]
mod db;

const BANNED: &str = "0x00000000000000000000000000000000000bad01";
const CLEAN: &str = "0x00000000000000000000000000000000000c1ea0";

#[derive(Default)]
struct Counters {
    denylist_hits: AtomicUsize,
}

async fn denylist(
    State(c): State<Arc<Counters>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    c.denylist_hits.fetch_add(1, Ordering::SeqCst);
    let mut users = vec![json!({ "wallet": "0xBAD0000000000000000000000000000000000bad" })];
    if let Some(w) = params.get("wallet") {
        users.push(json!({ "wallet": w }));
    }
    Json(json!({ "users": users }))
}

async fn start_mock() -> (u16, Arc<Counters>) {
    let counters = Arc::new(Counters::default());
    let app = Router::new()
        .route("/denylist.json", get(denylist))
        .with_state(counters.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (port, counters)
}

fn http() -> Outbound {
    Outbound::for_tests().allowing_private_hosts()
}

async fn block(pool: &sqlx::PgPool, wallet: &str) {
    sqlx::query("INSERT INTO worlds.blocked_wallets (wallet) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(wallet)
        .execute(pool)
        .await
        .unwrap();
}

fn policy(pool: sqlx::PgPool) -> Arc<Policy> {
    Arc::new(Policy::new(pool, PersonalWorldsPolicy::disabled(), None))
}

#[tokio::test]
async fn ban_checker_flags_blocked_wallets_and_allows_others() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    block(&scratch.pool, BANNED).await;
    let checker = archipelago::ban_checker(policy(scratch.pool.clone()));
    assert!(
        checker
            .is_banned(&BANNED.to_uppercase().replace("0X", "0x"))
            .await,
        "case-insensitive"
    );
    assert!(!checker.is_banned(CLEAN).await);
    assert!(
        !checker.is_banned("not-an-address").await,
        "a malformed address holds no ban row"
    );
    scratch.teardown().await;
}

#[tokio::test]
async fn ban_checker_fails_open_when_the_store_is_unreachable() {
    let checker = archipelago::ban_checker(policy(archipelago::lazy_pool()));
    assert!(!checker.is_banned(BANNED).await);
    assert!(!checker.is_banned_fresh(BANNED).await);
}

#[tokio::test]
async fn ban_checker_memoizes_verdicts_and_a_fresh_check_refreshes_them() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let checker = archipelago::ban_checker(policy(scratch.pool.clone()));
    assert!(!checker.is_banned(BANNED).await);
    block(&scratch.pool, BANNED).await;
    assert!(
        !checker.is_banned(BANNED).await,
        "the second check is served from the memo"
    );
    assert!(
        checker.is_banned_fresh(BANNED).await,
        "the sweep always asks"
    );
    assert!(
        checker.is_banned(BANNED).await,
        "the fresh verdict refreshed the memo"
    );
    scratch.teardown().await;
}

#[tokio::test]
async fn deny_list_flags_listed_wallet_case_insensitively() {
    let (port, _c) = start_mock().await;
    let deny = DenyList::new(
        Some(format!("http://127.0.0.1:{port}/denylist.json")),
        http(),
    );
    assert!(deny.is_armed());
    assert!(
        deny.is_denied("0xBAD0000000000000000000000000000000000BAD")
            .await
    );
    assert!(
        deny.is_denied("0xbad0000000000000000000000000000000000bad")
            .await
    );
    assert!(
        !deny
            .is_denied("0x1111111111111111111111111111111111111111")
            .await
    );
    assert!(
        deny.is_denied("0xsomeoneelse").await,
        "malformed addresses must be denied, not silently allowed"
    );
}

#[tokio::test]
async fn deny_list_caches_within_ttl() {
    let (port, counters) = start_mock().await;
    let deny = DenyList::new(
        Some(format!("http://127.0.0.1:{port}/denylist.json")),
        http(),
    );
    for who in [
        "0x1111111111111111111111111111111111111111",
        "0x2222222222222222222222222222222222222222",
        "0x3333333333333333333333333333333333333333",
    ] {
        deny.is_denied(who).await;
    }
    assert_eq!(
        counters.denylist_hits.load(Ordering::SeqCst),
        1,
        "the 5-minute TTL must collapse repeated lookups into one fetch"
    );
}

#[tokio::test]
async fn deny_list_refetches_after_ttl_and_fails_open_on_outage() {
    let (port, counters) = start_mock().await;
    let deny = DenyList::with_ttl(
        Some(format!("http://127.0.0.1:{port}/denylist.json")),
        http(),
        Duration::from_millis(0),
    );
    assert!(
        deny.is_denied("0xbad0000000000000000000000000000000000bad")
            .await
    );
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert!(
        deny.is_denied("0xbad0000000000000000000000000000000000bad")
            .await
    );
    assert_eq!(counters.denylist_hits.load(Ordering::SeqCst), 2);
}

/// The gatekeeper with configured LiveKit credentials: a kick would reach its RoomService.
fn armed_gatekeeper() -> catalyst::comms::gatekeeper::SharedState {
    archipelago::gatekeeper_with_livekit(
        None,
        archipelago::configured_livekit("http://127.0.0.1:1"),
    )
}

#[tokio::test]
async fn the_ban_sweep_evicts_the_banned_peer_and_leaves_the_clean_one() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    block(&scratch.pool, BANNED).await;
    let ban = archipelago::ban_checker(policy(scratch.pool.clone()));
    let registry = PeersRegistry::new();
    let peers = PeerDirectory::new(
        ClusterConfig::default(),
        armed_gatekeeper(),
        ban,
        Arc::clone(&registry),
    );
    let (_banned_link, mut banned_events, _) = registry.on_peer_connected(BANNED, "0xs1");
    let (_clean_link, mut clean_events, _) = registry.on_peer_connected(CLEAN, "0xs2");

    peers.upsert_peer(BANNED.into(), [0.0, 0.0, 0.0], [0, 0], "r".into());
    peers.upsert_peer(CLEAN.into(), [0.0, 0.0, 0.0], [0, 0], "r".into());

    peers.ban_sweep_once().await;

    assert!(peers.peer(BANNED).is_none(), "banned peer evicted");
    assert_eq!(peers.peers_count(), 1);
    assert!(peers.peer(CLEAN).is_some(), "clean peer survives");
    assert!(matches!(banned_events.try_recv(), Ok(SocketEvent::Kicked)));
    assert!(
        clean_events.try_recv().is_err(),
        "the clean peer's socket is left alone"
    );
    scratch.teardown().await;
}

#[tokio::test]
async fn the_ban_sweep_with_a_permissive_policy_keeps_everyone() {
    let ban = archipelago::allow_ban_checker();
    let peers = PeerDirectory::new(
        ClusterConfig::default(),
        armed_gatekeeper(),
        ban,
        PeersRegistry::new(),
    );
    peers.upsert_peer(BANNED.into(), [0.0, 0.0, 0.0], [0, 0], "r".into());
    peers.ban_sweep_once().await;
    assert!(peers.peer(BANNED).is_some());
    assert_eq!(peers.peers_count(), 1);
}

mod ws_deny {
    use super::*;
    use alloy::signers::{SignerSync, local::PrivateKeySigner};
    use catalyst::comms::protocol::archipelago::archipelago::{
        ChallengeRequestMessage, ClientPacket, ServerPacket, SignedChallengeMessage, client_packet,
        server_packet,
    };
    use futures::{SinkExt, StreamExt};
    use prost::Message as _;
    use tokio_tungstenite::tungstenite::Message as WsMessage;

    type WsStream = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    async fn start_archipelago(deny_list_url: Option<String>) -> u16 {
        let mut settings = archipelago::settings();
        settings.auth.deny_list_url = deny_list_url;
        archipelago::serve(archipelago::state(settings).await).await
    }

    fn encode(msg: client_packet::Message) -> tokio_tungstenite::tungstenite::Bytes {
        ClientPacket {
            request_sequence: 0,
            message: Some(msg),
        }
        .encode_to_vec()
        .into()
    }

    async fn recv_msg(ws: &mut WsStream, timeout: Duration) -> Option<server_packet::Message> {
        loop {
            let frame = tokio::time::timeout(timeout, ws.next()).await.ok()??;
            match frame.ok()? {
                WsMessage::Binary(bytes) => {
                    return ServerPacket::decode(bytes.as_ref()).ok()?.message;
                }
                WsMessage::Ping(_) | WsMessage::Pong(_) => continue,
                _ => return None,
            }
        }
    }

    async fn try_handshake(port: u16, wallet: &PrivateKeySigner) -> Option<()> {
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .expect("ws connect");
        let address = format!("{:#x}", wallet.address());
        ws.send(WsMessage::Binary(encode(
            client_packet::Message::ChallengeRequest(ChallengeRequestMessage {
                address: address.clone(),
            }),
        )))
        .await
        .expect("send challenge request");

        let challenge = match recv_msg(&mut ws, Duration::from_secs(3)).await {
            Some(server_packet::Message::ChallengeResponse(r)) => r.challenge_to_sign,
            _ => return None,
        };

        let hash = alloy::primitives::eip191_hash_message(challenge.as_bytes());
        let sig = wallet.sign_hash_sync(&hash).expect("sign");
        let chain = json!([
            { "type": "SIGNER", "payload": address, "signature": "" },
            { "type": "ECDSA_SIGNED_ENTITY", "payload": challenge, "signature": sig.to_string() }
        ]);
        ws.send(WsMessage::Binary(encode(
            client_packet::Message::SignedChallenge(SignedChallengeMessage {
                auth_chain_json: chain.to_string(),
            }),
        )))
        .await
        .expect("send signed challenge");

        matches!(
            recv_msg(&mut ws, Duration::from_secs(3)).await,
            Some(server_packet::Message::Welcome(_))
        )
        .then_some(())
    }

    #[tokio::test]
    async fn deny_listed_wallet_is_rejected_after_auth() {
        let (mock_port, _c) = start_mock().await;
        let wallet = PrivateKeySigner::random();
        let addr = format!("{:#x}", wallet.address());
        let deny_url = format!("http://127.0.0.1:{mock_port}/denylist.json?wallet={addr}");
        let port = start_archipelago(Some(deny_url)).await;

        assert!(
            try_handshake(port, &wallet).await.is_none(),
            "a deny-listed wallet must not receive a Welcome"
        );
    }

    #[tokio::test]
    async fn non_denied_wallet_completes_handshake() {
        let (mock_port, _c) = start_mock().await;
        let wallet = PrivateKeySigner::random();
        let deny_url = format!("http://127.0.0.1:{mock_port}/denylist.json?wallet=0xsomeoneelse");
        let port = start_archipelago(Some(deny_url)).await;

        assert!(
            try_handshake(port, &wallet).await.is_some(),
            "a non-denied wallet must complete the handshake"
        );
    }
}
