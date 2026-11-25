//! The curation routes beyond the authorization matrix: every cell of the
//! transition table over HTTP, what a write leaves (its row, its timeline
//! event, its `ops.admin_audit` row, the videos an approval pins), the Slack
//! notices and when they are sent, the callers the committee no longer or
//! never admits, the registry outage, concurrent curators, hostile input and
//! the item listing's curation filters.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::response::IntoResponse;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::support::{
    FakeCommittee, FakeRegistry, OWNER_KEY, address, call, on_chain, own_pool, registry_index,
};
use crate::curation::transitions::{
    self, Actor, Change, Curation, Refusal, Request as Asked, Status,
};
use crate::shared::signed_fetch::test_support::signer;

const STRANGER_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
const COMMITTEE_KEY: &str = "0x3333333333333333333333333333333333333333333333333333333333333333";
const FORMER_KEY: &str = "0x7777777777777777777777777777777777777777777777777777777777777777";
const ADMIN_KEY: &str = "0x8888888888888888888888888888888888888888888888888888888888888888";
const NEW_CREATOR_KEY: &str = "0x9999999999999999999999999999999999999999999999999999999999999999";
const PROVIDER: &str = "urn:decentraland:matic:collections-thirdparty:curation";
const TOKEN: &str = "xoxb-example123";

fn hash(n: u8) -> String {
    format!("{n:02x}").repeat(32)
}

/// A loopback Slack that records each message with the newest curation
/// status the database shows the moment it arrives, answering 500 while
/// `down`.
/// A message Slack received, with the newest curation status at that moment.
type Seen = (Value, Option<String>);

/// A collection curation's status, assignee, reviewer, reasons and message.
type Reviewed = (
    String,
    Option<String>,
    Option<String>,
    Option<Vec<String>>,
    Option<String>,
);

#[derive(Clone, Default)]
struct FakeSlack {
    seen: Arc<Mutex<Vec<Seen>>>,
    down: Arc<std::sync::atomic::AtomicBool>,
}

impl FakeSlack {
    async fn start(&self, pool: PgPool) -> String {
        let fake = self.clone();
        let app = Router::new().route(
            "/api/chat.postMessage",
            axum::routing::post(
                move |headers: axum::http::HeaderMap, axum::Json(body): axum::Json<Value>| {
                    let (fake, pool) = (fake.clone(), pool.clone());
                    async move {
                        if fake.down.load(std::sync::atomic::Ordering::SeqCst) {
                            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "down")
                                .into_response();
                        }
                        let ok = headers.get("authorization").and_then(|v| v.to_str().ok())
                            == Some(&format!("Bearer {TOKEN}"));
                        let status: Option<String> = sqlx::query_scalar(
                            "SELECT status FROM builder.collection_curations ORDER BY updated_at DESC LIMIT 1",
                        )
                        .fetch_optional(&pool)
                        .await
                        .unwrap();
                        if ok {
                            fake.seen.lock().unwrap().push((body, status));
                        }
                        axum::Json(json!({ "ok": ok })).into_response()
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/api")
    }

    fn texts(&self) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .map(|(body, _)| body["text"].as_str().unwrap_or_default().to_string())
            .collect()
    }
}

struct Node {
    pool: PgPool,
    app: Router,
    state: crate::SharedState,
}

struct Options {
    committee: FakeCommittee,
    registry: FakeRegistry,
    slack: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            committee: FakeCommittee::with(&[&address(COMMITTEE_KEY)]),
            registry: FakeRegistry::up(),
            slack: None,
        }
    }
}

async fn node_over(pool: PgPool, options: Options) -> Node {
    sqlx::query(
        "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, 'tp:1:Curation:') \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(PROVIDER)
    .bind(vec![address(OWNER_KEY)])
    .execute(&pool)
    .await
    .unwrap();
    let mut state = super::support::state(
        pool.clone(),
        Some(registry_index(&options.committee, &options.registry).await),
        crate::Settings {
            writes_enabled: true,
            admin_addresses: vec![address(ADMIN_KEY)],
            slack: options.slack.map(|api| crate::curation::slack::Slack {
                token: TOKEN.into(),
                channel: "curation-example".into(),
                api,
            }),
            links: crate::curation::slack::Links {
                creators_url: Some("https://creators.example.org".into()),
                builder_url: None,
            },
            ..crate::Settings::default()
        },
    )
    .await;
    state.guarded = dcl_http::Outbound::for_tests();
    let state = Arc::new(state);
    let app = options
        .registry
        .published(&pool, super::support::mounted(&state));
    Node { pool, app, state }
}

async fn node(options: Options) -> Option<Node> {
    Some(node_over(super::support::scratch_pool(8).await?, options).await)
}

/// A standard collection of the owner's on chain, unapproved, named `name`.
async fn standard(pool: &PgPool, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO builder.collections (id, name, eth_address) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(format!("{name} {}", &id.simple().to_string()[..8]))
        .bind(address(OWNER_KEY))
        .execute(pool)
        .await
        .unwrap();
    on_chain(pool, id, false).await;
    id
}

/// A third-party collection of the provider the owner manages, with one item
/// of the owner's whose content hash is `hash(1)`; `anchored` adds a second
/// item with an approved curation, so the collection is published whatever
/// the first one's curations.
async fn third_party(pool: &PgPool, anchored: bool) -> (Uuid, Uuid) {
    let id = Uuid::new_v4();
    let urn = format!("{PROVIDER}:{}", id.simple());
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(format!("Curated {}", &id.simple().to_string()[..8]))
    .bind(address(OWNER_KEY))
    .bind(PROVIDER)
    .bind(&urn)
    .execute(pool)
    .await
    .unwrap();
    let item = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix, local_content_hash) \
         VALUES ($1, $2, $3, 'Curated item', 'wearable', $4, $5)",
    )
    .bind(item)
    .bind(id)
    .bind(address(OWNER_KEY))
    .bind(format!("{urn}:item"))
    .bind(hash(1))
    .execute(pool)
    .await
    .unwrap();
    if anchored {
        let anchor = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix, local_content_hash) \
             VALUES ($1, $2, $3, 'Anchor', 'wearable', $4, $5)",
        )
        .bind(anchor)
        .bind(id)
        .bind(address(OWNER_KEY))
        .bind(format!("{urn}:anchor"))
        .bind(hash(8))
        .execute(pool)
        .await
        .unwrap();
        item_curation(pool, anchor, "approved").await;
    }
    (id, item)
}

