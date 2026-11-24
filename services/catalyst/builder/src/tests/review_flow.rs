//! The review flow as creators, curators and the validator meet it, through
//! the router as `serve` mounts it: with `builder-auto-curation-review-flow`
//! on, a review's timeline records its starts, verdicts, appeals and
//! comments; the validator's verdict rejects or reopens the review it was
//! asked about; creators retry, appeal and comment; with the flag off the
//! four routes answer as absent and nothing changes. Each test has a
//! database of its own and drives the node's clock.

use axum::body::Body;
use axum::http::Request;
use chrono::Duration;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::support::{FakeCommittee, OWNER_KEY, signed_map, unique};
use super::validator::{
    Node, Options, address, advance, call, claim, clock, contract_of, copy_holds, draft, json_of,
    node, node_over, own_pool, publish, published, requests, resave, result_of, review_state, send,
    set_clock, sweep, verdict,
};

const CURATOR_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
const STRANGER_KEY: &str = "0x3333333333333333333333333333333333333333333333333333333333333333";
const MANAGER_KEY: &str = "0x4444444444444444444444444444444444444444444444444444444444444444";
const NOT_FOUND: &str = r#"{"data":{},"error":"Not found","ok":false}"#;

/// The `builder` flags document with the automated review on and the review
/// flow holding `flow`.
async fn set_flags(pool: &PgPool, flow: Value) {
    sqlx::query("DELETE FROM explore_api.feature_flags WHERE app = 'builder'")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO explore_api.feature_flags (app, document, fetched_at) VALUES ('builder', $1, timezone('UTC', now()))",
    )
    .bind(json!({ "flags": { "builder-auto-curation": true, "builder-auto-curation-review-flow": flow }, "variants": {} }))
    .execute(pool)
    .await
    .unwrap();
}

/// A node with the review flow on and `curator` on the committee.
async fn flow_node() -> Option<Node> {
    flow_node_with(Options::default()).await
}

async fn unbounded_flow_node() -> Option<Node> {
    flow_node_with(Options {
        unbounded: true,
        ..Options::default()
    })
    .await
}

async fn flow_node_with(options: Options) -> Option<Node> {
    set_clock(
        chrono::DateTime::from_timestamp_millis(chrono::Utc::now().timestamp_millis()).unwrap(),
    );
    let node = node_over(
        own_pool().await?,
        Options {
            committee: FakeCommittee::with(&[&address(CURATOR_KEY)]),
            ..options
        },
    )
    .await;
    set_flags(&node.pool, json!(true)).await;
    Some(node)
}

type EventRow = (String, String, Option<String>, Value);

async fn events(pool: &PgPool, collection: Uuid) -> Vec<EventRow> {
    sqlx::query_as(
        "SELECT type, actor, actor_address, payload FROM builder.collection_events \
         WHERE collection_id = $1 ORDER BY created_at, id",
    )
    .bind(collection)
    .fetch_all(pool)
    .await
    .unwrap()
}

fn kinds(events: &[EventRow]) -> Vec<&str> {
    events.iter().map(|e| e.0.as_str()).collect()
}

