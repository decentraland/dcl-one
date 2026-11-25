//! The authorization matrix of the item file routes (`GET` and `HEAD
//! /v1/storage/contents/:hash`, `GET .../exists`) under both mounts: every
//! caller kind against every file state, then the same table with the copy
//! stale, with no complete pass and with the committee source down. A
//! refusal answers exactly what a hash nobody uploaded answers, headers and
//! body.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use chrono::{TimeDelta, Utc};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{FakeCommittee, committee_index, temp_storage, unique, wallet};
use crate::shared::signed_fetch::test_support::signed_headers_by;
use crate::storage::Storage;

const MOUNTS: [&str; 2] = ["", "/builder-api"];
const PROVIDER_ID: &str = "urn:decentraland:matic:collections-thirdparty:files";
const PUBLIC_CACHE: &str = "public,max-age=31536000,immutable";
const PRIVATE_CACHE: &str = "private, no-store";

#[derive(Clone, Copy, Debug, PartialEq)]
enum Caller {
    Anonymous,
    Stranger,
    Owner,
    Manager,
    Committee,
    Provider,
    Namer,
    Uploader,
}
use Caller::*;

const CALLERS: [Caller; 8] = [
    Anonymous, Stranger, Owner, Manager, Committee, Provider, Namer, Uploader,
];