async fn collection_curation(pool: &PgPool, collection: Uuid, status: &str) {
    sqlx::query(
        "INSERT INTO builder.collection_curations (id, collection_id, status, created_at, updated_at) \
         VALUES ($1, $2, $3, timezone('UTC', now()) - interval '1 hour', timezone('UTC', now()) - interval '1 hour')",
    )
    .bind(Uuid::new_v4())
    .bind(collection)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

async fn item_curation(pool: &PgPool, item: Uuid, status: &str) {
    sqlx::query(
        "INSERT INTO builder.item_curations (id, item_id, status, content_hash, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, timezone('UTC', now()) - interval '1 hour', timezone('UTC', now()) - interval '1 hour')",
    )
    .bind(Uuid::new_v4())
    .bind(item)
    .bind(status)
    .bind(hash(9))
    .execute(pool)
    .await
    .unwrap();
}

/// The statuses of a collection's or an item's curations, oldest first.
async fn statuses(pool: &PgPool, curation: Curation, id: Uuid) -> Vec<String> {
    let sql = match curation {
        Curation::Collection => {
            "SELECT status FROM builder.collection_curations WHERE collection_id = $1 ORDER BY created_at"
        }
        Curation::Item => {
            "SELECT status FROM builder.item_curations WHERE item_id = $1 ORDER BY created_at"
        }
    };
    sqlx::query_scalar(sql)
        .bind(id)
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn audits(pool: &PgPool, target: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM ops.admin_audit WHERE domain = 'create' AND target = $1",
    )
    .bind(target.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

fn refusal_status(refusal: Refusal) -> u16 {
    match refusal {
        Refusal::StatusIsCommittees | Refusal::AssigneeIsCommittees => 401,
        Refusal::NoCuration => 404,
        Refusal::Ongoing | Refusal::NeverCurated | Refusal::ItemAssignee => 400,
        Refusal::VerdictOnly | Refusal::Decided | Refusal::ItemStatusOnly => {
            unreachable!("no curation route sends a verdict or an operator's request")
        }
    }
}

/// Every cell of [`transitions`]' table, sent over HTTP by a creator (the
/// owner, who holds the role) and by the committee: the status answered, the
/// rows written (or none on a refusal) and an audit row for each committee
/// write and none for a creator's.
#[tokio::test]
async fn every_cell_of_the_transition_table_answers_and_writes_as_the_table_says() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let committee = address(COMMITTEE_KEY);
    let mut requests: Vec<Asked> = vec![
        Asked::Open { assignee: false },
        Asked::Open { assignee: true },
    ];
    for status in [
        None,
        Some(Status::Pending),
        Some(Status::Approved),
        Some(Status::Rejected),
    ] {
        for assignee in [false, true] {
            if status.is_some() || assignee {
                requests.push(Asked::Update { status, assignee });
            }
        }
    }
    let mut failures = Vec::new();
    let mut cells = 0;
    for curation in [Curation::Collection, Curation::Item] {
        for from in [
            None,
            Some(Status::Pending),
            Some(Status::Approved),
            Some(Status::Rejected),
        ] {
            for &request in &requests {
                if curation == Curation::Item && request == (Asked::Open { assignee: true }) {
                    continue;
                }
                for actor in [Actor::Committee, Actor::Creator] {
                    cells += 1;
                    let (subject, path) = match curation {
                        Curation::Collection => {
                            let id = standard(&n.pool, "Table").await;
                            if let Some(from) = from {
                                collection_curation(&n.pool, id, from.as_str()).await;
                            }
                            (id, format!("/v1/collections/{id}/curation"))
                        }
                        Curation::Item => {
                            let (_, item) = third_party(&n.pool, true).await;
                            if let Some(from) = from {
                                item_curation(&n.pool, item, from.as_str()).await;
                            }
                            (item, format!("/v1/items/{item}/curation"))
                        }
                    };
                    let (method, body) = match request {
                        Asked::Open { assignee: true } => (
                            "POST",
                            Some(json!({ "curation": { "assignee": committee } })),
                        ),
                        Asked::Open { assignee: false } => ("POST", None),
                        Asked::Update { status, assignee } => {
                            let mut c = serde_json::Map::new();
                            if let Some(status) = status {
                                c.insert("status".into(), status.as_str().into());
                            }
                            if assignee {
                                c.insert("assignee".into(), committee.clone().into());
                            }
                            if curation == Curation::Item && status == Some(Status::Approved) {
                                c.insert("contentHash".into(), hash(1).into());
                            }
                            ("PATCH", Some(json!({ "curation": c })))
                        }
                        Asked::Reject | Asked::Reopen => {
                            unreachable!("only a verdict rejects or reopens")
                        }
                    };
                    let key = match actor {
                        Actor::Committee => COMMITTEE_KEY,
                        Actor::Creator => OWNER_KEY,
                        Actor::Validator | Actor::Operator => {
                            unreachable!("the validator and the operator use other routes")
                        }
                    };
                    let before = statuses(&n.pool, curation, subject).await;
                    let audited = audits(&n.pool, subject).await;
                    let (status, answer) = call(&n.app, Some(key), method, &path, body).await;
                    let after = statuses(&n.pool, curation, subject).await;
                    let label = format!("{curation:?} from {from:?} {request:?} by {actor:?}");
                    let expected_rows =
                        match transitions::tests::expected(curation, from, request, actor) {
                            Err(refusal) => {
                                if status != refusal_status(refusal) {
                                    failures.push(format!("{label}: answered {status} {answer}"));
                                }
                                before.clone()
                            }
                            Ok(Change::Open { .. }) => {
                                if status != 200 || answer["data"]["status"] != "pending" {
                                    failures.push(format!("{label}: answered {status} {answer}"));
                                }
                                let mut rows = before.clone();
                                rows.push("pending".into());
                                rows
                            }
                            Ok(Change::OpenRejected) => {
                                unreachable!("only the validator opens a rejected curation")
                            }
                            Ok(Change::Update { status: to, .. }) => {
                                if status != 200 {
                                    failures.push(format!("{label}: answered {status} {answer}"));
                                }
                                let mut rows = before.clone();
                                if let (Some(to), Some(last)) = (to, rows.last_mut()) {
                                    *last = to.as_str().to_string();
                                }
                                rows
                            }
                        };
                    if after != expected_rows {
                        failures.push(format!(
                            "{label}: rows {after:?}, expected {expected_rows:?}"
                        ));
                    }
                    let wrote = status == 200;
                    let expected_audits = audited + i64::from(wrote && actor == Actor::Committee);
                    if audits(&n.pool, subject).await != expected_audits {
                        failures.push(format!("{label}: audit rows differ (wrote {wrote})"));
                    }
                }
            }
        }
    }
    assert_eq!(cells, 2 * 4 * 9 * 2 - 4 * 2);
    assert!(
        failures.is_empty(),
        "{} failing cells:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The operator's item status route, signed by an admin address, for every
/// status from every newest curation: the answer, the rows and the audit row
/// the table gives the operator, the content hash taken again on a write.
#[tokio::test]
async fn the_operator_sets_an_item_status_only_as_the_table_gives_it_and_every_write_is_audited() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let admin = address(ADMIN_KEY);
    let mut failures = Vec::new();
    for from in [
        None,
        Some(Status::Pending),
        Some(Status::Approved),
        Some(Status::Rejected),
    ] {
        for to in Status::ALL {
            let (collection, item) = third_party(&n.pool, true).await;
            if let Some(from) = from {
                item_curation(&n.pool, item, from.as_str()).await;
            }
            let label = format!("from {from:?} to {to:?}");
            let request = Asked::Update {
                status: Some(to),
                assignee: false,
            };
            let before = statuses(&n.pool, Curation::Item, item).await;
            let (status, answer) = call(
                &n.app,
                Some(ADMIN_KEY),
                "PATCH",
                &format!("/v1/collections/{collection}/items/{item}/status"),
                Some(json!({ "status": to.as_str() })),
            )
            .await;
            let after = statuses(&n.pool, Curation::Item, item).await;
            let audited: Vec<(String, String)> = sqlx::query_as(
                "SELECT actor, action FROM ops.admin_audit WHERE domain = 'create' AND target = $1",
            )
            .bind(item.to_string())
            .fetch_all(&n.pool)
            .await
            .unwrap();
            let hashes: Vec<String> = sqlx::query_scalar(
                "SELECT content_hash FROM builder.item_curations WHERE item_id = $1 ORDER BY created_at",
            )
            .bind(item)
            .fetch_all(&n.pool)
            .await
            .unwrap();
            match transitions::tests::expected(Curation::Item, from, request, Actor::Operator) {
                Err(Refusal::NoCuration) => {
                    if status != 404 || after != before || !audited.is_empty() {
                        failures.push(format!("{label}: {status} {answer} {after:?} {audited:?}"));
                    }
                }
                Ok(Change::Update {
                    status: Some(s), ..
                }) => {
                    let mut rows = before.clone();
                    *rows.last_mut().unwrap() = s.as_str().to_string();
                    if status != 200
                        || after != rows
                        || hashes.last() != Some(&hash(1))
                        || audited != [(admin.clone(), "builder.item_curation.update".to_string())]
                    {
                        failures.push(format!(
                            "{label}: {status} {answer} {after:?} {hashes:?} {audited:?}"
                        ));
                    }
                }
                other => failures.push(format!("{label}: the table answered {other:?}")),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// An operator's write waits for a curator holding the collection and then
/// decides the newest curation that curator left, never the one it replaced.
#[tokio::test]
async fn an_operator_write_waits_for_the_collection_lock_and_decides_the_newest_curation() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let (collection, item) = third_party(&n.pool, true).await;
    item_curation(&n.pool, item, "rejected").await;
    let mut holder = n.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(collection)
        .execute(&mut *holder)
        .await
        .unwrap();
    let app = n.app.clone();
    let path = format!("/v1/collections/{collection}/items/{item}/status");
    let operator = tokio::spawn(async move {
        call(
            &app,
            Some(ADMIN_KEY),
            "PATCH",
            &path,
            Some(json!({ "status": "approved" })),
        )
        .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(!operator.is_finished(), "the operator waits for the lock");
    crate::curation::record_item_curation(&mut holder, item, "pending", &hash(1), false)
        .await
        .unwrap();
    holder.commit().await.unwrap();
    let (status, answer) = operator.await.unwrap();
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        statuses(&n.pool, Curation::Item, item).await,
        ["rejected", "approved"]
    );
}

/// A collection deleted while an operator's bulk write waits for it is
/// decided again: the operator is told it is gone.
#[tokio::test]
async fn an_operator_bulk_write_on_a_collection_deleted_meanwhile_answers_not_found() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let (collection, item) = third_party(&n.pool, true).await;
    item_curation(&n.pool, item, "pending").await;
    let mut holder = n.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(collection)
        .execute(&mut *holder)
        .await
        .unwrap();
    let app = n.app.clone();
    let path = format!("/v1/collections/{collection}/items/status");
    let operator = tokio::spawn(async move {
        call(
            &app,
            Some(ADMIN_KEY),
            "PATCH",
            &path,
            Some(json!({ "status": "approved", "itemIds": [item] })),
        )
        .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    sqlx::query("DELETE FROM builder.collections WHERE id = $1")
        .bind(collection)
        .execute(&mut *holder)
        .await
        .unwrap();
    holder.commit().await.unwrap();
    let (status, answer) = operator.await.unwrap();
    assert_eq!(status, 404, "{answer}");
}

/// The operator decides items of a published collection only.
#[tokio::test]
async fn the_operator_cannot_set_the_status_of_an_item_of_an_unpublished_collection() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let (collection, item) = third_party(&n.pool, false).await;
    for (path, body) in [
        (
            format!("/v1/collections/{collection}/items/{item}/status"),
            json!({ "status": "approved" }),
        ),
        (
            format!("/v1/collections/{collection}/items/status"),
            json!({ "status": "approved", "itemIds": [item] }),
        ),
    ] {
        let (status, answer) = call(&n.app, Some(ADMIN_KEY), "PATCH", &path, Some(body)).await;
        assert_eq!(
            (status, answer["error"].as_str()),
            (409, Some("Unpublished collection")),
            "{path}: {answer}"
        );
    }
    assert_eq!(audits(&n.pool, item).await, 0);
}

/// A curator's decisions on the timeline: an assignee change, a new
/// approval, a repeated one (no event), a rejection and its edit, each with
/// the review fields builder-server sets.
#[tokio::test]
async fn decisions_write_the_review_fields_and_the_timeline_as_builder_server_does() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let id = standard(&n.pool, "Timeline").await;
    collection_curation(&n.pool, id, "pending").await;
    let path = format!("/v1/collections/{id}/curation");
    let committee = address(COMMITTEE_KEY);
    for body in [
        json!({ "assignee": committee }),
        json!({ "status": "approved", "note": " fine " }),
        json!({ "status": "approved" }),
        json!({ "status": "rejected", "rejectionReasons": ["clipping"], "rejectionMessage": "  Fix it  " }),
        json!({ "status": "rejected", "rejectionMessage": "Fix the hat" }),
        json!({ "status": "rejected", "rejectionMessage": "Fix the hat" }),
    ] {
        let (status, answer) = call(
            &n.app,
            Some(COMMITTEE_KEY),
            "PATCH",
            &path,
            Some(json!({ "curation": body })),
        )
        .await;
        assert_eq!(status, 200, "{answer}");
    }
    let row: Reviewed = sqlx::query_as(
        "SELECT status, assignee, reviewed_by, rejection_reasons, rejection_message \
             FROM builder.collection_curations WHERE collection_id = $1",
    )
    .bind(id)
    .fetch_one(&n.pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            "rejected".into(),
            Some(committee.clone()),
            Some(committee.clone()),
            Some(vec!["clipping".into()]),
            Some("Fix the hat".into())
        )
    );
    let events: Vec<(String, Value)> = sqlx::query_as(
        "SELECT type, payload FROM builder.collection_events WHERE collection_id = $1 ORDER BY created_at, id",
    )
    .bind(id)
    .fetch_all(&n.pool)
    .await
    .unwrap();
    assert_eq!(
        events,
        vec![
            ("review.assigned".into(), json!({ "assignee": committee })),
            ("review.approved".into(), json!({ "note": "fine" })),
            (
                "review.rejected".into(),
                json!({ "rejectionReasons": ["clipping"], "rejectionMessage": "Fix it" })
            ),
            (
                "review.rejected".into(),
                json!({ "rejectionReasons": ["clipping"], "rejectionMessage": "Fix the hat" })
            ),
        ]
    );
    assert_eq!(audits(&n.pool, id).await, 6);
}

#[tokio::test]
async fn an_empty_assignee_unassigns_the_review_as_builder_server_does() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let id = standard(&n.pool, "Unassigned").await;
    collection_curation(&n.pool, id, "pending").await;
    let path = format!("/v1/collections/{id}/curation");
    for assignee in [address(COMMITTEE_KEY), String::new()] {
        let (status, answer) = call(
            &n.app,
            Some(COMMITTEE_KEY),
            "PATCH",
            &path,
            Some(json!({ "curation": { "assignee": assignee } })),
        )
        .await;
        assert_eq!(status, 200, "{answer}");
    }
    let assignee: Option<String> = sqlx::query_scalar(
        "SELECT assignee FROM builder.collection_curations WHERE collection_id = $1",
    )
    .bind(id)
    .fetch_one(&n.pool)
    .await
    .unwrap();
    assert_eq!(assignee, None);
}

/// Who builder-server's curation routes never admit: a wallet the current
/// committee no longer lists, though it is the curation's assignee and
/// reviewer, and an `ADMIN_ADDRESSES` wallet, on a published collection and
/// on an item of one.
#[tokio::test]
async fn a_former_committee_member_and_an_admin_address_get_no_curation() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let id = standard(&n.pool, "Former").await;
    collection_curation(&n.pool, id, "pending").await;
    sqlx::query("UPDATE builder.collection_curations SET assignee = $2, reviewed_by = $2 WHERE collection_id = $1")
        .bind(id)
        .bind(address(FORMER_KEY))
        .execute(&n.pool)
        .await
        .unwrap();
    let (_, item) = third_party(&n.pool, true).await;
    item_curation(&n.pool, item, "pending").await;
    let path = format!("/v1/collections/{id}/curation");
    for key in [FORMER_KEY, ADMIN_KEY, STRANGER_KEY] {
        for (method, path, body) in [
            ("GET", path.clone(), None),
            (
                "PATCH",
                path.clone(),
                Some(json!({ "curation": { "status": "approved" } })),
            ),
            ("POST", path.clone(), None),
            ("GET", format!("/v1/collections/{id}/itemCurations"), None),
            ("GET", format!("/v1/items/{item}/curation"), None),
            (
                "PATCH",
                format!("/v1/items/{item}/curation"),
                Some(json!({ "curation": { "status": "approved" } })),
            ),
        ] {
            let (status, answer) = call(&n.app, Some(key), method, &path, body).await;
            assert_eq!(
                (status, answer["error"].as_str()),
                (401, Some("Unauthorized")),
                "{method} {path}"
            );
        }
        let (status, answer) = call(&n.app, Some(key), "GET", "/v1/curations", None).await;
        assert_eq!(
            (status, answer["data"].as_array().map(Vec::len)),
            (200, Some(0))
        );
    }
    assert_eq!(
        statuses(&n.pool, Curation::Collection, id).await,
        ["pending"]
    );
}

/// A third-party collection's curations wait on the registry: while it
/// can't be read, its manager gets 503 and nothing is written.
#[tokio::test]
async fn a_registry_that_cant_be_read_answers_503_to_third_party_curations() {
    let registry = FakeRegistry::up();
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let n = node_over(
        pool,
        Options {
            registry: registry.clone(),
            ..Options::default()
        },
    )
    .await;
    let (collection, item) = third_party(&n.pool, false).await;
    item_curation(&n.pool, item, "approved").await;
    registry.fail(true);
    for (method, path) in [
        ("GET", format!("/v1/collections/{collection}/curation")),
        ("POST", format!("/v1/collections/{collection}/curation")),
        ("GET", format!("/v1/items/{item}/curation")),
        ("POST", format!("/v1/items/{item}/curation")),
        ("GET", "/v1/curations".to_string()),
    ] {
        let (status, _) = call(&n.app, Some(OWNER_KEY), method, &path, None).await;
        assert_eq!(status, 503, "{method} {path}");
    }
    assert_eq!(statuses(&n.pool, Curation::Item, item).await, ["approved"]);
}

/// Two curators deciding at once, and two creators asking at once, end in
/// one consistent state: the row holds the last decision, the timeline both
/// in that order, the same approval twice is one decision, and only one new
/// request is opened.
#[tokio::test]
async fn concurrent_curators_end_in_one_consistent_state() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    for _ in 0..5 {
        let id = standard(&n.pool, "Race").await;
        collection_curation(&n.pool, id, "pending").await;
        let path = format!("/v1/collections/{id}/curation");
        let (a, b) = tokio::join!(
            call(
                &n.app,
                Some(COMMITTEE_KEY),
                "PATCH",
                &path,
                Some(json!({ "curation": { "status": "approved" } }))
            ),
            call(
                &n.app,
                Some(COMMITTEE_KEY),
                "PATCH",
                &path,
                Some(json!({ "curation": { "status": "rejected" } }))
            ),
        );
        assert_eq!((a.0, b.0), (200, 200));
        let status: String = sqlx::query_scalar(
            "SELECT status FROM builder.collection_curations WHERE collection_id = $1",
        )
        .bind(id)
        .fetch_one(&n.pool)
        .await
        .unwrap();
        let last: String = sqlx::query_scalar(
            "SELECT type FROM builder.collection_events WHERE collection_id = $1 ORDER BY created_at DESC, id DESC LIMIT 1",
        )
        .bind(id)
        .fetch_one(&n.pool)
        .await
        .unwrap();
        assert_eq!(last, format!("review.{status}"));
        let twice = standard(&n.pool, "Twice").await;
        collection_curation(&n.pool, twice, "pending").await;
        let path_twice = format!("/v1/collections/{twice}/curation");
        let approve = || Some(json!({ "curation": { "status": "approved" } }));
        let (a, b) = tokio::join!(
            call(&n.app, Some(COMMITTEE_KEY), "PATCH", &path_twice, approve()),
            call(&n.app, Some(COMMITTEE_KEY), "PATCH", &path_twice, approve()),
        );
        assert_eq!((a.0, b.0), (200, 200));
        let approvals: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM builder.collection_events WHERE collection_id = $1 AND type = 'review.approved'",
        )
        .bind(twice)
        .fetch_one(&n.pool)
        .await
        .unwrap();
        assert_eq!(
            approvals, 1,
            "the same approval twice at once is one decision"
        );
        let (x, y) = tokio::join!(
            call(&n.app, Some(OWNER_KEY), "POST", &path, None),
            call(&n.app, Some(OWNER_KEY), "POST", &path, None),
        );
        let mut answered = [x.0, y.0];
        answered.sort();
        assert_eq!(answered, [200, 400]);
        assert_eq!(statuses(&n.pool, Curation::Collection, id).await.len(), 2);
    }
}

/// Notices: sent after the commit (the row is already decided when Slack
/// hears of it), only for a real change, with the collection's name escaped;
/// a refused write sends none; Slack answering 500 fails no request.
#[tokio::test]
async fn slack_hears_of_a_decision_only_after_it_is_committed() {
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let slack = FakeSlack::default();
    let api = slack.start(pool.clone()).await;
    let n = node_over(
        pool,
        Options {
            slack: Some(api),
            ..Options::default()
        },
    )
    .await;
    let id = standard(&n.pool, "Hostile").await;
    sqlx::query("UPDATE builder.collections SET name = $2 WHERE id = $1")
        .bind(id)
        .bind("<!channel> *Hats*\n✅ <https://evil.example|forged>")
        .execute(&n.pool)
        .await
        .unwrap();
    collection_curation(&n.pool, id, "pending").await;
    let path = format!("/v1/collections/{id}/curation");
    let approve = Some(json!({ "curation": { "status": "approved" } }));
    assert_eq!(
        call(&n.app, Some(OWNER_KEY), "PATCH", &path, approve.clone())
            .await
            .0,
        401
    );
    assert_eq!(
        call(&n.app, Some(COMMITTEE_KEY), "PATCH", &path, approve.clone())
            .await
            .0,
        200
    );
    assert_eq!(
        call(&n.app, Some(COMMITTEE_KEY), "PATCH", &path, approve.clone())
            .await
            .0,
        200
    );
    n.state.notices.settled().await;
    let seen = slack.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(
        seen[0].1.as_deref(),
        Some("approved"),
        "the notice arrived before the commit"
    );
    let text = seen[0].0["text"].as_str().unwrap();
    assert!(
        text.starts_with("✅ <https://creators.example.org/collections/editor?collection="),
        "{text}"
    );
    assert!(
        text.contains(
            "|&lt;!channel&gt; \u{200b}*\u{200b}Hats\u{200b}*\u{200b} ✅ &lt;https:\u{200b}//evil.\u{200b}example|forged&gt;> was approved by"
        ),
        "{text}"
    );
    assert_eq!(seen[0].0["channel"], "curation-example");
    assert_eq!(seen[0].0["unfurl_links"], false);
    slack.down.store(true, std::sync::atomic::Ordering::SeqCst);
    let reject = Some(json!({ "curation": { "status": "rejected" } }));
    assert_eq!(
        call(&n.app, Some(COMMITTEE_KEY), "PATCH", &path, reject)
            .await
            .0,
        200
    );
    n.state.notices.settled().await;
    assert_eq!(
        statuses(&n.pool, Curation::Collection, id).await,
        ["rejected"]
    );
    assert_eq!(slack.texts().len(), 1);
}

/// Without a Slack channel the writes answer as before and nothing is sent.
#[tokio::test]
async fn without_slack_nothing_is_sent_and_nothing_fails() {
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let slack = FakeSlack::default();
    slack.start(pool.clone()).await;
    let n = node_over(pool, Options::default()).await;
    let id = standard(&n.pool, "Quiet").await;
    collection_curation(&n.pool, id, "pending").await;
    let path = format!("/v1/collections/{id}/curation");
    let (status, _) = call(
        &n.app,
        Some(COMMITTEE_KEY),
        "PATCH",
        &path,
        Some(json!({ "curation": { "status": "approved", "assignee": address(COMMITTEE_KEY) } })),
    )
    .await;
    assert_eq!(status, 200);
    n.state.notices.settled().await;
    assert!(slack.texts().is_empty());
}

#[tokio::test]
async fn an_approval_pins_each_smart_wearables_video() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let id = standard(&n.pool, "Smart").await;
    collection_curation(&n.pool, id, "pending").await;
    let owner = address(OWNER_KEY);
    let mut items = Vec::new();
    let mut olds = Vec::new();
    for (smart, new) in [
        (true, "bafkreinew"),
        (false, "bafkreinew"),
        (true, "bafkreiplain"),
    ] {
        let item = Uuid::new_v4();
        let old = format!("bafkreiold{}", item.simple());
        sqlx::query(
            "INSERT INTO builder.items (id, collection_id, eth_address, name, type, video, video_named_by) \
             VALUES ($1, $2, $3, 'Smart', 'wearable', $4, $3)",
        )
        .bind(item)
        .bind(id)
        .bind(&owner)
        .bind(&old)
        .execute(&n.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($1, 1, $2)",
        )
        .bind(&old)
        .bind(&owner)
        .execute(&n.pool)
        .await
        .unwrap();
        olds.push(old);
        let files: &[(&str, &str)] = if smart {
            &[("game.js", "bafkreiscript"), ("video.mp4", new)]
        } else {
            &[("video.mp4", new)]
        };
        for (position, (file, hash)) in files.iter().enumerate() {
            sqlx::query(
                "INSERT INTO builder.item_contents (item_id, file, hash, position, named_by) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(item)
            .bind(file)
            .bind(hash)
            .bind(position as i32)
            .bind(&owner)
            .execute(&n.pool)
            .await
            .unwrap();
        }
        items.push(item);
    }
    sqlx::query(
        "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by, video) \
         VALUES ('bafkreinew', 1, $1, true), ('bafkreiplain', 1, $1, false)",
    )
    .bind(&owner)
    .execute(&n.pool)
    .await
    .unwrap();
    let (status, _) = call(
        &n.app,
        Some(COMMITTEE_KEY),
        "PATCH",
        &format!("/v1/collections/{id}/curation"),
        Some(json!({ "curation": { "status": "approved" } })),
    )
    .await;
    assert_eq!(status, 200);
    let videos: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT video FROM builder.items WHERE id = ANY($1) ORDER BY array_position($1, id)",
    )
    .bind(&items)
    .fetch_all(&n.pool)
    .await
    .unwrap();
    assert_eq!(
        videos,
        [
            Some("bafkreinew".into()),
            Some(olds[1].clone()),
            Some(olds[2].clone())
        ]
    );
    let recorded: Vec<String> = sqlx::query_scalar(
        "SELECT hash FROM builder.uploaded_contents WHERE hash = ANY($1) ORDER BY hash",
    )
    .bind(&olds)
    .fetch_all(&n.pool)
    .await
    .unwrap();
    let mut kept = vec![olds[1].clone(), olds[2].clone()];
    kept.sort();
    assert_eq!(recorded, kept);
    sqlx::query("DELETE FROM builder.uploaded_contents WHERE hash = ANY($1)")
        .bind(&olds)
        .execute(&n.pool)
        .await
        .unwrap();
}

