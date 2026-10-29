// from: catalyrst/crates/catalyrst-archipelago/tests/feed_stats.rs
#[path = "support/archipelago.rs"]
mod archipelago;
use catalyst::comms::archipelago::AppState;
use catalyst::comms::archipelago::feed::{DISCOVERY_FRESHNESS_MS, Routed, dispatch, route_island};
use catalyst::comms::protocol::archipelago::Position;
use catalyst::comms::protocol::archipelago::archipelago::{
    IslandChangedMessage, IslandData, IslandStatusMessage, ServiceDiscoveryMessage, ServiceStatus,
};
use chrono::Utc;
use prost::Message as _;
use serde_json::Value;

async fn start_server() -> (u16, AppState) {
    let state = archipelago::state(archipelago::settings()).await;
    let port = archipelago::serve(state.clone()).await;
    (port, state)
}

async fn get_json(port: u16, path: &str) -> (u16, Value) {
    let resp = reqwest::get(format!("http://127.0.0.1:{port}{path}"))
        .await
        .expect("request");
    let status = resp.status().as_u16();
    let body: Value = resp.json().await.expect("json body");
    (status, body)
}

fn topology(peers: &[&str]) -> Vec<u8> {
    IslandStatusMessage {
        data: vec![IslandData {
            id: "I42".into(),
            peers: peers.iter().map(|p| (*p).to_string()).collect(),
            max_peers: 0,
            center: Some(Position {
                x: 16.0,
                y: 0.0,
                z: 32.0,
            }),
            radius: 12.5,
        }],
    }
    .encode_to_vec()
}

fn island(island_id: &str) -> IslandChangedMessage {
    IslandChangedMessage {
        assignment_context: None,
        island_id: island_id.into(),
        conn_str: "livekit:wss://example.invalid?access_token=t".into(),
        from_island_id: None,
        peers: Default::default(),
    }
}

fn discovery(current_time_ms: i64, user_count: u32) -> Vec<u8> {
    ServiceDiscoveryMessage {
        server_name: "archipelago".into(),
        status: Some(ServiceStatus {
            current_time: current_time_ms.max(0) as u64,
            commit_hash: Some("cafe".into()),
            user_count,
        }),
    }
    .encode_to_vec()
}

#[tokio::test]
async fn the_islands_endpoints_serve_the_topology_the_feed_announced() {
    let (port, state) = start_server().await;

    let (status, body) = get_json(port, "/islands").await;
    assert_eq!(status, 200);
    assert_eq!(
        body["islands"].as_array().unwrap().len(),
        0,
        "no topology announced yet"
    );

    state
        .peers
        .upsert_peer("0xaaaa".into(), [16.0, 0.0, 32.0], [1, 2], "r".into());
    assert_eq!(
        dispatch(&state, "engine.islands", &topology(&["0xaaaa", "0xghost"])),
        Routed::Topology
    );

    let (status, body) = get_json(port, "/islands").await;
    assert_eq!(status, 200);
    assert_eq!(body["ok"], true);
    let islands = body["islands"].as_array().expect("islands array");
    assert_eq!(islands.len(), 1);
    assert_eq!(islands[0]["id"], "I42");
    assert_eq!(islands[0]["maxPeers"], 0);
    assert_eq!(islands[0]["center"], serde_json::json!([16.0, 0.0, 32.0]));
    assert_eq!(islands[0]["radius"], 12.5);
    let peers = islands[0]["peers"].as_array().expect("peers array");
    assert_eq!(
        peers.len(),
        1,
        "a member this replica has never heard from carries no peer record"
    );
    assert_eq!(peers[0]["address"], "0xaaaa");

    let (status, body) = get_json(port, "/islands/I42").await;
    assert_eq!(status, 200);
    assert_eq!(body["id"], "I42");

    let missing = reqwest::get(format!("http://127.0.0.1:{port}/islands/nope"))
        .await
        .expect("request");
    assert_eq!(missing.status().as_u16(), 404);
}

#[tokio::test]
async fn core_status_reports_the_engines_own_discovery_heartbeat() {
    let (port, state) = start_server().await;

    let (status, body) = get_json(port, "/core-status").await;
    assert_eq!(status, 200);
    assert_eq!(
        body["healthy"], false,
        "a core that has never announced itself is not healthy"
    );
    assert_eq!(body["userCount"], 0);

    let now = Utc::now().timestamp_millis();
    assert_eq!(
        dispatch(&state, "engine.discovery", &discovery(now, 7)),
        Routed::Discovery
    );
    let (_, body) = get_json(port, "/core-status").await;
    assert_eq!(body["healthy"], true);
    assert_eq!(body["userCount"], 7);

    dispatch(
        &state,
        "engine.discovery",
        &discovery(now - DISCOVERY_FRESHNESS_MS - 1_000, 7),
    );
    let (_, body) = get_json(port, "/core-status").await;
    assert_eq!(
        body["healthy"], false,
        "a heartbeat older than the freshness window reads as unhealthy"
    );
    assert_eq!(
        body["userCount"], 7,
        "the last announced count is still reported"
    );
}