impl Caller {
    fn key(self) -> Option<&'static str> {
        Some(match self {
            Anonymous => return None,
            Stranger => "0x7101010101010101010101010101010101010101010101010101010101010101",
            Owner => "0x7202020202020202020202020202020202020202020202020202020202020202",
            Manager => "0x7303030303030303030303030303030303030303030303030303030303030303",
            Committee => "0x7404040404040404040404040404040404040404040404040404040404040404",
            Provider => "0x7505050505050505050505050505050505050505050505050505050505050505",
            Namer => "0x7606060606060606060606060606060606060606060606060606060606060606",
            Uploader => "0x7707070707070707070707070707070707070707070707070707070707070707",
        })
    }

    fn address(self) -> String {
        self.key()
            .map(|key| format!("{:#x}", wallet(key).address()))
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum File {
    Draft,
    OnChain,
    Minted,
    ManagerUploaded,
    ManagerRecordOnly,
    ThirdParty,
    ThirdPartySubmitted,
    CuratedPending,
    CuratedApproved,
    CuratedRejected,
    UncuratedInCurated,
    Unnamed,
    PublishedUnuploaded,
    NobodyUploaded,
    VictimManager,
    VictimCreator,
    CoWriterEdit,
    ImportedThirdParty,
    KeptVideo,
    VideoNamedByOther,
}
use File::*;

const FILES: [File; 20] = [
    Draft,
    OnChain,
    Minted,
    ManagerUploaded,
    ManagerRecordOnly,
    ThirdParty,
    ThirdPartySubmitted,
    CuratedPending,
    CuratedApproved,
    CuratedRejected,
    UncuratedInCurated,
    Unnamed,
    PublishedUnuploaded,
    NobodyUploaded,
    VictimManager,
    VictimCreator,
    CoWriterEdit,
    ImportedThirdParty,
    KeptVideo,
    VideoNamedByOther,
];

impl File {
    fn public(self) -> bool {
        matches!(
            self,
            OnChain
                | Minted
                | ManagerUploaded
                | ThirdPartySubmitted
                | CuratedPending
                | CuratedApproved
                | CuratedRejected
                | KeptVideo
        )
    }

    /// The decision table.
    fn readable(self, caller: Caller) -> bool {
        self.public()
            || match caller {
                Owner => matches!(
                    self,
                    Draft
                        | ThirdParty
                        | UncuratedInCurated
                        | Unnamed
                        | VictimManager
                        | VictimCreator
                        | CoWriterEdit
                        | ImportedThirdParty
                        | VideoNamedByOther
                ),
                Provider => matches!(self, ThirdParty | UncuratedInCurated),
                Manager => self == ManagerRecordOnly,
                Uploader => self != NobodyUploaded,
                _ => false,
            }
    }
}

async fn collection(
    pool: &PgPool,
    owner: Caller,
    third_party: bool,
    contract: Option<&str>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, contract_address, third_party_id, urn_suffix) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(format!("Files {id}"))
    .bind(owner.address())
    .bind(contract)
    .bind(third_party.then_some(PROVIDER_ID))
    .bind(third_party.then(|| format!("{PROVIDER_ID}:{}", id.simple())))
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn copy_row(pool: &PgPool, contract: &str, creator: Caller, managers: &[Caller]) {
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
           is_completed, is_approved, is_editable, managers, minters, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 137, 'Files', $2, $2, 'urn:files', true, false, true, $3, '{}', 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(contract)
    .bind(creator.address())
    .bind(
        managers
            .iter()
            .map(|m| m.address().to_uppercase().replace("0X", "0x"))
            .collect::<Vec<_>>(),
    )
    .execute(pool)
    .await
    .unwrap();
}

/// An item of `owner` naming `hash` as `model.glb`, the name written by
/// `named_by`.
async fn item(
    pool: &PgPool,
    owner: Caller,
    collection: Option<Uuid>,
    minted: bool,
    submitted: bool,
    hash: &str,
    named_by: Caller,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, blockchain_item_id, is_published, urn_suffix) \
         VALUES ($1, $2, $3, 'Files item', 'wearable', $4, $5, $6)",
    )
    .bind(id)
    .bind(collection)
    .bind(owner.address())
    .bind(if submitted { Some("files-item") } else { minted.then_some("0") })
    .bind(submitted)
    .bind(submitted.then(|| format!("{PROVIDER_ID}:{}:files-item", id.simple())))
    .execute(pool)
    .await
    .unwrap();
    if submitted {
        curate(pool, id, "pending").await;
    }
    sqlx::query(
        "INSERT INTO builder.item_contents (item_id, file, hash, named_by) VALUES ($1, 'model.glb', $2, $3)",
    )
    .bind(id)
    .bind(hash)
    .bind(named_by.address())
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Submits `item` for curation with `status`, as an imported item curation.
async fn curate(pool: &PgPool, item: Uuid, status: &str) {
    sqlx::query(
        "INSERT INTO builder.item_curations (id, item_id, status, content_hash) VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(item)
    .bind(status)
    .bind(format!("{}{}", item.simple(), item.simple()))
    .execute(pool)
    .await
    .unwrap();
}

/// A minted item of `owner` that keeps `hash` as its video, the name written
/// by `named_by`, and names nothing in its contents.
async fn video_item(pool: &PgPool, owner: Caller, collection: Uuid, hash: &str, named_by: Caller) {
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, blockchain_item_id, video, video_named_by) \
         VALUES ($1, $2, $3, 'Files video', 'wearable', '0', $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(collection)
    .bind(owner.address())
    .bind(hash)
    .bind(named_by.address())
    .execute(pool)
    .await
    .unwrap();
}

async fn record(pool: &PgPool, hash: &str, size: usize, uploader: Caller) {
    sqlx::query(
        "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($1, $2, $3)",
    )
    .bind(hash)
    .bind(size as i64)
    .bind(uploader.address())
    .execute(pool)
    .await
    .unwrap();
}

fn contract() -> String {
    format!("0x{}", Uuid::new_v4().simple().to_string()[..20].repeat(2))
}

/// A file in `state` with bytes no other cell uses. Another wallet's draft
/// names its hash without uploading it and a third wallet uploaded the same
/// bytes for its own draft, except where no item names the hash (the third
/// wallet keeps its record) or nobody uploaded it (neither). In the last
/// four states the file is the owner's private draft upload and another
/// wallet writes its name into an item whose writers include the owner.
async fn file(pool: &PgPool, store: &Storage, state: File) -> (String, Vec<u8>) {
    let bytes = unique("item-file");
    let hash = dcl_auth::hash_bytes_v1(&bytes);
    store.store(&hash, bytes.clone().into()).await.unwrap();
    if state != Unnamed {
        item(pool, Namer, None, false, false, &hash, Namer).await;
    }
    if state != NobodyUploaded {
        if state != Unnamed {
            item(pool, Uploader, None, false, false, &hash, Uploader).await;
        }
        record(pool, &hash, bytes.len(), Uploader).await;
    }
    if matches!(
        state,
        VictimManager | VictimCreator | CoWriterEdit | ImportedThirdParty | VideoNamedByOther
    ) {
        item(pool, Owner, None, false, false, &hash, Owner).await;
    }
    let owner_uploads = match state {
        Draft => {
            let c = collection(pool, Owner, false, None).await;
            item(pool, Owner, Some(c), false, false, &hash, Owner).await;
            true
        }
        OnChain | ManagerUploaded | ManagerRecordOnly | PublishedUnuploaded => {
            let contract = contract();
            let c = collection(pool, Owner, false, Some(&contract)).await;
            copy_row(pool, &contract, Owner, &[Manager]).await;
            let writer = if state == ManagerUploaded {
                Manager
            } else {
                Owner
            };
            item(pool, Owner, Some(c), false, false, &hash, writer).await;
            if matches!(state, ManagerUploaded | ManagerRecordOnly) {
                record(pool, &hash, bytes.len(), Manager).await;
            }
            state == OnChain
        }
        Minted => {
            let c = collection(pool, Owner, false, Some(&contract())).await;
            item(pool, Owner, Some(c), true, false, &hash, Owner).await;
            true
        }
        ThirdParty | ThirdPartySubmitted => {
            let c = collection(pool, Provider, true, None).await;
            item(
                pool,
                Owner,
                Some(c),
                false,
                state == ThirdPartySubmitted,
                &hash,
                Owner,
            )
            .await;
            true
        }
        CuratedPending | CuratedApproved | CuratedRejected => {
            let c = collection(pool, Provider, true, None).await;
            let id = item(pool, Owner, Some(c), false, false, &hash, Owner).await;
            sqlx::query("UPDATE builder.items SET urn_suffix = $2 WHERE id = $1")
                .bind(id)
                .bind(format!("{PROVIDER_ID}:{}:files-item", c.simple()))
                .execute(pool)
                .await
                .unwrap();
            let status = match state {
                CuratedPending => "pending",
                CuratedApproved => "approved",
                _ => "rejected",
            };
            curate(pool, id, status).await;
            true
        }
        UncuratedInCurated => {
            let c = collection(pool, Provider, true, None).await;
            let other = unique("curated-sibling");
            let sibling = item(
                pool,
                Owner,
                Some(c),
                false,
                false,
                &dcl_auth::hash_bytes_v1(&other),
                Owner,
            )
            .await;
            curate(pool, sibling, "approved").await;
            item(pool, Owner, Some(c), false, false, &hash, Owner).await;
            true
        }
        Unnamed => true,
        NobodyUploaded => {
            let c = collection(pool, Owner, false, Some(&contract())).await;
            item(pool, Owner, Some(c), true, false, &hash, Owner).await;
            false
        }
        VictimManager | VictimCreator => {
            let contract = contract();
            let c = collection(pool, Stranger, false, Some(&contract)).await;
            if state == VictimManager {
                copy_row(pool, &contract, Stranger, &[Owner]).await;
            } else {
                copy_row(pool, &contract, Owner, &[]).await;
            }
            item(pool, Stranger, Some(c), true, false, &hash, Stranger).await;
            true
        }
        CoWriterEdit => {
            let contract = contract();
            let c = collection(pool, Owner, false, Some(&contract)).await;
            copy_row(pool, &contract, Owner, &[Manager]).await;
            item(pool, Owner, Some(c), true, false, &hash, Manager).await;
            true
        }
        ImportedThirdParty => {
            let c = collection(pool, Provider, true, None).await;
            item(pool, Owner, Some(c), false, true, &hash, Provider).await;
            true
        }
        KeptVideo => {
            let c = collection(pool, Owner, false, Some(&contract())).await;
            video_item(pool, Owner, c, &hash, Owner).await;
            true
        }
        VideoNamedByOther => {
            let contract = contract();
            let c = collection(pool, Stranger, false, Some(&contract)).await;
            copy_row(pool, &contract, Stranger, &[]).await;
            video_item(pool, Stranger, c, &hash, Stranger).await;
            true
        }
    };
    if owner_uploads {
        record(pool, &hash, bytes.len(), Owner).await;
    }
    (hash, bytes)
}

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Answer {
    fn header(&self, name: header::HeaderName) -> &str {
        self.headers
            .get(name)
            .map(|v| v.to_str().unwrap())
            .unwrap_or_default()
    }

    /// Status, every header but `date`, and the body.
    fn shape(&self) -> (StatusCode, Vec<(String, String)>, Vec<u8>) {
        let mut headers: Vec<(String, String)> = self
            .headers
            .iter()
            .filter(|(name, _)| *name != header::DATE)
            .map(|(name, value)| (name.to_string(), value.to_str().unwrap().to_string()))
            .collect();
        headers.sort();
        (self.status, headers, self.body.clone())
    }
}

async fn send(
    app: &Router,
    caller: Caller,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> Answer {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(key) = caller.key() {
        let now = Utc::now().timestamp_millis().to_string();
        for (name, value) in
            signed_headers_by(&wallet(key), &method.to_lowercase(), path, "{}", true, &now)
                .await
                .iter()
        {
            request = request.header(name, value);
        }
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), 64 << 20)
        .await
        .unwrap()
        .to_vec();
    Answer {
        status,
        headers,
        body,
    }
}

struct Files {
    pool: PgPool,
    app: Router,
    store: Arc<Storage>,
    denylist: Arc<crate::content::Denylist>,
}

async fn fresh_pass(pool: &PgPool, minutes_ago: i64) {
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', $1, $1) \
         ON CONFLICT (mirror, network) DO UPDATE SET last_complete_pass_at = EXCLUDED.last_complete_pass_at, \
           pass_started_at = EXCLUDED.pass_started_at, cursor_id = NULL",
    )
    .bind(Utc::now().naive_utc() - TimeDelta::minutes(minutes_ago))
    .execute(pool)
    .await
    .unwrap();
    crate::tests::support::publish_registry_index(&mut pool.acquire().await.unwrap(), false).await;
}