/// Input that is not upstream's is refused before anything is read or
/// written, and nothing in it reaches SQL as text.
#[tokio::test]
async fn hostile_curation_input_is_refused_or_matches_nothing() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let id = standard(&n.pool, "Hostile input").await;
    collection_curation(&n.pool, id, "pending").await;
    let path = format!("/v1/collections/{id}/curation");
    for body in [
        json!({ "curation": { "status": "approved'; DROP TABLE builder.items; --" } }),
        json!({ "curation": { "assignee": { "$ne": null } } }),
        json!({ "curation": { "status": "rejected", "rejectionReasons": ["clipping' OR '1'='1"] } }),
        json!({ "curation": { "status": "rejected", "rejectionMessage": "x".repeat(3001) } }),
        json!({ "curation": "approved" }),
        json!({ "curation": { "status": "approved", "reviewed_by": "validator" } }),
    ] {
        let (status, _) = call(
            &n.app,
            Some(COMMITTEE_KEY),
            "PATCH",
            &path,
            Some(body.clone()),
        )
        .await;
        assert_eq!(status, 400, "{body}");
    }
    let (status, answer) = call(
        &n.app,
        Some(COMMITTEE_KEY),
        "PATCH",
        &path,
        Some(json!({ "curation": { "assignee": "0x' OR 1=1 --" } })),
    )
    .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (401, Some("The assignee must be a committee member"))
    );
    let (status, answer) = call(
        &n.app,
        Some(COMMITTEE_KEY),
        "GET",
        &format!(
            "/v1/collections/{id}/itemCurations?itemIds[]=x'%20OR%20'1'='1&itemIds[]={}",
            Uuid::nil()
        ),
        None,
    )
    .await;
    assert_eq!(
        (status, answer["data"].as_array().map(Vec::len)),
        (200, Some(0))
    );
    assert_eq!(
        statuses(&n.pool, Curation::Collection, id).await,
        ["pending"]
    );
}