#[tokio::test]
async fn stats_health_counts_local_peers_and_announced_islands() {
    let (port, state) = start_server().await;
    state
        .peers
        .upsert_peer("0xaaaa".into(), [0.0, 0.0, 0.0], [0, 0], "r".into());
    dispatch(&state, "engine.islands", &topology(&["0xaaaa"]));

    let (status, body) = get_json(port, "/stats/health").await;
    assert_eq!(status, 200);
    assert_eq!(body["healthy"], true);
    assert_eq!(body["peers_total"], 1);
    assert_eq!(body["islands_total"], 1);
    assert!(body["uptime_secs"].is_i64());
    assert_eq!(body["feed_connected"], false);
    assert_eq!(body["feed_delivered"], 0);
    assert_eq!(body["feed_no_session_socket"], 0);
    assert_eq!(body["feed_deduplicated"], 0);
    assert_eq!(body["feed_undecodable"], 0);
    assert_eq!(body["feed_publish_dropped"], 0);
}

#[tokio::test]
async fn stats_health_carries_the_feed_counters_the_runbook_reads() {
    let (port, state) = start_server().await;
    let address = "0x00000000000000000000000000000000000000aa";
    let (_link, _rx, _) = state.registry.on_peer_connected(address, "0xs1");

    assert_eq!(
        route_island(&state, address, "0xs1", &island("I1")),
        Routed::Delivered
    );
    assert_eq!(
        route_island(&state, address, "0xstale", &island("I1")),
        Routed::NoSessionSocket
    );
    assert_eq!(
        dispatch(&state, "engine.discovery", b"\xff\xff\xff\xff"),
        Routed::Undecodable
    );
    state.feed.on_deduplicated();

    let (status, body) = get_json(port, "/stats/health").await;
    assert_eq!(status, 200);
    assert_eq!(body["feed_delivered"], 1);
    assert_eq!(body["feed_no_session_socket"], 1);
    assert_eq!(body["feed_deduplicated"], 1);
    assert_eq!(body["feed_undecodable"], 1);
    assert_eq!(
        body["feed_publish_dropped"], 0,
        "the in-process publisher drops nothing it was never handed"
    );
}

#[tokio::test]
async fn assignment_pressure_counts_coalescing_and_rejects_oversize() {
    use catalyst::comms::archipelago::registry::{MAX_ASSIGNMENT_BYTES, SocketEvent};
    let (port, state) = start_server().await;
    let (link, mut events, _) = state.registry.on_peer_connected("wallet", "session");
    let deliver =
        |assignment: &IslandChangedMessage| route_island(&state, "wallet", "session", assignment);
    assert_eq!(deliver(&island("first")), Routed::Delivered);
    assert_eq!(deliver(&island("last")), Routed::Coalesced);
    let oversized = IslandChangedMessage {
        conn_str: "x".repeat(MAX_ASSIGNMENT_BYTES),
        ..island("oversized")
    };
    assert_eq!(deliver(&oversized), Routed::TooLarge);
    let SocketEvent::IslandChanged(bytes, _) = events.try_recv().unwrap() else {
        panic!("expected the last valid assignment");
    };
    assert_eq!(bytes, island("last").encode_to_vec());
    assert!(
        link.needs_assignment(),
        "queue acceptance is not a WebSocket write"
    );
    let (status, body) = get_json(port, "/stats/health").await;
    assert_eq!(status, 200);
    assert_eq!(body["feed_delivered"], 2);
    assert_eq!(body["feed_assignment_coalesced"], 1);
    assert_eq!(body["feed_assignment_oversized"], 1);
    assert_eq!(body["feed_undecodable"], 0);
    assert_eq!(body["socket_assignments_written"], 0);
    assert_eq!(body["socket_write_timeout"], 0);
    assert_eq!(state.peers.island_of("wallet").as_deref(), Some("last"));
}

#[tokio::test]
async fn an_undecodable_feed_message_costs_only_itself() {
    let (_port, state) = start_server().await;
    assert_eq!(
        dispatch(&state, "engine.islands", b"\xff\xff\xff\xff"),
        Routed::Undecodable
    );
    assert_eq!(
        dispatch(&state, "engine.something_else", b""),
        Routed::Ignored
    );
    assert_eq!(state.feed.undecodable_count(), 1);
    assert_eq!(state.feed.islands_count(), 0);
}
