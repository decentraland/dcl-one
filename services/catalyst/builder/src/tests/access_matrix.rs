//! The authorization matrix: every route the route ledger marks `Routed`,
//! under both mounts, called anonymously and as the owner, a signed
//! stranger, an on-chain manager, a committee member and the chain creator
//! of a transferred collection, against each resource state that changes the
//! answer. The matrix's provider has a virtual third party that lists the
//! owner and no registry record, so the owner is its manager; every caller
//! kind and provider state of the third-party role is in `third_party_matrix`. Each cell asserts the status and the envelope's `error`, and a
//! refused write leaves its rows as they were. The last tests are the "can't
//! tell" column: a stale copy, a copy with no complete pass and a committee
//! source that is down answer 503 and write nothing; the small public reads
//! answer alike in each, and only the rarities answer 503 while their
//! source is down.
//!
//! The wearable validator is a caller of its own: it signs with the shared
//! secret instead of a wallet, so every wallet route answers it as it
//! answers an anonymous caller, and its three routes refuse every wallet.
//!
//! A matrix test lists its cells ([`Cell`]) and runs them with
//! [`failures_in`] on worlds in one [`Condition`] ([`super::cells`]): each
//! cell gets a fresh fixture, and its world is reset to its seed after it.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{StatusCode, header};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{FakeCommittee, FakeOracle, address, committee_index, fresh_pass, request};
use super::upstream_routes::{Route, UPSTREAM};
use crate::shared::validator_signature::{SIGNATURE_HEADER, TIMESTAMP_HEADER};

const OWNER_KEY: &str = super::support::OWNER_KEY;
const STRANGER_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
const COMMITTEE_KEY: &str = "0x3333333333333333333333333333333333333333333333333333333333333333";
const MANAGER_KEY: &str = "0x4444444444444444444444444444444444444444444444444444444444444444";
const CREATOR_KEY: &str = "0x5555555555555555555555555555555555555555555555555555555555555555";
const SIBLING_KEY: &str = "0x6666666666666666666666666666666666666666666666666666666666666666";
const ADMIN_KEY: &str = "0x7777777777777777777777777777777777777777777777777777777777777777";
const FORMER_COMMITTEE_KEY: &str =
    "0x8888888888888888888888888888888888888888888888888888888888888888";
const MINTER_KEY: &str = "0x9999999999999999999999999999999999999999999999999999999999999999";
const PROVIDER: &str = "urn:decentraland:matic:collections-thirdparty:matrix";
const MOUNTS: [&str; 2] = ["", "/builder-api"];
const VALIDATOR_SECRET: &str = "example-validator-secret-0123456789abcdef";

#[derive(Clone, Copy, Debug, PartialEq)]
enum Caller {
    Anonymous,
    Owner,
    Stranger,
    Manager,
    Committee,
    Creator,
    Sibling,
    Validator,
    /// A wallet `BUILDER_ADMIN_ADDRESSES` lists.
    Admin,
    /// A wallet the collections subgraph no longer lists on the committee.
    FormerCommittee,
    /// A minter the copy lists for an on-chain collection.
    Minter,
}
use Caller::*;

const CALLERS: [Caller; 11] = [
    Anonymous,
    Owner,
    Stranger,
    Manager,
    Committee,
    Creator,
    Sibling,
    Validator,
    Admin,
    FormerCommittee,
    Minter,
];

impl Caller {
    fn key(self) -> Option<&'static str> {
        match self {
            Anonymous | Validator => None,
            Owner => Some(OWNER_KEY),
            Stranger => Some(STRANGER_KEY),
            Manager => Some(MANAGER_KEY),
            Committee => Some(COMMITTEE_KEY),
            Creator => Some(CREATOR_KEY),
            Sibling => Some(SIBLING_KEY),
            Admin => Some(ADMIN_KEY),
            FormerCommittee => Some(FORMER_COMMITTEE_KEY),
            Minter => Some(MINTER_KEY),
        }
    }

    fn address(self) -> String {
        self.key().map(address).unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Missing,
    Malformed,
    Draft,
    Published,
    Approved,
    Unminted,
    Publishing,
    Locked,
    ThirdParty,
    ThirdPartySubmitted,
    CuratedPending,
    CuratedApproved,
    CuratedRejected,
    ReviewPending,
    ReviewApproved,
    ReviewRejected,
    Transferred,
    Orphan,
}
use Kind::*;

const KINDS: [Kind; 18] = [
    Missing,
    Malformed,
    Draft,
    Published,
    Approved,
    Unminted,
    Publishing,
    Locked,
    ThirdParty,
    ThirdPartySubmitted,
    CuratedPending,
    CuratedApproved,
    CuratedRejected,
    ReviewPending,
    ReviewApproved,
    ReviewRejected,
    Transferred,
    Orphan,
];

impl Kind {
    /// The kind whose answers a curated kind shares: an item curation of any
    /// status publishes a third-party item, and a collection curation
    /// changes no access to a published collection.
    fn base(self) -> Kind {
        match self {
            CuratedPending | CuratedApproved | CuratedRejected => ThirdPartySubmitted,
            ReviewPending | ReviewApproved | ReviewRejected => Published,
            kind => kind,
        }
    }

    fn curation(self) -> Option<&'static str> {
        match self {
            ThirdPartySubmitted | CuratedPending | ReviewPending => Some("pending"),
            CuratedApproved | ReviewApproved => Some("approved"),
            CuratedRejected | ReviewRejected => Some("rejected"),
            _ => None,
        }
    }

    fn on_chain(self) -> bool {
        matches!(self.base(), Published | Approved | Unminted | Transferred)
    }

    fn published(self) -> bool {
        self.on_chain() || self.base() == ThirdPartySubmitted
    }

    fn third_party(self) -> bool {
        matches!(self.base(), ThirdParty | ThirdPartySubmitted)
    }

    fn owner(self, caller: Caller) -> bool {
        match self {
            Missing | Malformed => false,
            Transferred => caller == Creator,
            _ => caller == Owner,
        }
    }

    fn manager(self, caller: Caller) -> bool {
        self.on_chain() && caller == Manager
    }

    /// The item's owner: its collection's owner, or the wallet the node
    /// stored for an item with no collection.
    fn item_owner(self, caller: Caller) -> bool {
        self.owner(caller)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Expect(u16, String);

fn ok() -> Option<Expect> {
    Some(Expect(200, String::new()))
}
/// The keys of the first body in upstream's capture `name`
/// (`tests/fixtures/conformance/builder/source`).
fn upstream_keys(name: &str) -> std::collections::BTreeSet<String> {
    let path = format!(
        "{}/services/catalyst/tests/fixtures/conformance/builder/source/{name}.json",
        env!("DCL_ONE_ROOT")
    );
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let data = &raw["response"]["body_json"]["data"];
    let body = data.as_array().map_or(data, |items| &items[0]);
    body.as_object().unwrap().keys().cloned().collect()
}

/// [`upstream_keys`] of a collection body, plus `third_party_id`: the
/// provider prefix of the `urn` the same caller receives.
/// The keys upstream answers a collection read: the full body or the public
/// detail, of a standard or a third-party collection.
fn collection_keys(full: bool, third_party: bool) -> std::collections::BTreeSet<String> {
    let mut keys = upstream_keys(match (full, third_party) {
        (_, true) => "80-collection-third-party-manager",
        (true, false) => "02-collection-owner",
        (false, false) => "23-collection-published-stranger",
    });
    if third_party && !full {
        for private in ["salt", "forum_link", "forum_id", "lock"] {
            keys.remove(private);
        }
    }
    keys
}

/// The role is admitted: whatever the route answers next, it is not a 401,
/// 403, 404 or a 5xx.
fn admitted() -> Option<Expect> {
    Some(Expect(0, String::new()))
}
fn refused(status: u16, error: impl Into<String>) -> Option<Expect> {
    Some(Expect(status, error.into()))
}
fn unauthenticated() -> Option<Expect> {
    refused(401, "Unauthenticated")
}
fn unauthorized() -> Option<Expect> {
    refused(403, "Unauthorized")
}
fn not_found(id: &str, table: &str) -> Option<Expect> {
    refused(404, format!("Couldn't find \"{id}\" on {table}"))
}
fn invalid_uuid(id: &str) -> Option<Expect> {
    refused(400, format!("Invalid uuid {id}"))
}
fn unauthorized_user(signer: &str, table: &str, id: &str) -> Option<Expect> {
    refused(403, format!("Unauthorized user {signer} for {table} {id}"))
}
fn collection_published(action: &str) -> Option<Expect> {
    refused(
        409,
        format!("The collection is published. It can't be {action}."),
    )
}
fn collection_locked(action: &str) -> Option<Expect> {
    refused(
        423,
        format!("The collection is locked. It can't be {action}."),
    )
}
fn item_published(action: &str) -> Option<Expect> {
    refused(
        409,
        format!(
            "The collection that contains this item has been already published. The item can't be {action}."
        ),
    )
}
fn third_party_item_published(action: &str) -> Option<Expect> {
    refused(
        409,
        format!("The third party item is already published. It can't be {action}."),
    )
}
fn item_locked(action: &str) -> Option<Expect> {
    refused(
        423,
        format!("The collection for the item is locked. The item can't be {action}."),
    )
}

#[derive(Debug, Clone, Default)]
struct Fixture {
    collection: Option<Uuid>,
    item: Option<Uuid>,
    minted: Option<Uuid>,
    urn: Option<String>,
    contract: Option<String>,
}

impl Fixture {
    fn collection_id(&self, kind: Kind) -> String {
        match kind {
            Malformed => "not-a-uuid".into(),
            _ => self.collection.unwrap_or_else(Uuid::new_v4).to_string(),
        }
    }

    fn item_id(&self, kind: Kind) -> String {
        match kind {
            Malformed => "not-a-uuid".into(),
            _ => self.item.unwrap_or_else(Uuid::new_v4).to_string(),
        }
    }

    /// The contract and on-chain id the item is read by.
    fn by_contract(&self, kind: Kind) -> (String, &'static str) {
        match kind {
            Malformed => ("not-an-address".into(), "0"),
            Unminted => (self.contract.clone().unwrap(), "1"),
            _ => (self.contract.clone().unwrap_or_else(contract), "0"),
        }
    }
}

/// `text` with the fixture's generated ids and contract named by their role.
fn anonymized(text: String, f: &Fixture) -> String {
    let mut text = text;
    for (value, role) in [
        (f.collection.map(|c| c.to_string()), "{collection}"),
        (f.item.map(|i| i.to_string()), "{item}"),
        (f.minted.map(|i| i.to_string()), "{minted}"),
        (f.contract.clone(), "{contract}"),
    ] {
        if let Some(value) = value {
            text = text.replace(&value, role);
        }
    }
    text
}

fn contract() -> String {
    format!("0x{}", Uuid::new_v4().simple().to_string()[..20].repeat(2))
}

async fn insert_collection(
    pool: &PgPool,
    id: Uuid,
    third_party: bool,
    contract: Option<&str>,
    pending: bool,
) -> Option<String> {
    let urn = third_party.then(|| format!("{PROVIDER}:{id}"));
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, contract_address, third_party_id, urn_suffix, publication_pending) \
         VALUES ($1, 'Matrix', $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(Owner.address())
    .bind(contract)
    .bind(third_party.then_some(PROVIDER))
    .bind(&urn)
    .bind(pending)
    .execute(pool)
    .await
    .unwrap();
    urn
}

async fn copy_row(pool: &PgPool, contract: &str, creator: Caller, approved: bool) {
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
           is_completed, is_approved, is_editable, managers, minters, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 137, 'Matrix', $2, $2, 'urn:matrix', true, $3, true, $4, $5, 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(contract)
    .bind(creator.address())
    .bind(approved)
    .bind(vec![Manager.address().to_uppercase().replace("0X", "0x")])
    .bind(vec![Minter.address().to_uppercase().replace("0X", "0x")])
    .execute(pool)
    .await
    .unwrap();
}

/// The bytes of the one file every matrix item names, as `model.glb`.
const MATRIX_FILE: &[u8] = b"matrix model";

/// The MP4 every matrix item names as its video, `video.mp4`.
const MATRIX_VIDEO: &[u8] = b"\0\0\0\x10ftypisom\0\0\x02\0\0\0\0\x08moov";

/// A client-shaped `PUT /v1/items/:id` body for the matrix item `id`.
fn item_json(id: &str, collection: Option<Uuid>, urn: Option<String>) -> Value {
    let mut item = json!({
        "id": id, "name": "Matrix renamed", "description": "", "type": "wearable",
        "eth_address": Owner.address(), "rarity": "common", "thumbnail": "thumbnail.png",
        "collection_id": collection,
        "contents": { "model.glb": dcl_auth::hash_bytes_v1(MATRIX_FILE) },
        "metrics": { "meshes": 1, "bodies": 1, "materials": 1, "textures": 1, "triangles": 1, "entities": 1 },
        "data": {
            "category": "hat", "replaces": [], "hides": [], "tags": [],
            "representations": [{
                "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
                "mainFile": "model.glb", "contents": ["model.glb"],
                "overrideHides": [], "overrideReplaces": []
            }]
        }
    });
    if let Some(urn) = urn {
        item["urn"] = urn.into();
    }
    json!({ "item": item })
}

/// The URN of the matrix item of the third-party collection `urn`.
fn item_urn(collection_urn: &str) -> String {
    format!("{collection_urn}:matrix-item")
}

async fn insert_item(
    pool: &PgPool,
    collection: Option<Uuid>,
    minted: bool,
    third_party: Option<(&str, bool)>,
) -> Uuid {
    let id = Uuid::new_v4();
    let urn = third_party.map(|(urn, _)| item_urn(urn));
    let submitted = third_party.is_some_and(|(_, submitted)| submitted);
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, blockchain_item_id, local_content_hash, \
           urn_suffix, is_published) \
         VALUES ($1, $2, $3, 'Matrix item', 'wearable', $4, repeat('ab', 32), $5, $6)",
    )
    .bind(id)
    .bind(collection)
    .bind(Owner.address())
    .bind(if submitted { Some("matrix-item") } else { minted.then_some("0") })
    .bind(&urn)
    .bind(submitted)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO builder.item_contents (item_id, file, hash, named_by) VALUES ($1, 'model.glb', $2, $3)",
    )
    .bind(id)
    .bind(dcl_auth::hash_bytes_v1(MATRIX_FILE))
    .bind(Owner.address())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO builder.item_contents (item_id, file, hash, named_by, position) VALUES ($1, 'video.mp4', $2, $3, 1)",
    )
    .bind(id)
    .bind(dcl_auth::hash_bytes_v1(MATRIX_VIDEO))
    .bind(Owner.address())
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn fixture(pool: &PgPool, kind: Kind) -> Fixture {
    let id = Uuid::new_v4();
    match kind {
        Malformed => Fixture::default(),
        Missing => Fixture {
            collection: Some(id),
            item: Some(Uuid::new_v4()),
            ..Fixture::default()
        },
        Orphan => Fixture {
            item: Some(insert_item(pool, None, false, None).await),
            ..Fixture::default()
        },
        Draft | Publishing | Locked | ThirdParty | ThirdPartySubmitted | CuratedPending
        | CuratedApproved | CuratedRejected => {
            let urn =
                insert_collection(pool, id, kind.third_party(), None, kind == Publishing).await;
            if kind == Locked {
                sqlx::query(
                    "UPDATE builder.collections SET lock = timezone('UTC', now()) WHERE id = $1",
                )
                .bind(id)
                .execute(pool)
                .await
                .unwrap();
            }
            let third_party = urn.as_deref().map(|urn| (urn, kind == ThirdPartySubmitted));
            let item = insert_item(pool, Some(id), false, third_party).await;
            if let Some(status) = kind
                .curation()
                .or((kind == ThirdPartySubmitted).then_some("pending"))
            {
                sqlx::query(
                    "INSERT INTO builder.item_curations (id, item_id, status, content_hash) VALUES ($1, $2, $3, $4)",
                )
                .bind(Uuid::new_v4())
                .bind(item)
                .bind(status)
                .bind(format!("{}{}", id.simple(), item.simple()))
                .execute(pool)
                .await
                .unwrap();
            }
            Fixture {
                collection: Some(id),
                item: Some(item),
                minted: None,
                urn,
                contract: None,
            }
        }
        Published | Approved | Unminted | Transferred | ReviewPending | ReviewApproved
        | ReviewRejected => {
            let contract = contract();
            insert_collection(pool, id, false, Some(&contract), false).await;
            if let Some(status) = kind.curation() {
                sqlx::query(
                    "INSERT INTO builder.collection_curations (id, collection_id, status, assignee, reviewed_by) \
                     VALUES ($1, $2, $3, $4, CASE WHEN $3 = 'pending' THEN NULL ELSE $4 END)",
                )
                .bind(Uuid::new_v4())
                .bind(id)
                .bind(status)
                .bind(Committee.address())
                .execute(pool)
                .await
                .unwrap();
            }
            let creator = if kind == Transferred { Creator } else { Owner };
            copy_row(pool, &contract, creator, kind == Approved).await;
            let minted = insert_item(pool, Some(id), true, None).await;
            let unminted = insert_item(pool, Some(id), false, None).await;
            Fixture {
                collection: Some(id),
                item: Some(if kind == Unminted { unminted } else { minted }),
                minted: Some(minted),
                urn: None,
                contract: Some(contract),
            }
        }
    }
}