/// `GET /v1/collections/:id/items`' curation filters read each item's newest
/// item curation, as builder-server's do.
#[tokio::test]
async fn the_item_list_filters_by_each_items_newest_curation() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let (collection, synced) = third_party(&n.pool, false).await;
    let mut ids = vec![synced];
    for (label, hash_of_item) in [("changed", 2), ("pending", 3), ("mapped", 4), ("never", 5)] {
        let item = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix, local_content_hash, mappings) \
             SELECT $1, collection_id, eth_address, $2, type, urn_suffix || $2, $3, \
               CASE WHEN $2 IN ('mapped', 'never') THEN '{}'::jsonb END FROM builder.items WHERE id = $4",
        )
        .bind(item)
        .bind(label)
        .bind(hash(hash_of_item))
        .bind(synced)
        .execute(&n.pool)
        .await
        .unwrap();
        ids.push(item);
    }
    let curate = |item: Uuid, status: &'static str, content: String, complete: Option<bool>| {
        let pool = n.pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO builder.item_curations (id, item_id, status, content_hash, is_mapping_complete) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(Uuid::new_v4())
            .bind(item)
            .bind(status)
            .bind(content)
            .bind(complete)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    curate(ids[0], "approved", hash(1), None).await;
    curate(ids[1], "approved", hash(7), None).await;
    curate(ids[2], "pending", hash(3), None).await;
    curate(ids[3], "approved", hash(4), Some(true)).await;
    let listed = |query: &'static str| {
        let app = n.app.clone();
        async move {
            let (status, answer) = call(
                &app,
                Some(OWNER_KEY),
                "GET",
                &format!("/v1/collections/{collection}/items?{query}"),
                None,
            )
            .await;
            assert_eq!(status, 200, "{query}: {answer}");
            let mut names: Vec<String> = answer["data"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["name"].as_str().unwrap().to_string())
                .collect();
            names.sort();
            names
        }
    };
    assert_eq!(listed("synced=true").await, ["Curated item", "mapped"]);
    assert_eq!(listed("synced=false").await, ["changed"]);
    assert_eq!(listed("status=approved").await, ["pending"]);
    assert_eq!(listed("mappingStatus=unpublished_mapping").await, ["never"]);
    assert_eq!(listed("mappingStatus=missing_mapping").await.len(), 3);
}