/// A curation of `collection` with `status`, newer than its newest.
async fn collection_curation(pool: &PgPool, collection: Uuid, status: &str) {
    sqlx::query(
        "INSERT INTO builder.collection_curations (id, collection_id, status, created_at, updated_at) \
         SELECT $1, $2, $3, at, at FROM (SELECT GREATEST(timezone('UTC', now()), \
           (SELECT max(created_at) + interval '1 millisecond' FROM builder.collection_curations WHERE collection_id = $2)) AS at) s",
    )
    .bind(Uuid::new_v4())
    .bind(collection)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

async fn newest(pool: &PgPool, collection: Uuid) -> (String, Option<String>) {
    sqlx::query_as(
        "SELECT status, reviewed_by FROM builder.collection_curations WHERE collection_id = $1 \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(collection)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn curations(pool: &PgPool, collection: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM builder.collection_curations WHERE collection_id = $1")
        .bind(collection)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn audits(pool: &PgPool, actor: &str) -> Vec<(String, Option<String>)> {
    sqlx::query_as(
        "SELECT action, target FROM ops.admin_audit WHERE domain = 'create' AND actor = $1 ORDER BY id",
    )
    .bind(actor)
    .fetch_all(pool)
    .await
    .unwrap()
}

fn path(id: Uuid, route: &str) -> String {
    format!("/v1/collections/{id}/{route}")
}

async fn post(node: &Node, key: &str, id: Uuid, route: &str, body: Value) -> (u16, Value) {
    call(&node.app, Some(key), "POST", &path(id, route), Some(body)).await
}

/// `GET /v1/collections/:id/events` with `query`, signed over the path.
async fn timeline(node: &Node, key: Option<&str>, id: Uuid, query: &str) -> (u16, Value) {
    let signed = path(id, "events");
    let mut request = Request::builder()
        .method("GET")
        .uri(format!("{signed}{query}"));
    if let Some(key) = key {
        for (name, value) in signed_map(key, "get", &signed, "{}", false).await {
            request = request.header(name, value);
        }
    }
    let (status, body) = send(&node.app, request.body(Body::empty()).unwrap()).await;
    (status, json_of(&body))
}

fn link(id: Uuid, name: &str) -> String {
    format!(
        "<https://creators.example.org/collections/editor?collection={id}&reviewing=true|{name}>"
    )
}

fn described(id: Uuid, name: &str) -> String {
    format!("\"{name}\" ({id}) https://creators.example.org/collections/{id}")
}

fn creator(key: &str) -> String {
    let owner = address(key);
    format!(
        "<https://creators.example.org/shop/items/creator/{owner}|{}>",
        crate::curation::slack::shorten(&owner)
    )
}

/// The verdict body as the timeline keeps it: builder-server's validation
/// result, without the claim it was posted under.
fn kept(result: &Value) -> Value {
    let mut kept = result.clone();
    kept.as_object_mut().unwrap().remove("claimId");
    kept
}

async fn reject_as_curator(node: &Node, id: Uuid) {
    let (status, body) = call(
        &node.app,
        Some(CURATOR_KEY),
        "PATCH",
        &path(id, "curation"),
        Some(json!({ "curation": { "status": "rejected", "rejectionReasons": ["clipping"] } })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
}

/// A published collection the validator rejected: its newest review is the
/// validator's rejection.
async fn rejected_by_validator(node: &Node, name: &str, items: usize) -> (Uuid, Vec<Uuid>) {
    let (id, items) = published(node, name, items).await;
    collection_curation(&node.pool, id, "pending").await;
    let (_, claimed) = claim(node).await;
    assert_eq!(
        verdict(node, id, &result_of(&claimed, "rejected")).await.0,
        204
    );
    assert_eq!(
        newest(&node.pool, id).await,
        ("rejected".into(), Some("validator".into()))
    );
    (id, items)
}

#[tokio::test]
async fn with_the_review_flow_off_the_four_routes_answer_as_absent_and_change_nothing() {
    let Some(node) = node().await else {
        return;
    };
    let (id, _) = published(&node, "Quiet", 1).await;
    collection_curation(&node.pool, id, "rejected").await;
    let routes: [(&str, &str, Option<Value>); 4] = [
        ("POST", "validations", Some(json!({}))),
        ("POST", "curation/appeal", Some(json!({ "note": "please" }))),
        ("GET", "events", None),
        ("POST", "comments", Some(json!({ "message": "hello" }))),
    ];
    for (label, flow, secret) in [
        ("no review-flow key", None, true),
        ("the flag false", Some(json!(false)), true),
        ("the flag a string", Some(json!("true")), true),
        (
            "the flag on without the validator's secret",
            Some(json!(true)),
            false,
        ),
    ] {
        let node = if secret {
            node_over(node.pool.clone(), Options::default()).await
        } else {
            node_over(
                node.pool.clone(),
                Options {
                    secret: None,
                    ..Options::default()
                },
            )
            .await
        };
        match &flow {
            Some(flow) => set_flags(&node.pool, flow.clone()).await,
            None => super::validator::set_flag(&node.pool, Some(json!(true))).await,
        }
        let before = review_state(&node.pool).await;
        for (method, route, body) in &routes {
            let (status, answer) =
                call(&node.app, None, method, &path(id, route), body.clone()).await;
            assert_eq!(status, 401, "{label}: unsigned {route} is refused first");
            assert_eq!(answer["error"], "Unauthenticated", "{label}");
            for key in [OWNER_KEY, STRANGER_KEY] {
                let (status, answer) =
                    call(&node.app, Some(key), method, &path(id, route), body.clone()).await;
                assert_eq!(
                    (status, answer.to_string()),
                    (404, NOT_FOUND.to_string()),
                    "{label}: {route}"
                );
            }
            let (status, answer) = call(
                &node.app,
                Some(OWNER_KEY),
                method,
                &path(Uuid::new_v4(), route),
                body.clone(),
            )
            .await;
            assert_eq!(
                (status, answer.to_string()),
                (404, NOT_FOUND.to_string()),
                "{label}: {route} of an absent collection"
            );
        }
        assert_eq!(review_state(&node.pool).await, before, "{label}");
    }
}

#[tokio::test]
async fn with_creator_writes_off_the_three_review_writes_answer_503_and_the_timeline_reads() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, _) = rejected_by_validator(&node, "Closed", 1).await;
    let off = node_over(
        node.pool.clone(),
        Options {
            writes: false,
            storage: Some(node.storage.clone()),
            ..Options::default()
        },
    )
    .await;
    let before = review_state(&node.pool).await;
    for (route, body) in [
        ("validations", json!({})),
        ("curation/appeal", json!({})),
        ("comments", json!({ "message": "hello" })),
    ] {
        let (status, answer) = post(&off, OWNER_KEY, id, route, body).await;
        assert_eq!(status, 503, "{route}: {answer}");
    }
    assert_eq!(review_state(&node.pool).await, before);
    let (status, page) = timeline(&off, Some(OWNER_KEY), id, "").await;
    assert_eq!((status, page["data"]["total"].clone()), (200, json!(3)));
}

#[tokio::test]
async fn a_rejecting_verdict_with_the_flow_on_rejects_the_review_as_the_validator() {
    let Some(node) = flow_node().await else {
        return;
    };
    let owner = address(OWNER_KEY);
    let (id, items) = published(&node, "Judged", 2).await;
    let started = events(&node.pool, id).await;
    assert_eq!(
        kinds(&started),
        ["collection.published", "review.automated_started"]
    );
    assert_eq!(
        (&started[0].1, &started[0].2),
        (&"creator".to_string(), &Some(owner.clone()))
    );
    let request = requests(&node.pool, id).await[0].0;
    let start = &started[1];
    assert_eq!((start.1.as_str(), start.2.as_deref()), ("validator", None));
    assert_eq!(start.3["validationId"], json!(request));
    assert_eq!(start.3["trigger"], "publish");
    assert_eq!(start.3["itemIds"], json!(items));
    assert_eq!(
        start.3["contentHashes"].as_object().unwrap().len(),
        2,
        "{start:?}"
    );
    collection_curation(&node.pool, id, "pending").await;
    let (_, claimed) = claim(&node).await;
    let result = result_of(&claimed, "rejected");
    assert_eq!(verdict(&node, id, &result).await, (204, Value::Null));
    assert_eq!(
        newest(&node.pool, id).await,
        ("rejected".into(), Some("validator".into()))
    );
    assert_eq!(
        curations(&node.pool, id).await,
        1,
        "the pending review is rejected in place"
    );
    let after = events(&node.pool, id).await;
    assert_eq!(
        kinds(&after),
        [
            "collection.published",
            "review.automated_started",
            "review.automated_rejected"
        ]
    );
    assert_eq!(
        (after[2].1.as_str(), after[2].2.as_deref(), &after[2].3),
        ("validator", None, &kept(&result))
    );
    assert_eq!(
        audits(&node.pool, "validator").await,
        vec![("builder.curation.validator".into(), Some(id.to_string()))]
    );

    let (none, _) = published(&node, "Unreviewed", 1).await;
    let (_, claimed) = claim(&node).await;
    assert_eq!(
        verdict(&node, none, &result_of(&claimed, "rejected"))
            .await
            .0,
        204
    );
    assert_eq!(
        newest(&node.pool, none).await,
        ("rejected".into(), Some("validator".into())),
        "a collection with no review gets the validator's rejection"
    );

    set_flags(&node.pool, json!(false)).await;
    let (quiet, _) = published(&node, "Advisory", 1).await;
    collection_curation(&node.pool, quiet, "pending").await;
    let before = review_state(&node.pool).await;
    let (_, claimed) = claim(&node).await;
    assert_eq!(
        verdict(&node, quiet, &result_of(&claimed, "rejected"))
            .await
            .0,
        204
    );
    assert_eq!(
        (
            review_state(&node.pool).await,
            requests(&node.pool, quiet).await[0].4.clone()
        ),
        (before, Some("rejected".into())),
        "with the flow off the verdict settles its request and changes no review"
    );
    assert_eq!(audits(&node.pool, "validator").await.len(), 2);
}

#[tokio::test]
async fn a_passing_verdict_reopens_only_the_validators_own_rejection_and_never_approves() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, items) = rejected_by_validator(&node, "Fixed", 1).await;
    resave(&node, id, items[0], 0, &unique("fixed model")).await;
    let (status, body) = post(&node, OWNER_KEY, id, "validations", json!({})).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(newest(&node.pool, id).await, ("pending".into(), None));
    let (_, claimed) = claim(&node).await;
    let passed = result_of(&claimed, "passed");
    assert_eq!(verdict(&node, id, &passed).await.0, 204);
    assert_eq!(
        (
            newest(&node.pool, id).await,
            curations(&node.pool, id).await
        ),
        (("pending".into(), None), 2),
        "a pass leaves the review pending for a curator"
    );
    assert_eq!(
        kinds(&events(&node.pool, id).await).last(),
        Some(&"review.automated_passed")
    );
    node.state.notices.settled().await;
    assert!(
        node.slack.texts().contains(&format!(
            "Automated review passed for collection {}. It is ready for a curator approval.",
            described(id, "Fixed")
        )),
        "{:?}",
        node.slack.texts()
    );

    let (curated, _) = published(&node, "Curated", 1).await;
    collection_curation(&node.pool, curated, "pending").await;
    let (_, claimed) = claim(&node).await;
    reject_as_curator(&node, curated).await;
    assert_eq!(
        verdict(&node, curated, &result_of(&claimed, "passed"))
            .await
            .0,
        204
    );
    assert_eq!(
        newest(&node.pool, curated).await,
        ("rejected".into(), Some(address(CURATOR_KEY))),
        "a pass never reopens a curator's rejection"
    );

    let (imported, _) = published(&node, "Imported", 1).await;
    sqlx::query(
        "INSERT INTO builder.collection_curations (id, collection_id, status, reviewed_by, created_at, updated_at) \
         VALUES ($1, $2, 'rejected', $3, timezone('UTC', now()), timezone('UTC', now()))",
    )
    .bind(Uuid::new_v4())
    .bind(imported)
    .bind(address(CURATOR_KEY))
    .execute(&node.pool)
    .await
    .unwrap();
    let (_, claimed) = claim(&node).await;
    assert_eq!(
        verdict(&node, imported, &result_of(&claimed, "passed"))
            .await
            .0,
        204
    );
    assert_eq!(
        (
            newest(&node.pool, imported).await,
            curations(&node.pool, imported).await
        ),
        (("rejected".into(), Some(address(CURATOR_KEY))), 1),
        "a curator's rejection with no decision on the timeline, as an imported one, is not reopened either"
    );
}