/// A digest of every row a refused write must leave as it was.
async fn snapshot(pool: &PgPool) -> String {
    sqlx::query_scalar(
        "SELECT md5(jsonb_build_object( \
           'collections', (SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY c.id), '[]') FROM builder.collections c), \
           'items', (SELECT COALESCE(jsonb_agg(to_jsonb(i) ORDER BY i.id), '[]') FROM builder.items i), \
           'contents', (SELECT count(*) FROM builder.item_contents), \
           'uploads', (SELECT count(*) FROM builder.uploaded_contents), \
           'publications', (SELECT COALESCE(jsonb_agg(to_jsonb(p) ORDER BY p.collection_id), '[]') FROM builder.collection_publications p), \
           'cheques', (SELECT COALESCE(jsonb_agg(to_jsonb(q) ORDER BY q.id), '[]') FROM builder.slot_usage_cheques q), \
           'curations', (SELECT COALESCE(jsonb_agg(to_jsonb(k) ORDER BY k.id), '[]') FROM builder.collection_curations k), \
           'item_curations', (SELECT COALESCE(jsonb_agg(to_jsonb(k) ORDER BY k.id), '[]') FROM builder.item_curations k), \
           'validations', (SELECT COALESCE(jsonb_agg(to_jsonb(v) ORDER BY v.id), '[]') FROM builder.validation_requests v))::text)",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

enum Payload {
    None,
    Json(Value),
    Upload,
    Video,
}

const BOUNDARY: &str = "matrix-boundary";

async fn call(
    app: &Router,
    caller: Caller,
    method: &str,
    path: &str,
    payload: Payload,
) -> (StatusCode, Value) {
    let mut request = request(caller.key(), method, path).await;
    let body: Vec<u8> = match payload {
        Payload::None => Vec::new(),
        Payload::Json(value) => {
            let value = match (method, value.get("collection")) {
                ("PUT", Some(collection)) if path.contains("/v1/collections/") => {
                    let signer = caller.key().map_or(Owner.address(), address);
                    let mut collection = collection.clone();
                    if let (Some(id), Some(name)) =
                        (collection["id"].as_str(), collection["name"].as_str())
                    {
                        collection["name"] = format!("{name} {}", &id[..8]).into();
                    }
                    super::support::collection_body(collection, &signer)
                }
                _ => value,
            };
            request = request.header(header::CONTENT_TYPE, "application/json");
            value.to_string().into_bytes()
        }
        Payload::Upload => {
            request = request.header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={BOUNDARY}"),
            );
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"blob\"\r\n\
                 Content-Type: application/octet-stream\r\n\r\n{}\r\n--{BOUNDARY}--\r\n",
                dcl_auth::hash_bytes_v1(MATRIX_FILE),
                String::from_utf8_lossy(MATRIX_FILE)
            )
            .into_bytes()
        }
        Payload::Video => {
            request = request.header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={BOUNDARY}"),
            );
            let mut body = format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"video.mp4\"; filename=\"video.mp4\"\r\n\
                 Content-Type: video/mp4\r\n\r\n"
            )
            .into_bytes();
            body.extend_from_slice(MATRIX_VIDEO);
            body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
            body
        }
    };
    if caller == Validator {
        let now = Utc::now().timestamp_millis().to_string();
        let signature = crate::shared::validator_signature::sign(
            VALIDATOR_SECRET.as_bytes(),
            &now,
            method,
            path.split('?').next().unwrap(),
            &body,
        );
        request = request
            .header(TIMESTAMP_HEADER, now)
            .header(SIGNATURE_HEADER, signature);
    }
    let body = Body::from(body);
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 64 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Op {
    ReadCollection,
    SaveCollection,
    CollectionItems,
    ReadItem,
    SaveItem,
    DeleteItem,
    UploadFiles,
    UploadVideo,
    ListCollections,
    ListItems,
    AddressCollections,
    AddressItems,
    Committee,
    StorageGet,
    StorageHead,
    StorageExists,
    Newsletter,
    DraftCollections,
    DraftItems,
    PublicationPrepare,
    PublicationBegin,
    PublicationTransaction,
    PublicationCancel,
    PublicationClaim,
    PublicationStatus,
    CurationQueue,
    ItemStatus,
    ItemsStatus,
    Ping,
    Info,
    Rarities,
    Rarity,
    Utility,
    ItemContents,
    Addresses,
    Lock,
    DeleteCollection,
    OwnerSync,
    Terms,
    Publish,
    ApprovalData,
    ThirdParties,
    ThirdPartyRead,
    ThirdPartySlots,
    ThirdPartyDelete,
    ThirdPartyUpdate,
    Curations,
    CollectionCuration,
    CollectionItemCurations,
    CollectionCurationUpdate,
    CollectionCurationRequest,
    ItemCuration,
    ItemCurationUpdate,
    ItemCurationRequest,
    ForumPost,
    ForumReply,
    ValidatorClaim,
    ValidatorLease,
    ValidationResult,
    ReviewRetry,
    ReviewAppeal,
    ReviewEvents,
    ReviewComment,
}

const NOT_PUBLISHED: &str = "The collection is not published.";

const RARITIES_UNAVAILABLE: &str = "The rarities are not available. Try again later.";

const NOT_ADMIN: &str =
    "Not authorized: curation requires the admin token or a committee-address signed request";