/// A write decides again inside its transaction: a creator who lost the
/// collection between the decision and the write writes nothing.
#[tokio::test]
async fn a_role_lost_after_the_decision_writes_nothing() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let id = standard(&n.pool, "Lost").await;
    collection_curation(&n.pool, id, "approved").await;
    let owner = signer(&address(OWNER_KEY));
    let curator = crate::access::curate_collection(
        &n.state,
        &owner,
        &id.to_string(),
        crate::kept::Since::write(),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.collections SET creator = $2 WHERE contract_address = (SELECT contract_address FROM builder.collections WHERE id = $1)",
    )
    .bind(id)
    .bind(address(NEW_CREATOR_KEY))
    .execute(&n.pool)
    .await
    .unwrap();
    let refused = crate::curation::writes::open_collection(&n.state, curator, Default::default())
        .await
        .unwrap_err();
    assert_eq!(refused.status(), 401);
    assert_eq!(
        statuses(&n.pool, Curation::Collection, id).await,
        ["approved"]
    );
}

/// A collection the chain moved to a committee member: its new creator's
/// request for a review is a creator's request, which the curators hear
/// of, whoever the row first named.
#[tokio::test]
async fn the_chains_creator_on_the_committee_asks_for_a_review_as_its_creator() {
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let slack = FakeSlack::default();
    let api = slack.start(pool.clone()).await;
    let n = node_over(
        pool,
        Options {
            slack: Some(api),
            ..Options::default()
        },
    )
    .await;
    let id = standard(&n.pool, "Moved").await;
    collection_curation(&n.pool, id, "rejected").await;
    sqlx::query(
        "UPDATE shop.collections s SET creator = $2, owner = $2 FROM builder.collections b \
         WHERE b.id = $1 AND s.contract_address = b.contract_address",
    )
    .bind(id)
    .bind(address(COMMITTEE_KEY))
    .execute(&n.pool)
    .await
    .unwrap();
    let (status, body) = call(
        &n.app,
        Some(COMMITTEE_KEY),
        "POST",
        &format!("/v1/collections/{id}/curation"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    n.state.notices.settled().await;
    let texts = slack.texts();
    assert_eq!(texts.len(), 1, "{texts:?}");
}

/// A creator whose address the chain copy holds in another case still reads
/// its collection's curation.
#[tokio::test]
async fn a_creator_reads_its_curations_whatever_case_the_copy_holds_its_address_in() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let id = standard(&n.pool, "Cased").await;
    collection_curation(&n.pool, id, "pending").await;
    sqlx::query(
        "UPDATE shop.collections s SET creator = upper(s.creator) FROM builder.collections b \
         WHERE b.id = $1 AND s.contract_address = b.contract_address",
    )
    .bind(id)
    .execute(&n.pool)
    .await
    .unwrap();
    let (status, answer) = call(&n.app, Some(OWNER_KEY), "GET", "/v1/curations", None).await;
    let listed = answer["data"]
        .as_array()
        .is_some_and(|rows| rows.iter().any(|row| row["collection_id"] == json!(id)));
    assert_eq!((status, listed), (200, true), "{answer}");
}

/// A curator, or an assignee, the collections subgraph drops after the
/// decision writes nothing, even one that still holds a role on the
/// collection: the write reads the committee again inside its transaction.
#[tokio::test]
async fn a_committee_member_dropped_after_the_decision_writes_nothing() {
    let Some(pool) = own_pool(6).await else {
        return;
    };
    let committee = FakeCommittee::with(&[
        &address(COMMITTEE_KEY),
        &address(FORMER_KEY),
        &address(ADMIN_KEY),
        &address(OWNER_KEY),
    ]);
    let n = node_over(
        pool,
        Options {
            committee: committee.clone(),
            ..Options::default()
        },
    )
    .await;
    let drop_member = |key: &str| committee.remove(&address(key));
    let id = standard(&n.pool, "Dropped").await;
    collection_curation(&n.pool, id, "pending").await;
    let assigning = crate::access::curate_collection(
        &n.state,
        &signer(&address(COMMITTEE_KEY)),
        &id.to_string(),
        crate::kept::Since::write(),
    )
    .await
    .unwrap();
    let rejecting = crate::access::curate_collection(
        &n.state,
        &signer(&address(FORMER_KEY)),
        &id.to_string(),
        crate::kept::Since::write(),
    )
    .await
    .unwrap();
    let owner_rejecting = crate::access::as_committee(
        &n.state,
        crate::access::curate_collection(
            &n.state,
            &signer(&address(OWNER_KEY)),
            &id.to_string(),
            crate::kept::Since::write(),
        )
        .await
        .unwrap(),
    )
    .await
    .unwrap();
    let commenting =
        crate::access::review_member(&n.state, &signer(&address(FORMER_KEY)), &id.to_string())
            .await
            .unwrap();
    let owner_commenting =
        crate::access::review_member(&n.state, &signer(&address(OWNER_KEY)), &id.to_string())
            .await
            .unwrap();
    let assign = || crate::curation::writes::Update {
        assignee: Some(Some(address(ADMIN_KEY))),
        ..Default::default()
    };
    let reject = || crate::curation::writes::Update {
        status: Some(crate::curation::transitions::Status::Rejected),
        ..Default::default()
    };
    drop_member(OWNER_KEY);
    drop_member(FORMER_KEY);
    let rejected = crate::curation::writes::update_collection(&n.state, rejecting, reject()).await;
    let owner_rejected =
        crate::curation::writes::update_collection(&n.state, owner_rejecting, reject()).await;
    let commented =
        crate::review::comment(&n.state, commenting, "Too late".to_string(), None).await;
    let owner_commented = crate::review::comment(
        &n.state,
        owner_commenting,
        "Still the creator".to_string(),
        None,
    )
    .await;
    assert!(
        owner_commented.is_ok(),
        "the creator leaving the committee still comments: {:?}",
        owner_commented.err().map(|e| e.status())
    );
    let mut holder = n.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(id)
        .execute(&mut *holder)
        .await
        .unwrap();
    let assigning = tokio::spawn({
        let state = n.state.clone();
        async move { crate::curation::writes::update_collection(&state, assigning, assign()).await }
    });
    for _ in 0..500 {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_locks l JOIN pg_database d ON d.oid = l.database \
             WHERE NOT l.granted AND d.datname = current_database()",
        )
        .fetch_one(&n.pool)
        .await
        .unwrap();
        if waiting > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    drop_member(ADMIN_KEY);
    holder.commit().await.unwrap();
    let assigned = assigning.await.unwrap();
    let assignee: Option<String> = sqlx::query_scalar(
        "SELECT assignee FROM builder.collection_curations WHERE collection_id = $1",
    )
    .bind(id)
    .fetch_one(&n.pool)
    .await
    .unwrap();
    assert_eq!(
        (
            assigned.err().map(|e| e.status()),
            rejected.err().map(|e| e.status()),
            owner_rejected.err().map(|e| e.status()),
            commented.err().map(|e| e.status())
        ),
        (Some(401), Some(401), Some(401), Some(401))
    );
    assert_eq!(assignee, None);
    assert_eq!(
        statuses(&n.pool, Curation::Collection, id).await,
        ["pending"]
    );
}

/// A member the collections subgraph stops listing is refused its next
/// curation write at once, and its curation reads once the kept read is
/// older than the read window.
#[tokio::test]
async fn a_member_the_subgraph_stops_listing_is_refused_its_next_write_and_its_reads_past_the_window()
 {
    let committee = FakeCommittee::with(&[&address(COMMITTEE_KEY)]);
    let Some(n) = node(Options {
        committee: committee.clone(),
        ..Options::default()
    })
    .await
    else {
        return;
    };
    let id = standard(&n.pool, "Unlisted").await;
    collection_curation(&n.pool, id, "pending").await;
    let path = format!("/v1/collections/{id}/curation");
    let reply = format!("/v1/collections/{id}/curation/post");
    let reply_body = || Some(json!({ "forumPost": { "raw": "Looks good" } }));
    let (listed, body) = call(&n.app, Some(COMMITTEE_KEY), "GET", &path, None).await;
    assert_eq!(listed, 200, "{body}");
    let (replied, body) = call(&n.app, Some(COMMITTEE_KEY), "POST", &reply, reply_body()).await;
    assert_eq!(
        replied, 409,
        "a member gets past the decision to the topic check: {body}"
    );
    committee.set(Some(&[]));
    let (kept, _) = call(&n.app, Some(COMMITTEE_KEY), "GET", &path, None).await;
    let (late_reply, _) = call(&n.app, Some(COMMITTEE_KEY), "POST", &reply, reply_body()).await;
    let (write, body) = call(
        &n.app,
        Some(COMMITTEE_KEY),
        "PATCH",
        &path,
        Some(json!({ "curation": { "status": "approved" } })),
    )
    .await;
    assert_eq!(
        (kept, write, body["error"].as_str(), late_reply),
        (200, 401, Some("Unauthorized"), 401)
    );
    assert_eq!(
        statuses(&n.pool, Curation::Collection, id).await,
        ["pending"]
    );
    n.state.committee.outdate();
    let (late, _) = call(&n.app, Some(COMMITTEE_KEY), "GET", &path, None).await;
    assert_eq!(late, 401);
}

/// A committee member who also creates the collection on chain decides its
/// curation as the committee: its approval is taken and audited, where the
/// creator alone is refused.
#[tokio::test]
async fn a_creator_on_the_committee_approves_as_the_committee() {
    let Some(n) = node(Options::default()).await else {
        return;
    };
    let id = standard(&n.pool, "Both").await;
    collection_curation(&n.pool, id, "pending").await;
    let path = format!("/v1/collections/{id}/curation");
    let approve = || Some(json!({ "curation": { "status": "approved" } }));
    let (creator_alone, _) = call(&n.app, Some(OWNER_KEY), "PATCH", &path, approve()).await;
    sqlx::query(
        "UPDATE shop.collections s SET creator = $2, owner = $2 FROM builder.collections b \
         WHERE b.id = $1 AND s.contract_address = b.contract_address",
    )
    .bind(id)
    .bind(address(COMMITTEE_KEY))
    .execute(&n.pool)
    .await
    .unwrap();
    let (both, body) = call(&n.app, Some(COMMITTEE_KEY), "PATCH", &path, approve()).await;
    assert_eq!((creator_alone, both), (401, 200), "{body}");
    assert_eq!(
        statuses(&n.pool, Curation::Collection, id).await,
        ["approved"]
    );
    assert_eq!(audits(&n.pool, id).await, 1);
}
