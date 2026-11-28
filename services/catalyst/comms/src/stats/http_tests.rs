use super::*;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::get,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;

#[derive(Default)]
struct Documents {
    calls: AtomicUsize,
    values: Mutex<Vec<Value>>,
    pointers: Mutex<Vec<String>>,
}
#[async_trait::async_trait]
impl content::Source for Documents {
    async fn fetch(
        &self,
        pointers: &[String],
    ) -> Result<Vec<crate::content::OrderedJson>, content::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.pointers.lock() = pointers.to_vec();
        Ok(self
            .values
            .lock()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect())
    }
}
#[derive(Default)]
struct Runtime {
    lifecycle: Mutex<http::Lifecycle>,
}
#[async_trait::async_trait]
impl http::Hooks for Runtime {
    async fn probe(&self, _startup: bool) -> http::Lifecycle {
        self.lifecycle.lock().clone()
    }
    async fn diagnostics(&self) -> Result<String, content::Error> {
        Ok(r#"{"peers_total":3}"#.into())
    }
}
fn setup() -> (
    http::State,
    Arc<Documents>,
    Arc<Runtime>,
    InProcessBus,
    NatsSubscription,
) {
    let reader = Arc::new(Reader::default());
    let bus = InProcessBus::new();
    let sub = reader.subscribe(&bus);
    let docs = Arc::new(Documents::default());
    let runtime = Arc::new(Runtime::default());
    (
        http::State {
            reader,
            content: docs.clone(),
            hooks: runtime.clone(),
            settings: http::Settings::default(),
            now: Arc::new(|| 1234),
            environment: Arc::new(|| ("v".into(), "hash".into())),
        },
        docs,
        runtime,
        bus,
        sub,
    )
}
async fn request(
    router: Router,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut request = Request::builder().method(method).uri(path);
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    let response = router
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, headers, String::from_utf8(body.to_vec()).unwrap())
}
fn publish(bus: &InProcessBus, s: Snapshot) {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    for bytes in frames(&s, uuid::Uuid::nil(), sequence, 200).unwrap() {
        bus.inject(SNAPSHOTS_SUBJECT, &bytes);
    }
}

#[tokio::test]
async fn method_admission_preserves_the_stats_response_and_bounds_process_labels() {
    let (state, _, _, _, _sub) = setup();
    let recorder = dcl_metrics::PrometheusBuilder::new().build_recorder();
    let _local = dcl_metrics::set_default_local_recorder(&recorder);
    let app = http::mount_services(
        Router::new(),
        state.clone(),
        http::standalone(state),
        Router::new(),
    );
    for path in ["/peers", "/archipelago-ea-stats/peers"] {
        for method in ["UNKNOWN-A", "UNKNOWN-B", "get"] {
            let (status, headers, body) = request(app.clone(), method, path, &[]).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(body.is_empty());
            assert!(!headers.contains_key("access-control-allow-origin"));
        }
        for (method, status) in [
            ("OPTIONS", 204),
            ("TRACE", 500),
            ("CONNECT", 501),
            ("PROPFIND", 501),
            ("QUERY", 501),
        ] {
            assert_eq!(
                request(app.clone(), method, path, &[]).await.0.as_u16(),
                status
            );
        }
    }
    let body = recorder.handle().render();
    assert!(
        body.lines()
            .any(|l| l.starts_with("dcl_http_requests_total{")
                && l.contains("method=\"OTHER\"")
                && l.contains("status_code=\"400\"")
                && l.ends_with(" 6")),
        "{body}"
    );
    assert!(!body.contains("UNKNOWN-"));
}