#[tokio::test]
async fn a_verdict_never_overrides_a_decision_a_curator_made_since_the_request() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, _) = published(&node, "Approved", 1).await;
    collection_curation(&node.pool, id, "pending").await;
    let (_, claimed) = claim(&node).await;
    let (status, body) = call(
        &node.app,
        Some(CURATOR_KEY),
        "PATCH",
        &path(id, "curation"),
        Some(json!({ "curation": { "status": "approved" } })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        verdict(&node, id, &result_of(&claimed, "rejected")).await.0,
        204
    );
    assert_eq!(
        newest(&node.pool, id).await,
        ("approved".into(), Some(address(CURATOR_KEY))),
        "the curator's approval stands"
    );
    assert_eq!(
        kinds(&events(&node.pool, id).await).last(),
        Some(&"review.automated_rejected"),
        "the verdict is kept for the timeline"
    );
    assert!(audits(&node.pool, "validator").await.is_empty());

    let (reopened, _) = published(&node, "Reopened", 1).await;
    collection_curation(&node.pool, reopened, "pending").await;
    let (_, claimed) = claim(&node).await;
    for status in ["approved", "pending"] {
        let (got, body) = call(
            &node.app,
            Some(CURATOR_KEY),
            "PATCH",
            &path(reopened, "curation"),
            Some(json!({ "curation": { "status": status } })),
        )
        .await;
        assert_eq!(got, 200, "{body}");
    }
    assert_eq!(
        verdict(&node, reopened, &result_of(&claimed, "rejected"))
            .await
            .0,
        204
    );
    assert_eq!(
        newest(&node.pool, reopened).await.0,
        "pending",
        "a curator approved and reopened the review since: it stays open for them"
    );
}

#[tokio::test]
async fn a_stale_or_superseded_claims_verdict_changes_no_review() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, items) = published(&node, "Stale", 1).await;
    collection_curation(&node.pool, id, "pending").await;
    let (_, first) = claim(&node).await;
    advance(Duration::minutes(30) + Duration::milliseconds(1));
    let (_, second) = claim(&node).await;
    let before = review_state(&node.pool).await;
    assert_eq!(
        verdict(&node, id, &result_of(&first, "rejected")).await.0,
        204
    );
    assert_eq!(
        review_state(&node.pool).await,
        before,
        "a lapsed claim taken over decides nothing"
    );

    reject_as_curator(&node, id).await;
    resave(&node, id, items[0], 0, &unique("superseding model")).await;
    let (status, body) = post(&node, OWNER_KEY, id, "curation", json!({})).await;
    assert_eq!(status, 200, "{body}");
    let rows = requests(&node.pool, id).await;
    assert_eq!(
        (rows[0].4.as_deref(), rows[1].1.as_str()),
        (Some("superseded"), "changes")
    );
    let before = review_state(&node.pool).await;
    assert_eq!(
        verdict(&node, id, &result_of(&second, "rejected")).await.0,
        204
    );
    assert_eq!(
        (review_state(&node.pool).await, newest(&node.pool, id).await),
        (before, ("pending".into(), None)),
        "a superseded request's verdict decides nothing"
    );

    let (other, _) = published(&node, "Forged", 1).await;
    collection_curation(&node.pool, other, "pending").await;
    let mut forged = result_of(&second, "rejected");
    forged["collectionId"] = json!(other);
    forged["validationId"] = json!(Uuid::new_v4());
    let before = review_state(&node.pool).await;
    assert_eq!(verdict(&node, other, &forged).await.0, 204);
    assert_eq!(
        review_state(&node.pool).await,
        before,
        "a verdict about a collection the validator wasn't asked about decides nothing"
    );
}

#[tokio::test]
async fn an_abandoned_review_with_the_flow_on_tells_the_timeline_it_needs_a_curator() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, _) = published(&node, "Abandoned", 1).await;
    for _ in 0..3 {
        let (status, _) = claim(&node).await;
        assert_eq!(status, 200);
        advance(Duration::minutes(30) + Duration::milliseconds(1));
    }
    sweep(&node).await;
    sweep(&node).await;
    let timeline = events(&node.pool, id).await;
    let request = requests(&node.pool, id).await[0].0;
    let errors: Vec<&EventRow> = timeline
        .iter()
        .filter(|e| e.0 == "review.automated_error")
        .collect();
    assert_eq!(errors.len(), 1, "{timeline:?}");
    assert_eq!(
        errors[0].3,
        json!({ "reason": "sweep_exhausted", "validationId": request })
    );
}

#[tokio::test]
async fn a_sweep_meeting_a_verdict_on_the_same_collection_waits_for_it_without_a_deadlock() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, _) = published(&node, "Crossed", 1).await;
    for _ in 0..3 {
        assert_eq!(claim(&node).await.0, 200);
        advance(Duration::minutes(30) + Duration::milliseconds(1));
    }
    let request = requests(&node.pool, id).await[0].0;
    let mut verdict_tx = node.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(id)
        .execute(&mut *verdict_tx)
        .await
        .unwrap();
    let verdict_side = async {
        loop {
            let waiting: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_locks l JOIN pg_stat_activity a ON a.pid = l.pid \
                 WHERE NOT l.granted AND a.datname = current_database()",
            )
            .fetch_one(&node.pool)
            .await
            .unwrap();
            if waiting > 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
        let locked =
            sqlx::query("SELECT id FROM builder.validation_requests WHERE id = $1 FOR UPDATE")
                .bind(request)
                .execute(&mut *verdict_tx)
                .await
                .map(|_| ());
        verdict_tx.commit().await.unwrap();
        locked
    };
    let (swept, locked) = tokio::join!(crate::validator::sweep(&node.state), verdict_side);
    assert!(locked.is_ok(), "the verdict's request lock: {locked:?}");
    assert!(swept.is_ok(), "the sweep: {swept:?}");
    assert_eq!(
        requests(&node.pool, id).await[0].4.as_deref(),
        Some("abandoned")
    );
}

#[tokio::test]
async fn a_creator_appeals_a_rejection_and_the_curators_see_it() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, _) = rejected_by_validator(&node, "Appealed", 1).await;
    for key in [STRANGER_KEY, CURATOR_KEY] {
        let (got, body) = post(&node, key, id, "curation/appeal", json!({})).await;
        assert_eq!(
            (got, body["error"].clone()),
            (
                401,
                json!(format!(
                    "Unauthorized user {} for collections {id}",
                    address(key)
                ))
            ),
            "{body}"
        );
    }
    for body in [
        json!({ "note": "x".repeat(2001) }),
        json!({ "note": 1 }),
        json!({ "note": "ok", "extra": true }),
    ] {
        assert_eq!(
            post(&node, OWNER_KEY, id, "curation/appeal", body.clone())
                .await
                .0,
            400,
            "{body}"
        );
    }
    let note = "  <!channel> look *here*\nsecond line & more  ";
    let (status, body) = post(
        &node,
        OWNER_KEY,
        id,
        "curation/appeal",
        json!({ "note": note }),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["status"], "pending");
    assert_eq!(body["data"]["collection_id"], json!(id));
    assert_eq!(newest(&node.pool, id).await, ("pending".into(), None));
    let appeal = events(&node.pool, id).await.pop().unwrap();
    assert_eq!(
        appeal,
        (
            "review.appeal_requested".into(),
            "creator".into(),
            Some(address(OWNER_KEY)),
            json!({ "note": note.trim() })
        )
    );
    node.state.notices.settled().await;
    assert!(
        node.slack.texts().contains(&format!(
            "Human review requested for collection {}.\n> &lt;!channel&gt; look \u{200b}*\u{200b}here\u{200b}*\u{200b} second line &amp; more",
            described(id, "Appealed")
        )),
        "{:?}",
        node.slack.texts()
    );
    let (status, body) = post(&node, OWNER_KEY, id, "curation/appeal", json!({})).await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            409,
            json!("There is already an open appeal for this collection")
        )
    );
    let (_, page) = timeline(&node, Some(CURATOR_KEY), id, "").await;
    assert_eq!(
        page["data"]["results"][0]["type"],
        "review.appeal_requested"
    );

    for status in ["pending", "approved"] {
        let (other, _) = published(&node, status, 1).await;
        collection_curation(&node.pool, other, status).await;
        let (got, body) = post(&node, OWNER_KEY, other, "curation/appeal", json!({})).await;
        assert_eq!(
            (got, body["error"].clone()),
            (409, json!("Only rejected collections can be appealed")),
            "a {status} review"
        );
    }
}