async fn files(committee: &FakeCommittee) -> Option<Files> {
    let pool = crate::db::scratch::isolated_pool(8).await?;
    fresh_pass(&pool, 0).await;
    sqlx::query(
        "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, 'tp:1:Files:')",
    )
    .bind(PROVIDER_ID)
    .bind(vec![Provider.address()])
    .execute(&pool)
    .await
    .unwrap();
    let store = temp_storage().await;
    let denylist = Arc::<crate::content::Denylist>::default();
    let state = Arc::new(
        crate::State::new(
            pool.clone(),
            store.clone(),
            denylist.clone(),
            dcl_http::Outbound::for_tests(),
            Some(committee_index(committee).await),
            None,
            crate::Settings {
                writes_enabled: true,
                ..crate::Settings::default()
            },
        )
        .unwrap(),
    );
    let app = Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state));
    Some(Files {
        pool,
        app,
        store: store.item_files().unwrap().clone(),
        denylist,
    })
}

/// Every cell of the table against `f`; the failures, one line each.
async fn table(f: &Files) -> Vec<String> {
    let mut failures = Vec::new();
    for state in FILES {
        let (hash, bytes) = file(&f.pool, &f.store, state).await;
        let absent = dcl_auth::hash_bytes_v1(&unique("never-uploaded"));
        for caller in CALLERS {
            for mount in MOUNTS {
                let readable = state.readable(caller);
                for method in ["GET", "HEAD"] {
                    let path = format!("{mount}/v1/storage/contents/{hash}");
                    let got = send(&f.app, caller, method, &path, &[]).await;
                    let cell = format!("{method} {path} {state:?} as {caller:?}");
                    if !readable {
                        let refusal = send(
                            &f.app,
                            caller,
                            method,
                            &format!("{mount}/v1/storage/contents/{absent}"),
                            &[],
                        )
                        .await;
                        if got.shape() != refusal.shape() {
                            failures.push(format!(
                                "{cell}: answered {:?}, a hash nobody uploaded {:?}",
                                got.shape(),
                                refusal.shape()
                            ));
                        }
                        if got.status != StatusCode::NOT_FOUND
                            || got.header(header::CACHE_CONTROL) != "no-store"
                        {
                            failures.push(format!(
                                "{cell}: refused with {} cache {:?}",
                                got.status,
                                got.header(header::CACHE_CONTROL)
                            ));
                        }
                        continue;
                    }
                    let cache = if caller == Anonymous {
                        PUBLIC_CACHE
                    } else {
                        PRIVATE_CACHE
                    };
                    let body = if method == "GET" {
                        bytes.clone()
                    } else {
                        Vec::new()
                    };
                    let expected = (
                        StatusCode::OK,
                        cache,
                        "application/octet-stream",
                        "attachment",
                        "nosniff",
                        "sandbox",
                        bytes.len().to_string(),
                        body,
                    );
                    let answered = (
                        got.status,
                        got.header(header::CACHE_CONTROL),
                        got.header(header::CONTENT_TYPE),
                        got.header(header::CONTENT_DISPOSITION),
                        got.header(header::X_CONTENT_TYPE_OPTIONS),
                        got.header(header::CONTENT_SECURITY_POLICY),
                        got.header(header::CONTENT_LENGTH).to_string(),
                        got.body.clone(),
                    );
                    if answered != expected {
                        failures.push(format!(
                            "{cell}: answered {:?}, expected {:?}",
                            (
                                &answered.0,
                                answered.1,
                                answered.2,
                                answered.3,
                                answered.4,
                                answered.5,
                                &answered.6
                            ),
                            (
                                &expected.0,
                                expected.1,
                                expected.2,
                                expected.3,
                                expected.4,
                                expected.5,
                                &expected.6
                            ),
                        ));
                    }
                }
                let path = format!("{mount}/v1/storage/contents/{hash}/exists");
                let got = send(&f.app, caller, "GET", &path, &[]).await;
                let status = if readable {
                    StatusCode::OK
                } else {
                    StatusCode::NOT_FOUND
                };
                if got.status != status || got.body != br#"{"ok":true}"# {
                    failures.push(format!(
                        "GET {path} {state:?} as {caller:?}: answered {} {}, expected {status}",
                        got.status,
                        String::from_utf8_lossy(&got.body)
                    ));
                }
                if !readable {
                    let refusal = send(
                        &f.app,
                        caller,
                        "GET",
                        &format!("{mount}/v1/storage/contents/{absent}/exists"),
                        &[],
                    )
                    .await;
                    if got.shape() != refusal.shape() {
                        failures.push(format!(
                            "GET {path} {state:?} as {caller:?}: the probe tells the file from an absent one"
                        ));
                    }
                }
            }
        }
    }
    failures
}

