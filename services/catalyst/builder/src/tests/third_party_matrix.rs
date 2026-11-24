//! The third-party rows of the authorization matrix: every operation on a
//! third-party collection, its items and its files, and the `/v1/thirdParties`
//! routes, called by a manager of the collection's provider, a manager of
//! another provider, the holder of a virtual third party, the wallet that
//! created the collection and last saved its item and uploaded its file but
//! manages its provider no longer, a stranger, a committee member and anonymously,
//! against each state of the provider in the registry and among the virtual
//! third parties. The registry-down column answers 503 and writes nothing; a
//! manager the registry stops listing is refused on the next request.
//!
//! The matrix tests run their cells on worlds ([`super::cells`]): each cell
//! gets a fresh fixture, and its world is reset after it.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use alloy::signers::SignerSync;

use super::support::{FakeCommittee, FakeRegistry, registry_index, temp_storage, unique, wallet};
use crate::shared::signed_fetch::test_support::signed_headers_by;
use crate::storage::Storage;

const MANAGER_KEY: &str = "0x7101010101010101010101010101010101010101010101010101010101010101";
const OTHER_KEY: &str = "0x7202020202020202020202020202020202020202020202020202020202020202";
const HOLDER_KEY: &str = "0x7303030303030303030303030303030303030303030303030303030303030303";
const CREATOR_KEY: &str = "0x7404040404040404040404040404040404040404040404040404040404040404";
const STRANGER_KEY: &str = "0x7505050505050505050505050505050505050505050505050505050505050505";
const COMMITTEE_KEY: &str = "0x7606060606060606060606060606060606060606060606060606060606060606";
const FORMER_KEY: &str = "0x7707070707070707070707070707070707070707070707070707070707070707";

#[derive(Clone, Copy, Debug, PartialEq)]
enum Caller {
    Anonymous,
    Manager,
    Other,
    Holder,
    Creator,
    Stranger,
    Committee,
}
use Caller::*;

const CALLERS: [Caller; 7] = [
    Anonymous, Manager, Other, Holder, Creator, Stranger, Committee,
];

impl Caller {
    fn key(self) -> Option<&'static str> {
        match self {
            Anonymous => None,
            Manager => Some(MANAGER_KEY),
            Other => Some(OTHER_KEY),
            Holder => Some(HOLDER_KEY),
            Creator => Some(CREATOR_KEY),
            Stranger => Some(STRANGER_KEY),
            Committee => Some(COMMITTEE_KEY),
        }
    }

    fn address(self) -> String {
        self.key().map(address).unwrap_or_default()
    }
}

fn address(key: &str) -> String {
    format!("{:#x}", wallet(key).address())
}

/// The provider of the fixture's collection.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Provider {
    /// The registry approved it, listing the manager; a virtual third party
    /// lists the holder.
    Approved,
    /// The registry lists the manager, unapproved; no virtual third party.
    Unapproved,
    /// The registry lists the manager and the holder, unapproved; the
    /// virtual third party lists the holder.
    Claimed,
    /// The registry lists the manager, unapproved; the virtual third party
    /// lists the holder.
    Squatted,
    /// No record; the virtual third party lists the holder.
    Virtual,
    /// Neither the registry nor a virtual third party.
    Unknown,
}
use Provider::*;

const PROVIDERS: [Provider; 6] = [Approved, Unapproved, Claimed, Squatted, Virtual, Unknown];

impl Provider {
    fn manages(self, caller: Caller) -> bool {
        match self {
            Approved | Unapproved => caller == Manager,
            Claimed | Virtual => caller == Holder,
            Squatted | Unknown => false,
        }
    }

    fn registered(self) -> bool {
        !matches!(self, Virtual | Unknown)
    }
}

struct World {
    pool: PgPool,
    app: Router,
    registry: FakeRegistry,
    storage: Arc<Storage>,
    state: crate::SharedState,
    hash: String,
    bytes: Vec<u8>,
}

struct Fixture {
    provider: String,
    collection: Uuid,
    collection_urn: String,
    item: Uuid,
    hash: String,
}

async fn world() -> Option<World> {
    let pool = super::cells::world_pool(8).await?;
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now()))",
    )
    .execute(&pool)
    .await
    .unwrap();
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let registry = FakeRegistry::up();
    registry.put(
        "urn:decentraland:matic:collections-thirdparty:other",
        &[&Other.address()],
        true,
        false,
    );
    let storage = temp_storage().await;
    let bytes = unique("third-party-file");
    let hash = dcl_auth::hash_bytes_v1(&bytes);
    storage
        .item_files()
        .unwrap()
        .store(&hash, bytes.clone().into())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO explore_api.feature_flags (app, document, fetched_at) \
         VALUES ('builder', '{\"flags\":{\"builder-auto-curation\":true,\"builder-auto-curation-review-flow\":true},\"variants\":{}}', timezone('UTC', now())) \
         ON CONFLICT (app) DO NOTHING",
    )
    .execute(&pool)
    .await
    .unwrap();
    let mut state = crate::State::new(
        pool.clone(),
        storage.clone(),
        Arc::default(),
        dcl_http::Outbound::for_tests(),
        Some(registry_index(&committee, &registry).await),
        None,
        crate::Settings {
            writes_enabled: true,
            validator_secret: Some("example-validator-secret-0123456789abcdef".into()),
            ..crate::Settings::default()
        },
    )
    .unwrap();
    state.limits = crate::limits::Limits::unbounded();
    let state = Arc::new(state);
    let app = crate::root_router(state.clone());
    Some(World {
        pool,
        app,
        registry,
        storage: storage.item_files().unwrap().clone(),
        state,
        hash,
        bytes,
    })
}