#[tokio::test]
async fn comments_are_written_by_role_holders_and_the_committee_and_stored_as_text() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, items) = published(&node, "Talk", 1).await;
    collection_curation(&node.pool, id, "pending").await;
    let message = "  <script>alert(1)</script>'; DROP TABLE x; --\n@here  ";
    let (status, body) = post(
        &node,
        OWNER_KEY,
        id,
        "comments",
        json!({ "message": message, "itemId": items[0] }),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let data = &body["data"];
    assert_eq!(
        (
            data["type"].clone(),
            data["actor"].clone(),
            data["actor_address"].clone(),
            data["payload"].clone(),
            data["collection_id"].clone()
        ),
        (
            json!("review.comment"),
            json!("creator"),
            json!(address(OWNER_KEY)),
            json!({ "message": message.trim(), "itemId": items[0] }),
            json!(id)
        )
    );
    let (status, body) = post(
        &node,
        CURATOR_KEY,
        id,
        "comments",
        json!({ "message": "Fix it" }),
    )
    .await;
    assert_eq!(
        (status, body["data"]["actor"].clone()),
        (200, json!("curator"))
    );
    node.state.notices.settled().await;
    let comments: Vec<String> = node
        .slack
        .texts()
        .into_iter()
        .filter(|t| t.starts_with("💬"))
        .collect();
    assert_eq!(
        comments,
        vec![format!(
            "💬 New comment on {} by {}: &lt;script&gt;alert(1)&lt;/script&gt;'; DROP TABLE x; -- @\u{200b}here",
            link(id, "Talk"),
            creator(OWNER_KEY)
        )],
        "only a creator's comment reaches the channel"
    );

    let before = review_state(&node.pool).await;
    let (status, _) = post(
        &node,
        STRANGER_KEY,
        id,
        "comments",
        json!({ "message": "hi" }),
    )
    .await;
    assert_eq!(status, 401);
    for body in [
        json!({ "message": "   " }),
        json!({ "message": "x".repeat(2001) }),
        json!({ "message": "hi", "itemId": Uuid::new_v4() }),
        json!({ "message": "hi", "itemId": "not-a-uuid" }),
        json!({ "message": "hi", "extra": 1 }),
        json!({}),
    ] {
        assert_eq!(
            post(&node, OWNER_KEY, id, "comments", body.clone()).await.0,
            400,
            "{body}"
        );
    }
    assert_eq!(
        post(
            &node,
            OWNER_KEY,
            id,
            "comments",
            json!({ "message": "é".repeat(2000) })
        )
        .await
        .0,
        200
    );
    assert_ne!(review_state(&node.pool).await, before);

    collection_curation(&node.pool, id, "approved").await;
    let (status, body) = post(
        &node,
        OWNER_KEY,
        id,
        "comments",
        json!({ "message": "late" }),
    )
    .await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            409,
            json!("The collection has no open review to comment on")
        )
    );
    let (unpublished, _) = draft(&node, "Draft", 1).await;
    let (status, body) = post(
        &node,
        OWNER_KEY,
        unpublished,
        "comments",
        json!({ "message": "hi" }),
    )
    .await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            409,
            json!("The collection is not published, so it has no review yet")
        )
    );
}

#[tokio::test]
async fn the_timeline_reads_for_its_owner_manager_and_committee_and_is_absent_to_a_stranger() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, _) = rejected_by_validator(&node, "Read", 1).await;
    let contract = contract_of(&node.pool, id).await;
    copy_holds(&node.pool, &contract, &[]).await;
    sqlx::query("UPDATE shop.collections SET managers = ARRAY[$2] WHERE contract_address = $1")
        .bind(&contract)
        .bind(address(MANAGER_KEY))
        .execute(&node.pool)
        .await
        .unwrap();
    let request = requests(&node.pool, id).await[0].0;
    let (status, page) = timeline(&node, Some(CURATOR_KEY), id, "").await;
    assert_eq!(status, 200, "{page}");
    let data = &page["data"];
    assert_eq!(
        (
            data["total"].clone(),
            data["page"].clone(),
            data["limit"].clone(),
            data["pages"].clone()
        ),
        (json!(3), json!(1), json!(50), json!(1))
    );
    let results = data["results"].as_array().unwrap();
    assert_eq!(
        results
            .iter()
            .map(|e| e["type"].clone())
            .collect::<Vec<_>>(),
        [
            "review.automated_rejected",
            "review.automated_started",
            "collection.published"
        ]
    );
    assert_eq!(results[0]["payload"]["validationId"], json!(request));
    let mut keys: Vec<&String> = results[0].as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "actor",
            "actor_address",
            "collection_id",
            "created_at",
            "id",
            "payload",
            "type"
        ]
    );
    for key in [OWNER_KEY, MANAGER_KEY] {
        let (status, page) = timeline(&node, Some(key), id, "").await;
        assert_eq!(status, 200, "{page}");
        let first = &page["data"]["results"][0]["payload"];
        assert!(first.get("validationId").is_none(), "{first}");
        assert!(
            page["data"]["results"][1]["payload"]
                .get("validationId")
                .is_none(),
            "a start is redacted too"
        );
    }
    let (status, page) = timeline(&node, Some(OWNER_KEY), id, "?page=2&limit=2").await;
    assert_eq!(
        (
            status,
            page["data"]["results"].as_array().unwrap().len(),
            page["data"]["pages"].clone(),
            page["data"]["results"][0]["type"].clone()
        ),
        (200, 1, json!(2), json!("collection.published"))
    );
    let (_, page) = timeline(&node, Some(OWNER_KEY), id, "?page=-4&limit=999").await;
    assert_eq!(
        (page["data"]["page"].clone(), page["data"]["limit"].clone()),
        (json!(1), json!(200)),
        "page and limit are clamped"
    );
    let (_, page) = timeline(&node, Some(OWNER_KEY), id, "?page=99999&limit=abc").await;
    assert_eq!(
        (page["data"]["page"].clone(), page["data"]["limit"].clone()),
        (json!(10000), json!(50))
    );

    let (stranger, hidden) = timeline(&node, Some(STRANGER_KEY), id, "").await;
    let absent_id = Uuid::new_v4();
    let (absent, missing) = timeline(&node, Some(STRANGER_KEY), absent_id, "").await;
    assert_eq!((stranger, absent), (404, 404));
    assert_eq!(
        hidden.to_string().replace(&id.to_string(), "ID"),
        missing.to_string().replace(&absent_id.to_string(), "ID"),
        "a stranger can't tell an existing collection from an absent one"
    );
    assert_eq!(timeline(&node, None, id, "").await.0, 401);
}