#[tokio::test]
async fn selected_sql_metadata_order_reaches_hot_scenes_http() {
    let pool = crate::db::scratch::pool(2)
        .await
        .expect("test database required");
    // Synthetic persisted handler input: current scene deployment schemas require string titles.
    let metadata = r#"{"v":{"id":"spoof","scene":{"base":"0,0","parcels":["0,0"]},"display":{"title":{"z":1,"a":2},"description":[{"z":true,"a":null}]},"contact":{"name":{"z":3,"a":4}},"source":{"projectId":{"$serde_json::private::Number":"12"}}}}"#;
    sqlx::query("INSERT INTO content.deployments (deployer_address,version,entity_type,entity_id,entity_metadata,entity_timestamp,entity_pointers,local_timestamp,auth_chain) VALUES ('0x0000000000000000000000000000000000000001','v3','scene','ordered-scene',$1::json,timezone('UTC',now()),ARRAY['0,0'],timezone('UTC',now()),'[]'::json)").bind(metadata).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO content.active_pointers (pointer,entity_id,entity_type) VALUES ('0,0','ordered-scene','scene')").execute(&pool).await.unwrap();
    let (mut state, _, _, bus, _sub) = setup();
    state.content = Arc::new(content::Content {
        pool: pool.clone(),
        denylist: Arc::new(crate::content::Denylist::default()),
    });
    publish(
        &bus,
        Snapshot {
            peers: vec![Peer {
                address: "one".into(),
                position: [0.; 3],
                time: 1,
            }],
            ..Default::default()
        },
    );
    let app = http::standalone(state);
    let (status, headers, body) = request(app.clone(), "GET", "/hot-scenes", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        r#"[{"id":"ordered-scene","name":{"z":1,"a":2},"baseCoords":[0,0],"usersTotalCount":1,"parcels":[[0,0]],"creator":{"z":3,"a":4},"projectId":{"$serde_json::private::Number":"12"},"description":[{"z":true,"a":null}]}]"#
    );
    assert_eq!(headers["content-length"], body.len().to_string());
    let (status, head_headers, body) = request(app.clone(), "HEAD", "/hot-scenes", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_empty());
    assert!(!head_headers.contains_key("content-length"));
    assert!(!head_headers.contains_key("content-type"));
    for (title, expected) in [
        (
            r#"{"z":1,"10":10,"2":2,"01":1,"0":0,"-0":0,"4294967294":4,"4294967295":5,"a":1,"\u0061":2,"z":9,"2":3}"#,
            r#"{"0":0,"2":3,"10":10,"4294967294":4,"z":9,"01":1,"-0":0,"4294967295":5,"a":2}"#,
        ),
        (
            "[9007199254740991,9007199254740992,9007199254740993,18446744073709551615,-0,5e-324,1e-7,1e-6,1e20,1e21]",
            "[9007199254740991,9007199254740992,9007199254740992,18446744073709552000,0,5e-324,1e-7,0.000001,100000000000000000000,1e+21]",
        ),
        (
            r#"["\ud83d\ude00","é\n/\u2028",false,null]"#,
            "[\"😀\",\"é\\n/\u{2028}\",false,null]",
        ),
        ("null", "null"),
        ("false", "false"),
        (r#""""#, r#""""#),
    ] {
        let metadata = format!(
            r#"{{"v":{{"scene":{{"base":"0,0","parcels":["0,0"]}},"display":{{"title":{title}}}}}}}"#
        );
        sqlx::query("UPDATE content.deployments SET entity_metadata=$1::json WHERE entity_id='ordered-scene'").bind(metadata).execute(&pool).await.unwrap();
        let (status, _, body) = request(app.clone(), "GET", "/hot-scenes", &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            format!(
                r#"[{{"id":"ordered-scene","name":{expected},"baseCoords":[0,0],"usersTotalCount":1,"parcels":[[0,0]]}}]"#
            )
        );
    }
    // Synthetic selected documents bypass the scene logs-permissions expression index,
    // which rejects unpaired surrogates before they can reach either read projection.
    sqlx::query(
        "UPDATE content.active_pointers SET entity_type='profile' WHERE entity_id='ordered-scene'",
    )
    .execute(&pool)
    .await
    .unwrap();
    for invalid in ["1e400", r#""\ud800""#] {
        let metadata = format!(
            r#"{{"v":{{"scene":{{"base":"0,0","parcels":["0,0"]}},"display":{{"title":{invalid}}}}}}}"#
        );
        sqlx::query("UPDATE content.deployments SET entity_type='profile',entity_metadata=$1::json WHERE entity_id='ordered-scene'").bind(metadata).execute(&pool).await.unwrap();
        assert!(
            crate::content::queries::active_entity_docs_by_pointers(&pool, &["0,0".into()])
                .await
                .is_err()
        );
        for method in ["GET", "HEAD"] {
            let (status, headers, body) = request(app.clone(), method, "/hot-scenes", &[]).await;
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            assert!(body.is_empty());
            assert!(!headers.contains_key("content-type"));
        }
    }
    sqlx::query("DELETE FROM content.active_pointers WHERE entity_id='ordered-scene'")
        .execute(&pool)
        .await
        .unwrap();
}
#[tokio::test]
async fn standalone_normalizes_dot_segments_without_collapsing_slashes() {
    let (state, _, _, _, _sub) = setup();
    let app = http::standalone(state);
    for path in ["/x/../peers", "/x/%2e%2e/peers", "/./peers"] {
        let (status, _, body) = request(app.clone(), "GET", path, &[]).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(body, r#"{"ok":true,"peers":[]}"#);
    }
    assert_eq!(
        request(app, "GET", "/peers//", &[]).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn socket_alias_uses_stats_fallback_except_for_get_upgrades() {
    let (state, _, _, _, _sub) = setup();
    let app = http::socket_alias(
        http::standalone(state.clone())
            .route("/ws", get(|| async { StatusCode::SWITCHING_PROTOCOLS })),
        state,
    );
    for (method, expected) in [
        ("GET", 404),
        ("HEAD", 404),
        ("POST", 404),
        ("OPTIONS", 204),
        ("PROPFIND", 501),
    ] {
        let (status, headers, body) = request(
            app.clone(),
            method,
            "/ws",
            &[("origin", "https://client.example")],
        )
        .await;
        assert_eq!(status.as_u16(), expected, "{method}");
        assert_eq!(
            headers["access-control-allow-origin"],
            "https://client.example"
        );
        if method == "GET" {
            assert_eq!(body, "Not found");
        }
    }
    assert_eq!(
        request(app, "GET", "/ws", &[("upgrade", "websocket")])
            .await
            .0,
        StatusCode::SWITCHING_PROTOCOLS
    );
}
#[tokio::test]
async fn all_application_aliases_and_missing_bodies_match_the_stats_contract() {
    let (state, _, _, bus, _sub) = setup();
    publish(
        &bus,
        Snapshot {
            peers: vec![Peer {
                address: "A/B+%".into(),
                position: [16., 0., -0.1],
                time: 123,
            }],
            ..Default::default()
        },
    );
    let app = http::merge_root(http::standalone(state.clone()), state);
    for prefix in ["", "/comms", "/stats"] {
        for (suffix, key) in [
            ("peers", "peers"),
            ("parcels", "parcels"),
            ("islands", "islands"),
        ] {
            let (status, headers, body) =
                request(app.clone(), "GET", &format!("{prefix}/{suffix}"), &[]).await;
            assert_eq!(status, 200);
            assert_eq!(headers["content-type"], "application/json");
            assert!(serde_json::from_str::<Value>(&body).unwrap()[key].is_array());
        }
        assert_eq!(
            request(
                app.clone(),
                "GET",
                &format!("{prefix}/peers/A%2FB%2B%25"),
                &[]
            )
            .await
            .0,
            200
        );
        assert_eq!(
            request(app.clone(), "GET", &format!("{prefix}/peers/missing"), &[])
                .await
                .2,
            r#"{"ok":false,"peer":null}"#
        );
        let (status, headers, body) = request(
            app.clone(),
            "GET",
            &format!("{prefix}/islands/missing"),
            &[],
        )
        .await;
        assert_eq!(status, 404);
        assert_eq!(body, "");
        assert!(!headers.contains_key("content-type"));
    }
    assert_eq!(request(app.clone(), "GET", "/PeErS/", &[]).await.0, 200);
    assert_eq!(
        request(app.clone(), "GET", "/unknown", &[]).await.2,
        "Not found"
    );
    assert_eq!(request(app.clone(), "GET", "/peers//", &[]).await.0, 404);
    assert_eq!(
        request(app.clone(), "GET", "/core-status", &[]).await.2,
        r#"{"healthy":false,"userCount":0}"#
    );
    assert_eq!(
        request(app.clone(), "GET", "/status", &[]).await.2,
        r#"{"version":"v","currentTime":1234,"commitHash":"hash"}"#
    );
    assert_eq!(request(app.clone(), "GET", "/ping", &[]).await.2, "/ping");
    assert_eq!(
        request(app, "GET", "/stats/health", &[]).await.2,
        r#"{"peers_total":3}"#
    );
}
#[tokio::test]
async fn query_filters_are_urlsearchparams_with_exact_case_and_store_order() {
    let (state, _, _, bus, _sub) = setup();
    publish(
        &bus,
        Snapshot {
            peers: ["B", "a", "A", "a b", "%FF"]
                .into_iter()
                .map(|a| Peer {
                    address: a.into(),
                    position: [0.; 3],
                    time: 1,
                })
                .collect(),
            ..Default::default()
        },
    );
    let app = http::standalone(state);
    for (query, want) in [
        ("id=a&id=B&id=a", vec!["B", "a"]),
        ("id=", vec![]),
        ("id=a,B", vec![]),
        ("id[]=A&ID=a", vec!["B", "a", "A", "a b", "%FF"]),
        ("id=a+b", vec!["a b"]),
        ("id=%FF", vec![]),
    ] {
        let response = request(app.clone(), "GET", &format!("/peers?{query}"), &[]).await;
        let v: Value = serde_json::from_str(&response.2).unwrap();
        assert_eq!(
            v["peers"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            want
        );
    }
    assert_eq!(request(app.clone(), "GET", "/peers/%FF", &[]).await.0, 200);
    assert_eq!(request(app, "GET", "/peers/A?id=a", &[]).await.0, 200);
}
#[tokio::test]
async fn head_options_method_and_cors_headers_follow_the_original_middleware() {
    let (state, _, _, _, _sub) = setup();
    let app = http::standalone(state);
    for path in ["/peers", "/islands/missing", "/unknown", "/status"] {
        let (_, headers, body) =
            request(app.clone(), "HEAD", path, &[("Origin", "https://caller")]).await;
        assert_eq!(body, "");
        assert!(!headers.contains_key("content-type"));
        assert!(!headers.contains_key("content-length"));
        assert_eq!(headers["access-control-allow-origin"], "https://caller");
        assert_eq!(headers["vary"], "Origin");
        assert_eq!(headers["access-control-allow-credentials"], "true");
    }
    for path in ["/peers", "/unknown"] {
        let (status, headers, body) = request(app.clone(), "OPTIONS", path, &[]).await;
        assert_eq!(status, 204);
        assert_eq!(body, "");
        assert!(!headers.contains_key("content-length"));
        assert_eq!(
            headers["access-control-allow-methods"],
            "GET,HEAD,POST,PUT,DELETE,CONNECT,TRACE,PATCH"
        );
        assert_eq!(
            headers["access-control-allow-headers"],
            "Cache-Control,Content-Type,Origin,Accept,User-Agent,X-Upload-Origin"
        );
        assert_eq!(headers["access-control-max-age"], "86400");
    }
    for path in ["/peers", "/health/live"] {
        let r = request(app.clone(), "POST", path, &[]).await;
        assert_eq!(r.0, 405);
        assert_eq!(r.1["allow"], "HEAD, GET");
        assert_eq!(r.2, "");
    }
    let r = request(app.clone(), "PROPFIND", "/unknown", &[]).await;
    assert_eq!(r.0, 501);
    assert_eq!(r.1["allow"], "");
    assert_eq!(request(app.clone(), "POST", "/unknown", &[]).await.0, 404);
    assert_eq!(
        request(app.clone(), "GET", "/status", &[]).await.1["access-control-allow-origin"],
        "*"
    );
}
#[tokio::test]
async fn retired_scrapes_follow_the_stats_fallback_and_lifecycle_probes_keep_their_values() {
    let (state, _, runtime, _, _sub) = setup();
    let app = http::standalone(state);
    for path in ["/metrics", "/custom/metrics"] {
        for method in ["GET", "HEAD", "POST"] {
            assert_eq!(request(app.clone(), method, path, &[]).await.0, 404);
        }
    }
    assert_eq!(
        request(app.clone(), "GET", "/health/live", &[]).await.2,
        "alive"
    );
    assert_eq!(
        request(app.clone(), "GET", "/health/ready", &[]).await.2,
        r#"{"status":"initializing"}"#
    );
    assert_eq!(
        request(app.clone(), "GET", "/health/startup", &[]).await.2,
        r#"{"status":"bootstrapping"}"#
    );
    *runtime.lifecycle.lock() = http::Lifecycle {
        initialized: true,
        ..Default::default()
    };
    assert_eq!(
        request(app.clone(), "GET", "/health/ready", &[]).await.2,
        "waiting"
    );
    runtime.lifecycle.lock().live = true;
    assert_eq!(
        request(app.clone(), "GET", "/health/ready", &[]).await.2,
        "unready"
    );
    assert_eq!(
        request(app.clone(), "GET", "/health/startup", &[]).await.2,
        r#"{"status":"starting"}"#
    );
    runtime.lifecycle.lock().started = true;
    runtime.lifecycle.lock().checks = vec![("bus".into(), false), ("content".into(), true)];
    let r = request(app.clone(), "GET", "/health/ready", &[]).await;
    assert_eq!(r.0, 503);
    assert_eq!(
        r.2,
        r#"{"details":{"bus":{"status":"fail"},"content":{"status":"pass"}},"status":"fail"}"#
    );
    runtime.lifecycle.lock().checks[0].1 = true;
    let r = request(app, "HEAD", "/health/startup", &[]).await;
    assert_eq!(r.0, 200);
    assert_eq!(
        r.1["content-type"],
        "application/health+json; charset=utf-8"
    );
}
#[tokio::test]
async fn hot_scenes_fetches_ordered_tiles_on_each_request_and_returns_empty_500_on_throw() {
    let (state, docs, _, bus, _sub) = setup();
    let app = http::standalone(state);
    assert_eq!(
        request(app.clone(), "GET", "/hot-scenes", &[]).await.2,
        "[]"
    );
    assert_eq!(docs.calls.load(Ordering::SeqCst), 0);
    publish(
        &bus,
        Snapshot {
            peers: vec![
                Peer {
                    address: "a".into(),
                    position: [32., 0., 0.],
                    time: 1,
                },
                Peer {
                    address: "b".into(),
                    position: [0.; 3],
                    time: 1,
                },
            ],
            ..Default::default()
        },
    );
    *docs.values.lock() = vec![
        json!({"id":"s","metadata":{"scene":{"base":"0","parcels":["0,0"]},"display":{"title":"old"}}}),
    ];
    let r = request(app.clone(), "GET", "/hot-scenes", &[]).await;
    assert!(r.2.contains("old"));
    assert_eq!(*docs.pointers.lock(), vec!["2,0", "0,0"]);
    docs.values.lock()[0]["metadata"]["display"]["title"] = json!("new");
    assert!(
        request(app.clone(), "GET", "/hot-scenes", &[])
            .await
            .2
            .contains("new")
    );
    assert_eq!(docs.calls.load(Ordering::SeqCst), 2);
    docs.values.lock()[0]["metadata"]["scene"]["base"] = json!(true);
    let r = request(app, "GET", "/hot-scenes", &[("Origin", "https://caller")]).await;
    assert_eq!(r.0, 500);
    assert_eq!(r.2, "");
    assert!(!r.1.contains_key("content-type"));
    assert!(!r.1.contains_key("access-control-allow-origin"));
}
#[tokio::test]
async fn root_projection_preserves_other_service_routes_and_fallback() {
    let (state, _, _, _, _sub) = setup();
    let root = Router::new()
        .route("/status", get(|| async { "content-status" }))
        .fallback(|| async { (StatusCode::IM_A_TEAPOT, "other") });
    let app = http::merge_root(root, state.clone())
        .nest("/archipelago-ea-stats", http::standalone(state));
    assert_eq!(
        request(app.clone(), "GET", "/status", &[]).await.2,
        "content-status"
    );
    assert_eq!(request(app.clone(), "GET", "/unknown", &[]).await.0, 418);
    assert_eq!(
        request(app.clone(), "OPTIONS", "/unknown", &[]).await.0,
        418
    );
    assert_eq!(
        request(app.clone(), "GET", "/CoMmS/PeErS/", &[]).await.2,
        r#"{"ok":true,"peers":[]}"#
    );
    assert_eq!(
        request(app.clone(), "GET", "/archipelago-ea-stats/status", &[])
            .await
            .2,
        r#"{"version":"v","currentTime":1234,"commitHash":"hash"}"#
    );
    assert_eq!(
        request(app, "OPTIONS", "/archipelago-ea-stats/unknown", &[])
            .await
            .0,
        204
    );
}
#[tokio::test]
async fn nested_head_and_options_do_not_invent_content_length_and_status_reads_each_time() {
    let (mut state, _, _, _, _sub) = setup();
    let version = Arc::new(Mutex::new("first".to_string()));
    let read = version.clone();
    state.environment = Arc::new(move || (read.lock().clone(), String::new()));
    let app = Router::new().nest("/archipelago-ea-stats", http::standalone(state));
    assert!(
        request(app.clone(), "GET", "/archipelago-ea-stats/status", &[])
            .await
            .2
            .contains("first")
    );
    *version.lock() = "second".into();
    assert!(
        request(app.clone(), "GET", "/archipelago-ea-stats/status", &[])
            .await
            .2
            .contains("second")
    );
    for method in ["HEAD", "OPTIONS"] {
        let r = request(app.clone(), method, "/archipelago-ea-stats/peers", &[]).await;
        assert!(!r.1.contains_key("content-length"), "{method}: {:?}", r.1);
        assert_eq!(r.2, "");
    }
}

#[tokio::test]
async fn an_empty_origin_sets_credentials_but_does_not_reflect_or_replace_status_origin() {
    let (state, _, _, _, _sub) = setup();
    let app = http::standalone(state);
    for method in ["GET", "OPTIONS"] {
        let r = request(app.clone(), method, "/peers", &[("Origin", "")]).await;
        assert!(!r.1.contains_key("access-control-allow-origin"));
        assert!(!r.1.contains_key("vary"));
        assert_eq!(r.1["access-control-allow-credentials"], "true");
    }
    assert_eq!(
        request(app, "GET", "/status", &[("Origin", "")]).await.1["access-control-allow-origin"],
        "*"
    );
}

#[test]
fn default_content_url_has_no_trailing_slash() {
    assert_eq!(
        http::Settings::default().content_url,
        "https://peer.decentraland.org/content"
    );
}

/// Replays output generated by the pinned TypeScript handlers, preserving JSON bytes.
#[tokio::test]
async fn source_handler_fixtures_match_pulse_state_and_all_http_aliases() {
    use crate::protocol::archipelago::archipelago::Heartbeat;
    use crate::protocol::bus::statistics::{Event, Observation, ProducerReply, ProducerRequest};
    use crate::protocol::pulse::decentraland::kernel::comms::v3::{
        IslandStatusMessage, ServiceDiscoveryMessage,
    };
    use prost::Message;
    let fixture: Value = serde_json::from_str(include_str!("source-fixtures.json")).unwrap();
    let (mut state, docs, _, bus, _sub) = setup();
    // The pinned revision predates upstream's trailing-slash strip; replay its base as is.
    state.settings.content_url = fixture["baseUrl"].as_str().unwrap().into();
    let mut store = crate::pulse::statistics::Statistics::default();
    let ProducerReply::Grant(grant) = store.control(ProducerRequest::Begin {
        incarnation: uuid::Uuid::nil(),
        producer: uuid::Uuid::nil(),
    }) else {
        panic!("grant")
    };
    assert_eq!(
        store.control(ProducerRequest::Confirm(grant.clone())),
        ProducerReply::Active(grant.clone())
    );
    let mut sequence = 0;
    for case in fixture["cases"].as_array().unwrap() {
        for event in case["events"].as_array().unwrap() {
            let subject = event["subject"].as_str().unwrap();
            let bytes: Vec<u8> = event["bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| u8::try_from(v.as_u64().unwrap()).unwrap())
                .collect();
            let observation = if subject.ends_with(".heartbeat") {
                Heartbeat::decode(bytes.as_slice())
                    .unwrap()
                    .position
                    .map(|p| Event::Heartbeat {
                        address: subject.split('.').nth(1).unwrap().into(),
                        position: [p.x as f64, p.y as f64, p.z as f64],
                    })
            } else if subject.ends_with(".disconnect") {
                Some(Event::Disconnect {
                    address: subject.split('.').nth(1).unwrap().into(),
                })
            } else {
                if subject == "engine.islands"
                    && IslandStatusMessage::decode(bytes.as_slice()).is_ok()
                {
                    store.topology(bytes.clone());
                }
                if subject == "engine.discovery"
                    && ServiceDiscoveryMessage::decode(bytes.as_slice()).is_ok()
                {
                    store.discovery(bytes);
                }
                None
            };
            if let Some(event_kind) = observation {
                sequence += 1;
                assert!(store.observe(
                    Observation {
                        version: 2,
                        incarnation: grant.incarnation,
                        generation: grant.generation,
                        producer: uuid::Uuid::nil(),
                        sequence,
                        event: event_kind
                    },
                    event["time"].as_i64().unwrap()
                ));
            }
        }
        let snapshot = store.snapshot();
        assert_eq!(
            snapshot.peers.len(),
            case["peers"].as_array().unwrap().len()
        );
        for (actual, expected) in snapshot.peers.iter().zip(case["peers"].as_array().unwrap()) {
            assert_eq!(actual.address, expected["address"].as_str().unwrap());
            assert_eq!(actual.time, expected["time"].as_i64().unwrap());
            for (actual, expected) in actual
                .position
                .iter()
                .zip(expected["positionBits"].as_array().unwrap())
            {
                let expected = f64::from_bits(expected.as_str().unwrap().parse().unwrap());
                assert!(
                    (actual.is_nan() && expected.is_nan())
                        || actual.to_bits() == expected.to_bits()
                );
            }
        }
        publish(&bus, snapshot);
        let now = case["now"].as_i64().unwrap();
        state.now = Arc::new(move || now);
        *docs.values.lock() = case["documents"].as_array().unwrap().clone();
        for read in case["reads"].as_array().unwrap() {
            let path = read["path"].as_str().unwrap();
            let paths = if ["/peers", "/islands", "/parcels"]
                .iter()
                .any(|prefix| path.starts_with(prefix))
            {
                vec![
                    path.to_string(),
                    format!("/comms{path}"),
                    format!("/stats{path}"),
                ]
            } else {
                vec![path.to_string()]
            };
            for path in paths {
                let (status, headers, body) = request(
                    http::merge_root(http::standalone(state.clone()), state.clone()),
                    "GET",
                    &path,
                    &[],
                )
                .await;
                assert_eq!(
                    status.as_u16() as u64,
                    read["status"].as_u64().unwrap(),
                    "{} {path}",
                    case["name"]
                );
                assert_eq!(
                    body,
                    read["body"].as_str().unwrap(),
                    "{} {path}",
                    case["name"]
                );
                if !body.is_empty() {
                    assert_eq!(headers["content-type"], "application/json");
                }
            }
        }
    }
}

#[tokio::test]
async fn source_host_fallback_excludes_shared_root_statistics_aliases() {
    let (state, _, _, _, _sub) = setup();
    let standalone = http::standalone(state.clone());
    let root = http::merge_root(Router::new(), state);
    for path in [
        "/stats/peers",
        "/stats/parcels",
        "/stats/islands",
        "/stats/health",
    ] {
        let (code, headers, body) = request(standalone.clone(), "GET", path, &[]).await;
        assert_eq!(code, 404, "standalone {path}");
        assert_eq!(headers["content-type"], "text/plain; charset=utf-8");
        assert_eq!(body, "Not found");
        assert_eq!(
            request(root.clone(), "GET", path, &[]).await.0,
            200,
            "shared root {path}"
        );
    }
}

#[tokio::test]
async fn camera_reel_paths_bypass_shared_trailing_slash_normalization() {
    let (state, _, _, _, _sub) = setup();
    let shared = Router::new()
        .route(
            "/camera-reel-service/api/users/{address}",
            get(
                |axum::extract::Path(address): axum::extract::Path<String>,
                 matched: axum::extract::MatchedPath,
                 axum::extract::OriginalUri(original): axum::extract::OriginalUri| async move {
                    axum::Json(json!({
                        "address": address,
                        "handler": matched.as_str().strip_prefix("/camera-reel-service").unwrap(),
                        "original": original.to_string(),
                    }))
                },
            ),
        )
        .route(
            "/camera-reel-service/api/docs/ui/",
            get(|| async { "docs" }),
        )
        .route("/other", get(|| async { "other" }));
    let app = http::mount_services(
        shared,
        state.clone(),
        http::standalone(state),
        Router::new(),
    );
    for (path, status) in [
        ("/camera-reel-service/api/users/example", StatusCode::OK),
        (
            "/camera-reel-service/api/users/example/",
            StatusCode::NOT_FOUND,
        ),
        ("/camera-reel-service/api/docs/ui/", StatusCode::OK),
        ("/camera-reel-service/api/docs/ui", StatusCode::NOT_FOUND),
        ("/other/", StatusCode::OK),
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{path}");
        if path == "/camera-reel-service/api/users/example" {
            let body: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap())
                    .unwrap();
            assert_eq!(
                body,
                json!({
                    "address": "example",
                    "handler": "/api/users/{address}",
                    "original": "/camera-reel-service/api/users/example",
                })
            );
        }
    }
}

#[tokio::test]
async fn status_reports_the_host_commit_hash() {
    let (mut state, _, _, _, _sub) = setup();
    state.settings.commit_hash = Some("built-commit".into());
    let app = http::standalone(state);
    let response = request(app, "GET", "/status", &[]).await;
    assert_eq!(response.0, StatusCode::OK);
    let status: Value = serde_json::from_str(&response.2).unwrap();
    assert_eq!(status["commitHash"], "built-commit");
    assert_eq!(status["version"], "v");
}

#[tokio::test]
async fn process_http_metrics_cover_claimed_routes_nested_services_and_retired_scrapes() {
    let (state, _, _, _, _sub) = setup();
    let recorder = dcl_metrics::PrometheusBuilder::new().build_recorder();
    let _local = dcl_metrics::set_default_local_recorder(&recorder);
    let catalog = dcl_metrics::http::component(
        Router::new().route("/items/{id}", get(|| async { "ok" })),
        "catalog",
    );
    let shared = Router::new().nest("/shop", catalog);
    let app = http::mount_services(
        shared,
        state.clone(),
        http::standalone(state),
        Router::new(),
    );
    for path in [
        "/peers/sensitive-peer",
        "/archipelago-ea-stats/peers/another-private-peer",
        "/shop/items/secret-item/",
        "/archipelago-ea-stats/metrics",
    ] {
        request(app.clone(), "GET", path, &[]).await;
    }
    let body = recorder.handle().render();
    let counts: Vec<_> = body
        .lines()
        .filter(|line| line.starts_with("dcl_http_requests_total{"))
        .collect();
    assert_eq!(counts.len(), 3, "{body}");
    assert!(
        counts.iter().any(|l| l.contains("component=\"stats\"")
            && l.contains("route=\"unmatched\"")
            && l.contains("status_code=\"404\"")),
        "{body}"
    );
    assert!(
        counts
            .iter()
            .any(|line| line.contains("component=\"stats\"")
                && line.contains("route=\"/peers/{id}\"")
                && line.ends_with(" 2")),
        "{body}"
    );
    assert!(
        counts
            .iter()
            .any(|line| line.contains("component=\"catalog\"")
                && line.contains("route=\"/shop/items/{id}\"")
                && line.ends_with(" 1")),
        "{body}"
    );
    assert!(!body.contains("sensitive-peer") && !body.contains("secret-item"));
}