/// Every method and path the builder's router serves, read from where
/// `lib.rs` declares them, so a route added there without a matrix row
/// fails [`ops`].
fn router_routes() -> Vec<(String, String)> {
    const METHODS: [&str; 6] = ["get", "post", "put", "patch", "delete", "head"];
    let source = include_str!("../lib.rs");
    for other in [
        "route_service(",
        "nest_service(",
        ".nest(",
        ".merge(",
        ".fallback(",
        "routing::any",
        "routing::on",
        "MethodFilter",
    ] {
        assert!(
            !source.contains(other),
            "lib.rs declares a route with {other}, which the matrix can't read"
        );
    }
    let imported = source
        .split("use axum::routing::{")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("lib.rs imports its routing methods from axum::routing");
    for name in imported.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        assert!(
            METHODS.contains(&name),
            "lib.rs routes with axum::routing::{name}, which the matrix can't read"
        );
    }
    let mut routes = Vec::new();
    for chunk in source.split(".route(").skip(1) {
        let chunk = chunk.split("\n}\n").next().unwrap();
        let path = chunk.split('"').nth(1).unwrap().to_string();
        let before = routes.len();
        for method in METHODS {
            for (at, _) in chunk.match_indices(&format!("{method}(")) {
                if chunk[..at]
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    continue;
                }
                routes.push((method.to_uppercase(), path.clone()));
            }
        }
        assert!(
            routes.len() > before,
            "route {path} declares no method the matrix can read"
        );
    }
    routes
}

impl Op {
    fn of(method: &str, path: &str) -> Option<Self> {
        const PUBLICATION: &str = "/v1/collections/{id}/publication";
        Some(match (method, path) {
            ("GET", "/v1/collections/{id}") => Op::ReadCollection,
            ("PUT", "/v1/collections/{id}") => Op::SaveCollection,
            ("GET", "/v1/collections/{id}/items") => Op::CollectionItems,
            ("GET", "/v1/items/{id}") => Op::ReadItem,
            ("PUT", "/v1/items/{id}") => Op::SaveItem,
            ("DELETE", "/v1/items/{id}") => Op::DeleteItem,
            ("POST", "/v1/items/{id}/files") => Op::UploadFiles,
            ("POST", "/v1/items/{id}/videos") => Op::UploadVideo,
            ("GET", "/v1/collections") => Op::ListCollections,
            ("GET", "/v1/items") => Op::ListItems,
            ("GET", "/v1/{address}/collections") => Op::AddressCollections,
            ("GET", "/v1/{address}/items") => Op::AddressItems,
            ("GET", "/v1/committee") => Op::Committee,
            ("GET", "/v1/storage/contents/{hash}") => Op::StorageGet,
            ("HEAD", "/v1/storage/contents/{hash}") => Op::StorageHead,
            ("GET", "/v1/storage/contents/{hash}/exists") => Op::StorageExists,
            ("POST", "/v1/newsletter") => Op::Newsletter,
            ("GET", "/v1/drafts/collections") => Op::DraftCollections,
            ("GET", "/v1/drafts/items") => Op::DraftItems,
            ("GET", PUBLICATION) => Op::PublicationPrepare,
            ("POST", PUBLICATION) => Op::PublicationBegin,
            ("PUT", PUBLICATION) => Op::PublicationTransaction,
            ("DELETE", PUBLICATION) => Op::PublicationCancel,
            ("PATCH", PUBLICATION) => Op::PublicationClaim,
            ("GET", "/v1/collections/{id}/publication/status") => Op::PublicationStatus,
            ("GET", "/v1/collections/curation") => Op::CurationQueue,
            ("PATCH", "/v1/collections/{id}/items/{item}/status") => Op::ItemStatus,
            ("PATCH", "/v1/collections/{id}/items/status") => Op::ItemsStatus,
            ("GET", "/ping") => Op::Ping,
            ("GET", "/v1/info") => Op::Info,
            ("GET", "/v1/rarities") => Op::Rarities,
            ("GET", "/v1/rarities/{name}") => Op::Rarity,
            ("GET", "/v1/published-collections/{address}/items/{id}/utility") => Op::Utility,
            ("GET", "/v1/items/{id}/{item}/contents") => Op::ItemContents,
            ("GET", "/v1/addresses") => Op::Addresses,
            ("POST", "/v1/collections/{id}/lock") => Op::Lock,
            ("DELETE", "/v1/collections/{id}") => Op::DeleteCollection,
            ("POST", "/v1/collections/{id}/owner/sync") => Op::OwnerSync,
            ("POST", "/v1/collections/{id}/tos") => Op::Terms,
            ("POST", "/v1/collections/{id}/publish") => Op::Publish,
            ("GET", "/v1/collections/{id}/approvalData") => Op::ApprovalData,
            ("GET", "/v1/thirdParties") => Op::ThirdParties,
            ("GET", "/v1/thirdParties/{id}") => Op::ThirdPartyRead,
            ("GET", "/v1/thirdParties/{id}/slots") => Op::ThirdPartySlots,
            ("DELETE", "/v1/thirdParties/{id}") => Op::ThirdPartyDelete,
            ("PATCH", "/v1/thirdParties/{id}") => Op::ThirdPartyUpdate,
            ("GET", "/v1/curations") => Op::Curations,
            ("GET", "/v1/collections/{id}/curation") => Op::CollectionCuration,
            ("PATCH", "/v1/collections/{id}/curation") => Op::CollectionCurationUpdate,
            ("POST", "/v1/collections/{id}/curation") => Op::CollectionCurationRequest,
            ("GET", "/v1/collections/{id}/itemCurations") => Op::CollectionItemCurations,
            ("GET", "/v1/items/{id}/curation") => Op::ItemCuration,
            ("PATCH", "/v1/items/{id}/curation") => Op::ItemCurationUpdate,
            ("POST", "/v1/items/{id}/curation") => Op::ItemCurationRequest,
            ("POST", "/v1/collections/{id}/post") => Op::ForumPost,
            ("POST", "/v1/collections/{id}/curation/post") => Op::ForumReply,
            ("POST", "/v1/validator/claims") => Op::ValidatorClaim,
            ("POST", "/v1/validator/claims/{id}/lease") => Op::ValidatorLease,
            ("POST", "/v1/collections/{id}/validation-result") => Op::ValidationResult,
            ("POST", "/v1/collections/{id}/validations") => Op::ReviewRetry,
            ("POST", "/v1/collections/{id}/curation/appeal") => Op::ReviewAppeal,
            ("GET", "/v1/collections/{id}/events") => Op::ReviewEvents,
            ("POST", "/v1/collections/{id}/comments") => Op::ReviewComment,
            _ => return None,
        })
    }

    fn publication(self) -> bool {
        matches!(
            self,
            Op::PublicationPrepare
                | Op::PublicationBegin
                | Op::PublicationTransaction
                | Op::PublicationCancel
                | Op::PublicationClaim
                | Op::PublicationStatus
        )
    }

    fn small_read(self) -> bool {
        matches!(
            self,
            Op::Info | Op::Rarities | Op::Rarity | Op::Utility | Op::ItemContents | Op::Addresses
        )
    }

    fn by_contract(self) -> bool {
        matches!(self, Op::Utility | Op::ItemContents)
    }

    /// A route only the wearable validator may call.
    fn validator(self) -> bool {
        matches!(
            self,
            Op::ValidatorClaim | Op::ValidatorLease | Op::ValidationResult
        )
    }

    /// A review-flow route; the matrix's worlds have the review flow on.
    fn review(self) -> bool {
        matches!(
            self,
            Op::ReviewRetry | Op::ReviewAppeal | Op::ReviewEvents | Op::ReviewComment
        )
    }