/// While the third-party registry can't be read, nobody is known to manage a
/// third-party collection: a stranger asking for its timeline gets what an
/// absent collection gets and the committee still reads it.
#[tokio::test]
async fn a_stranger_cannot_tell_a_third_party_timeline_from_an_absent_one_while_the_registry_is_down()
 {
    let registry = super::support::FakeRegistry::up();
    let Some(node) = flow_node_with(Options {
        registry: registry.clone(),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let provider = "urn:decentraland:matic:collections-thirdparty:timeline";
    registry.put(provider, &[&address(MANAGER_KEY)], true, false);
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) VALUES ($1, 'TP', $2, $3, $4)",
    )
    .bind(id)
    .bind(address(MANAGER_KEY))
    .bind(provider)
    .bind(format!("{provider}:timeline"))
    .execute(&node.pool)
    .await
    .unwrap();
    let absent_id = Uuid::new_v4();
    let answers = async |id: Uuid| {
        let mut statuses = Vec::new();
        for key in [STRANGER_KEY, CURATOR_KEY, MANAGER_KEY] {
            statuses.push(timeline(&node, Some(key), id, "").await.0);
        }
        statuses
    };
    assert_eq!(answers(id).await, [404, 200, 200]);
    registry.fail(true);
    node.state.third_parties.outdate();
    assert_eq!(answers(id).await, [404, 200, 404]);
    assert_eq!(answers(absent_id).await, [404, 404, 404]);
    let (_, hidden) = timeline(&node, Some(STRANGER_KEY), id, "").await;
    let (_, missing) = timeline(&node, Some(STRANGER_KEY), absent_id, "").await;
    assert_eq!(
        hidden.to_string().replace(&id.to_string(), "ID"),
        missing.to_string().replace(&absent_id.to_string(), "ID")
    );
}

#[tokio::test]
async fn a_creator_retries_the_automated_review_once_something_changed() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, items) = published(&node, "Retry", 2).await;
    collection_curation(&node.pool, id, "pending").await;
    let (status, body) = post(&node, OWNER_KEY, id, "validations", json!({})).await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            409,
            json!("A validation is already in progress for this collection")
        )
    );
    let (_, claimed) = claim(&node).await;
    assert_eq!(
        verdict(&node, id, &result_of(&claimed, "rejected")).await.0,
        204
    );
    let (status, body) = post(&node, OWNER_KEY, id, "validations", json!({})).await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            409,
            json!("Nothing changed since the last validation: edit an item or appeal the decision")
        )
    );
    for key in [STRANGER_KEY, CURATOR_KEY] {
        assert_eq!(post(&node, key, id, "validations", json!({})).await.0, 401);
    }
    resave(&node, id, items[1], 1, &unique("retried model")).await;
    advance(Duration::milliseconds(1));
    let (status, body) = post(&node, OWNER_KEY, id, "validations", json!({})).await;
    assert_eq!(status, 200, "{body}");
    let data = &body["data"];
    assert_eq!(
        (
            data["type"].clone(),
            data["actor"].clone(),
            data["actor_address"].clone(),
            data["payload"]["trigger"].clone(),
            data["payload"]["itemIds"].clone()
        ),
        (
            json!("review.automated_started"),
            json!("validator"),
            json!(address(OWNER_KEY)),
            json!("retry"),
            json!([items[1], items[0]])
        )
    );
    assert!(data["payload"].get("validationId").is_none(), "{data}");
    let rows = requests(&node.pool, id).await;
    assert_eq!(
        (rows.len(), rows[1].1.as_str(), rows[1].2.clone()),
        (2, "retry", vec![items[1], items[0]])
    );
    assert_eq!(newest(&node.pool, id).await, ("pending".into(), None));

    let (curated, curated_items) = published(&node, "Curator rejected", 1).await;
    collection_curation(&node.pool, curated, "pending").await;
    reject_as_curator(&node, curated).await;
    resave(
        &node,
        curated,
        curated_items[0],
        0,
        &unique("after curator"),
    )
    .await;
    let (status, body) = post(&node, OWNER_KEY, curated, "validations", json!({})).await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            409,
            json!(
                "A curator rejected this collection: submit your changes for review instead of validating again"
            )
        )
    );
    collection_curation(&node.pool, curated, "approved").await;
    let (status, body) = post(&node, OWNER_KEY, curated, "validations", json!({})).await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            409,
            json!("There is nothing to validate again: submit your changes for review")
        )
    );
    let (unpublished, _) = draft(&node, "Draft", 1).await;
    let (status, body) = post(&node, OWNER_KEY, unpublished, "validations", json!({})).await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            409,
            json!("The collection is not published, so it has no review yet")
        )
    );
}

#[tokio::test]
async fn the_sixth_retry_of_a_utc_day_is_refused_until_the_next_day() {
    let Some(node) = flow_node().await else {
        return;
    };
    set_clock(
        super::validator::clock()
            .date_naive()
            .and_hms_opt(1, 0, 0)
            .unwrap()
            .and_utc(),
    );
    let (id, items) = published(&node, "Busy", 1).await;
    collection_curation(&node.pool, id, "pending").await;
    for n in 0..5 {
        let (_, claimed) = claim(&node).await;
        assert_eq!(
            verdict(&node, id, &result_of(&claimed, "rejected")).await.0,
            204
        );
        if n == 4 {
            break;
        }
        resave(&node, id, items[0], 0, &unique(&format!("busy {n}"))).await;
        let (status, body) = post(&node, OWNER_KEY, id, "validations", json!({})).await;
        assert_eq!(status, 200, "retry {n}: {body}");
    }
    resave(&node, id, items[0], 0, &unique("busy last")).await;
    let before = review_state(&node.pool).await;
    let (status, body) = post(&node, OWNER_KEY, id, "validations", json!({})).await;
    let tomorrow = clock()
        .date_naive()
        .succ_opt()
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    assert_eq!(
        (
            status,
            body["error"].clone(),
            body["data"]["retryAt"].clone()
        ),
        (
            429,
            json!("The daily validation limit for this collection was reached"),
            json!(crate::http::iso(&tomorrow))
        )
    );
    assert_eq!(
        review_state(&node.pool).await,
        before,
        "refused before anything is written"
    );
    set_clock(tomorrow);
    assert_eq!(
        post(&node, OWNER_KEY, id, "validations", json!({})).await.0,
        200
    );
}

