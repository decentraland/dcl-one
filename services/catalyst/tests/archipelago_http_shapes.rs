// from: catalyrst/crates/catalyrst-archipelago/tests/http_shapes.rs
#[path = "support/archipelago.rs"]
mod archipelago;
#[path = "support/db.rs"]
mod db;
use catalyst::comms::archipelago::AppState;
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

fn assert_peer_shape(peer: &Value, address: &str) {
    let obj = peer.as_object().expect("peer object");

    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["address", "id", "lastPing", "parcel", "position"]);
    assert_eq!(peer["id"], address);
    assert_eq!(peer["address"], address);
    assert!(peer["lastPing"].is_i64(), "lastPing must be a millis int");
    assert_eq!(peer["parcel"].as_array().unwrap().len(), 2);
    assert_eq!(peer["position"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn peers_shape_and_comms_alias_match() {
    let (port, state) = start_server().await;
    state.peers.upsert_peer(
        "0xaaaa".into(),
        [16.0, 0.0, 32.0],
        [1, 2],
        "catalyrst".into(),
    );

    for path in ["/peers", "/comms/peers"] {
        let (status, body) = get_json(port, path).await;
        assert_eq!(status, 200, "{path}");
        assert_eq!(body["ok"], true, "{path} ok flag");
        let peers = body["peers"].as_array().expect("peers array");
        assert_eq!(peers.len(), 1, "{path}");
        assert_peer_shape(&peers[0], "0xaaaa");

        assert_eq!(peers[0]["parcel"], serde_json::json!([1, 2]), "{path}");
        assert_eq!(
            peers[0]["position"],
            serde_json::json!([16.0, 0.0, 32.0]),
            "{path}"
        );
    }
}

#[tokio::test]
async fn parcels_count_peers_per_tile_on_every_prefix() {
    let (port, state) = start_server().await;
    state
        .peers
        .upsert_peer("0xaaaa".into(), [16.0, 0.0, 32.0], [1, 2], "r".into());
    state
        .peers
        .upsert_peer("0xbbbb".into(), [31.0, 3.0, 47.0], [1, 2], "r".into());
    state
        .peers
        .upsert_peer("0xcccc".into(), [-1.0, 0.0, 0.0], [-1, 0], "r".into());

    for path in ["/parcels", "/comms/parcels", "/stats/parcels"] {
        let (status, body) = get_json(port, path).await;
        assert_eq!(status, 200, "{path}");
        let obj = body.as_object().expect("parcels object");
        assert_eq!(obj.keys().collect::<Vec<_>>(), ["parcels"], "{path}");
        let mut parcels = body["parcels"].as_array().expect("parcels array").clone();
        parcels.sort_by_key(|p| (p["parcel"]["x"].as_i64(), p["parcel"]["y"].as_i64()));
        assert_eq!(
            parcels,
            vec![
                serde_json::json!({"peersCount": 1, "parcel": {"x": -1, "y": 0}}),
                serde_json::json!({"peersCount": 2, "parcel": {"x": 1, "y": 2}}),
            ],
            "{path}"
        );
    }

    state.peers.remove_peer("0xaaaa");
    state.peers.remove_peer("0xbbbb");
    state.peers.remove_peer("0xcccc");
    let (status, body) = get_json(port, "/stats/parcels").await;
    assert_eq!(status, 200);
    assert_eq!(body, serde_json::json!({"parcels": []}));
}

#[tokio::test]
async fn stats_prefix_serves_peers_and_islands() {
    let (port, state) = start_server().await;
    state
        .peers
        .upsert_peer("0xaaaa".into(), [16.0, 0.0, 32.0], [1, 2], "r".into());

    let (status, body) = get_json(port, "/stats/peers").await;
    assert_eq!(status, 200);
    assert_eq!(body["ok"], true);
    let peers = body["peers"].as_array().expect("peers array");
    assert_eq!(peers.len(), 1);
    assert_peer_shape(&peers[0], "0xaaaa");

    let (status, body) = get_json(port, "/stats/peers/0xAAAA").await;
    assert_eq!(status, 200);
    assert_peer_shape(&body["peer"], "0xaaaa");

    let (status, body) = get_json(port, "/stats/islands").await;
    assert_eq!(status, 200);
    assert_eq!(body["ok"], true);
    assert!(body["islands"].is_array());

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/stats/islands/nope"))
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn peers_id_filter_is_case_insensitive() {
    let (port, state) = start_server().await;
    state
        .peers
        .upsert_peer("0xaaaa".into(), [0.0, 0.0, 0.0], [0, 0], "r".into());
    state
        .peers
        .upsert_peer("0xbbbb".into(), [0.0, 0.0, 0.0], [0, 0], "r".into());

    let (status, body) = get_json(port, "/peers?id=0xAAAA").await;
    assert_eq!(status, 200);
    let peers = body["peers"].as_array().unwrap();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0]["address"], "0xaaaa");
}

#[tokio::test]
async fn peer_by_id_found_and_not_found() {
    let (port, state) = start_server().await;
    state
        .peers
        .upsert_peer("0xaaaa".into(), [0.0, 0.0, 0.0], [0, 0], "r".into());

    for path in ["/peers/0xaaaa", "/comms/peers/0xAAAA"] {
        let (status, body) = get_json(port, path).await;
        assert_eq!(status, 200, "{path}");
        assert_eq!(body["ok"], true, "{path}");
        assert_peer_shape(&body["peer"], "0xaaaa");
    }

    let (status, body) = get_json(port, "/peers/0xdoesnotexist").await;
    assert_eq!(status, 404);
    assert_eq!(body["ok"], false);
    assert!(body["peer"].is_null());
}

/// `/hot-scenes` reads `content.active_pointers` + `content.deployments`, so this one runs on
/// a scratch database (skips without `CATALYST_TEST_DATABASE_URL`); with the store unreachable
/// it answers 500, as the old service did when its content database failed.
#[tokio::test]
async fn hot_scenes_is_json_array() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let state = archipelago::state_with(
        archipelago::settings(),
        scratch.pool.clone(),
        archipelago::gatekeeper(None),
    )
    .await;
    let port = archipelago::serve(state.clone()).await;
    state
        .peers
        .upsert_peer("0xaaaa".into(), [0.0, 0.0, 0.0], [0, 0], "r".into());

    let (status, body) = get_json(port, "/hot-scenes").await;
    assert_eq!(status, 200);
    assert!(body.is_array(), "hot-scenes body must be a JSON array");
    assert_eq!(body.as_array().unwrap().len(), 0);

    let (unreachable, offline) = start_server().await;
    offline
        .peers
        .upsert_peer("0xaaaa".into(), [0.0, 0.0, 0.0], [0, 0], "r".into());
    let resp = reqwest::get(format!("http://127.0.0.1:{unreachable}/hot-scenes"))
        .await
        .expect("request");
    assert_eq!(
        resp.status().as_u16(),
        500,
        "an unreachable content store is an error, not an empty answer"
    );
    scratch.teardown().await;
}

#[tokio::test]
async fn status_uses_camelcase_fields() {
    let (port, _state) = start_server().await;
    let (status, body) = get_json(port, "/status").await;
    assert_eq!(status, 200);
    let obj = body.as_object().unwrap();

    assert!(obj.contains_key("version"));
    assert!(obj.contains_key("currentTime"));
    assert_eq!(body["commitHash"], "deadbeef");
}