    fn kinds(self) -> &'static [Kind] {
        if self == Op::ValidationResult {
            &[Missing, Malformed, Published]
        } else if self.by_contract() {
            &[
                Missing,
                Malformed,
                Draft,
                Published,
                Approved,
                Unminted,
                Transferred,
                ThirdParty,
            ]
        } else if self.by_id() {
            &KINDS
        } else if self.publication() {
            &[Missing, Draft]
        } else {
            &[Missing]
        }
    }

    fn by_id(self) -> bool {
        matches!(
            self,
            Op::ReadCollection
                | Op::Lock
                | Op::DeleteCollection
                | Op::OwnerSync
                | Op::Terms
                | Op::Publish
                | Op::ApprovalData
                | Op::SaveCollection
                | Op::CollectionItems
                | Op::ReadItem
                | Op::SaveItem
                | Op::DeleteItem
                | Op::UploadFiles
                | Op::UploadVideo
                | Op::CollectionCuration
                | Op::CollectionItemCurations
                | Op::CollectionCurationUpdate
                | Op::CollectionCurationRequest
                | Op::ItemCuration
                | Op::ItemCurationUpdate
                | Op::ItemCurationRequest
                | Op::ForumPost
                | Op::ForumReply
                | Op::ReviewRetry
                | Op::ReviewAppeal
                | Op::ReviewEvents
                | Op::ReviewComment
        )
    }

    /// A curation route: its cells read the curation a kind carries.
    fn curation(self) -> bool {
        matches!(
            self,
            Op::Curations
                | Op::CollectionCuration
                | Op::CollectionItemCurations
                | Op::CollectionCurationUpdate
                | Op::CollectionCurationRequest
                | Op::ItemCuration
                | Op::ItemCurationUpdate
                | Op::ItemCurationRequest
        )
    }

    fn writes(self) -> bool {
        matches!(
            self,
            Op::SaveCollection
                | Op::Lock
                | Op::DeleteCollection
                | Op::Terms
                | Op::Publish
                | Op::SaveItem
                | Op::DeleteItem
                | Op::UploadFiles
                | Op::UploadVideo
                | Op::PublicationBegin
                | Op::PublicationTransaction
                | Op::PublicationCancel
                | Op::PublicationClaim
                | Op::ItemStatus
                | Op::ItemsStatus
                | Op::ThirdPartyDelete
                | Op::ThirdPartyUpdate
                | Op::CollectionCurationUpdate
                | Op::CollectionCurationRequest
                | Op::ItemCurationUpdate
                | Op::ItemCurationRequest
                | Op::ForumPost
                | Op::ForumReply
                | Op::ValidatorClaim
                | Op::ValidatorLease
                | Op::ValidationResult
                | Op::ReviewRetry
                | Op::ReviewAppeal
                | Op::ReviewComment
        )
    }

    fn third_party_record(self) -> bool {
        matches!(
            self,
            Op::ThirdParties
                | Op::ThirdPartyRead
                | Op::ThirdPartySlots
                | Op::ThirdPartyDelete
                | Op::ThirdPartyUpdate
        )
    }

    fn on_items(self) -> bool {
        matches!(
            self,
            Op::ReadItem
                | Op::SaveItem
                | Op::DeleteItem
                | Op::UploadFiles
                | Op::UploadVideo
                | Op::ItemCuration
                | Op::ItemCurationUpdate
                | Op::ItemCurationRequest
        )
    }

    fn request(self, kind: Kind, f: &Fixture) -> (&'static str, String, Payload) {
        let owner = Owner.address();
        let cid = "bafkreigh2akiscaildcqabsyg3dfr6chu3fgpregiymsck7e7aqa4s52zy";
        match self {
            Op::ReadCollection => (
                "GET",
                format!("/v1/collections/{}", f.collection_id(kind)),
                Payload::None,
            ),
            Op::SaveCollection => {
                let id = f.collection_id(kind);
                let mut collection = json!({ "id": id, "name": "Matrix renamed" });
                if let Some(urn) = &f.urn {
                    collection["urn"] = urn.clone().into();
                }
                (
                    "PUT",
                    format!("/v1/collections/{id}"),
                    Payload::Json(json!({ "collection": collection })),
                )
            }
            Op::CollectionItems => (
                "GET",
                format!("/v1/collections/{}/items", f.collection_id(kind)),
                Payload::None,
            ),
            Op::Lock | Op::OwnerSync | Op::Terms | Op::Publish => {
                let (route, payload) = match self {
                    Op::Lock => ("lock", Payload::None),
                    Op::OwnerSync => ("owner/sync", Payload::None),
                    Op::Terms => (
                        "tos",
                        Payload::Json(json!({ "email": "jane.doe@example.com" })),
                    ),
                    _ => ("publish", Payload::None),
                };
                (
                    "POST",
                    format!("/v1/collections/{}/{route}", f.collection_id(kind)),
                    payload,
                )
            }
            Op::DeleteCollection => (
                "DELETE",
                format!("/v1/collections/{}", f.collection_id(kind)),
                Payload::None,
            ),
            Op::ApprovalData => (
                "GET",
                format!("/v1/collections/{}/approvalData", f.collection_id(kind)),
                Payload::None,
            ),
            Op::ValidatorClaim => (
                "POST",
                "/v1/validator/claims".into(),
                Payload::Json(json!({})),
            ),
            Op::ValidatorLease => (
                "POST",
                format!("/v1/validator/claims/{}/lease", Uuid::new_v4()),
                Payload::Json(json!({ "claimId": Uuid::new_v4() })),
            ),
            Op::ValidationResult => {
                let collection = f.collection_id(kind);
                (
                    "POST",
                    format!("/v1/collections/{collection}/validation-result"),
                    Payload::Json(json!({
                        "validationId": Uuid::new_v4(), "collectionId": collection, "verdict": "passed",
                        "rulesVersion": "1.0.0", "items": [], "claimId": Uuid::new_v4(),
                    })),
                )
            }
            Op::ReviewRetry | Op::ReviewAppeal | Op::ReviewEvents | Op::ReviewComment => {
                let (method, route, payload) = match self {
                    Op::ReviewRetry => ("POST", "validations", Payload::None),
                    Op::ReviewAppeal => (
                        "POST",
                        "curation/appeal",
                        Payload::Json(json!({ "note": "Matrix" })),
                    ),
                    Op::ReviewEvents => ("GET", "events", Payload::None),
                    _ => (
                        "POST",
                        "comments",
                        Payload::Json(json!({ "message": "Matrix" })),
                    ),
                };
                (
                    method,
                    format!("/v1/collections/{}/{route}", f.collection_id(kind)),
                    payload,
                )
            }
            Op::ForumPost => (
                "POST",
                format!("/v1/collections/{}/post", f.collection_id(kind)),
                Payload::Json(json!({ "forumPost": { "title": "t", "raw": "r" } })),
            ),
            Op::ForumReply => (
                "POST",
                format!("/v1/collections/{}/curation/post", f.collection_id(kind)),
                Payload::Json(
                    json!({ "forumPost": { "raw": "The collection has been unassigned." } }),
                ),
            ),
            Op::ReadItem => (
                "GET",
                format!("/v1/items/{}", f.item_id(kind)),
                Payload::None,
            ),
            Op::SaveItem => {
                let id = f.item_id(kind);
                let collection = if kind == Missing { None } else { f.collection };
                let urn = f.urn.as_deref().map(item_urn);
                (
                    "PUT",
                    format!("/v1/items/{id}"),
                    Payload::Json(item_json(&id, collection, urn)),
                )
            }
            Op::DeleteItem => (
                "DELETE",
                format!("/v1/items/{}", f.item_id(kind)),
                Payload::None,
            ),
            Op::UploadFiles => (
                "POST",
                format!("/v1/items/{}/files", f.item_id(kind)),
                Payload::Upload,
            ),
            Op::UploadVideo => (
                "POST",
                format!("/v1/items/{}/videos", f.item_id(kind)),
                Payload::Video,
            ),
            Op::ListCollections => ("GET", "/v1/collections".into(), Payload::None),
            Op::ListItems => ("GET", "/v1/items".into(), Payload::None),
            Op::AddressCollections => ("GET", format!("/v1/{owner}/collections"), Payload::None),
            Op::AddressItems => ("GET", format!("/v1/{owner}/items"), Payload::None),
            Op::Committee => ("GET", "/v1/committee".into(), Payload::None),
            Op::ThirdParties => ("GET", "/v1/thirdParties".into(), Payload::None),
            Op::ThirdPartyRead => ("GET", format!("/v1/thirdParties/{PROVIDER}"), Payload::None),
            Op::ThirdPartySlots => (
                "GET",
                format!("/v1/thirdParties/{PROVIDER}/slots"),
                Payload::None,
            ),
            Op::ThirdPartyDelete => (
                "DELETE",
                format!("/v1/thirdParties/{PROVIDER}"),
                Payload::None,
            ),
            Op::ThirdPartyUpdate => (
                "PATCH",
                format!("/v1/thirdParties/{PROVIDER}"),
                Payload::Json(json!({ "isProgrammatic": false })),
            ),
            Op::StorageGet => ("GET", format!("/v1/storage/contents/{cid}"), Payload::None),
            Op::StorageHead => ("HEAD", format!("/v1/storage/contents/{cid}"), Payload::None),
            Op::StorageExists => (
                "GET",
                format!("/v1/storage/contents/{cid}/exists"),
                Payload::None,
            ),
            Op::Newsletter => ("POST", "/v1/newsletter".into(), Payload::Json(json!({}))),
            Op::DraftCollections => ("GET", "/v1/drafts/collections".into(), Payload::None),
            Op::DraftItems => ("GET", "/v1/drafts/items".into(), Payload::None),
            Op::CurationQueue => ("GET", "/v1/collections/curation".into(), Payload::None),
            Op::Curations => ("GET", "/v1/curations".into(), Payload::None),
            Op::CollectionCuration
            | Op::CollectionItemCurations
            | Op::CollectionCurationUpdate
            | Op::CollectionCurationRequest => {
                let (method, route) = match self {
                    Op::CollectionCuration => ("GET", "curation"),
                    Op::CollectionItemCurations => ("GET", "itemCurations"),
                    Op::CollectionCurationUpdate => ("PATCH", "curation"),
                    _ => ("POST", "curation"),
                };
                (
                    method,
                    format!("/v1/collections/{}/{route}", f.collection_id(kind)),
                    approval(method),
                )
            }
            Op::ItemCuration | Op::ItemCurationUpdate | Op::ItemCurationRequest => {
                let method = match self {
                    Op::ItemCuration => "GET",
                    Op::ItemCurationUpdate => "PATCH",
                    _ => "POST",
                };
                let body = match approval(method) {
                    Payload::Json(mut body) => {
                        body["curation"]["contentHash"] = json!("ab".repeat(32));
                        Payload::Json(body)
                    }
                    other => other,
                };
                (
                    method,
                    format!("/v1/items/{}/curation", f.item_id(kind)),
                    body,
                )
            }
            Op::Ping => ("GET", "/ping".into(), Payload::None),
            Op::Info => ("GET", "/v1/info".into(), Payload::None),
            Op::Rarities => ("GET", "/v1/rarities".into(), Payload::None),
            Op::Rarity => ("GET", "/v1/rarities/common".into(), Payload::None),
            Op::Addresses => ("GET", "/v1/addresses?tag=matrix".into(), Payload::None),
            Op::Utility => {
                let (contract, id) = f.by_contract(kind);
                (
                    "GET",
                    format!("/v1/published-collections/{contract}/items/{id}/utility"),
                    Payload::None,
                )
            }
            Op::ItemContents => {
                let (contract, id) = f.by_contract(kind);
                (
                    "GET",
                    format!("/v1/items/{contract}/{id}/contents"),
                    Payload::None,
                )
            }
            _ => {
                let c = f.collection_id(kind);
                let revision = Payload::Json(json!({ "revision": "r" }));
                let (method, path, payload) = match self {
                    Op::PublicationPrepare => ("GET", "publication", Payload::None),
                    Op::PublicationBegin => ("POST", "publication", revision),
                    Op::PublicationTransaction => (
                        "PUT",
                        "publication",
                        Payload::Json(json!({ "tx_hash": format!("0x{}", "1".repeat(64)) })),
                    ),
                    Op::PublicationCancel => ("DELETE", "publication", Payload::None),
                    Op::PublicationClaim => ("PATCH", "publication", revision),
                    Op::PublicationStatus => ("GET", "publication/status", Payload::None),
                    Op::ItemStatus => (
                        "PATCH",
                        "items/{item}/status",
                        Payload::Json(json!({ "status": "approved" })),
                    ),
                    _ => (
                        "PATCH",
                        "items/status",
                        Payload::Json(
                            json!({ "status": "approved", "itemIds": [f.item_id(kind)] }),
                        ),
                    ),
                };
                let path = path.replace("{item}", &f.item_id(kind));
                (method, format!("/v1/collections/{c}/{path}"), payload)
            }
        }
    }

    /// The decision table; `None` where the cell doesn't apply.
    fn expect(self, kind: Kind, caller: Caller, f: &Fixture) -> Option<Expect> {
        if self.validator() {
            return match (self, caller) {
                (Op::ValidatorLease, Validator) => refused(409, "The claim is not current"),
                (_, Validator) => Some(Expect(204, String::new())),
                _ => refused(401, "Unauthorized"),
            };
        }
        let caller = match caller {
            Validator => Anonymous,
            caller => caller,
        };
        if self.curation() {
            return self.expect_curation(kind, caller, f);
        }
        if self.review() {
            return self.expect_review(kind, caller, f);
        }
        let kind = kind.base();
        let signed = caller != Anonymous;
        if self == Op::Ping {
            return ok();
        }
        if self.small_read() {
            let minted = matches!(kind, Published | Approved | Transferred);
            return match self {
                Op::Utility if !minted => refused(404, "The item doesn't exist."),
                Op::ItemContents if kind == Malformed => {
                    refused(400, "Invalid address not-an-address")
                }
                _ => ok(),
            };
        }
        if !signed
            && !matches!(
                self,
                Op::ListCollections
                    | Op::Committee
                    | Op::StorageGet
                    | Op::StorageHead
                    | Op::StorageExists
                    | Op::Newsletter
            )
        {
            return unauthenticated();
        }
        if self.publication() {
            let collection = f.collection_id(kind);
            return match kind {
                Missing => not_found(&collection, "collections"),
                _ if caller == Owner && self == Op::PublicationTransaction => refused(
                    404,
                    "Prepare this collection before submitting its transaction",
                ),
                _ if caller == Owner => admitted(),
                _ => unauthorized_user(&caller.address(), "collections", &collection),
            };
        }
        if self.third_party_record() {
            let not_manager = "You are not the manager of this Third Party.";
            return match self {
                Op::ThirdParties => ok(),
                Op::ThirdPartyRead if caller == Owner => ok(),
                Op::ThirdPartyRead => refused(404, "The Third Party doesn't exists."),
                Op::ThirdPartySlots if caller == Owner => ok(),
                Op::ThirdPartySlots => refused(
                    403,
                    "Unauthorized access. Account is not manager of the collection",
                ),
                Op::ThirdPartyDelete if caller == Owner => refused(
                    400,
                    "The Third Party can only be deleted if it's already on the graph.",
                ),
                Op::ThirdPartyUpdate if caller == Owner => ok(),
                _ => refused(403, not_manager),
            };
        }
        match self {
            Op::DraftCollections | Op::DraftItems => return ok(),
            Op::CurationQueue if caller == Admin => return ok(),
            Op::ItemStatus | Op::ItemsStatus if caller == Admin => {
                return match kind {
                    Missing | Malformed | Orphan => refused(404, "Not found"),
                    _ if !kind.published() => refused(409, "Unpublished collection"),
                    _ if self == Op::ItemStatus && kind != ThirdPartySubmitted => {
                        refused(404, "Not found")
                    }
                    _ => ok(),
                };
            }
            Op::CurationQueue | Op::ItemStatus | Op::ItemsStatus => return refused(403, NOT_ADMIN),
            _ => {}
        }
        if !self.by_id() {
            return match self {
                Op::ListCollections if caller == Committee => ok(),
                Op::ListCollections if !signed => refused(401, "Unauthorized"),
                Op::ListCollections => unauthorized(),
                Op::ListItems | Op::AddressCollections | Op::AddressItems if !signed => {
                    unauthenticated()
                }
                Op::ListItems if caller == Committee => ok(),
                Op::AddressCollections | Op::AddressItems if caller == Owner => ok(),
                Op::ListItems | Op::AddressCollections | Op::AddressItems => unauthorized(),
                Op::Committee => ok(),
                Op::StorageGet => refused(404, "Not Found"),
                Op::StorageHead | Op::StorageExists => refused(404, ""),
                _ => refused(400, "email is required"),
            };
        }
        if !self.on_items() && matches!(kind, Orphan | Unminted) {
            return None;
        }
        if !signed {
            return unauthenticated();
        }
        let collection = f.collection_id(kind);
        let item = f.item_id(kind);
        let committee = caller == Committee;
        let owner = kind.owner(caller);
        let manager = kind.manager(caller);
        if self == Op::ForumReply {
            return match kind {
                _ if !committee => unauthorized(),
                Missing => refused(404, "Collection not found"),
                Malformed => invalid_uuid(&collection),
                _ => refused(409, "The collection does not have a forum post yet"),
            };
        }
        match self {
            Op::ReadCollection => match kind {
                Missing => not_found(&collection, "collections"),
                Malformed => invalid_uuid(&collection),
                _ if kind.published() || owner || committee => ok(),
                _ => unauthorized(),
            },
            Op::SaveCollection => match kind {
                Missing => ok(),
                Malformed => None,
                _ if !owner => refused(403, "Unauthorized to upsert collection"),
                ThirdPartySubmitted => ok(),
                _ if kind.published() => collection_published("updated"),
                Publishing | Locked => collection_locked("updated"),
                _ => ok(),
            },
            Op::Lock
            | Op::DeleteCollection
            | Op::OwnerSync
            | Op::Terms
            | Op::Publish
            | Op::ApprovalData
            | Op::ForumPost
                if matches!(kind, Missing | Malformed) =>
            {
                match kind {
                    Missing => not_found(&collection, "collections"),
                    _ => invalid_uuid(&collection),
                }
            }
            Op::Lock if owner && !kind.third_party() => ok(),
            Op::Lock => unauthorized_user(&caller.address(), "collections", &collection),
            Op::DeleteCollection | Op::Terms | Op::ForumPost if !owner && !manager => {
                unauthorized_user(&caller.address(), "collections", &collection)
            }
            Op::ForumPost if kind.published() || kind.third_party() => ok(),
            Op::ForumPost => refused(409, "The collection is not published"),
            Op::ApprovalData if !committee => unauthorized(),
            Op::ApprovalData if kind.third_party() => refused(409, NOT_PUBLISHED),
            Op::ApprovalData => refused(409, "Collection is not Third Party"),
            Op::OwnerSync | Op::Publish if kind.third_party() && !owner => {
                unauthorized_user(&caller.address(), "collections", &collection)
            }
            Op::OwnerSync | Op::Publish if !owner && !manager && !committee => {
                unauthorized_user(&caller.address(), "collections", &collection)
            }
            Op::DeleteCollection => match kind {
                ThirdPartySubmitted => refused(
                    409,
                    "The third party collection already has published items. It can't be deleted.",
                ),
                _ if kind.published() => collection_published("deleted"),
                Publishing | Locked => collection_locked("deleted"),
                _ => ok(),
            },
            Op::Terms => ok(),
            Op::OwnerSync if kind.third_party() => {
                refused(400, "Third party collections have no on-chain owner")
            }
            Op::OwnerSync => ok(),
            Op::Publish if kind.third_party() => ok(),
            Op::Publish => refused(409, NOT_PUBLISHED),
            Op::CollectionItems => match kind {
                Missing => not_found(&collection, "collections"),
                Malformed => refused(
                    400,
                    format!("Invalid collection ID or contract address format: {collection}"),
                ),
                _ if kind.published() || owner || manager || committee => ok(),
                _ => unauthorized(),
            },
            Op::ReadItem => match kind {
                Missing => not_found(&item, "items"),
                Malformed => invalid_uuid(&item),
                Published | Approved | Transferred | ThirdPartySubmitted => ok(),
                _ if kind.item_owner(caller) || manager || committee => ok(),
                _ => unauthorized(),
            },
            Op::SaveItem => match kind {
                Missing => ok(),
                Malformed => None,
                Orphan if caller == Owner => ok(),
                Orphan => refused(403, "The user is unauthorized to upsert the collection."),
                _ if kind.third_party() && !owner => {
                    refused(403, "The user is unauthorized to upsert the collection.")
                }
                _ if !owner && !manager => {
                    refused(403, "You're not authorized to to change this collection.")
                }
                Publishing | Locked => item_locked("inserted or updated"),
                _ => ok(),
            },
            Op::DeleteItem => match kind {
                Missing => not_found(&item, "items"),
                Malformed => invalid_uuid(&item),
                Orphan if caller == Owner => ok(),
                _ if !owner && !manager => unauthorized_user(&caller.address(), "items", &item),
                ThirdPartySubmitted => third_party_item_published("deleted"),
                _ if kind.published() => item_published("deleted"),
                Publishing | Locked => item_locked("deleted"),
                _ => ok(),
            },
            Op::UploadFiles | Op::UploadVideo => match kind {
                Missing => not_found(&item, "items"),
                Malformed => invalid_uuid(&item),
                Orphan if caller == Owner => ok(),
                _ if !owner && !manager => unauthorized_user(&caller.address(), "items", &item),
                _ => ok(),
            },
            _ => None,
        }
    }

    /// The review routes' decisions: the collection, then a role on it (for
    /// the timeline and comments, or the committee); the timeline answers
    /// anyone else as an absent collection; then the review the kind
    /// carries.
    fn expect_review(self, kind: Kind, caller: Caller, f: &Fixture) -> Option<Expect> {
        if caller == Anonymous {
            return unauthenticated();
        }
        if matches!(kind, Orphan | Unminted) {
            return None;
        }
        let collection = f.collection_id(kind);
        match kind {
            Missing => return not_found(&collection, "collections"),
            Malformed => return invalid_uuid(&collection),
            _ => {}
        }
        let role = kind.owner(caller) || kind.manager(caller);
        let committee = caller == Committee;
        let not_published = "The collection is not published, so it has no review yet";
        match self {
            Op::ReviewEvents if role || committee => ok(),
            Op::ReviewEvents => not_found(&collection, "collections"),
            Op::ReviewComment if !role && !committee => unauthorized(),
            Op::ReviewComment if !kind.published() => refused(409, not_published),
            Op::ReviewComment if matches!(kind, ReviewPending | ReviewRejected) => ok(),
            Op::ReviewComment => refused(409, "The collection has no open review to comment on"),
            _ if !role => unauthorized_user(&caller.address(), "collections", &collection),
            Op::ReviewAppeal if kind == ReviewRejected => ok(),
            Op::ReviewAppeal => refused(409, "Only rejected collections can be appealed"),
            _ if !kind.published() => refused(409, not_published),
            _ if kind.third_party() => refused(
                409,
                "Only standard collections can be validated automatically",
            ),
            _ if kind == ReviewApproved => refused(
                409,
                "There is nothing to validate again: submit your changes for review",
            ),
            _ if kind == ReviewRejected => refused(
                409,
                "A curator rejected this collection: submit your changes for review instead of validating again",
            ),
            _ => ok(),
        }
    }

    /// The curation routes' decisions: the collection, then its
    /// publication, then a role (for an item's curation, the item's own
    /// owner instead of the collection's), then the committee; a write then
    /// follows the transition table with the curation the kind carries.
    fn expect_curation(self, kind: Kind, caller: Caller, f: &Fixture) -> Option<Expect> {
        if caller == Anonymous {
            return unauthenticated();
        }
        if self == Op::Curations {
            return ok();
        }
        let on_item = self.on_items();
        if !on_item && matches!(kind, Orphan | Unminted) {
            return None;
        }
        let id = if on_item {
            f.item_id(kind)
        } else {
            f.collection_id(kind)
        };
        match kind {
            Missing | Orphan => {
                return refused(404, "Not found");
            }
            Malformed => return invalid_uuid(&id),
            _ if !kind.published() => return refused(409, "Unpublished collection"),
            _ => {}
        }
        let role = if on_item {
            kind.item_owner(caller) || kind.manager(caller)
        } else {
            kind.owner(caller) || kind.manager(caller)
        };
        let committee = caller == Committee;
        if !role && !committee {
            return unauthorized();
        }
        let item_curation = kind.base() == ThirdPartySubmitted;
        let collection_curation = matches!(kind, ReviewPending | ReviewApproved | ReviewRejected);
        let (curated, pending) = if on_item {
            (
                item_curation,
                matches!(kind, ThirdPartySubmitted | CuratedPending),
            )
        } else {
            (collection_curation, kind == ReviewPending)
        };
        match self {
            Op::CollectionCurationUpdate | Op::ItemCurationUpdate if !committee => refused(
                403,
                "Only committee members can approve or reject a curation",
            ),
            Op::CollectionCurationUpdate | Op::ItemCurationUpdate if !curated => {
                refused(404, "Curation does not exist")
            }
            Op::ItemCurationRequest if !curated => refused(
                400,
                "Item curations can't be created for items that weren't curated before",
            ),
            Op::CollectionCurationRequest | Op::ItemCurationRequest if pending => {
                refused(400, "There is already an ongoing review request")
            }
            _ => ok(),
        }
    }
}