#[tokio::test]
async fn a_changes_submission_with_the_flow_on_waits_for_the_running_review_and_records_its_start()
{
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, items) = published(&node, "Submitted", 1).await;
    let (_, claimed) = claim(&node).await;
    assert_eq!(
        verdict(&node, id, &result_of(&claimed, "rejected")).await.0,
        204
    );
    assert_eq!(
        curations(&node.pool, id).await,
        1,
        "the validator opened the review rejected"
    );
    let before = review_state(&node.pool).await;
    let note = json!({ "curation": { "note": "first request" } });
    let (status, body) = post(&node, OWNER_KEY, id, "curation", note.clone()).await;
    assert_eq!(
        (status, body["data"]["status"].clone()),
        (200, json!("rejected")),
        "nothing changed since the validator's own rejection: it answers the request ({body})"
    );
    assert_eq!(
        review_state(&node.pool).await,
        before,
        "and nothing is written"
    );

    resave(&node, id, items[0], 0, &unique("submitted model")).await;
    let (status, body) = post(
        &node,
        OWNER_KEY,
        id,
        "curation",
        json!({ "curation": { "note": " new hat " } }),
    )
    .await;
    assert_eq!(
        (status, body["data"]["status"].clone()),
        (200, json!("pending")),
        "{body}"
    );
    let timeline = events(&node.pool, id).await;
    let tail: Vec<&str> = kinds(&timeline).into_iter().rev().take(2).collect();
    assert_eq!(tail, ["review.automated_started", "changes.submitted"]);
    let submitted = &timeline[timeline.len() - 2];
    assert_eq!(
        (
            submitted.1.as_str(),
            submitted.2.clone(),
            submitted.3.clone()
        ),
        (
            "creator",
            Some(address(OWNER_KEY)),
            json!({ "itemIds": [items[0]], "note": "new hat" })
        )
    );
    assert_eq!(timeline.last().unwrap().3["trigger"], "changes");

    collection_curation(&node.pool, id, "approved").await;
    resave(&node, id, items[0], 0, &unique("while running")).await;
    let before = review_state(&node.pool).await;
    let (status, body) = post(&node, OWNER_KEY, id, "curation", json!({})).await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            409,
            json!("A validation is already in progress for this collection")
        )
    );
    assert_eq!(
        review_state(&node.pool).await,
        before,
        "refused before anything is written"
    );
    let (_, claimed) = claim(&node).await;
    assert_eq!(
        verdict(&node, id, &result_of(&claimed, "passed")).await.0,
        204
    );
    reject_as_curator(&node, id).await;
    let (status, body) = post(
        &node,
        OWNER_KEY,
        id,
        "curation",
        json!({ "curation": { "note": "please look" } }),
    )
    .await;
    assert_eq!(
        (status, body["data"]["status"].clone()),
        (200, json!("pending")),
        "{body}"
    );
    let last = events(&node.pool, id).await.pop().unwrap();
    assert_eq!(
        (last.0.as_str(), last.3),
        ("review.appeal_requested", json!({ "note": "please look" })),
        "nothing changed after a curator's rejection: an appeal"
    );
}

#[tokio::test]
async fn creator_text_is_bounded_per_collection_and_per_caller_each_utc_day() {
    let Some(node) = unbounded_flow_node().await else {
        return;
    };
    set_clock(clock().date_naive().and_hms_opt(1, 0, 0).unwrap().and_utc());
    let (appealed, _) = rejected_by_validator(&node, "Appeals", 1).await;
    let (first, _) = published(&node, "Chatty", 1).await;
    let (second, _) = published(&node, "Chatty too", 1).await;
    for id in [first, second] {
        collection_curation(&node.pool, id, "pending").await;
    }
    let comment = json!({ "message": "again" });
    for n in 0..crate::review::COMMENTS_PER_COLLECTION {
        let (status, body) = post(&node, OWNER_KEY, first, "comments", comment.clone()).await;
        assert_eq!(status, 200, "comment {n}: {body}");
    }
    let (status, body) = post(&node, OWNER_KEY, first, "comments", comment.clone()).await;
    assert_eq!(
        (status, body["error"].clone()),
        (429, json!("The daily limit of comments was reached"))
    );
    let (status, _) = post(&node, CURATOR_KEY, first, "comments", comment.clone()).await;
    assert_eq!(status, 429, "the collection's bound holds for everyone");
    let caller_left = crate::review::COMMENTS_PER_CALLER - crate::review::COMMENTS_PER_COLLECTION;
    for _ in 0..caller_left {
        assert_eq!(
            post(&node, OWNER_KEY, second, "comments", comment.clone())
                .await
                .0,
            200
        );
    }
    assert_eq!(
        post(&node, OWNER_KEY, second, "comments", comment.clone())
            .await
            .0,
        429,
        "the caller's bound across collections"
    );
    assert_eq!(
        post(&node, CURATOR_KEY, second, "comments", comment.clone())
            .await
            .0,
        200
    );
    advance(Duration::days(1));
    assert_eq!(
        post(&node, OWNER_KEY, first, "comments", comment.clone())
            .await
            .0,
        200
    );

    for n in 0..crate::review::APPEALS_PER_COLLECTION {
        let (status, body) = post(&node, OWNER_KEY, appealed, "curation/appeal", json!({})).await;
        assert_eq!(status, 200, "appeal {n}: {body}");
        reject_as_curator(&node, appealed).await;
    }
    let (status, body) = post(&node, OWNER_KEY, appealed, "curation/appeal", json!({})).await;
    assert_eq!(
        (status, body["error"].clone()),
        (429, json!("The daily limit of appeals was reached"))
    );
}

#[tokio::test]
async fn concurrent_appeals_open_one_review_and_concurrent_comments_keep_the_bound() {
    let Some(node) = unbounded_flow_node().await else {
        return;
    };
    let (id, _) = rejected_by_validator(&node, "Racing", 1).await;
    let appeals = futures::future::join_all(
        (0..4).map(|_| post(&node, OWNER_KEY, id, "curation/appeal", json!({}))),
    )
    .await;
    let mut statuses: Vec<u16> = appeals.iter().map(|(s, _)| *s).collect();
    statuses.sort();
    assert_eq!(statuses, [200, 409, 409, 409], "{appeals:?}");
    assert_eq!(curations(&node.pool, id).await, 2);
    let appealed = events(&node.pool, id).await;
    assert_eq!(
        appealed
            .iter()
            .filter(|e| e.0 == "review.appeal_requested")
            .count(),
        1
    );

    let bound = crate::review::COMMENTS_PER_COLLECTION;
    for _ in 0..bound - 3 {
        assert_eq!(
            post(
                &node,
                CURATOR_KEY,
                id,
                "comments",
                json!({ "message": "m" })
            )
            .await
            .0,
            200
        );
    }
    let racing = futures::future::join_all((0..8).map(|_| {
        post(
            &node,
            CURATOR_KEY,
            id,
            "comments",
            json!({ "message": "m" }),
        )
    }))
    .await;
    assert_eq!(
        racing.iter().filter(|(s, _)| *s == 200).count(),
        3,
        "{racing:?}"
    );
    let stored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM builder.collection_events WHERE collection_id = $1 AND type = 'review.comment'",
    )
    .bind(id)
    .fetch_one(&node.pool)
    .await
    .unwrap();
    assert_eq!(stored, bound as i64);

    let (other, _) = published(&node, "Racing too", 1).await;
    collection_curation(&node.pool, other, "pending").await;
    let mut holder = node.pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("builder.review-text:{}", address(OWNER_KEY)))
        .execute(&mut *holder)
        .await
        .unwrap();
    let app = node.app.clone();
    let commenting = tokio::spawn(async move {
        call(
            &app,
            Some(OWNER_KEY),
            "POST",
            &path(other, "comments"),
            Some(json!({ "message": "m" })),
        )
        .await
    });
    let mut waited = false;
    for _ in 0..500 {
        assert!(
            !commenting.is_finished(),
            "a comment was written while another of the wallet's texts was being counted"
        );
        waited = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE wait_event_type = 'Lock' AND wait_event = 'advisory')",
        )
        .fetch_one(&node.pool)
        .await
        .unwrap();
        if waited {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(waited, "the comment waits for the wallet's other texts");
    holder.rollback().await.unwrap();
    assert_eq!(commenting.await.unwrap().0, 200);
}

#[tokio::test]
async fn a_first_publication_without_the_flow_records_no_timeline() {
    let Some(pool) = own_pool().await else {
        return;
    };
    let node = node_over(pool, Options::default()).await;
    let (id, _) = draft(&node, "Plain", 1).await;
    assert_eq!(publish(&node, id, 1).await.0, 200);
    assert!(events(&node.pool, id).await.is_empty());
    assert_eq!(requests(&node.pool, id).await.len(), 1);
}