/// Brings `w` back to the world [`world`] made: no builder rows and only
/// the other provider in the registry.
async fn reset(w: &World, f: &Fixture) {
    sqlx::raw_sql(
        "DELETE FROM builder.slot_usage_cheques; \
         DELETE FROM builder.collection_events; \
         DELETE FROM builder.items; \
         DELETE FROM builder.collections; \
         DELETE FROM builder.virtual_third_parties; \
         DELETE FROM builder.uploaded_contents",
    )
    .execute(&w.pool)
    .await
    .unwrap();
    w.registry.remove(&f.provider);
}

async fn fixture(w: &World, provider: Provider) -> Fixture {
    let id = Uuid::new_v4().simple().to_string();
    let provider_id = format!("urn:decentraland:matic:collections-thirdparty:m{id}");
    match provider {
        Approved => w
            .registry
            .put(&provider_id, &[&Manager.address()], true, false),
        Unapproved | Squatted => w
            .registry
            .put(&provider_id, &[&Manager.address()], false, false),
        Claimed => w.registry.put(
            &provider_id,
            &[&Manager.address(), &Holder.address()],
            false,
            false,
        ),
        Virtual | Unknown => {}
    }
    if matches!(provider, Approved | Claimed | Squatted | Virtual) {
        sqlx::query(
            "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, 'tp:1:Virtual:')",
        )
        .bind(&provider_id)
        .bind(vec![Holder.address()])
        .execute(&w.pool)
        .await
        .unwrap();
    }
    let collection = Uuid::new_v4();
    let collection_urn = format!("{provider_id}:c{id}");
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(collection)
    .bind(format!("Linked {id}"))
    .bind(Creator.address())
    .bind(&provider_id)
    .bind(&collection_urn)
    .execute(&w.pool)
    .await
    .unwrap();
    let item = Uuid::new_v4();
    let item_urn = format!("{collection_urn}:hat");
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix, local_content_hash) \
         VALUES ($1, $2, $3, 'Hat', 'wearable', $4, repeat('ab', 32))",
    )
    .bind(item)
    .bind(collection)
    .bind(Creator.address())
    .bind(&item_urn)
    .execute(&w.pool)
    .await
    .unwrap();
    let hash = w.hash.clone();
    sqlx::query(
        "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($1, 16, $2) ON CONFLICT DO NOTHING",
    )
    .bind(&hash)
    .bind(Creator.address())
    .execute(&w.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO builder.item_contents (item_id, file, hash, named_by) VALUES ($1, 'model.glb', $2, $3)",
    )
    .bind(item)
    .bind(&hash)
    .bind(Creator.address())
    .execute(&w.pool)
    .await
    .unwrap();
    w.state.third_parties.outdate();
    Fixture {
        provider: provider_id,
        collection,
        collection_urn,
        item,
        hash,
    }
}

async fn call(
    app: &Router,
    caller: Caller,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let body = body.map(|value| {
        (
            "application/json".to_string(),
            value.to_string().into_bytes(),
        )
    });
    call_with(app, caller, method, path, body).await
}

/// Gives the fixture's item the curation `op` needs.
async fn prepare(w: &World, op: Op, f: &Fixture) {
    if let Some(status) = op.curated() {
        sqlx::query(
            "INSERT INTO builder.item_curations (id, item_id, status, content_hash) \
             VALUES (gen_random_uuid(), $1, $2, repeat('ab', 32))",
        )
        .bind(f.item)
        .bind(status)
        .execute(&w.pool)
        .await
        .unwrap();
    }
}

/// `op`'s request on `f` as `caller`; an upload sends the world's file as
/// its one part.
async fn send_op(w: &World, op: Op, f: &Fixture, caller: Caller) -> (StatusCode, Value) {
    let (method, path, body) = op.request(f, caller);
    if op != UploadFiles {
        return call(&w.app, caller, method, &path, body).await;
    }
    let boundary = "third-party-matrix";
    let mut bytes = format!(
        "--{boundary}\r\ncontent-disposition: form-data; name=\"{}\"; filename=\"blob\"\r\n\
         content-type: application/octet-stream\r\n\r\n",
        f.hash
    )
    .into_bytes();
    bytes.extend_from_slice(&w.bytes);
    bytes.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let kind = format!("multipart/form-data; boundary={boundary}");
    call_with(&w.app, caller, method, &path, Some((kind, bytes))).await
}