/// A curation write's body: a committee approval for a `PATCH`, none
/// otherwise.
fn approval(method: &str) -> Payload {
    match method {
        "PATCH" => Payload::Json(json!({ "curation": { "status": "approved" } })),
        _ => Payload::None,
    }
}

struct Matrix {
    pool: PgPool,
    app: Router,
    state: crate::SharedState,
    forum: crate::curation::forum::fake::FakeForum,
    sibling: Uuid,
}

/// Brings `m`'s database back to the rows [`matrix_with`] seeded: the
/// sibling collection and its item. Rows outside [`snapshot`]'s tables stay.
async fn reset(m: &Matrix) {
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DELETE FROM builder.slot_usage_cheques; \
         DELETE FROM builder.items WHERE collection_id IS NULL; \
         DELETE FROM builder.collections WHERE id <> '{}'; \
         DELETE FROM builder.uploaded_contents; \
         DELETE FROM shop.collections",
        m.sibling
    )))
    .execute(&m.pool)
    .await
    .unwrap();
}

async fn matrix(committee: &FakeCommittee) -> Option<Matrix> {
    matrix_with(committee, 8, true).await
}

async fn matrix_with(committee: &FakeCommittee, connections: u32, rpc: bool) -> Option<Matrix> {
    let pool = super::cells::world_pool(connections).await?;
    fresh_pass(&pool, 0).await;
    sqlx::query(
        "INSERT INTO explore_api.feature_flags (app, document, fetched_at) \
         VALUES ('builder', '{\"flags\":{\"builder-auto-curation\":true,\"builder-auto-curation-review-flow\":true},\"variants\":{}}', timezone('UTC', now())) \
         ON CONFLICT (app) DO NOTHING",
    )
    .execute(&pool)
    .await
    .unwrap();
    committee.set_rarities(Some(&[("common", "10000000000000000000", "100000")]));
    let oracle = FakeOracle::with(&[("common", "4000000000000000000")]);
    let forum = crate::curation::forum::fake::FakeForum::default();
    let mut state = super::support::state(
        pool.clone(),
        Some(committee_index(committee).await),
        crate::Settings {
            writes_enabled: true,
            polygon_rpc_url: if rpc { Some(oracle.url().await) } else { None },
            forum: Some(crate::curation::forum::fake::FakeForum::forum(
                &forum.start().await,
            )),
            validator_secret: Some(VALIDATOR_SECRET.into()),
            admin_addresses: vec![Admin.address()],
            ..crate::Settings::default()
        },
    )
    .await;
    state.validator_calls =
        crate::shared::limits::RateLimiter::new(1_000_000, std::time::Duration::from_secs(60));
    state.limits = crate::limits::Limits::unbounded();
    let shared = Arc::new(state);
    let app = super::support::mounted(&shared);
    sqlx::query(
        "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, 'tp:1:Matrix:')",
    )
    .bind(PROVIDER)
    .bind(vec![Owner.address()])
    .execute(&pool)
    .await
    .unwrap();
    let sibling = sibling_collection(&pool).await;
    sqlx::query(
        "UPDATE builder.items SET data = '{\"tags\":[\"matrix\"]}'::json WHERE blockchain_item_id IS NOT NULL",
    )
    .execute(&pool)
    .await
    .unwrap();
    Some(Matrix {
        pool,
        app,
        state: shared,
        forum,
        sibling,
    })
}