async fn on_chain_unapproved(pool: &PgPool, collections: &[Uuid]) {
    for id in collections {
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
               is_completed, is_approved, is_editable, managers, minters, created_at, updated_at, reviewed_at, mirrored_at) \
             SELECT 'matic', lower(contract_address), 137, name, eth_address, eth_address, 'urn:listed', true, false, true, \
               '{}', '{}', 1, 1, 1, timezone('UTC', now()) FROM builder.collections WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn listed(node: &Node, query: &str) -> Vec<String> {
    let mut request = Request::builder().uri(format!("/v1/collections?{query}"));
    for (name, value) in signed_map(CURATOR_KEY, "get", "/v1/collections", "{}", false).await {
        request = request.header(name, value);
    }
    let (status, body) = send(&node.app, request.body(Body::empty()).unwrap()).await;
    assert_eq!(status, 200, "{query}: {body}");
    let answer = json_of(&body);
    let rows = answer["data"].get("results").unwrap_or(&answer["data"]);
    let mut names: Vec<String> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn the_committee_lists_the_collections_in_an_automated_review_apart_while_the_flow_is_on() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (running, _) = published(&node, "Running", 1).await;
    let (waiting, _) = published(&node, "Waiting", 1).await;
    let (assigned, _) = published(&node, "Assigned", 1).await;
    for id in [waiting, assigned] {
        sqlx::query(
            "UPDATE builder.validation_requests SET settled_at = timezone('UTC', now()), outcome = 'passed' WHERE collection_id = $1",
        )
        .bind(id)
        .execute(&node.pool)
        .await
        .unwrap();
    }
    on_chain_unapproved(&node.pool, &[running, waiting, assigned]).await;
    for id in [running, waiting, assigned] {
        collection_curation(&node.pool, id, "pending").await;
    }
    let curator = address(CURATOR_KEY);
    sqlx::query("UPDATE builder.collection_curations SET assignee = $2 WHERE collection_id = $1")
        .bind(assigned)
        .bind(&curator)
        .execute(&node.pool)
        .await
        .unwrap();

    assert_eq!(listed(&node, "status=auto_reviewing").await, ["Running"]);
    assert_eq!(listed(&node, "status=to_review").await, ["Waiting"]);
    assert_eq!(listed(&node, "status=under_review").await, ["Assigned"]);
    assert_eq!(
        listed(&node, "status=pending").await,
        ["Assigned", "Waiting"]
    );
    assert_eq!(
        listed(&node, &format!("assignee={curator}")).await,
        ["Assigned"]
    );
    assert_eq!(
        listed(&node, "assignee=unassigned").await,
        ["Running", "Waiting"]
    );
    assert_eq!(
        listed(&node, "status=to_review&status=auto_reviewing").await,
        ["Running", "Waiting"]
    );

    sqlx::query(
        "INSERT INTO builder.collection_events (id, collection_id, type, actor, actor_address, payload, created_at) \
         VALUES (gen_random_uuid(), $1, 'review.rejected', 'curator', $2, '{}', timezone('UTC', now()) + interval '1 second')",
    )
    .bind(running)
    .bind(&curator)
    .execute(&node.pool)
    .await
    .unwrap();
    assert_eq!(
        listed(&node, "status=auto_reviewing").await,
        Vec::<String>::new(),
        "a curator's decision ends the automated review"
    );
    assert_eq!(
        listed(&node, "status=to_review").await,
        ["Running", "Waiting"]
    );

    set_flags(&node.pool, json!(false)).await;
    sqlx::query("DELETE FROM builder.collection_events WHERE collection_id = $1 AND type = 'review.rejected'")
        .bind(running)
        .execute(&node.pool)
        .await
        .unwrap();
    assert_eq!(
        listed(&node, "status=auto_reviewing").await,
        Vec::<String>::new()
    );
    assert_eq!(
        listed(&node, "status=to_review").await,
        ["Running", "Waiting"]
    );
}

#[tokio::test]
async fn the_committee_filters_by_each_collections_newest_curation() {
    let Some(node) = flow_node().await else {
        return;
    };
    set_flags(&node.pool, json!(false)).await;
    let (approved, _) = published(&node, "Approved", 1).await;
    let (rejected, _) = published(&node, "Rejected", 1).await;
    let (fresh, _) = published(&node, "Fresh", 1).await;
    collection_curation(&node.pool, approved, "pending").await;
    collection_curation(&node.pool, approved, "approved").await;
    collection_curation(&node.pool, rejected, "approved").await;
    collection_curation(&node.pool, rejected, "rejected").await;
    on_chain_unapproved(&node.pool, &[approved, rejected, fresh]).await;
    assert_eq!(listed(&node, "status=rejected").await, ["Rejected"]);
    assert_eq!(listed(&node, "status=to_review").await, ["Fresh"]);
    assert_eq!(listed(&node, "status=pending").await, ["Fresh"]);
    assert_eq!(listed(&node, "status=approved").await, Vec::<String>::new());
}

/// A standard collection of the owner's named `name`, created at `created`
/// (`YYYY-MM-DD`); on chain when `chain` is `Some((approved, reviewed))`,
/// `reviewed` meaning the chain touched it after its creation.
async fn listed_collection(
    pool: &PgPool,
    name: &str,
    created: &str,
    chain: Option<(bool, bool)>,
) -> Uuid {
    let id = Uuid::new_v4();
    let contract = format!("0x{}", Uuid::new_v4().simple().to_string()[..20].repeat(2));
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, contract_address, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5::date, $5::date)",
    )
    .bind(id)
    .bind(name)
    .bind(address(OWNER_KEY))
    .bind(&contract)
    .bind(created)
    .execute(pool)
    .await
    .unwrap();
    if let Some((approved, reviewed)) = chain {
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
               is_completed, is_approved, is_editable, managers, minters, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, 137, $2, $3, $3, 'urn:listed', true, $4, true, '{}', '{}', 1, 1, $5, timezone('UTC', now()))",
        )
        .bind(&contract)
        .bind(name)
        .bind(address(OWNER_KEY))
        .bind(approved)
        .bind(if reviewed { 2_i64 } else { 1 })
        .execute(pool)
        .await
        .unwrap();
    }
    id
}

/// A curation of `collection` with `status` created at `at`, assigned to
/// `assignee`.
async fn curation_at(
    pool: &PgPool,
    collection: Uuid,
    status: &str,
    at: &str,
    assignee: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO builder.collection_curations (id, collection_id, status, assignee, created_at, updated_at) \
         VALUES (gen_random_uuid(), $1, $2, $3, $4::timestamp, $4::timestamp)",
    )
    .bind(collection)
    .bind(status)
    .bind(assignee)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// An item of `collection` with a curation of `status` created at `at`.
async fn item_curation_at(pool: &PgPool, collection: Uuid, status: &str, at: &str) {
    let item = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type) VALUES ($1, $2, $3, 'Hat', 'wearable')",
    )
    .bind(item)
    .bind(collection)
    .bind(address(OWNER_KEY))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO builder.item_curations (id, item_id, status, content_hash, created_at, updated_at) \
         VALUES (gen_random_uuid(), $1, $2, repeat('ab', 32), $3::timestamp, $3::timestamp)",
    )
    .bind(item)
    .bind(status)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// The names `GET {path}?{query}` answers `key`, in the order answered, and
/// the body.
async fn ordered(node: &Node, key: &str, path: &str, query: &str) -> (Vec<String>, Value) {
    let mut request = Request::builder().uri(format!("{path}?{query}"));
    for (name, value) in signed_map(key, "get", path, "{}", false).await {
        request = request.header(name, value);
    }
    let (status, body) = send(&node.app, request.body(Body::empty()).unwrap()).await;
    assert_eq!(status, 200, "{query}: {body}");
    let answer = json_of(&body);
    let rows = answer["data"].get("results").unwrap_or(&answer["data"]);
    let names = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect();
    (names, answer)
}