async fn call_with(
    app: &Router,
    caller: Caller,
    method: &str,
    path: &str,
    body: Option<(String, Vec<u8>)>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(key) = caller.key() {
        let now = Utc::now().timestamp_millis().to_string();
        for (name, value) in signed_headers_by(
            &wallet(key),
            &method.to_lowercase(),
            path.split('?').next().unwrap(),
            "{}",
            true,
            &now,
        )
        .await
        .iter()
        {
            request = request.header(name, value);
        }
    }
    let body = match body {
        Some((kind, bytes)) => {
            request = request.header(header::CONTENT_TYPE, kind);
            Body::from(bytes)
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 16 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// A digest of every row a refused write must leave as it was.
async fn snapshot(pool: &PgPool) -> String {
    sqlx::query_scalar(
        "SELECT md5(jsonb_build_object( \
           'collections', (SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY c.id), '[]') FROM builder.collections c), \
           'items', (SELECT COALESCE(jsonb_agg(to_jsonb(i) ORDER BY i.id), '[]') FROM builder.items i), \
           'contents', (SELECT COALESCE(jsonb_agg(to_jsonb(e) ORDER BY e.item_id, e.file), '[]') FROM builder.item_contents e), \
           'virtual', (SELECT COALESCE(jsonb_agg(to_jsonb(v) ORDER BY v.id), '[]') FROM builder.virtual_third_parties v), \
           'cheques', (SELECT count(*) FROM builder.slot_usage_cheques), \
           'curations', (SELECT count(*) FROM builder.item_curations) + (SELECT count(*) FROM builder.collection_curations))::text)",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Op {
    ReadCollection,
    CollectionItems,
    ReadItem,
    SaveCollection,
    CreateCollection,
    DeleteItem,
    DeleteCollection,
    Terms,
    ReadFile,
    AddressCollections,
    AddressItems,
    Slots,
    ThirdParty,
    ThirdParties,
    Publish,
    DraftCollections,
    ForumPost,
    ItemCurationRequest,
    ItemCurationUpdate,
    ReviewEvents,
    SaveItem,
    UploadFiles,
}
use Op::*;

const OPS: [Op; 22] = [
    ReadCollection,
    CollectionItems,
    ReadItem,
    SaveCollection,
    CreateCollection,
    DeleteItem,
    DeleteCollection,
    Terms,
    ReadFile,
    AddressCollections,
    AddressItems,
    Slots,
    ThirdParty,
    ThirdParties,
    Publish,
    DraftCollections,
    ForumPost,
    ItemCurationRequest,
    ItemCurationUpdate,
    ReviewEvents,
    SaveItem,
    UploadFiles,
];

impl Op {
    fn writes(self) -> bool {
        matches!(
            self,
            SaveCollection
                | CreateCollection
                | DeleteItem
                | DeleteCollection
                | Terms
                | ForumPost
                | Publish
                | ItemCurationRequest
                | ItemCurationUpdate
                | SaveItem
                | UploadFiles
        )
    }

    /// The newest item curation the fixture's item holds for this
    /// operation: an approved one to reopen, a pending one to refresh.
    fn curated(self) -> Option<&'static str> {
        match self {
            ItemCurationRequest => Some("approved"),
            ItemCurationUpdate => Some("pending"),
            _ => None,
        }
    }

    fn request(self, f: &Fixture, caller: Caller) -> (&'static str, String, Option<Value>) {
        let me = match caller {
            Anonymous => Stranger.address(),
            _ => caller.address(),
        };
        match self {
            ReadCollection => ("GET", format!("/v1/collections/{}", f.collection), None),
            CollectionItems => (
                "GET",
                format!("/v1/collections/{}/items", f.collection),
                None,
            ),
            ReadItem => ("GET", format!("/v1/items/{}", f.item), None),
            SaveCollection => (
                "PUT",
                format!("/v1/collections/{}", f.collection),
                Some(super::support::collection_body(
                    json!({ "id": f.collection.to_string(), "name": format!("Renamed {}", f.collection.simple()), "urn": f.collection_urn }),
                    &me,
                )),
            ),
            CreateCollection => {
                let id = Uuid::new_v4();
                (
                    "PUT",
                    format!("/v1/collections/{id}"),
                    Some(super::support::collection_body(
                        json!({ "id": id.to_string(), "name": format!("New {}", id.simple()), "urn": format!("{}:n{}", f.provider, id.simple()) }),
                        &me,
                    )),
                )
            }
            DeleteItem => ("DELETE", format!("/v1/items/{}", f.item), None),
            DeleteCollection => ("DELETE", format!("/v1/collections/{}", f.collection), None),
            Terms => (
                "POST",
                format!("/v1/collections/{}/tos", f.collection),
                Some(
                    json!({ "email": "jane.doe@example.com", "event": "publish_third_party_items_tos", "hashes": [f.hash] }),
                ),
            ),
            ReadFile => ("GET", format!("/v1/storage/contents/{}", f.hash), None),
            ForumPost => (
                "POST",
                format!("/v1/collections/{}/post", f.collection),
                Some(json!({})),
            ),
            AddressCollections => ("GET", format!("/v1/{me}/collections"), None),
            AddressItems => ("GET", format!("/v1/{me}/items"), None),
            DraftCollections => ("GET", "/v1/drafts/collections".to_string(), None),
            Slots => (
                "GET",
                format!("/v1/thirdParties/{}/slots", f.provider),
                None,
            ),
            ThirdParty => ("GET", format!("/v1/thirdParties/{}", f.provider), None),
            ItemCurationRequest => ("POST", format!("/v1/items/{}/curation", f.item), None),
            ItemCurationUpdate => (
                "PATCH",
                format!("/v1/items/{}/curation", f.item),
                Some(json!({ "curation": { "status": "pending" } })),
            ),
            ReviewEvents => (
                "GET",
                format!("/v1/collections/{}/events", f.collection),
                None,
            ),
            SaveItem => (
                "PUT",
                format!("/v1/items/{}", f.item),
                Some(json!({ "item": {
                    "id": f.item, "name": "Hat", "description": null, "type": "wearable",
                    "eth_address": me, "collection_id": f.collection,
                    "urn": format!("{}:hat", f.collection_urn),
                    "contents": { "model.glb": f.hash },
                    "metrics": { "meshes": 1, "bodies": 1, "materials": 1, "textures": 1, "triangles": 1, "entities": 1 },
                    "data": {
                        "category": "hat", "replaces": [], "hides": [], "tags": [],
                        "representations": [{
                            "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
                            "mainFile": "model.glb", "contents": ["model.glb"], "overrideHides": [], "overrideReplaces": []
                        }]
                    }
                } })),
            ),
            UploadFiles => ("POST", format!("/v1/items/{}/files", f.item), None),
            ThirdParties => ("GET", "/v1/thirdParties".to_string(), None),
            Publish => {
                let salt = format!("0x{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
                let digest = crate::cheques::digest(
                    &crate::network::POLYGON,
                    &f.provider,
                    1,
                    salt.parse().unwrap(),
                );
                let signature = wallet(caller.key().unwrap_or(STRANGER_KEY))
                    .sign_hash_sync(&alloy_primitives::B256::from(digest))
                    .unwrap();
                (
                    "POST",
                    format!("/v1/collections/{}/publish", f.collection),
                    Some(json!({ "itemIds": [f.item], "cheque": {
                        "qty": 1, "salt": salt,
                        "signature": format!("0x{}", hex::encode(signature.as_bytes())),
                    } })),
                )
            }
        }
    }

    /// The status a caller gets; for a listing, whether the answer holds
    /// the fixture.
    fn expect(self, p: Provider, caller: Caller) -> Expect {
        let manages = p.manages(caller);
        let signed = caller != Anonymous;
        let allowed = |granted: bool| {
            Expect::Status(if !signed {
                401
            } else if granted {
                200
            } else {
                401
            })
        };
        match self {
            ReadCollection | CollectionItems => allowed(manages || caller == Committee),
            ItemCurationRequest | ItemCurationUpdate => allowed(manages || caller == Committee),
            ReviewEvents => Expect::Status(match (signed, manages || caller == Committee) {
                (false, _) => 401,
                (true, true) => 200,
                (true, false) => 404,
            }),
            SaveItem | UploadFiles => allowed(manages),
            ReadItem => allowed(manages || caller == Committee),
            SaveCollection | DeleteItem | DeleteCollection | Terms => allowed(manages),
            CreateCollection => allowed(manages || p == Unknown),
            ReadFile => Expect::Status(if manages || caller == Creator {
                200
            } else {
                404
            }),
            AddressCollections | AddressItems | DraftCollections => {
                if signed {
                    Expect::Lists(manages)
                } else {
                    Expect::Status(401)
                }
            }
            Slots => allowed(manages),
            ForumPost => Expect::Status(match (signed, manages) {
                (false, _) => 401,
                (true, true) => 503,
                (true, false) => 401,
            }),
            ThirdParty => {
                if !signed {
                    Expect::Status(401)
                } else if p.registered() || (p == Virtual && manages) {
                    Expect::Status(200)
                } else {
                    Expect::Status(404)
                }
            }
            ThirdParties => {
                if !signed {
                    Expect::Status(401)
                } else {
                    Expect::Lists(listed(p, caller))
                }
            }
            Publish => Expect::Status(match (signed, manages, p.registered()) {
                (true, true, true) => 200,
                (true, true, false) => 409,
                _ => 401,
            }),
        }
    }
}

/// Whether `GET /v1/thirdParties` lists the fixture's provider to `caller`:
/// exactly when the caller manages it, so a wallet a virtual third party
/// lists is never shown a provider the registry holds for others.
fn listed(p: Provider, caller: Caller) -> bool {
    p.manages(caller)
}

#[derive(Debug, PartialEq)]
enum Expect {
    Status(u16),
    Lists(bool),
}

fn holds(body: &Value, f: &Fixture, op: Op) -> bool {
    let rows = match &body["data"] {
        Value::Array(rows) => rows.clone(),
        other => other["results"].as_array().cloned().unwrap_or_default(),
    };
    rows.iter().any(|row| match op {
        AddressCollections | DraftCollections => row["id"] == f.collection.to_string(),
        AddressItems => row["id"] == f.item.to_string(),
        _ => row["id"] == f.provider,
    })
}

async fn cell(w: &World, op: Op, p: Provider, caller: Caller) -> Option<String> {
    let f = fixture(w, p).await;
    prepare(w, op, &f).await;
    let expected = op.expect(p, caller);
    let before = match op.writes() {
        true => Some(snapshot(&w.pool).await),
        false => None,
    };
    let (status, answer) = send_op(w, op, &f, caller).await;
    let got = match expected {
        Expect::Lists(_) if status.is_success() => Expect::Lists(holds(&answer, &f, op)),
        _ => Expect::Status(status.as_u16()),
    };
    let mut problems = Vec::new();
    if got != expected {
        problems.push(format!(
            "answered {got:?} ({answer}), expected {expected:?}"
        ));
    }
    let write_checked = op.writes() && !status.is_success();
    if write_checked && Some(snapshot(&w.pool).await) != before {
        problems.push("a refused write changed rows".into());
    }
    super::cells::record(|| {
        format!(
            "every\t{op:?}\t{p:?}\t{caller:?}\t\t{expected:?}\t{got:?}\twrite={write_checked} problems={}",
            problems.len()
        )
    });
    reset(w, &f).await;
    (!problems.is_empty()).then(|| format!("{op:?} {p:?} {caller:?}: {}", problems.join("; ")))
}

#[tokio::test(flavor = "multi_thread")]
async fn each_caller_gets_its_providers_answer_on_every_third_party_operation() {
    let mut cells = Vec::new();
    for op in OPS {
        for p in PROVIDERS {
            for caller in CALLERS {
                cells.push((op, p, caller));
            }
        }
    }
    let Some(mut failures) =
        super::cells::in_worlds(cells, world, |w: Arc<World>, (op, p, caller)| async move {
            cell(&w, op, p, caller).await
        })
        .await
    else {
        return;
    };
    failures.sort();
    assert!(
        failures.is_empty(),
        "{} cells:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manager_the_registry_stops_listing_is_refused_its_next_write_and_its_reads_past_the_window()
 {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Approved).await;
    let former = address(FORMER_KEY);
    w.registry
        .set(&f.provider, "managers", json!([Manager.address(), former]));
    let as_former = |path: String| {
        let app = w.app.clone();
        async move {
            let now = Utc::now().timestamp_millis().to_string();
            let mut request = Request::builder().method("GET").uri(&path);
            for (name, value) in
                signed_headers_by(&wallet(FORMER_KEY), "get", &path, "{}", true, &now)
                    .await
                    .iter()
            {
                request = request.header(name, value);
            }
            app.oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status()
        }
    };
    let path = format!("/v1/collections/{}", f.collection);
    assert_eq!(as_former(path.clone()).await, StatusCode::OK);
    w.registry
        .set(&f.provider, "managers", json!([Manager.address()]));
    let (_, publish, body) = Publish.request(&f, Manager);
    let salt = format!("0x{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let digest = crate::cheques::digest(
        &crate::network::POLYGON,
        &f.provider,
        1,
        salt.parse().unwrap(),
    );
    let signature = wallet(FORMER_KEY)
        .sign_hash_sync(&alloy_primitives::B256::from(digest))
        .unwrap();
    let mut body = body.unwrap();
    body["cheque"] = json!({ "qty": 1, "salt": salt, "signature": format!("0x{}", hex::encode(signature.as_bytes())) });
    let before = snapshot(&w.pool).await;
    let now = Utc::now().timestamp_millis().to_string();
    let mut request = Request::builder()
        .method("POST")
        .uri(&publish)
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in signed_headers_by(&wallet(FORMER_KEY), "post", &publish, "{}", true, &now)
        .await
        .iter()
    {
        request = request.header(name, value);
    }
    let status = w
        .app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(snapshot(&w.pool).await, before);
    w.state.third_parties.outdate();
    assert_eq!(as_former(path).await, StatusCode::UNAUTHORIZED);
}

/// A manager the registry stops listing loses the third-party item it last
/// saved: its body, a file another manager adds to it, its place in the
/// wallet's item listings and its curations.
#[tokio::test(flavor = "multi_thread")]
async fn a_manager_the_registry_stops_listing_loses_the_item_it_saved() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Approved).await;
    w.registry.set(
        &f.provider,
        "managers",
        json!([Manager.address(), Creator.address()]),
    );
    let bytes = unique("added by the remaining manager");
    let added = dcl_auth::hash_bytes_v1(&bytes);
    w.storage.store(&added, bytes.into()).await.unwrap();
    sqlx::query(
        "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($1, 16, $2)",
    )
    .bind(&added)
    .bind(Manager.address())
    .execute(&w.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO builder.item_contents (item_id, file, hash, named_by) VALUES ($1, 'texture.png', $2, $3)",
    )
    .bind(f.item)
    .bind(&added)
    .bind(Manager.address())
    .execute(&w.pool)
    .await
    .unwrap();
    let item = f.item.to_string();
    let sees = |caller: Caller| {
        let (w, added, item) = (&w, added.clone(), item.clone());
        async move {
            let read = call(&w.app, caller, "GET", &format!("/v1/items/{item}"), None)
                .await
                .0;
            let file = call(
                &w.app,
                caller,
                "GET",
                &format!("/v1/storage/contents/{added}"),
                None,
            )
            .await
            .0;
            let me = caller.address();
            let (_, listed) = call(&w.app, caller, "GET", &format!("/v1/{me}/items"), None).await;
            let (_, drafts) = call(&w.app, caller, "GET", "/v1/drafts/items", None).await;
            let holds = |body: &Value| {
                body["data"]
                    .as_array()
                    .is_some_and(|rows| rows.iter().any(|r| r["id"] == item.as_str()))
            };
            (read.as_u16(), file.as_u16(), holds(&listed), holds(&drafts))
        }
    };
    assert_eq!(sees(Creator).await, (200, 200, true, true), "while listed");
    w.registry
        .set(&f.provider, "managers", json!([Manager.address()]));
    w.state.third_parties.outdate();
    assert_eq!(
        sees(Creator).await,
        (401, 404, false, false),
        "once dropped"
    );
    assert_eq!(
        sees(Manager).await,
        (200, 200, true, true),
        "the remaining manager"
    );
    sqlx::query(
        "INSERT INTO builder.item_curations (id, item_id, status, content_hash, created_at, updated_at) \
         VALUES (gen_random_uuid(), $1, 'approved', repeat('ab', 32), timezone('UTC', now()), timezone('UTC', now()))",
    )
    .bind(f.item)
    .execute(&w.pool)
    .await
    .unwrap();
    let before = snapshot(&w.pool).await;
    let curation = format!("/v1/items/{item}/curation");
    for (method, body) in [
        ("POST", None),
        (
            "PATCH",
            Some(json!({ "curation": { "status": "pending" } })),
        ),
    ] {
        let (status, answer) = call(&w.app, Creator, method, &curation, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method}: {answer}");
    }
    assert_eq!(snapshot(&w.pool).await, before);
}

/// One cell of the registry-down column: `op` on a fresh fixture of `p`
/// as `caller` while the registry answers an error. Every decision answers
/// 503 but the review timeline's, which knows no role holder of the
/// collection then: the committee reads it and anyone else finds it absent.
async fn registry_down_cell(w: &World, op: Op, p: Provider, caller: Caller) -> Option<String> {
    let mut failures = Vec::new();
    let f = fixture(w, p).await;
    prepare(w, op, &f).await;
    w.registry.fail(true);
    w.state.third_parties.outdate();
    let before = snapshot(&w.pool).await;
    let (status, answer) = send_op(w, op, &f, caller).await;
    w.registry.fail(false);
    w.registry.put(
        "urn:decentraland:matic:collections-thirdparty:other",
        &[&Other.address()],
        true,
        false,
    );
    let readable_anyway = op == ReadFile && caller == Creator;
    let expected = match (op, caller) {
        (ReviewEvents, Committee) => StatusCode::OK,
        (ReviewEvents, _) => StatusCode::NOT_FOUND,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    let changed = snapshot(&w.pool).await != before;
    super::cells::record(|| {
        format!(
            "registry down\t{op:?}\t{p:?}\t{caller:?}\t\t{} unless readable_anyway={readable_anyway}\t{}\twrite=true",
            expected.as_u16(),
            status.as_u16()
        )
    });
    if status != expected && !readable_anyway {
        failures.push(format!("{op:?} {p:?} {caller:?}: {status} {answer}"));
    }
    if changed {
        failures.push(format!("{op:?} {p:?} {caller:?}: rows changed"));
    }
    reset(w, &f).await;
    (!failures.is_empty()).then(|| failures.join("\n"))
}

#[tokio::test(flavor = "multi_thread")]
async fn while_the_registry_cant_tell_every_third_party_decision_is_503_and_writes_nothing() {
    let mut cells = Vec::new();
    for p in [Approved, Virtual, Unknown] {
        for op in OPS {
            for caller in [Manager, Holder, Creator, Committee] {
                cells.push((op, p, caller));
            }
        }
    }
    let Some(mut failures) =
        super::cells::in_worlds(cells, world, |w: Arc<World>, (op, p, caller)| async move {
            registry_down_cell(&w, op, p, caller).await
        })
        .await
    else {
        return;
    };
    failures.sort();
    assert!(
        failures.is_empty(),
        "{} cells:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_decisions_share_one_registry_read() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Approved).await;
    let before = w.registry.hits();
    let path = format!("/v1/collections/{}", f.collection);
    let calls: Vec<_> = (0..16)
        .map(|_| {
            let app = w.app.clone();
            let path = path.clone();
            tokio::spawn(async move { call(&app, Manager, "GET", &path, None).await.0 })
        })
        .collect();
    for c in calls {
        assert_eq!(c.await.unwrap(), StatusCode::OK);
    }
    let reads = w.registry.hits() - before;
    assert!(
        reads <= 2,
        "{reads} registry reads for 16 concurrent decisions"
    );
    let _ = &w.state;
}

#[tokio::test(flavor = "multi_thread")]
async fn while_the_copy_is_stale_a_manager_still_reads_its_providers_collection() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Approved).await;
    sqlx::query(
        "UPDATE shop.mirror_state SET last_complete_pass_at = timezone('UTC', now()) - interval '2 hours'",
    )
    .execute(&w.pool)
    .await
    .unwrap();
    let path = format!("/v1/collections/{}", f.collection);
    let (status, body) = call(&w.app, Manager, "GET", &path, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = call(&w.app, Creator, "GET", &path, None).await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "the wallet that created it is no manager: {body}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_first_collection_of_a_new_provider_makes_its_signer_the_providers_only_manager() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Unknown).await;
    let (method, path, body) = CreateCollection.request(&f, Holder);
    let (status, answer) = call(&w.app, Holder, method, &path, body).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let managers: Vec<String> =
        sqlx::query_scalar("SELECT managers FROM builder.virtual_third_parties WHERE id = $1")
            .bind(&f.provider)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert_eq!(managers, vec![Holder.address()]);
    let (method, path, body) = CreateCollection.request(&f, Stranger);
    let (status, answer) = call(&w.app, Stranger, method, &path, body).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{answer}");
    let (status, answer) = call(
        &w.app,
        Holder,
        "GET",
        &format!("/v1/collections/{}", f.collection),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the holder manages every collection of it: {answer}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unreadable_registry_record_grants_its_provider_to_nobody() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Unknown).await;
    w.registry
        .put(&f.provider, &[&Holder.address()], true, false);
    w.registry.set(&f.provider, "root", json!("0x12"));
    w.state.third_parties.outdate();
    for caller in [Holder, Stranger] {
        let (method, path, body) = CreateCollection.request(&f, caller);
        let (status, answer) = call(&w.app, caller, method, &path, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{caller:?}: {answer}");
        let (method, path, _) = ReadCollection.request(&f, caller);
        let (status, answer) = call(&w.app, caller, method, &path, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{caller:?}: {answer}");
    }
    let claimed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder.virtual_third_parties WHERE id = $1)",
    )
    .bind(&f.provider)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert!(
        !claimed,
        "nobody claimed the provider as a virtual third party"
    );
    let (status, answer) = call(&w.app, Other, "GET", "/v1/thirdParties", None).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(
        answer["data"][0]["id"],
        json!("urn:decentraland:matic:collections-thirdparty:other"),
        "{answer}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_virtual_third_party_goes_with_its_providers_last_collection() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Virtual).await;
    let (method, path, body) = CreateCollection.request(&f, Holder);
    let created = path.rsplit('/').next().unwrap().to_string();
    assert_eq!(
        call(&w.app, Holder, method, &path, body).await.0,
        StatusCode::OK
    );
    let exists = || async {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM builder.virtual_third_parties WHERE id = $1)",
        )
        .bind(&f.provider)
        .fetch_one(&w.pool)
        .await
        .unwrap()
    };
    let (status, body) = call(
        &w.app,
        Holder,
        "DELETE",
        &format!("/v1/collections/{created}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(exists().await, "another collection of the provider is left");
    let (status, body) = call(
        &w.app,
        Holder,
        "DELETE",
        &format!("/v1/collections/{}", f.collection),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!exists().await, "no collection of the provider is left");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_linked_contract_is_listed_on_the_providers_virtual_third_party() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Virtual).await;
    let first = "0x00000000000000000000000000000000000000c1";
    let second = "0x00000000000000000000000000000000000000c2";
    for contract in [first, second] {
        let body = super::support::collection_body(
            json!({
                "id": f.collection.to_string(),
                "name": format!("Linked {}", f.collection.simple()),
                "urn": f.collection_urn,
                "linked_contract_address": contract,
                "linked_contract_network": "amoy",
            }),
            &Holder.address(),
        );
        let (status, answer) = call(
            &w.app,
            Holder,
            "PUT",
            &format!("/v1/collections/{}", f.collection),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{answer}");
    }
    let raw: String =
        sqlx::query_scalar("SELECT raw_metadata FROM builder.virtual_third_parties WHERE id = $1")
            .bind(&f.provider)
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert_eq!(raw, format!("tp:1:Virtual::amoy-{second}"));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_free_slots_are_the_registry_allowance_less_the_submitted_items() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Approved).await;
    sqlx::query(
        "INSERT INTO builder.item_curations (id, item_id, status, content_hash, created_at) \
         SELECT gen_random_uuid(), $1, s.status, repeat('ab', 32), s.at \
         FROM (VALUES ('approved', '2025-01-01'::timestamp), ('pending', '2025-02-01'::timestamp)) s (status, at)",
    )
    .bind(f.item)
    .execute(&w.pool)
    .await
    .unwrap();
    let (status, body) = call(
        &w.app,
        Manager,
        "GET",
        &format!("/v1/thirdParties/{}/slots", f.provider),
        None,
    )
    .await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({ "ok": true, "data": 9 }))
    );
    let virtual_only = fixture(&w, Virtual).await;
    let (status, body) = call(
        &w.app,
        Holder,
        "GET",
        &format!("/v1/thirdParties/{}/slots", virtual_only.provider),
        None,
    )
    .await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({ "ok": true, "data": 0 }))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manager_reads_a_file_only_where_its_writer_holds_the_bytes() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Approved).await;
    let bytes = unique("third-party-unheld");
    let hash = dcl_auth::hash_bytes_v1(&bytes);
    w.storage.store(&hash, bytes.into()).await.unwrap();
    sqlx::query(
        "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($1, 16, $2)",
    )
    .bind(&hash)
    .bind(Stranger.address())
    .execute(&w.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO builder.item_contents (item_id, file, hash, named_by) VALUES ($1, 'other.glb', $2, $3)",
    )
    .bind(f.item)
    .bind(&hash)
    .bind(Creator.address())
    .execute(&w.pool)
    .await
    .unwrap();
    let path = format!("/v1/storage/contents/{hash}");
    let (status, _) = call(&w.app, Manager, "GET", &path, None).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the name's writer never uploaded these bytes"
    );
    let (status, _) = call(
        &w.app,
        Manager,
        "GET",
        &format!("/v1/storage/contents/{}", f.hash),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unsigned_request_never_reads_the_registry() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Approved).await;
    let before = w.registry.hits();
    for op in OPS {
        let (method, path, body) = op.request(&f, Anonymous);
        let (status, _) = call(&w.app, Anonymous, method, &path, body).await;
        assert_ne!(status, StatusCode::OK, "{op:?}");
    }
    for path in [
        format!("/v1/thirdParties/{}", "x".repeat(4000)),
        "/v1/thirdParties/%22%7D%20query%20%7B".to_string(),
    ] {
        let (status, _) = call(&w.app, Anonymous, "GET", &path, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
    }
    assert_eq!(w.registry.hits(), before);
    let (status, _) = call(
        &w.app,
        Stranger,
        "GET",
        &format!("/v1/thirdParties/{}", "x".repeat(4000)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        w.registry.hits(),
        before,
        "an id over the bound reads nothing"
    );
}

/// A wallet the virtual third party lists but the registry's unapproved
/// record leaves out neither removes nor changes the virtual third party,
/// so it can't widen the provider's managers to all the registry's; a
/// wallet both list still does.
#[tokio::test(flavor = "multi_thread")]
async fn only_a_manager_both_lists_name_changes_or_removes_a_virtual_third_party() {
    let Some(w) = world().await else { return };
    let squatted = fixture(&w, Squatted).await;
    let path = format!("/v1/thirdParties/{}", squatted.provider);
    let patched = call(
        &w.app,
        Holder,
        "PATCH",
        &path,
        Some(json!({ "isProgrammatic": true })),
    )
    .await
    .0;
    let deleted = call(&w.app, Holder, "DELETE", &path, None).await.0;
    let kept: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder.virtual_third_parties WHERE id = $1 AND NOT is_programmatic)",
    )
    .bind(&squatted.provider)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    let claimed = fixture(&w, Claimed).await;
    let removed = call(
        &w.app,
        Holder,
        "DELETE",
        &format!("/v1/thirdParties/{}", claimed.provider),
        None,
    )
    .await
    .0;
    assert_eq!(
        (patched, deleted, kept, removed),
        (
            StatusCode::UNAUTHORIZED,
            StatusCode::UNAUTHORIZED,
            true,
            StatusCode::OK
        )
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_virtual_third_partys_manager_sets_it_programmatic_whatever_else_the_body_holds() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Claimed).await;
    let (status, body) = call(
        &w.app,
        Holder,
        "PATCH",
        &format!("/v1/thirdParties/{}", f.provider),
        Some(json!({ "isProgrammatic": true, "name": "ignored" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let programmatic: bool = sqlx::query_scalar(
        "SELECT is_programmatic FROM builder.virtual_third_parties WHERE id = $1",
    )
    .bind(&f.provider)
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert!(programmatic);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_public_search_answers_a_linked_collection_with_its_providers_programmatic_flag() {
    let Some(w) = world().await else { return };
    let f = fixture(&w, Approved).await;
    w.registry
        .put(&f.provider, &[&Manager.address()], true, true);
    w.state.third_parties.outdate();
    prepare(&w, ItemCurationRequest, &f).await;
    let name: String = sqlx::query_scalar("SELECT name FROM builder.collections WHERE id = $1")
        .bind(f.collection)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    let (status, body) = call(
        &w.app,
        Anonymous,
        "GET",
        &format!(
            "/v1/collections?q={}&is_published=true",
            name.trim_start_matches("Linked ")
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["urn"], json!(f.collection_urn), "{body}");
    assert_eq!(body["data"][0]["is_programmatic"], json!(true), "{body}");
}