/// A third-party collection under the matrix's provider that the sibling
/// wallet created, with one item: the provider's virtual third party lists
/// the owner, so the sibling manages nothing of the provider, its own
/// collection included.
async fn sibling_collection(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    let urn = format!("{PROVIDER}:sibling-{}", id.simple());
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) \
         VALUES ($1, 'Sibling', $2, $3, $4)",
    )
    .bind(id)
    .bind(Sibling.address())
    .bind(PROVIDER)
    .bind(&urn)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix) \
         VALUES ($1, $2, $3, 'Sibling item', 'wearable', $4)",
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind(Sibling.address())
    .bind(item_urn(&urn))
    .execute(pool)
    .await
    .unwrap();
    id
}

fn ops() -> Vec<Op> {
    let routes = router_routes();
    let upstream = UPSTREAM
        .iter()
        .filter(|(_, _, route)| *route == Route::Routed);
    assert_eq!(
        routes
            .iter()
            .filter(|(_, path)| path.starts_with("/v1"))
            .count(),
        routes.len() - 1,
        "every route but /ping is under /v1: {routes:?}"
    );
    assert!(
        upstream.count() <= routes.len(),
        "the router declares fewer routes than the ledger marks routed: {routes:?}"
    );
    routes
        .iter()
        .map(|(method, path)| {
            Op::of(method, path)
                .unwrap_or_else(|| panic!("{method} {path} is routed and has no matrix row"))
        })
        .collect()
}

/// Names a forum post's collection after its id, so no two posts of one
/// world share a topic title.
async fn distinct_topic(pool: &PgPool, op: Op, f: &Fixture) {
    if op == Op::ForumPost
        && let Some(id) = f.collection
    {
        sqlx::query("UPDATE builder.collections SET name = $2 WHERE id = $1")
            .bind(id)
            .bind(id.simple().to_string())
            .execute(pool)
            .await
            .unwrap();
    }
}