/// An owner's listing names each standard collection's status as
/// builder-server does, from its newest curation and the chain: a rejection
/// wins, an approved collection with changes under review is under review,
/// an approval the chain doesn't show is disabled; the counts agree.
#[tokio::test]
async fn an_address_lists_and_counts_each_collection_by_its_newest_curation_and_the_chain() {
    let Some(node) = flow_node().await else {
        return;
    };
    let pool = &node.pool;
    let rejected = listed_collection(pool, "Rejected", "2025-01-01", Some((false, true))).await;
    curation_at(pool, rejected, "rejected", "2025-02-01", None).await;
    let changes = listed_collection(pool, "Changes", "2025-01-02", Some((true, true))).await;
    curation_at(pool, changes, "approved", "2025-02-01", None).await;
    curation_at(pool, changes, "pending", "2025-03-01", None).await;
    let live = listed_collection(pool, "Live", "2025-01-03", Some((true, true))).await;
    curation_at(pool, live, "approved", "2025-02-01", None).await;
    let unshown = listed_collection(pool, "Unshown", "2025-01-04", Some((false, false))).await;
    curation_at(pool, unshown, "approved", "2025-02-01", None).await;
    listed_collection(pool, "Disabled", "2025-01-05", Some((false, true))).await;
    listed_collection(pool, "Waiting", "2025-01-06", Some((false, false))).await;
    let first = listed_collection(pool, "First", "2025-01-07", Some((false, false))).await;
    curation_at(pool, first, "pending", "2025-02-01", None).await;
    listed_collection(pool, "Draft", "2025-01-08", None).await;
    let owner = address(OWNER_KEY);
    let path = format!("/v1/{owner}/collections");
    for (status, names) in [
        ("rejected", vec!["Rejected"]),
        ("under_review", vec!["First", "Waiting", "Changes"]),
        ("published", vec!["Live"]),
        ("disabled", vec!["Disabled", "Unshown"]),
        ("draft", vec!["Draft"]),
    ] {
        let (got, answer) = ordered(
            &node,
            OWNER_KEY,
            &path,
            &format!("status={status}&page=1&limit=20"),
        )
        .await;
        assert_eq!(got, names, "{status}");
        assert_eq!(
            answer["data"]["counts"],
            json!({ "draft": 1, "under_review": 3, "published": 1, "rejected": 1, "disabled": 2 }),
            "{status}"
        );
    }
}

/// The committee's REQUESTED_AT sort orders by when a review was last asked
/// for: an item's pending request, else any item request, else the
/// collection's request, else its creation.
#[tokio::test]
async fn the_committee_sorts_collections_by_when_their_review_was_requested() {
    let Some(node) = flow_node().await else {
        return;
    };
    let pool = &node.pool;
    let items = listed_collection(pool, "Item asked", "2025-01-01", Some((false, false))).await;
    item_curation_at(pool, items, "approved", "2025-04-15").await;
    item_curation_at(pool, items, "pending", "2025-06-01").await;
    let asked =
        listed_collection(pool, "Collection asked", "2025-02-01", Some((false, false))).await;
    curation_at(pool, asked, "pending", "2025-05-01", None).await;
    let decided = listed_collection(pool, "Item decided", "2025-01-15", Some((false, false))).await;
    item_curation_at(pool, decided, "approved", "2025-04-01").await;
    curation_at(pool, decided, "pending", "2025-07-01", None).await;
    listed_collection(pool, "Never asked", "2025-03-01", Some((false, false))).await;
    let desc = vec![
        "Item asked",
        "Collection asked",
        "Item decided",
        "Never asked",
    ];
    let (got, _) = ordered(
        &node,
        CURATOR_KEY,
        "/v1/collections",
        "sort=REQUESTED_AT_DESC",
    )
    .await;
    assert_eq!(got, desc);
    let (got, _) = ordered(
        &node,
        CURATOR_KEY,
        "/v1/collections",
        "sort=REQUESTED_AT_ASC",
    )
    .await;
    assert_eq!(got, desc.into_iter().rev().collect::<Vec<_>>());
}

/// The committee's MOST_RELEVANT sort lists what is to review first, then
/// what is under review, approved, rejected and the rest, newest first
/// within each.
#[tokio::test]
async fn the_committee_sorts_the_most_relevant_collections_first() {
    let Some(node) = flow_node().await else {
        return;
    };
    set_flags(&node.pool, json!(false)).await;
    let pool = &node.pool;
    let curator = address(CURATOR_KEY);
    listed_collection(pool, "To review", "2025-01-01", Some((false, false))).await;
    let assigned =
        listed_collection(pool, "Under review", "2025-01-02", Some((false, false))).await;
    curation_at(pool, assigned, "pending", "2025-02-01", Some(&curator)).await;
    let approved = listed_collection(pool, "Approved", "2025-01-03", Some((true, true))).await;
    curation_at(pool, approved, "approved", "2025-02-01", None).await;
    let rejected = listed_collection(pool, "Rejected", "2025-01-04", Some((false, true))).await;
    curation_at(pool, rejected, "rejected", "2025-02-01", None).await;
    listed_collection(pool, "Draft", "2025-01-05", None).await;
    listed_collection(pool, "Also to review", "2024-12-01", Some((false, false))).await;
    let (got, _) = ordered(&node, CURATOR_KEY, "/v1/collections", "sort=MOST_RELEVANT").await;
    assert_eq!(
        got,
        [
            "To review",
            "Also to review",
            "Under review",
            "Approved",
            "Rejected",
            "Draft"
        ]
    );
}

/// A review a curator rejected with reasons and put back to pending before
/// the automated review started keeps those reasons on its row; the
/// validator's rejection of it reads as the validator's own, with none of
/// the curator's reasons.
#[tokio::test]
async fn the_validators_rejection_carries_no_reasons_a_curator_gave_before() {
    let Some(node) = flow_node().await else {
        return;
    };
    let (id, _) = published(&node, "Twice judged", 1).await;
    collection_curation(&node.pool, id, "pending").await;
    sqlx::query(
        "UPDATE builder.collection_curations SET reviewed_by = $2, rejection_reasons = ARRAY['clipping'], \
           rejection_message = 'Fix the hat' WHERE collection_id = $1",
    )
    .bind(id)
    .bind(address(CURATOR_KEY))
    .execute(&node.pool)
    .await
    .unwrap();
    let (_, claimed) = claim(&node).await;
    assert_eq!(
        verdict(&node, id, &result_of(&claimed, "rejected")).await,
        (204, Value::Null)
    );
    let row: (String, Option<String>, Option<Vec<String>>, Option<String>) = sqlx::query_as(
        "SELECT status, reviewed_by, rejection_reasons, rejection_message FROM builder.collection_curations \
         WHERE collection_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(id)
    .fetch_one(&node.pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        ("rejected".into(), Some("validator".into()), None, None)
    );
}

/// While the review flow is on, MOST_RELEVANT ranks a collection in an
/// automated review neither to review nor under review, as the committee's
/// status filters don't.
#[tokio::test]
async fn the_most_relevant_sort_leaves_an_automated_review_out_of_the_queue_while_the_flow_is_on() {
    let Some(node) = flow_node().await else {
        return;
    };
    let pool = &node.pool;
    let (running, _) = published(&node, "Running", 1).await;
    collection_curation(pool, running, "pending").await;
    on_chain_unapproved(pool, &[running]).await;
    listed_collection(pool, "To review", "2025-01-01", Some((false, false))).await;
    let approved = listed_collection(pool, "Approved", "2025-01-02", Some((true, true))).await;
    curation_at(pool, approved, "approved", "2025-02-01", None).await;
    let (got, _) = ordered(&node, CURATOR_KEY, "/v1/collections", "sort=MOST_RELEVANT").await;
    assert_eq!(got, ["To review", "Approved", "Running"]);
}