#[tokio::test]
async fn each_caller_reads_an_item_file_only_as_the_published_rule_decides() {
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let Some(f) = files(&committee).await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let failures = table(&f).await;
    assert!(
        failures.is_empty(),
        "{} failing cells:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[tokio::test]
async fn a_stale_copy_or_one_without_a_pass_answers_the_same_table() {
    let committee = FakeCommittee::with(&[&Committee.address()]);
    let Some(f) = files(&committee).await else {
        return;
    };
    fresh_pass(&f.pool, 31).await;
    let stale = table(&f).await;
    assert!(stale.is_empty(), "stale copy:\n{}", stale.join("\n"));
    sqlx::query("DELETE FROM shop.mirror_state WHERE mirror = 'collections' AND network = 'matic'")
        .execute(&f.pool)
        .await
        .unwrap();
    let no_pass = table(&f).await;
    assert!(
        no_pass.is_empty(),
        "no complete pass:\n{}",
        no_pass.join("\n")
    );
}

#[tokio::test]
async fn a_committee_source_that_is_down_changes_no_cell() {
    let committee = FakeCommittee::down();
    let Some(f) = files(&committee).await else {
        return;
    };
    let failures = table(&f).await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test]
async fn a_range_is_answered_from_the_file_and_one_past_its_end_is_refused() {
    let committee = FakeCommittee::down();
    let Some(f) = files(&committee).await else {
        return;
    };
    let (hash, bytes) = file(&f.pool, &f.store, Minted).await;
    let path = format!("/v1/storage/contents/{hash}");
    let got = send(&f.app, Anonymous, "GET", &path, &[("range", "bytes=2-5")]).await;
    assert_eq!(got.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(got.body, bytes[2..=5]);
    assert_eq!(
        got.header(header::CONTENT_RANGE),
        format!("bytes 2-5/{}", bytes.len())
    );
    assert_eq!(got.header(header::CACHE_CONTROL), PUBLIC_CACHE);
    let reversed = send(&f.app, Anonymous, "GET", &path, &[("range", "bytes=5-1")]).await;
    assert_eq!(
        (reversed.status, reversed.body.clone()),
        (StatusCode::OK, bytes.clone()),
        "a range whose last byte precedes its first is invalid and ignored"
    );
    let past = format!("bytes={}-", bytes.len());
    let got = send(&f.app, Anonymous, "GET", &path, &[("range", &past)]).await;
    assert_eq!(got.status, StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        got.header(header::CONTENT_RANGE),
        format!("bytes */{}", bytes.len())
    );
    let (private, _) = file(&f.pool, &f.store, Draft).await;
    let got = send(
        &f.app,
        Anonymous,
        "GET",
        &format!("/v1/storage/contents/{private}"),
        &[("range", "bytes=0-1")],
    )
    .await;
    assert_eq!(
        got.status,
        StatusCode::NOT_FOUND,
        "a range opens nothing private"
    );
}

#[tokio::test]
async fn the_type_comes_from_the_bytes_and_never_makes_active_content() {
    let committee = FakeCommittee::down();
    let Some(f) = files(&committee).await else {
        return;
    };
    let png = [b"\x89PNG\r\n\x1a\n".as_slice(), &unique("png")].concat();
    let page = [
        b"<html><script>alert(1)</script>".as_slice(),
        &unique("html"),
    ]
    .concat();
    for (bytes, content_type, disposition) in [
        (png, "image/png", ""),
        (page, "application/octet-stream", "attachment"),
    ] {
        let hash = dcl_auth::hash_bytes_v1(&bytes);
        f.store.store(&hash, bytes.clone().into()).await.unwrap();
        let c = collection(&f.pool, Owner, false, Some(&contract())).await;
        item(&f.pool, Owner, Some(c), true, false, &hash, Owner).await;
        record(&f.pool, &hash, bytes.len(), Owner).await;
        let got = send(
            &f.app,
            Anonymous,
            "GET",
            &format!("/v1/storage/contents/{hash}"),
            &[("accept", "text/html")],
        )
        .await;
        assert_eq!(got.status, StatusCode::OK);
        assert_eq!(got.header(header::CONTENT_TYPE), content_type);
        assert_eq!(got.header(header::CONTENT_DISPOSITION), disposition);
        assert_eq!(got.header(header::X_CONTENT_TYPE_OPTIONS), "nosniff");
        assert_eq!(got.header(header::CONTENT_SECURITY_POLICY), "sandbox");
    }
}

#[tokio::test]
async fn a_path_that_is_not_a_canonical_hash_is_refused_as_an_absent_file() {
    let committee = FakeCommittee::down();
    let Some(f) = files(&committee).await else {
        return;
    };
    let absent = send(
        &f.app,
        Anonymous,
        "GET",
        &format!(
            "/v1/storage/contents/{}",
            dcl_auth::hash_bytes_v1(b"nobody")
        ),
        &[],
    )
    .await;
    for raw in [
        "..%2F..%2Fetc%2Fpasswd",
        "%2E%2E",
        "A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2",
        "bafkreigh2akiscaildcqabsyg3dfr6chu3fgpregiymsck7e7aqa4s52z%00",
    ] {
        let got = send(
            &f.app,
            Anonymous,
            "GET",
            &format!("/v1/storage/contents/{raw}"),
            &[],
        )
        .await;
        assert_eq!(got.shape(), absent.shape(), "{raw}");
        let probe = send(
            &f.app,
            Anonymous,
            "GET",
            &format!("/v1/storage/contents/{raw}/exists"),
            &[],
        )
        .await;
        assert_eq!(
            (probe.status, probe.body),
            (StatusCode::NOT_FOUND, br#"{"ok":true}"#.to_vec()),
            "{raw}"
        );
    }
}

#[tokio::test]
async fn a_signature_that_does_not_verify_is_refused_whatever_the_hash() {
    let committee = FakeCommittee::down();
    let Some(f) = files(&committee).await else {
        return;
    };
    let (public, _) = file(&f.pool, &f.store, Minted).await;
    let (private, _) = file(&f.pool, &f.store, Draft).await;
    let absent = dcl_auth::hash_bytes_v1(&unique("absent"));
    let mut answers = Vec::new();
    for hash in [public, private, absent] {
        let path = format!("/v1/storage/contents/{hash}");
        let now = Utc::now().timestamp_millis().to_string();
        let mut request = Request::builder().method("GET").uri(&path);
        for (name, value) in signed_headers_by(
            &wallet(Owner.key().unwrap()),
            "get",
            "/v1/storage/contents/another",
            "{}",
            true,
            &now,
        )
        .await
        .iter()
        {
            request = request.header(name, value);
        }
        let response = f
            .app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 1 << 16)
            .await
            .unwrap();
        answers.push((status, body));
    }
    assert_eq!(answers[0].0, StatusCode::UNAUTHORIZED);
    assert!(answers.iter().all(|a| *a == answers[0]), "{answers:?}");
}

async fn write(
    app: &Router,
    caller: Caller,
    method: &str,
    path: &str,
    body: Body,
    content_type: &str,
) -> StatusCode {
    let now = Utc::now().timestamp_millis().to_string();
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, content_type);
    for (name, value) in signed_headers_by(
        &wallet(caller.key().unwrap()),
        &method.to_lowercase(),
        path,
        "{}",
        true,
        &now,
    )
    .await
    .iter()
    {
        request = request.header(name, value);
    }
    app.clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap()
        .status()
}

/// `PUT /v1/items/:id` as `caller` with the item's `contents`.
async fn save(
    app: &Router,
    caller: Caller,
    id: Uuid,
    collection: Uuid,
    contents: serde_json::Value,
) -> StatusCode {
    let body = serde_json::json!({ "item": {
        "id": id, "name": "Files item", "description": "", "type": "wearable",
        "eth_address": Owner.address(), "rarity": "common", "thumbnail": "thumbnail.png",
        "collection_id": collection, "contents": contents,
        "metrics": { "meshes": 1, "bodies": 1, "materials": 1, "textures": 1, "triangles": 1, "entities": 1 },
        "data": {
            "category": "hat", "replaces": [], "hides": [], "tags": [],
            "representations": [{
                "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
                "mainFile": "model.glb", "contents": ["model.glb"],
                "overrideHides": [], "overrideReplaces": []
            }]
        }
    }});
    write(
        app,
        caller,
        "PUT",
        &format!("/v1/items/{id}"),
        Body::from(body.to_string()),
        "application/json",
    )
    .await
}

async fn upload(app: &Router, caller: Caller, id: Uuid, bytes: &[u8]) -> StatusCode {
    let boundary = "files-boundary";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"blob\"\r\n\
         Content-Type: application/octet-stream\r\n\r\n",
        dcl_auth::hash_bytes_v1(bytes)
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    write(
        app,
        caller,
        "POST",
        &format!("/v1/items/{id}/files"),
        Body::from(body),
        &format!("multipart/form-data; boundary={boundary}"),
    )
    .await
}

async fn writers(pool: &PgPool, id: Uuid) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT file, named_by FROM builder.item_contents WHERE item_id = $1 ORDER BY file",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn served(app: &Router, hash: &str) -> (StatusCode, StatusCode) {
    let file = send(
        app,
        Anonymous,
        "GET",
        &format!("/builder-api/v1/storage/contents/{hash}"),
        &[],
    )
    .await;
    let probe = send(
        app,
        Anonymous,
        "GET",
        &format!("/v1/storage/contents/{hash}/exists"),
        &[],
    )
    .await;
    (file.status, probe.status)
}

#[tokio::test]
async fn a_name_opens_a_file_only_when_the_wallet_that_wrote_it_holds_the_bytes() {
    let committee = FakeCommittee::down();
    let Some(f) = files(&committee).await else {
        return;
    };
    let (model, private, fresh) = (
        unique("attributed model"),
        unique("owner draft"),
        unique("manager file"),
    );
    let [model_h, private_h, fresh_h] =
        [&model, &private, &fresh].map(|b| dcl_auth::hash_bytes_v1(b));
    for (hash, bytes) in [(&model_h, &model), (&private_h, &private)] {
        f.store.store(hash, bytes.clone().into()).await.unwrap();
        record(&f.pool, hash, bytes.len(), Owner).await;
    }
    let contract = contract();
    let c = collection(&f.pool, Owner, false, Some(&contract)).await;
    copy_row(&f.pool, &contract, Owner, &[Manager]).await;
    let id = item(&f.pool, Owner, Some(c), true, false, &model_h, Owner).await;
    item(&f.pool, Owner, None, false, false, &private_h, Owner).await;
    let open = (StatusCode::OK, StatusCode::OK);
    let closed = (StatusCode::NOT_FOUND, StatusCode::NOT_FOUND);
    assert_eq!(served(&f.app, &model_h).await, open);
    assert_eq!(served(&f.app, &private_h).await, closed);

    let named = serde_json::json!({ "model.glb": model_h, "extra.png": private_h });
    assert_eq!(save(&f.app, Manager, id, c, named).await, StatusCode::OK);
    assert_eq!(
        served(&f.app, &private_h).await,
        closed,
        "a manager naming the owner's draft file in the owner's published item opens nothing"
    );
    assert_eq!(
        served(&f.app, &model_h).await,
        open,
        "a re-sent name keeps its writer"
    );

    let reordered = serde_json::json!({ "extra.png": private_h, "model.glb": model_h });
    assert_eq!(save(&f.app, Owner, id, c, reordered).await, StatusCode::OK);
    assert_eq!(
        served(&f.app, &private_h).await,
        closed,
        "the owner re-sending a name another wallet wrote doesn't take it over"
    );
    let moved = serde_json::json!({ "model.glb": model_h, "other.png": private_h });
    assert_eq!(save(&f.app, Owner, id, c, moved).await, StatusCode::OK);
    assert_eq!(
        writers(&f.pool, id).await,
        [
            ("model.glb".to_string(), Owner.address()),
            ("other.png".to_string(), Manager.address())
        ],
        "a hash the item already named keeps its writer under another file name"
    );
    assert_eq!(served(&f.app, &private_h).await, closed);

    let added =
        serde_json::json!({ "model.glb": model_h, "other.png": private_h, "fresh.png": fresh_h });
    assert_eq!(save(&f.app, Manager, id, c, added).await, StatusCode::OK);
    assert_eq!(
        served(&f.app, &fresh_h).await,
        closed,
        "named, not yet uploaded"
    );
    assert_eq!(upload(&f.app, Manager, id, &fresh).await, StatusCode::OK);
    assert_eq!(
        served(&f.app, &fresh_h).await,
        open,
        "an on-chain manager who writes a name and uploads its bytes publishes the file"
    );
}

#[tokio::test]
async fn a_withheld_file_is_refused_as_an_absent_one_before_any_read() {
    let committee = FakeCommittee::down();
    let Some(f) = files(&committee).await else {
        return;
    };
    let (hash, _) = file(&f.pool, &f.store, Minted).await;
    let absent = dcl_auth::hash_bytes_v1(&unique("absent"));
    f.denylist.operator_insert(&hash);
    for caller in [Anonymous, Owner] {
        for (path, other) in [
            (
                format!("/v1/storage/contents/{hash}"),
                format!("/v1/storage/contents/{absent}"),
            ),
            (
                format!("/v1/storage/contents/{hash}/exists"),
                format!("/v1/storage/contents/{absent}/exists"),
            ),
        ] {
            let withheld = send(&f.app, caller, "GET", &path, &[]).await;
            let refusal = send(&f.app, caller, "GET", &other, &[]).await;
            assert_eq!(withheld.shape(), refusal.shape(), "{path} as {caller:?}");
        }
    }
}

async fn read(app: &Router, caller: Caller, hash: &str) -> [StatusCode; 4] {
    let mut answers = [StatusCode::OK; 4];
    for (n, path) in [
        format!("/v1/storage/contents/{hash}"),
        format!("/builder-api/v1/storage/contents/{hash}"),
        format!("/v1/storage/contents/{hash}/exists"),
        format!("/builder-api/v1/storage/contents/{hash}/exists"),
    ]
    .iter()
    .enumerate()
    {
        answers[n] = send(app, caller, if n == 1 { "HEAD" } else { "GET" }, path, &[])
            .await
            .status;
    }
    answers
}

#[tokio::test]
async fn re_pointing_an_existing_item_at_another_wallets_files_opens_none_of_them() {
    let committee = FakeCommittee::down();
    let Some(f) = files(&committee).await else {
        return;
    };
    let victims: Vec<Vec<u8>> = (0..3)
        .map(|n| unique(&format!("victim draft {n}")))
        .collect();
    let hashes: Vec<String> = victims.iter().map(|b| dcl_auth::hash_bytes_v1(b)).collect();
    for (hash, bytes) in hashes.iter().zip(&victims) {
        f.store.store(hash, bytes.clone().into()).await.unwrap();
        item(&f.pool, Owner, None, false, false, hash, Owner).await;
        record(&f.pool, hash, bytes.len(), Owner).await;
    }
    let own = unique("attacker model");
    let own_h = dcl_auth::hash_bytes_v1(&own);
    f.store.store(&own_h, own.clone().into()).await.unwrap();
    record(&f.pool, &own_h, own.len(), Stranger).await;
    let contract = contract();
    let c = collection(&f.pool, Stranger, false, Some(&contract)).await;
    copy_row(&f.pool, &contract, Stranger, &[Owner]).await;
    let id = item(&f.pool, Stranger, Some(c), false, false, &own_h, Stranger).await;
    assert_eq!(read(&f.app, Anonymous, &own_h).await, [StatusCode::OK; 4]);

    let mut contents = serde_json::json!({ "model.glb": own_h });
    for (n, hash) in hashes.iter().enumerate() {
        contents[format!("texture-{n}.png")] = hash.clone().into();
    }
    assert_eq!(
        save(&f.app, Stranger, id, c, contents).await,
        StatusCode::OK
    );
    for hash in &hashes {
        for caller in [Anonymous, Stranger, Manager, Committee] {
            assert_eq!(
                read(&f.app, caller, hash).await,
                [StatusCode::NOT_FOUND; 4],
                "{hash} as {caller:?}"
            );
        }
        assert_eq!(
            read(&f.app, Owner, hash).await,
            [StatusCode::OK; 4],
            "the victim still reads {hash}"
        );
    }
}

#[tokio::test]
async fn mixed_edits_of_a_published_item_keep_each_file_with_the_wallet_that_named_it() {
    let committee = FakeCommittee::down();
    let Some(f) = files(&committee).await else {
        return;
    };
    let (model, thumbnail) = (unique("owner model"), unique("owner thumbnail"));
    let [model_h, thumbnail_h] = [&model, &thumbnail].map(|b| dcl_auth::hash_bytes_v1(b));
    for (hash, bytes) in [(&model_h, &model), (&thumbnail_h, &thumbnail)] {
        f.store.store(hash, bytes.clone().into()).await.unwrap();
        record(&f.pool, hash, bytes.len(), Owner).await;
    }
    let contract = contract();
    let c = collection(&f.pool, Owner, false, Some(&contract)).await;
    copy_row(&f.pool, &contract, Owner, &[Manager]).await;
    let id = item(&f.pool, Owner, Some(c), true, false, &model_h, Owner).await;
    assert_eq!(
        save(
            &f.app,
            Owner,
            id,
            c,
            serde_json::json!({ "model.glb": model_h, "thumbnail.png": thumbnail_h })
        )
        .await,
        StatusCode::OK
    );
    let open = [StatusCode::OK; 4];

    assert_eq!(
        save(
            &f.app,
            Manager,
            id,
            c,
            serde_json::json!({ "model.glb": model_h, "thumbnail.png": thumbnail_h })
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        read(&f.app, Anonymous, &model_h).await,
        open,
        "a co-manager's re-save keeps the owner's files public"
    );
    assert_eq!(read(&f.app, Anonymous, &thumbnail_h).await, open);

    let new_model = unique("manager model");
    let new_model_h = dcl_auth::hash_bytes_v1(&new_model);
    assert_eq!(
        save(
            &f.app,
            Manager,
            id,
            c,
            serde_json::json!({ "model.glb": new_model_h, "thumbnail.png": thumbnail_h })
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        upload(&f.app, Manager, id, &new_model).await,
        StatusCode::OK
    );
    assert_eq!(
        read(&f.app, Anonymous, &new_model_h).await,
        open,
        "the manager named and uploaded its model"
    );

    let new_thumbnail = unique("owner new thumbnail");
    let new_thumbnail_h = dcl_auth::hash_bytes_v1(&new_thumbnail);
    assert_eq!(
        save(
            &f.app,
            Owner,
            id,
            c,
            serde_json::json!({ "model.glb": new_model_h, "thumbnail.png": new_thumbnail_h })
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        upload(&f.app, Owner, id, &new_thumbnail).await,
        StatusCode::OK
    );
    assert_eq!(
        read(&f.app, Anonymous, &new_model_h).await,
        open,
        "the owner's thumbnail-only save leaves the manager's model with the manager"
    );
    assert_eq!(read(&f.app, Anonymous, &new_thumbnail_h).await, open);
}

/// How many times the plan scanned `relation`, over every node.
fn scans(plan: &serde_json::Value, relation: &str) -> f64 {
    let own = if plan["Relation Name"] == relation {
        plan["Actual Loops"].as_f64().unwrap_or(0.0)
    } else {
        0.0
    };
    own + plan["Plans"]
        .as_array()
        .map(|plans| plans.iter().map(|p| scans(p, relation)).sum())
        .unwrap_or(0.0)
}

#[tokio::test]
async fn a_signed_read_of_a_public_file_never_runs_the_signers_branches() {
    let committee = FakeCommittee::down();
    let Some(f) = files(&committee).await else {
        return;
    };
    let bytes = unique("public with many holder drafts");
    let hash = dcl_auth::hash_bytes_v1(&bytes);
    let c = collection(&f.pool, Owner, false, Some(&contract())).await;
    item(&f.pool, Owner, Some(c), true, false, &hash, Owner).await;
    record(&f.pool, &hash, bytes.len(), Owner).await;
    record(&f.pool, &hash, bytes.len(), Uploader).await;
    sqlx::query(
        "WITH held AS (INSERT INTO builder.items (id, eth_address, name, type) \
           SELECT gen_random_uuid(), $1, 'Held', 'wearable' FROM generate_series(1, 1000) RETURNING id) \
         INSERT INTO builder.item_contents (item_id, file, hash, named_by) SELECT id, 'model.glb', $2, $1 FROM held",
    )
    .bind(Uploader.address())
    .bind(&hash)
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::raw_sql("ANALYZE builder.items, builder.item_contents, builder.uploaded_contents")
        .execute(&f.pool)
        .await
        .unwrap();
    let mut conn = f.pool.acquire().await.unwrap();
    for mode in ["force_custom_plan", "force_generic_plan"] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("SET plan_cache_mode = {mode}")))
            .execute(&mut *conn)
            .await
            .unwrap();
        for signer in [None, Some(Stranger.address()), Some(Uploader.address())] {
            let plan: serde_json::Value = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "EXPLAIN (ANALYZE, FORMAT JSON) {}",
                crate::access::file_read_sql()
            )))
            .bind(&hash)
            .bind(&signer)
            .bind(20_i64)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
            let probed = scans(&plan[0]["Plan"], "uploaded_contents");
            assert_eq!(
                probed, 1.0,
                "{mode}, signer {signer:?}: a published file's read probes the upload records once, in the public branch alone"
            );
        }
    }
    sqlx::raw_sql("RESET plan_cache_mode")
        .execute(&mut *conn)
        .await
        .unwrap();
}