/// Runs `c` on `m` ([`Cell`]), audited under `condition`; its failure.
async fn cell(m: &Matrix, condition: &str, c: Cell, committee_up: bool) -> Option<String> {
    let Cell {
        op,
        kind,
        caller,
        mount,
        expect,
    } = c;
    if op == Op::Ping && mount.is_empty() || expect(&Fixture::default()).is_none() {
        return None;
    }
    let f = fixture(&m.pool, kind).await;
    let expected = expect(&f)?;
    distinct_topic(&m.pool, op, &f).await;
    let (method, path, payload) = op.request(kind, &f);
    let label = kind;
    let kind = kind.base();
    let write_checked = op.writes() && !matches!(expected.0, 0 | 200);
    let before = match write_checked {
        true => Some(snapshot(&m.pool).await),
        false => None,
    };
    let (status, body) = call(&m.app, caller, method, &format!("{mount}{path}"), payload).await;
    let error = body["error"].as_str().unwrap_or_default().to_string();
    let got = Expect(
        status.as_u16(),
        if status.is_success() {
            String::new()
        } else {
            error
        },
    );
    let mut problems = Vec::new();
    let refusal = matches!(got.0, 401 | 403 | 404) || got.0 >= 500;
    if (expected.0 == 0 && refusal) || (expected.0 != 0 && got != expected) {
        problems.push(format!("answered {got:?}, expected {expected:?}"));
    }
    if write_checked && Some(snapshot(&m.pool).await) != before {
        problems.push("a refused write changed rows".to_string());
    }
    let sees_all = (op == Op::ReadItem && kind.item_owner(caller))
        || kind.owner(caller)
        || kind.manager(caller)
        || (caller == Minter && kind.on_chain())
        || (committee_up && caller == Committee);
    let items: Vec<&Value> = match op {
        Op::ReadItem if status.is_success() => vec![&body["data"]],
        Op::CollectionItems if status.is_success() => body["data"]
            .as_array()
            .map(|a| a.iter().collect())
            .unwrap_or_default(),
        _ => vec![],
    };
    let allowed = match op {
        Op::ReadItem | Op::CollectionItems if kind.third_party() => {
            let mut keys = upstream_keys("161-item-linked-owner");
            if !sees_all {
                keys.remove("local_content_hash");
            }
            keys
        }
        Op::ReadItem | Op::CollectionItems if sees_all => upstream_keys("37-item-published-owner"),
        Op::ReadItem | Op::CollectionItems => upstream_keys("36-item-published-stranger"),
        _ => collection_keys(sees_all, kind.third_party()),
    };
    let bodies: Vec<&Value> = match op {
        Op::ReadCollection if status.is_success() => vec![&body["data"]],
        _ => items.clone(),
    };
    let bodies_checked = bodies.len();
    let items_checked = items.len();
    for answered in bodies {
        let extra: Vec<&String> = answered
            .as_object()
            .map(|fields| fields.keys().filter(|k| !allowed.contains(*k)).collect())
            .unwrap_or_default();
        if !extra.is_empty() {
            problems.push(format!("keys upstream doesn't send this caller: {extra:?}"));
        }
        if op == Op::ReadCollection {
            let missing: Vec<&String> = allowed
                .iter()
                .filter(|k| answered.get(k.as_str()).is_none())
                .collect();
            if !missing.is_empty() {
                problems.push(format!(
                    "keys upstream sends this caller missing: {missing:?}"
                ));
            }
        }
    }
    for item in items {
        if item.get("local_content_hash").is_some() != sees_all {
            problems.push(format!(
                "local_content_hash {} for a caller who {} sees the full item",
                if sees_all { "missing" } else { "sent" },
                if sees_all { "" } else { "never" }
            ));
        }
    }
    let listing_checked = op == Op::CollectionItems && status.is_success();
    if listing_checked {
        let listed: Vec<String> = body["data"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|i| i["id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let full = kind.owner(caller)
            || kind.manager(caller)
            || (caller == Minter && kind.on_chain())
            || (committee_up && caller == Committee);
        let expected: Vec<String> = match (full, f.minted) {
            (false, Some(minted)) => vec![minted.to_string()],
            (false, None) if kind == ThirdPartySubmitted => {
                f.item.iter().map(Uuid::to_string).collect()
            }
            (false, None) => vec![],
            (true, _) => {
                let mut all: Vec<String> = sqlx::query_scalar(
                    "SELECT id::text FROM builder.items WHERE collection_id = $1 ORDER BY created_at, id",
                )
                .bind(f.collection)
                .fetch_all(&m.pool)
                .await
                .unwrap();
                all.sort();
                all
            }
        };
        let mut listed = listed;
        listed.sort();
        if listed != expected {
            problems.push(format!("listed {listed:?}, expected {expected:?}"));
        }
    }
    super::cells::record(|| {
        anonymized(
            format!(
                "{condition}\t{op:?}\t{label:?}\t{caller:?}\t{mount}\t{expected:?}\t{got:?}\twrite={write_checked} bodies={bodies_checked} items={items_checked} listing={listing_checked} problems={}",
                problems.len()
            ),
            &f,
        )
    });
    (!problems.is_empty()).then(|| {
        format!(
            "{mount}{method} {path} {label:?} as {caller:?}: {}",
            problems.join("; ")
        )
    })
}

/// The state every world of a matrix test starts in.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Condition {
    Normal,
    Stale,
    NoPass,
    CommitteeDown,
    RaritiesDown,
    NoVerifier,
}

/// A matrix world in `condition`.
async fn matrix_in(condition: Condition) -> Option<Matrix> {
    let committee = match condition {
        Condition::CommitteeDown => FakeCommittee::down(),
        _ => FakeCommittee::with(&[&Committee.address()]),
    };
    let m = matrix_with(&committee, 4, condition != Condition::NoVerifier).await?;
    match condition {
        Condition::Stale => fresh_pass(&m.pool, 31).await,
        Condition::NoPass => {
            sqlx::query("DELETE FROM shop.mirror_state")
                .execute(&m.pool)
                .await
                .unwrap();
        }
        Condition::RaritiesDown => committee.set_rarities(None),
        _ => {}
    }
    Some(m)
}

/// What a cell answers on its fixture; `None` where the cell doesn't apply.
type Expectation = Box<dyn Fn(&Fixture) -> Option<Expect> + Send + Sync>;

/// One cell of a matrix test: `op` on a resource in `kind`, called by
/// `caller` under `mount`, answering what `expect` says.
struct Cell {
    op: Op,
    kind: Kind,
    caller: Caller,
    mount: &'static str,
    expect: Expectation,
}

impl Cell {
    fn new(
        op: Op,
        kind: Kind,
        caller: Caller,
        mount: &'static str,
        expect: impl Fn(&Fixture) -> Option<Expect> + Send + Sync + 'static,
    ) -> Self {
        Self {
            op,
            kind,
            caller,
            mount,
            expect: Box::new(expect),
        }
    }
}

/// Runs `cells` on matrix worlds in `condition` ([`cell`], audited under
/// `label`) and returns their failures, sorted; `None` (skip) without a
/// database.
async fn failures_in(
    condition: Condition,
    label: &'static str,
    committee_up: bool,
    cells: Vec<Cell>,
) -> Option<Vec<String>> {
    let mut failures = super::cells::in_worlds(
        cells,
        move || matrix_in(condition),
        move |m: Arc<Matrix>, c: Cell| async move {
            let failure = cell(&m, label, c, committee_up).await;
            reset(&m).await;
            failure
        },
    )
    .await?;
    failures.sort();
    Some(failures)
}

#[tokio::test(flavor = "multi_thread")]
async fn every_routed_operation_answers_each_caller_as_upstream_decides() {
    let mut cells = Vec::new();
    for op in ops() {
        for &kind in op.kinds() {
            for caller in CALLERS {
                for mount in MOUNTS {
                    cells.push(Cell::new(op, kind, caller, mount, move |f| {
                        op.expect(kind, caller, f)
                    }));
                }
            }
        }
    }
    let count = cells.len();
    let Some(failures) = failures_in(Condition::Normal, "every", true, cells).await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    assert!(count > 1000, "{count} cells");
    assert!(
        failures.is_empty(),
        "{} failing cells:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn unavailable(op: Op, kind: Kind, caller: Caller) -> Option<Expect> {
    match caller {
        Anonymous | Validator => unauthenticated(),
        _ if matches!(op, Op::ForumReply | Op::ApprovalData) => None,
        _ if op.by_id()
            && matches!(
                kind.base(),
                Draft | ThirdParty | ThirdPartySubmitted | Published
            ) =>
        {
            Some(Expect(503, String::new()))
        }
        _ => None,
    }
}

/// One cell of [`assert_cannot_tell`]: `op` on a fresh `kind` as `caller`
/// under `mount` answers `expected` (503 with the chain-state error) and
/// writes nothing.
async fn cannot_tell_cell(
    m: &Matrix,
    condition: &str,
    (op, kind, caller, mount, expected): (Op, Kind, Caller, &str, u16),
) -> Vec<String> {
    let f = fixture(&m.pool, kind).await;
    distinct_topic(&m.pool, op, &f).await;
    let (method, path, payload) = op.request(kind, &f);
    let before = snapshot(&m.pool).await;
    let (status, body) = call(&m.app, caller, method, &format!("{mount}{path}"), payload).await;
    let changed = snapshot(&m.pool).await != before;
    super::cells::record(|| {
        anonymized(
            format!(
                "{condition}\t{op:?}\t{kind:?}\t{caller:?}\t{mount}\t{expected}\t{} {}\twrite=true",
                status.as_u16(),
                body["error"]
            ),
            &f,
        )
    });
    let mut failures = Vec::new();
    if status.as_u16() != expected
        || (expected == 503
            && body["error"] != "Chain state is not available yet. Try again later.")
    {
        failures.push(format!(
            "{mount}{method} {path} as {caller:?}: {status} {body}"
        ));
    }
    if changed {
        failures.push(format!(
            "{mount}{method} {path} as {caller:?}: wrote while the copy can't tell"
        ));
    }
    failures
}

/// With the copy stale (`stale`) or without a complete pass, every by-id
/// decision answers 503, but a stored owner's read of their own draft
/// (standard, not on chain, or third-party) while the copy is only stale:
/// no chain fact can change that answer. `approvalData` reads no chain fact.
async fn assert_cannot_tell(stale: bool) {
    let mut cells = Vec::new();
    for op in ops().into_iter().filter(|op| op.by_id()) {
        for caller in CALLERS {
            for mount in MOUNTS {
                for kind in [
                    Draft,
                    ThirdParty,
                    Published,
                    CuratedPending,
                    CuratedApproved,
                    CuratedRejected,
                    ReviewPending,
                    ReviewApproved,
                    ReviewRejected,
                ] {
                    let Some(Expect(mut expected, _)) = unavailable(op, kind, caller) else {
                        continue;
                    };
                    if stale
                        && op == Op::ReadCollection
                        && caller == Owner
                        && kind.base() != Published
                    {
                        expected = 200;
                    }
                    cells.push((op, kind, caller, mount, expected));
                }
            }
        }
    }
    let (condition, label) = match stale {
        true => (Condition::Stale, "stale"),
        false => (Condition::NoPass, "no pass"),
    };
    let Some(mut failures) = super::cells::in_worlds(
        cells,
        move || matrix_in(condition),
        move |m: Arc<Matrix>, c| async move {
            let failures = cannot_tell_cell(&m, label, c).await;
            reset(&m).await;
            (!failures.is_empty()).then(|| failures.join("\n"))
        },
    )
    .await
    else {
        return;
    };
    failures.sort();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stale_copy_grants_nothing_and_writes_nothing() {
    assert_cannot_tell(true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_copy_without_a_complete_pass_grants_nothing_and_writes_nothing() {
    assert_cannot_tell(false).await;
}

/// With the committee source down, a decision the committee could change
/// answers 503, and so does the curation console's committee list; one it
/// can't change (an owner's, any write, and what any signer may see of
/// published content) answers as before, a committee member seeing a
/// published collection as any signer does.
#[tokio::test(flavor = "multi_thread")]
async fn a_committee_source_that_is_down_refuses_only_what_the_committee_decides() {
    let committee_unavailable = Expect(
        503,
        "The curation committee is not available. Try again later.".to_string(),
    );
    let mut cells = Vec::new();
    for op in ops()
        .into_iter()
        .filter(|op| !matches!(op, Op::AddressCollections | Op::AddressItems))
    {
        for &kind in op.kinds() {
            for caller in CALLERS {
                let decides = match op {
                    Op::ReadCollection => {
                        !matches!(kind, Missing | Malformed)
                            && !kind.published()
                            && !kind.owner(caller)
                    }
                    Op::CollectionItems => {
                        !matches!(kind, Missing | Malformed)
                            && !kind.published()
                            && !kind.owner(caller)
                            && !kind.manager(caller)
                    }
                    Op::ReadItem => {
                        !matches!(
                            kind.base(),
                            Missing
                                | Malformed
                                | Published
                                | Approved
                                | Transferred
                                | ThirdPartySubmitted
                        ) && !kind.item_owner(caller)
                            && !kind.manager(caller)
                    }
                    Op::OwnerSync | Op::Publish => {
                        !matches!(kind, Missing | Malformed)
                            && !kind.third_party()
                            && !kind.owner(caller)
                            && !kind.manager(caller)
                    }
                    Op::ApprovalData => !matches!(kind, Missing | Malformed),
                    Op::CurationQueue => matches!(caller, Admin),
                    Op::ListCollections
                    | Op::ListItems
                    | Op::Committee
                    | Op::Curations
                    | Op::ForumReply => true,
                    Op::CollectionCurationUpdate | Op::ItemCurationUpdate => kind.published(),
                    Op::CollectionCuration
                    | Op::CollectionItemCurations
                    | Op::CollectionCurationRequest => {
                        kind.published() && !kind.owner(caller) && !kind.manager(caller)
                    }
                    Op::ItemCuration | Op::ItemCurationRequest => {
                        kind.published() && !kind.item_owner(caller) && !kind.manager(caller)
                    }
                    Op::ReviewEvents => {
                        !matches!(kind, Malformed | Orphan | Unminted)
                            && !kind.owner(caller)
                            && !kind.manager(caller)
                    }
                    Op::ReviewComment => {
                        !matches!(kind, Missing | Malformed | Orphan | Unminted)
                            && !kind.owner(caller)
                            && !kind.manager(caller)
                    }
                    _ => false,
                };
                let committee_unavailable = committee_unavailable.clone();
                cells.push(Cell::new(op, kind, caller, "", move |f: &Fixture| {
                    let normal = op.expect(kind, caller, f)?;
                    if (decides && caller.key().is_some()) || op == Op::Committee {
                        Some(committee_unavailable.clone())
                    } else {
                        Some(normal)
                    }
                }));
            }
        }
    }
    let Some(failures) =
        failures_in(Condition::CommitteeDown, "committee down", false, cells).await
    else {
        return;
    };
    assert!(
        failures.is_empty(),
        "{} failing cells:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A save that would change a collection's kind or third party is a
/// conflict only for a signer who may save it; anyone else is refused
/// before the body's URN is compared, and nothing is written.
#[tokio::test]
async fn a_kind_conflict_is_answered_only_to_a_signer_with_access() {
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let Some(m) = matrix(&committee).await else {
        return;
    };
    for (kind, urn, expected) in [
        (
            Draft,
            format!("{PROVIDER}:converted"),
            "The collection can't be converted into a third party collection.",
        ),
        (
            ThirdParty,
            "urn:decentraland:matic:collections-thirdparty:other:moved".to_string(),
            "The third party id cannot be changed",
        ),
    ] {
        let f = fixture(&m.pool, kind).await;
        let id = f.collection.unwrap();
        let body = json!({ "collection": { "id": id, "name": "Matrix renamed", "urn": urn } });
        let before = snapshot(&m.pool).await;
        for (caller, status, error) in [
            (Stranger, 403, "Unauthorized to upsert collection"),
            (Committee, 403, "Unauthorized to upsert collection"),
            (Owner, 409, expected),
        ] {
            let (got, answer) = call(
                &m.app,
                caller,
                "PUT",
                &format!("/v1/collections/{id}"),
                Payload::Json(body.clone()),
            )
            .await;
            assert_eq!(
                (got.as_u16(), answer["error"].as_str()),
                (status, Some(error)),
                "{kind:?} as {caller:?}: {answer}"
            );
            assert!(
                caller != Stranger || answer["data"].get("third_party_id").is_none(),
                "{answer}"
            );
        }
        assert_eq!(snapshot(&m.pool).await, before);
    }
}

/// Every move of an existing item between collections that involves a
/// third-party collection, or into or out of one, is refused with
/// 409 before roles; moves among standard collections and
/// out of a standard collection follow the roles.
#[tokio::test]
async fn an_item_moves_into_or_out_of_a_third_party_collection_never() {
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let Some(m) = matrix(&committee).await else {
        return;
    };
    let cant_move = "Item can't change between collections.";
    for (from, to, caller, expected) in [
        (Orphan, Some(ThirdParty), Owner, Some(cant_move)),
        (Orphan, Some(ThirdParty), Stranger, Some(cant_move)),
        (ThirdParty, None, Owner, Some(cant_move)),
        (ThirdParty, Some(ThirdParty), Owner, Some(cant_move)),
        (Draft, Some(ThirdParty), Owner, Some(cant_move)),
        (ThirdParty, Some(Draft), Owner, Some(cant_move)),
        (Orphan, Some(Draft), Owner, None),
        (Draft, None, Owner, None),
        (Draft, Some(Draft), Owner, None),
    ] {
        let source = fixture(&m.pool, from).await;
        let target = match to {
            Some(kind) => fixture(&m.pool, kind).await.collection,
            None => None,
        };
        let id = source.item.unwrap();
        let before = snapshot(&m.pool).await;
        let (status, body) = call(
            &m.app,
            caller,
            "PUT",
            &format!("/v1/items/{id}"),
            Payload::Json(item_json(&id.to_string(), target, None)),
        )
        .await;
        let label = format!("{from:?} to {to:?} as {caller:?}: {status} {body}");
        match expected {
            Some(error) => {
                assert_eq!(
                    (status.as_u16(), &body["error"], &body["data"]),
                    (409, &json!(error), &json!({ "id": id })),
                    "{label}"
                );
                assert_eq!(snapshot(&m.pool).await, before, "{label}");
            }
            None => assert_eq!(status, StatusCode::OK, "{label}"),
        }
    }
}

/// A read the committee decides releases its database connection before it
/// reads the committee, so with a pool of one every caller's read
/// answers instead of waiting on a connection it holds itself.
#[tokio::test]
async fn a_committee_read_holds_no_database_connection() {
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let Some(m) = matrix_with(&committee, 1, true).await else {
        return;
    };
    for op in [Op::ReadCollection, Op::CollectionItems, Op::ReadItem] {
        let f = fixture(&m.pool, Draft).await;
        let (method, path, _) = op.request(Draft, &f);
        for (caller, expected) in [
            (Owner, StatusCode::OK),
            (Committee, StatusCode::OK),
            (Stranger, StatusCode::FORBIDDEN),
        ] {
            let answer = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                call(&m.app, caller, method, &path, Payload::None),
            )
            .await
            .unwrap_or_else(|_| panic!("{op:?} as {caller:?} never answered"));
            assert_eq!(answer.0, expected, "{op:?} as {caller:?}: {}", answer.1);
        }
    }
}

/// Two signers creating one new collection id at once: one creates it, the
/// other is refused as for an id that exists and isn't theirs.
#[tokio::test]
async fn the_loser_of_a_concurrent_creation_is_refused_as_a_stranger() {
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let Some(m) = matrix(&committee).await else {
        return;
    };
    let id = Uuid::new_v4();
    let mut winner = m.pool.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *winner)
        .await
        .unwrap();
    sqlx::query("INSERT INTO builder.collections (id, name, eth_address) VALUES ($1, 'First', $2)")
        .bind(id)
        .bind(Owner.address())
        .execute(&mut *winner)
        .await
        .unwrap();
    let app = m.app.clone();
    let loser = tokio::spawn(async move {
        call(
            &app,
            Stranger,
            "PUT",
            &format!("/v1/collections/{id}"),
            Payload::Json(json!({ "collection": { "id": id, "name": "Second" } })),
        )
        .await
    });
    let queued = crate::shared::test_wait::until(async || {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))",
        )
        .bind(blocker)
        .fetch_one(&m.pool)
        .await
        .unwrap()
    })
    .await;
    assert!(queued, "the second creation never waited on the first");
    winner.commit().await.unwrap();
    let (status, body) = loser.await.unwrap();
    assert_eq!(
        (status.as_u16(), &body["error"], &body["data"]),
        (
            403,
            &json!("Unauthorized to upsert collection"),
            &json!({ "id": id, "eth_address": Stranger.address() })
        ),
        "{body}"
    );
    let name: String = sqlx::query_scalar("SELECT name FROM builder.collections WHERE id = $1")
        .bind(id)
        .fetch_one(&m.pool)
        .await
        .unwrap();
    assert_eq!(name, "First");
}

/// What any signer sees of published content never depends on the
/// committee; a committee member gets the wide view of a published
/// collection's items and of a minted item only while the collections
/// subgraph lists it: not once it stops listing it (past the read window),
/// nor while the subgraph can't tell.
#[tokio::test]
async fn the_committee_widens_published_content_only_while_the_subgraph_lists_it() {
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let Some(m) = matrix(&committee).await else {
        return;
    };
    let f = fixture(&m.pool, Published).await;
    let collection = f.collection.unwrap();
    let minted = f.minted.unwrap();
    let check = async |caller: Caller, wide: bool, moment: &str| {
        for path in [
            format!("/v1/collections/{collection}/items"),
            format!("/v1/items/{minted}"),
        ] {
            let (status, body) = call(&m.app, caller, "GET", &path, Payload::None).await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{moment} {path} as {caller:?}: {body}"
            );
            let items: Vec<&Value> = match body["data"].as_array() {
                Some(list) => list.iter().collect(),
                None => vec![&body["data"]],
            };
            let full = items.iter().all(|i| i.get("local_content_hash").is_some());
            let listed = body["data"].as_array().map(Vec::len);
            assert_eq!(
                (full, listed.is_none_or(|n| n == if wide { 2 } else { 1 })),
                (wide, true),
                "{moment} {path} as {caller:?}: {body}"
            );
        }
    };
    check(Committee, true, "listed").await;
    check(Stranger, false, "listed").await;
    check(Owner, true, "listed").await;
    committee.remove(&Committee.address());
    check(Committee, true, "within the read window").await;
    m.state.committee.outdate();
    check(Committee, false, "no longer listed").await;
    check(Owner, true, "no longer listed").await;
    committee.set(Some(&[&Committee.address()]));
    m.state.committee.outdate();
    check(Committee, true, "listed again").await;
    committee.set(None);
    m.state.committee.outdate();
    check(Committee, false, "subgraph down").await;
    check(Owner, true, "subgraph down").await;
    check(Stranger, false, "subgraph down").await;
}

#[tokio::test]
async fn a_sibling_collection_of_the_provider_grants_nothing_over_the_others() {
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let Some(m) = matrix(&committee).await else {
        return;
    };
    let f = fixture(&m.pool, ThirdParty).await;
    let (status, body) = call(
        &m.app,
        Sibling,
        "GET",
        &format!("/v1/{}/items", Sibling.address()),
        Payload::None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["id"].as_str())
        .collect();
    assert!(
        listed.is_empty(),
        "the sibling manages no provider and lists none of its items: {body}"
    );
    for (method, path, payload) in [
        Op::ReadItem.request(ThirdParty, &f),
        Op::CollectionItems.request(ThirdParty, &f),
        Op::SaveItem.request(ThirdParty, &f),
        Op::UploadFiles.request(ThirdParty, &f),
        Op::DeleteItem.request(ThirdParty, &f),
    ] {
        let before = snapshot(&m.pool).await;
        let (status, body) = call(&m.app, Sibling, method, &path, payload).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {body}");
        assert_eq!(
            snapshot(&m.pool).await,
            before,
            "{method} {path} changed rows"
        );
    }
}

/// The small public reads read no signature, no committee and, but for
/// `addresses`, no chain copy: they answer as in the matrix with the copy
/// stale and with the committee source down; without a complete pass only
/// `addresses` answers 503, and with the rarities source down only the
/// rarities do.
#[tokio::test(flavor = "multi_thread")]
async fn the_small_reads_cannot_tell_only_without_their_own_source() {
    let small_reads: Vec<Op> = ops().into_iter().filter(|op| op.small_read()).collect();
    assert_eq!(small_reads.len(), 6);
    let cells = |label: &'static str| {
        let mut cells = Vec::new();
        for &op in &small_reads {
            for &kind in op.kinds() {
                for caller in CALLERS {
                    for mount in MOUNTS {
                        cells.push(Cell::new(
                            op,
                            kind,
                            caller,
                            mount,
                            move |f: &Fixture| match (label, op) {
                                ("no pass", Op::Addresses) => Some(Expect(
                                    503,
                                    "Chain state is not available yet. Try again later."
                                        .to_string(),
                                )),
                                ("rarities down", Op::Rarities | Op::Rarity) => {
                                    refused(503, RARITIES_UNAVAILABLE)
                                }
                                _ => op.expect(kind, caller, f),
                            },
                        ));
                    }
                }
            }
        }
        cells
    };
    let conditions = tokio::join!(
        failures_in(Condition::Stale, "stale", true, cells("stale")),
        failures_in(Condition::NoPass, "no pass", true, cells("no pass")),
        failures_in(
            Condition::CommitteeDown,
            "committee down",
            false,
            cells("committee down")
        ),
        failures_in(
            Condition::RaritiesDown,
            "rarities down",
            true,
            cells("rarities down")
        ),
    );
    let mut failures = Vec::new();
    for (label, found) in [
        ("stale", conditions.0),
        ("no pass", conditions.1),
        ("committee down", conditions.2),
        ("rarities down", conditions.3),
    ] {
        let Some(found) = found else { return };
        failures.extend(found.into_iter().map(|f| format!("{label}: {f}")));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Without a Polygon verifier, the publication routes that need one answer
/// 503 to the collection's owner, and every other caller gets the refusal
/// the matrix gives it, ahead of the 503.
#[tokio::test(flavor = "multi_thread")]
async fn without_a_polygon_verifier_publication_answers_503_after_the_refusals() {
    let mut cells = Vec::new();
    for op in [
        Op::PublicationBegin,
        Op::PublicationTransaction,
        Op::PublicationClaim,
    ] {
        for kind in [Missing, Draft] {
            for caller in CALLERS {
                for mount in MOUNTS {
                    cells.push(Cell::new(op, kind, caller, mount, move |f: &Fixture| {
                        match (kind, caller) {
                            (Draft, Owner) => refused(
                                503,
                                "Collection publication is unavailable until the Polygon verifier is configured",
                            ),
                            _ => op.expect(kind, caller, f),
                        }
                    }));
                }
            }
        }
    }
    let Some(failures) = failures_in(Condition::NoVerifier, "no verifier", true, cells).await
    else {
        return;
    };
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The node's own publication and item status bodies take only their
/// fields, and a revision of at most 128 characters; a refused body writes
/// nothing.
#[tokio::test]
async fn the_publication_and_item_status_bodies_refuse_unknown_fields_and_long_revisions() {
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let Some(m) = matrix(&committee).await else {
        return;
    };
    let f = fixture(&m.pool, Draft).await;
    let invalid = json!({ "ok": false, "data": {}, "error": "Invalid request body" });
    let tx_hash = format!("0x{}", "1".repeat(64));
    for (op, caller, body) in [
        (
            Op::PublicationBegin,
            Owner,
            json!({ "revision": "r", "extra": 1 }),
        ),
        (
            Op::PublicationBegin,
            Owner,
            json!({ "revision": "r".repeat(129) }),
        ),
        (
            Op::PublicationClaim,
            Owner,
            json!({ "revision": "r".repeat(129) }),
        ),
        (
            Op::PublicationTransaction,
            Owner,
            json!({ "tx_hash": tx_hash, "extra": 1 }),
        ),
        (
            Op::ItemStatus,
            Admin,
            json!({ "status": "approved", "extra": 1 }),
        ),
        (
            Op::ItemsStatus,
            Admin,
            json!({ "status": "approved", "itemIds": [f.item_id(Draft)], "extra": 1 }),
        ),
    ] {
        let (method, path, _) = op.request(Draft, &f);
        let before = snapshot(&m.pool).await;
        let answer = call(&m.app, caller, method, &path, Payload::Json(body.clone())).await;
        assert_eq!(
            answer,
            (StatusCode::BAD_REQUEST, invalid.clone()),
            "{op:?} {body}"
        );
        assert_eq!(snapshot(&m.pool).await, before, "{op:?} {body}");
    }
    let (method, path, _) = Op::PublicationBegin.request(Draft, &f);
    let (status, body) = call(
        &m.app,
        Owner,
        method,
        &path,
        Payload::Json(json!({ "revision": "r".repeat(128) })),
    )
    .await;
    assert_ne!(body["error"], "Invalid request body", "{status} {body}");
}

/// A collection with its topic: who may post gets the link back and the
/// forum is not asked again; only the committee replies in it.
#[tokio::test]
async fn a_posted_collection_answers_its_link_to_its_posters_and_takes_replies_from_the_committee()
{
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let Some(m) = matrix(&committee).await else {
        return;
    };
    let f = fixture(&m.pool, Published).await;
    let id = f.collection_id(Published);
    let (status, posted) = call(
        &m.app,
        Owner,
        "POST",
        &format!("/v1/collections/{id}/post"),
        Payload::None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{posted}");
    m.forum.take();
    let mut failures = Vec::new();
    for caller in CALLERS {
        for mount in MOUNTS {
            let (status, body) = call(
                &m.app,
                caller,
                "POST",
                &format!("{mount}/v1/collections/{id}/post"),
                Payload::None,
            )
            .await;
            let expected = match caller {
                Anonymous | Validator => (401, json!("Unauthenticated")),
                Owner | Manager => (409, json!("Forum post already exists")),
                _ => (
                    403,
                    json!(format!(
                        "Unauthorized user {} for collections {id}",
                        caller.address()
                    )),
                ),
            };
            if (status.as_u16(), body["error"].clone()) != expected
                || (status == StatusCode::CONFLICT && body["data"]["forum_link"] != posted["data"])
            {
                failures.push(format!("{mount} post as {caller:?}: {status} {body}"));
            }
            let (status, body) = call(
                &m.app,
                caller,
                "POST",
                &format!("{mount}/v1/collections/{id}/curation/post"),
                Payload::Json(
                    json!({ "forumPost": { "raw": "The collection has been unassigned." } }),
                ),
            )
            .await;
            let expected = match caller {
                Anonymous | Validator => (401, json!("Unauthenticated")),
                Committee => (200, Value::Null),
                _ => (403, json!("Unauthorized")),
            };
            if (status.as_u16(), body["error"].clone()) != expected {
                failures.push(format!("{mount} reply as {caller:?}: {status} {body}"));
            }
        }
    }
    let replies = m
        .forum
        .take()
        .iter()
        .filter(|c| c["method"] == "POST")
        .count();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(
        replies,
        MOUNTS.len(),
        "one reply per committee request, no other call out"
    );
}

#[tokio::test]
async fn independent_forum_cells_can_post_each_third_party_collection() {
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let Some(m) = matrix(&committee).await else {
        return;
    };
    for kind in [
        ThirdPartySubmitted,
        CuratedPending,
        CuratedApproved,
        CuratedRejected,
    ] {
        for mount in MOUNTS {
            let only_posts = Cell::new(Op::ForumPost, kind, Owner, mount, |_| {
                Some(Expect(200, String::new()))
            });
            let failure = cell(&m, "forum", only_posts, true).await;
            assert!(failure.is_none(), "{failure:?}");
        }
    }
}
