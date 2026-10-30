use super::*;
use crate::start::testkit::{Tmp, scene};
use std::net::SocketAddr;

#[test]
fn utterances_upsert_by_id_on_an_append_only_log_and_refuse_malformed_lines() {
    let t = Tmp::new("assistant-utterances");
    let log = Log::open(&t.0).unwrap();
    let first = parse(
        "{\"id\":\"u1\",\"transcript\":\"make the door red\",\"outcome\":\"pending\"}\n\n{\"id\":\"u2\",\"transcript\":\"add a chair\",\"outcome\":\"pending\"}\n",
    )
    .unwrap();
    assert_eq!(first.len(), 2);
    log.upsert(&first).unwrap();
    log.upsert(
        &parse(r#"{"id":"u1","transcript":"make the door red","outcome":"sent","turnId":"t1"}"#)
            .unwrap(),
    )
    .unwrap();
    let latest = log.latest().unwrap();
    assert_eq!(latest.len(), 2, "an update replaces, never duplicates");
    assert_eq!(latest[0]["id"], "u1");
    assert_eq!(latest[0]["outcome"], "sent");
    assert_eq!(latest[0]["turnId"], "t1");
    assert_eq!(latest[1]["id"], "u2");
    assert_eq!(latest[1]["outcome"], "pending");
    let raw = std::fs::read_to_string(t.0.join(".dcl-one").join(FILE)).unwrap();
    assert_eq!(raw.lines().count(), 3, "every upsert is one appended line");
    assert_eq!(
        std::fs::read_to_string(t.0.join(".dcl-one/.gitignore")).unwrap(),
        "*\n",
        "the state directory ignores itself, so no scene .gitignore has to"
    );
    assert_eq!(Log::open(&t.0).unwrap().latest().unwrap().len(), 2);
    assert!(parse(r#"{"transcript":"no id"}"#).is_err());
    assert!(parse(r#"{"id":"../etc"}"#).is_err());
    assert!(parse(r#"{"id":""}"#).is_err());
    assert!(parse(r#"["u1"]"#).is_err());
    assert!(parse("not json").is_err());
    assert!(
        parse(&format!(
            "{{\"id\":\"u1\",\"pad\":\"{}\"}}",
            "x".repeat(MAX_LINE)
        ))
        .is_err()
    );
    assert!(parse("\n  \n").unwrap().is_empty());
}

#[tokio::test]
async fn utterances_route_needs_the_run_token_and_answers_204() {
    let t = Tmp::new("assistant-utterances-http");
    let p = scene(&t.0, "demo", &["0,0"], "compiled");
    let st = Arc::new(crate::start::testkit::state(vec![p.clone()]));
    let bearer = format!("Bearer {}", st.token);
    let app = crate::start::build_router(st, Arc::new(crate::comms::CommsState::default()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let client = reqwest::Client::new();
    let endpoint = format!("{origin}/api/project/assistant/utterances");
    let body = "{\"id\":\"u1\",\"transcript\":\"make the door red\",\"outcome\":\"pending\"}\n";
    let tokenless = client
        .post(&endpoint)
        .header("Origin", &origin)
        .header("Content-Type", "application/x-ndjson")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        tokenless.status(),
        StatusCode::UNAUTHORIZED,
        "an allowed origin alone does not write"
    );
    assert!(
        !p.root.join(".dcl-one").join(FILE).exists(),
        "nothing is stored without the token"
    );
    let stored = client
        .post(&endpoint)
        .header("Origin", &origin)
        .header("Authorization", &bearer)
        .header("Content-Type", "application/x-ndjson")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(stored.status(), StatusCode::NO_CONTENT);
    let latest = Log::open(&p.root).unwrap().latest().unwrap();
    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0]["transcript"], "make the door red");
    let malformed = client
        .post(&endpoint)
        .header("Origin", &origin)
        .header("Authorization", &bearer)
        .body("{\"transcript\":\"no id\"}\n")
        .send()
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    server.abort();
}