/// A published file holding `bytes`, read unsigned.
async fn published(f: &Files, bytes: &[u8]) -> Answer {
    let hash = dcl_auth::hash_bytes_v1(bytes);
    f.store.store(&hash, bytes.to_vec().into()).await.unwrap();
    let c = collection(&f.pool, Owner, false, Some(&contract())).await;
    item(&f.pool, Owner, Some(c), true, false, &hash, Owner).await;
    record(&f.pool, &hash, bytes.len(), Owner).await;
    send(
        &f.app,
        Anonymous,
        "GET",
        &format!("/v1/storage/contents/{hash}"),
        &[],
    )
    .await
}

/// Stored files are served inert (ADR `2026-11-21-stored-files-served-inert`):
/// an SVG, scripted or not, and any other document is an attachment.
#[tokio::test]
async fn an_svg_or_any_other_document_is_an_attachment() {
    let committee = FakeCommittee::down();
    let Some(f) = files(&committee).await else {
        return;
    };
    let tag = |label: &str| format!("<!-- {label} {} -->", Uuid::new_v4());
    let svg = format!(
        "{}\n<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"><rect width=\"1\" height=\"1\" fill=\"red\"/></svg>\n",
        tag("thumbnail")
    );
    let scripted = format!(
        "<?xml version=\"1.0\"?>{}<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>",
        tag("scripted")
    );
    let html = format!(
        "<html><body>{}<svg xmlns=\"http://www.w3.org/2000/svg\"></svg></body></html>",
        tag("html")
    );
    for (label, bytes) in [
        ("svg", svg),
        ("svg with a script", scripted),
        ("html holding an svg", html),
    ] {
        let got = published(&f, bytes.as_bytes()).await;
        assert_eq!(got.status, StatusCode::OK, "{label}");
        assert_eq!(
            (
                got.header(header::CONTENT_TYPE),
                got.header(header::CONTENT_DISPOSITION),
                got.header(header::X_CONTENT_TYPE_OPTIONS),
                got.header(header::CONTENT_SECURITY_POLICY),
            ),
            (
                "application/octet-stream",
                "attachment",
                "nosniff",
                "sandbox"
            ),
            "{label}"
        );
        assert_eq!(got.body, bytes.as_bytes(), "{label}");
    }
}
