//! The MLS delivery service (`/mls/*`, every route signed by the caller's
//! wallet): key packages are published and claimed once each, groups carry
//! only routing metadata and an authorization roster, commits advance the
//! epoch one at a time on the group's epoch-author catalyst, and application
//! ciphertexts are appended to a group's history. Commit, welcome and
//! ciphertext bytes live in the ONE blob store under their CID; the wire
//! still names them by sha256 (`commit_hash`, `ciphertext_hash`).
//!
//! A community-scoped group can only be created with the platform service
//! token (a wallet signature proves no community membership).
//!
//! The service is social's (private messaging; the `social.mls_*` tables),
//! so it stays here when the gatekeeper it shipped in moved to
//! `comms::gatekeeper`, and it owns its state ([`MessagingState`]): the ONE
//! pool, the ONE blob store, the EIP-1654 validator, the platform service
//! token and this catalyst's federation identity (the epoch author). From
//! the gatekeeper it takes the route contract only: `require_signer`,
//! `ApiError`; [`router`] keeps the body limit, NUL guard and CORS. `serve` merges
//! it into the `/comms-gatekeeper` nest, so the routes answer exactly as
//! when they were part of that router.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Extension, OriginalUri, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::Address;
use crate::auth::eip1654::Eip1654Validator;
use crate::comms::gatekeeper::auth::require_signer;
use crate::comms::gatekeeper::errors::{ApiError, forbidden, service_unavailable, unauthorized};
use crate::mls;
use crate::shared::now_utc;
use crate::shared::signed_fetch::SignedPath;

/// The settings `serve` resolves from `Config`.
#[derive(Clone, Default)]
pub struct Settings {
    /// `FED_PEER_ID`: this catalyst's federation identity, the epoch author
    /// of the MLS groups created here; unset falls back to the id
    /// `social.fed_instance_identity` minted at schema apply.
    pub fed_peer_id: Option<String>,
    /// `COMMS_GATEKEEPER_AUTH_TOKEN`: the platform service token, the only
    /// proof of community membership a community-scoped group accepts.
    pub gatekeeper_auth_token: Option<String>,
}

/// Masks the service token.
impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("fed_peer_id", &self.fed_peer_id)
            .field(
                "gatekeeper_auth_token",
                &self.gatekeeper_auth_token.as_ref().map(|_| "<set>"),
            )
            .finish()
    }
}

/// What the MLS routes read.
pub struct MessagingState {
    pub pool: PgPool,
    /// The ONE blob store: commit, welcome and ciphertext bytes under their
    /// CID.
    pub storage: Arc<crate::storage::Storage>,
    /// Contract-wallet (EIP-1654) signature check for signed-fetch chains;
    /// `None` refuses such chains.
    pub eip1654: Option<Arc<dyn Eip1654Validator>>,
    pub settings: Settings,
    /// `FED_PEER_ID`, else the persisted instance id, read once.
    peer_id: tokio::sync::OnceCell<String>,
}

pub type SharedState = Arc<MessagingState>;

impl MessagingState {
    pub fn new(
        pool: PgPool,
        storage: Arc<crate::storage::Storage>,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
        settings: Settings,
    ) -> Self {
        Self {
            pool,
            storage,
            eip1654,
            settings,
            peer_id: tokio::sync::OnceCell::new(),
        }
    }

    pub fn eip1654(&self) -> Option<&dyn Eip1654Validator> {
        self.eip1654.as_deref()
    }

    /// This catalyst's federation identity (the MLS epoch author).
    pub async fn peer_id(&self) -> Result<String, sqlx::Error> {
        self.peer_id
            .get_or_try_init(|| async {
                match self.settings.fed_peer_id.clone() {
                    Some(id) => Ok(id),
                    None => {
                        sqlx::query_scalar("SELECT peer_id FROM social.fed_instance_identity")
                            .fetch_one(&self.pool)
                            .await
                    }
                }
            })
            .await
            .cloned()
    }
}

/// MLS routes with the body limit, NUL guard and CORS. Signatures and
/// community-scoped service credentials are enforced by their handlers.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/mls/key-packages", post(publish_key_packages))
        .route("/mls/key-packages/{owner}", get(claim_key_package))
        .route("/mls/key-packages/{owner}/count", get(key_package_count))
        .route("/mls/groups", post(create_group))
        .route(
            "/mls/groups/{group_id}/commits",
            get(fetch_commits).post(submit_commit),
        )
        .route(
            "/mls/groups/{group_id}/messages",
            get(fetch_history).post(send_message),
        )
        .route("/mls/blobs/{hash}", get(fetch_blob))
        .layer(axum::extract::DefaultBodyLimit::max(512 * 1024))
        .layer(axum::middleware::from_fn(
            crate::shared::nul_guard::nul_guard_middleware,
        ))
        .layer(axum::middleware::from_fn(
            crate::shared::cors::cors_middleware,
        ))
        .with_state(state)
}

const MAX_OPAQUE_BYTES: usize = 256 * 1024;
const MAX_KEY_PACKAGES: usize = 100;
const MAX_COMMITS_PAGE: i64 = 500;
const MEMBER_EXISTS: &str = "EXISTS (SELECT 1 FROM social.mls_group_members \
     WHERE group_id = $1 AND member = $2 AND removed_epoch IS NULL)";

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn decode_b64(value: &str, what: &str) -> Result<Vec<u8>, ApiError> {
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| ApiError::bad_request(format!("{what} is not valid base64")))
}

fn b64_field(v: &Value, key: &str) -> Result<Vec<u8>, ApiError> {
    let s = v
        .get(key)
        .and_then(|x| x.as_str())
        .ok_or_else(|| ApiError::bad_request(format!("missing base64 field `{key}`")))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|_| ApiError::bad_request(format!("field `{key}` is not valid base64")))?;
    if bytes.is_empty() || bytes.len() > MAX_OPAQUE_BYTES {
        return Err(ApiError::bad_request(format!(
            "field `{key}` length out of range"
        )));
    }
    Ok(bytes)
}

async fn auth(
    state: &MessagingState,
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
) -> Result<String, ApiError> {
    require_signer(headers, method, path, state.eip1654())
        .await
        .map(Address::into_string)
        .map_err(|e| unauthorized(format!("invalid identity: {}", e.message)))
}

fn authorized_community_binding(
    state: &MessagingState,
    headers: &HeaderMap,
    requested: Option<&str>,
) -> Result<Option<uuid::Uuid>, ApiError> {
    let Some(requested) = requested.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let presented_service_token = crate::auth::admin_bearer::matches(
        state.settings.gatekeeper_auth_token.as_deref(),
        headers,
    );
    if !presented_service_token {
        return Err(forbidden(
            "a wallet signature cannot prove community membership; community-scoped groups may only be created with the platform service token",
        ));
    }
    uuid::Uuid::parse_str(requested)
        .map(Some)
        .map_err(|_| ApiError::bad_request("community_id must be a uuid"))
}

async fn store_blob(state: &MessagingState, bytes: Vec<u8>) -> Result<String, ApiError> {
    let storage = &state.storage;
    let bytes = Bytes::from(bytes);
    let cid = {
        let body = bytes.clone();
        tokio::task::spawn_blocking(move || crate::auth::hash::hash_bytes_v1(&body))
            .await
            .map_err(|e| ApiError::internal(format!("mls blob hash task: {e}")))?
    };
    let stored = async {
        if !storage.exist(&cid).await? {
            storage.store(&cid, bytes).await?;
        }
        Ok::<_, crate::storage::StorageError>(())
    }
    .await;
    if let Err(e) = stored {
        tracing::error!(error = %e, "mls blob store failed");
        return Err(service_unavailable("The message store is unavailable"));
    }
    Ok(cid)
}

async fn load_blob(state: &MessagingState, cid: &str) -> Result<Bytes, ApiError> {
    match state.storage.retrieve(cid).await {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => {
            tracing::error!(cid, "an mls row names a blob the store does not hold");
            Err(service_unavailable("The message store is unavailable"))
        }
        Err(e) => {
            tracing::error!(error = %e, "mls blob read failed");
            Err(service_unavailable("The message store is unavailable"))
        }
    }
}

pub async fn publish_key_packages(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let signer = auth(
        &state,
        &headers,
        "post",
        &SignedPath::at(&uri, "/mls/key-packages"),
    )
    .await?;
    let payload: Value =
        serde_json::from_slice(&body).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let arr = payload
        .get("key_packages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ApiError::bad_request("missing `key_packages` array"))?;
    if arr.is_empty() || arr.len() > MAX_KEY_PACKAGES {
        return Err(ApiError::bad_request(
            "`key_packages` must contain 1..=100 entries",
        ));
    }
    let mut refs: Vec<String> = Vec::new();
    let mut ciphersuites: Vec<i32> = Vec::new();
    let mut packages: Vec<Vec<u8>> = Vec::new();
    for entry in arr {
        let s = entry.as_str().ok_or_else(|| {
            ApiError::bad_request("`key_packages` entries must be base64 strings")
        })?;
        let bytes = decode_b64(s, "key package")?;
        if bytes.is_empty() || bytes.len() > MAX_OPAQUE_BYTES {
            return Err(ApiError::bad_request("key package length out of range"));
        }
        let parsed =
            mls::parse_key_package(&bytes).map_err(|e| ApiError::bad_request(e.to_string()))?;
        let cred = String::from_utf8_lossy(&parsed.credential_identity)
            .trim()
            .to_lowercase();
        if !cred.is_empty() && cred != signer && cred != signer.trim_start_matches("0x") {
            return Err(forbidden(
                "key package credential identity does not match the authenticated wallet",
            ));
        }
        refs.push(parsed.ref_hash);
        ciphersuites.push(parsed.ciphersuite_id as i32);
        packages.push(bytes);
    }
    sqlx::query(
        "INSERT INTO social.mls_key_packages (owner, ref_hash, ciphersuite, key_package, created_at) \
         SELECT $1, k.ref_hash, k.ciphersuite, k.key_package, $5 \
         FROM unnest($2::text[], $3::int4[], $4::bytea[]) AS k(ref_hash, ciphersuite, key_package) \
         ON CONFLICT (ref_hash) DO NOTHING",
    )
    .bind(&signer)
    .bind(&refs)
    .bind(&ciphersuites)
    .bind(&packages)
    .bind(now_utc())
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({ "published": refs.len(), "refs": refs })))
}

/// `GET /mls/key-packages/{owner}` consumes the package it returns: a
/// social write behind the social switch and the restore stamps.
pub async fn claim_key_package(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    switch: Option<Extension<crate::comms::writes::Switch>>,
    Path(owner): Path<String>,
) -> Result<Response, ApiError> {
    let owner = owner.to_lowercase();
    if !crate::auth::is_eth_address(&owner) {
        return Err(ApiError::bad_request("owner must be an eth address"));
    }
    auth(
        &state,
        &headers,
        "get",
        &SignedPath::at(&uri, format!("/mls/key-packages/{owner}")),
    )
    .await?;
    if let Some(refusal) = crate::comms::writes::side_refusal(switch) {
        return Ok(crate::writes::refused_response(&refusal));
    }
    let row: Option<(String, Vec<u8>, i32)> = sqlx::query_as(
        "UPDATE social.mls_key_packages SET consumed_at = $2 \
         WHERE id = ( \
            SELECT id FROM social.mls_key_packages \
            WHERE owner = $1 AND consumed_at IS NULL \
            ORDER BY created_at ASC LIMIT 1 FOR UPDATE SKIP LOCKED \
         ) RETURNING ref_hash, key_package, ciphersuite",
    )
    .bind(&owner)
    .bind(now_utc())
    .fetch_optional(&state.pool)
    .await?;
    match row {
        Some((ref_hash, kp, cs)) => Ok(Json(
            json!({ "owner": owner, "ref": ref_hash, "ciphersuite": cs, "key_package": b64(&kp) }),
        )
        .into_response()),
        None => Err(ApiError::labeled(
            404,
            "no key package available",
            format!("no key package available for {owner}"),
        )),
    }
}

pub async fn key_package_count(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(owner): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let owner = owner.to_lowercase();
    if !crate::auth::is_eth_address(&owner) {
        return Err(ApiError::bad_request("owner must be an eth address"));
    }
    auth(
        &state,
        &headers,
        "get",
        &SignedPath::at(&uri, format!("/mls/key-packages/{owner}/count")),
    )
    .await?;
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.mls_key_packages WHERE owner = $1 AND consumed_at IS NULL",
    )
    .bind(&owner)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(json!({ "owner": owner, "available": n })))
}

#[derive(Debug, Deserialize)]
pub struct CreateGroupBody {
    pub group_id: String,
    pub group_kind: String,
    #[serde(default)]
    pub community_id: Option<String>,
    pub initial_members: Vec<String>,
    #[serde(default)]
    pub initial_commit: Option<String>,
    #[serde(default)]
    pub welcome: Option<String>,
}

pub async fn create_group(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let creator = auth(
        &state,
        &headers,
        "post",
        &SignedPath::at(&uri, "/mls/groups"),
    )
    .await?;
    let b: CreateGroupBody =
        serde_json::from_slice(&body).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let group_id = b.group_id.to_lowercase();
    if group_id.is_empty() || hex::decode(&group_id).is_err() {
        return Err(ApiError::bad_request("group_id must be hex"));
    }
    if b.group_kind != "dm" && b.group_kind != "channel" {
        return Err(ApiError::bad_request(
            "group_kind must be 'dm' or 'channel'",
        ));
    }
    let members: Vec<String> = b.initial_members.iter().map(|m| m.to_lowercase()).collect();
    if members.iter().any(|m| !crate::auth::is_eth_address(m)) {
        return Err(ApiError::bad_request(
            "initial_members must be eth addresses",
        ));
    }
    if !members.contains(&creator) {
        return Err(forbidden("creator must be among initial_members"));
    }
    if b.group_kind == "dm" && members.len() != 2 {
        return Err(ApiError::bad_request(
            "a 'dm' group must have exactly two members",
        ));
    }
    let community_id = authorized_community_binding(&state, &headers, b.community_id.as_deref())?;
    let epoch_author = state.peer_id().await?;
    let initial = match &b.initial_commit {
        Some(c) => {
            let commit_bytes = decode_b64(c, "initial_commit")?;
            mls::parse_commit_routing(&commit_bytes)
                .map_err(|e| ApiError::bad_request(e.to_string()))?;
            let welcome_bytes = b
                .welcome
                .as_deref()
                .map(|w| decode_b64(w, "welcome"))
                .transpose()?;
            let commit_hash = mls::content_hash(&commit_bytes);
            let commit_cid = store_blob(&state, commit_bytes).await?;
            let welcome_cid = match welcome_bytes {
                Some(w) => Some(store_blob(&state, w).await?),
                None => None,
            };
            Some((commit_cid, welcome_cid, commit_hash))
        }
        None => None,
    };
    let (commit_cid, welcome_cid, commit_hash) = match initial {
        Some((c, w, h)) => (Some(c), w, Some(h)),
        None => (None, None, None),
    };
    let now = now_utc();
    let created: bool = sqlx::query_scalar(
        "WITH g AS ( \
            INSERT INTO social.mls_groups \
                (group_id, creator, group_kind, community_id, epoch_author, current_epoch, ciphersuite, created_at, updated_at) \
            VALUES ($1, $2, $3, $4, $5, 0, $6, $12, $12) ON CONFLICT (group_id) DO NOTHING \
            RETURNING group_id \
         ), m AS ( \
            INSERT INTO social.mls_group_members (group_id, member, added_epoch) \
            SELECT g.group_id, x.member, 0 FROM g, unnest($7::text[]) AS x(member) \
            ON CONFLICT DO NOTHING \
         ), c AS ( \
            INSERT INTO social.mls_commits \
                (group_id, epoch, commit_cid, welcome_cid, committer, commit_hash, signed_at, received_at) \
            SELECT g.group_id, 0, $8, $9, $2, $10, $11, $12 FROM g WHERE $8::text IS NOT NULL \
            ON CONFLICT DO NOTHING \
         ) \
         SELECT EXISTS (SELECT 1 FROM g)",
    )
    .bind(&group_id)
    .bind(&creator)
    .bind(&b.group_kind)
    .bind(community_id)
    .bind(&epoch_author)
    .bind(mls::PINNED_CIPHERSUITE_ID as i32)
    .bind(&members)
    .bind(&commit_cid)
    .bind(&welcome_cid)
    .bind(&commit_hash)
    .bind(chrono::Utc::now().timestamp())
    .bind(now)
    .fetch_one(&state.pool)
    .await?;
    if !created {
        return Err(ApiError::http(409, "group already exists"));
    }
    Ok(Json(json!({
        "group_id": group_id,
        "group_kind": b.group_kind,
        "epoch_author": epoch_author,
        "current_epoch": 0,
        "ciphersuite": mls::PINNED_CIPHERSUITE_ID,
        "members": members,
    })))
}

#[derive(Debug, Deserialize)]
pub struct CommitBody {
    pub epoch: i64,
    pub commit: String,
    #[serde(default)]
    pub welcome: Option<String>,
    #[serde(default)]
    pub added_members: Vec<String>,
    #[serde(default)]
    pub removed_members: Vec<String>,
}

/// Advances the epoch by exactly one, on the group's epoch-author catalyst
/// only, for a current member; the guard re-reads the group row under
/// `FOR UPDATE` so two concurrent commits cannot both land.
pub async fn submit_commit(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(group_id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let group_id = group_id.to_lowercase();
    let signer = auth(
        &state,
        &headers,
        "post",
        &SignedPath::at(&uri, format!("/mls/groups/{group_id}/commits")),
    )
    .await?;
    let b: CommitBody =
        serde_json::from_slice(&body).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let (epoch_author, current_epoch, is_member): (String, i64, bool) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT g.epoch_author, g.current_epoch, {MEMBER_EXISTS} FROM social.mls_groups g WHERE g.group_id = $1"
    )))
    .bind(&group_id)
    .bind(&signer)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("group not found"))?;
    if !is_member {
        return Err(forbidden("only group members may submit commits"));
    }
    if epoch_author != state.peer_id().await? {
        return Err(ApiError::labeled(
            409,
            "wrong epoch author",
            format!(
                "submit epoch-advancing commits to the group's epoch-author catalyst ({epoch_author})"
            ),
        ));
    }
    if b.epoch != current_epoch + 1 {
        return Err(ApiError::labeled(
            409,
            "epoch conflict",
            format!(
                "commit epoch must be current_epoch + 1 (current_epoch={current_epoch}, submitted_epoch={})",
                b.epoch
            ),
        ));
    }
    let commit_bytes = decode_b64(&b.commit, "commit")?;
    if commit_bytes.len() > MAX_OPAQUE_BYTES {
        return Err(ApiError::bad_request("commit too large"));
    }
    mls::parse_commit_routing(&commit_bytes).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let welcome_bytes = b
        .welcome
        .as_deref()
        .map(|w| decode_b64(w, "welcome"))
        .transpose()?;
    let commit_hash = mls::content_hash(&commit_bytes);
    let commit_cid = store_blob(&state, commit_bytes).await?;
    let welcome_cid = match welcome_bytes {
        Some(w) => Some(store_blob(&state, w).await?),
        None => None,
    };
    let mut added: Vec<String> = b
        .added_members
        .iter()
        .map(|m| m.to_lowercase())
        .filter(|m| crate::auth::is_eth_address(m))
        .collect();
    added.sort();
    added.dedup();
    let removed: Vec<String> = b.removed_members.iter().map(|m| m.to_lowercase()).collect();
    let now = now_utc();
    let locked: i64 = sqlx::query_scalar(
        "WITH g AS ( \
            SELECT current_epoch FROM social.mls_groups WHERE group_id = $1 FOR UPDATE \
         ), ok AS ( \
            SELECT current_epoch FROM g WHERE current_epoch + 1 = $2 \
         ), c AS ( \
            INSERT INTO social.mls_commits \
                (group_id, epoch, commit_cid, welcome_cid, committer, commit_hash, signed_at, received_at) \
            SELECT $1, $2, $3, $4, $5, $6, $7, $10 FROM ok \
         ), u AS ( \
            UPDATE social.mls_groups SET current_epoch = $2, last_commit_hash = $6, updated_at = $10 \
            FROM ok WHERE social.mls_groups.group_id = $1 \
         ), a AS ( \
            INSERT INTO social.mls_group_members (group_id, member, added_epoch) \
            SELECT $1, x.member, $2 FROM ok, unnest($8::text[]) AS x(member) \
            ON CONFLICT (group_id, member) DO UPDATE SET removed_epoch = NULL, added_epoch = EXCLUDED.added_epoch \
         ), r AS ( \
            UPDATE social.mls_group_members SET removed_epoch = $2 \
            FROM ok WHERE group_id = $1 AND member = ANY($9::text[]) AND removed_epoch IS NULL \
         ) \
         SELECT current_epoch FROM g",
    )
    .bind(&group_id)
    .bind(b.epoch)
    .bind(&commit_cid)
    .bind(&welcome_cid)
    .bind(&signer)
    .bind(&commit_hash)
    .bind(chrono::Utc::now().timestamp())
    .bind(&added)
    .bind(&removed)
    .bind(now)
    .fetch_one(&state.pool)
    .await?;
    if b.epoch != locked + 1 {
        return Err(ApiError::http(409, "epoch advanced concurrently; retry"));
    }
    Ok(Json(
        json!({ "group_id": group_id, "epoch": b.epoch, "commit_hash": commit_hash }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct CommitsQuery {
    #[serde(default)]
    pub from: i64,
}

pub async fn fetch_commits(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(group_id): Path<String>,
    Query(q): Query<CommitsQuery>,
) -> Result<Json<Value>, ApiError> {
    let group_id = group_id.to_lowercase();
    let signer = auth(
        &state,
        &headers,
        "get",
        &SignedPath::at(&uri, format!("/mls/groups/{group_id}/commits")),
    )
    .await?;
    type Row = (
        bool,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
    );
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT m.ok, c.epoch, c.commit_cid, c.welcome_cid, c.commit_hash, c.signed_at \
         FROM (SELECT {MEMBER_EXISTS} AS ok) m \
         LEFT JOIN LATERAL ( \
            SELECT epoch, commit_cid, welcome_cid, commit_hash, signed_at \
            FROM social.mls_commits WHERE m.ok AND group_id = $1 AND epoch >= $3 \
            ORDER BY epoch ASC LIMIT $4 \
         ) c ON true ORDER BY c.epoch ASC"
    )))
    .bind(&group_id)
    .bind(&signer)
    .bind(q.from)
    .bind(MAX_COMMITS_PAGE)
    .fetch_all(&state.pool)
    .await?;
    if !rows.first().is_some_and(|row| row.0) {
        return Err(forbidden("only group members may fetch commits"));
    }
    let mut commits = Vec::with_capacity(rows.len());
    for (_, epoch, commit_cid, welcome_cid, hash, signed_at) in rows {
        let (Some(epoch), Some(commit_cid), Some(hash), Some(signed_at)) =
            (epoch, commit_cid, hash, signed_at)
        else {
            continue;
        };
        let commit = load_blob(&state, &commit_cid).await?;
        let welcome = match welcome_cid {
            Some(cid) => Some(b64(&load_blob(&state, &cid).await?)),
            None => None,
        };
        commits.push(json!({ "epoch": epoch, "commit": b64(&commit), "welcome": welcome, "commit_hash": hash, "signed_at": signed_at }));
    }
    Ok(Json(json!({ "group_id": group_id, "commits": commits })))
}

pub async fn send_message(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(group_id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let group_id = group_id.to_lowercase();
    let signer = auth(
        &state,
        &headers,
        "post",
        &SignedPath::at(&uri, format!("/mls/groups/{group_id}/messages")),
    )
    .await?;
    let row: Option<(i64, bool)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT g.current_epoch, {MEMBER_EXISTS} FROM social.mls_groups g WHERE g.group_id = $1"
    )))
    .bind(&group_id)
    .bind(&signer)
    .fetch_optional(&state.pool)
    .await?;
    let Some((current_epoch, is_member)) = row else {
        return Err(ApiError::not_found("group not found"));
    };
    if !is_member {
        return Err(forbidden("only group members may send messages"));
    }
    let payload: Value =
        serde_json::from_slice(&body).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let ciphertext = b64_field(&payload, "ciphertext")?;
    let routing = mls::parse_message_routing(&ciphertext)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    if routing.group_id_hex.to_lowercase() != group_id {
        return Err(ApiError::bad_request(
            "ciphertext group_id does not match the URL group",
        ));
    }
    if routing.epoch as i64 > current_epoch {
        return Err(ApiError::labeled(
            409,
            "epoch ahead",
            format!(
                "message epoch is ahead of the group's current epoch; submit the commit first (current_epoch={current_epoch}, message_epoch={})",
                routing.epoch
            ),
        ));
    }
    let ciphertext_hash = mls::content_hash(&ciphertext);
    let now = chrono::Utc::now().timestamp();
    let signature_hash = mls::content_hash(
        format!("{group_id}:{}:{signer}:{ciphertext_hash}", routing.epoch).as_bytes(),
    );
    let ciphertext_cid = store_blob(&state, ciphertext).await?;
    sqlx::query(
        "INSERT INTO social.mls_message_refs \
            (signature_hash, group_id, author, epoch, ciphertext_hash, ciphertext_cid, signed_at, received_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (signature_hash) DO NOTHING",
    )
    .bind(&signature_hash)
    .bind(&group_id)
    .bind(&signer)
    .bind(routing.epoch as i64)
    .bind(&ciphertext_hash)
    .bind(&ciphertext_cid)
    .bind(now)
    .bind(now_utc())
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({
        "group_id": group_id,
        "epoch": routing.epoch,
        "ciphertext_hash": ciphertext_hash,
        "signature_hash": signature_hash,
        "signed_at": now,
    })))
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    #[serde(default)]
    pub before: Option<i64>,
    #[serde(default)]
    pub limit: Option<i64>,
}

/// The old `extract(epoch FROM received_at)::bigint < before` (whole seconds,
/// rounded) as a bound the history index can serve: `received_at <
/// before - 0.5 s`. `None` (no or an out-of-range `before`) is unbounded.
fn history_bound(before: Option<i64>) -> Option<NaiveDateTime> {
    let before = before?;
    let at = chrono::DateTime::from_timestamp(before, 0)?.naive_utc();
    at.checked_sub_signed(chrono::TimeDelta::milliseconds(500))
}

pub async fn fetch_history(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(group_id): Path<String>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Value>, ApiError> {
    let group_id = group_id.to_lowercase();
    let signer = auth(
        &state,
        &headers,
        "get",
        &SignedPath::at(&uri, format!("/mls/groups/{group_id}/messages")),
    )
    .await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    type Row = (
        bool,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<String>,
    );
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT m.ok, r.signature_hash, r.author, r.epoch, r.signed_at, r.ciphertext_cid \
         FROM (SELECT {MEMBER_EXISTS} AS ok) m \
         LEFT JOIN LATERAL ( \
            SELECT signature_hash, author, epoch, signed_at, received_at, ciphertext_cid \
            FROM social.mls_message_refs \
            WHERE m.ok AND group_id = $1 AND ($3::timestamp IS NULL OR received_at < $3) \
            ORDER BY received_at DESC LIMIT $4 \
         ) r ON true ORDER BY r.received_at DESC"
    )))
    .bind(&group_id)
    .bind(&signer)
    .bind(history_bound(q.before))
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    if !rows.first().is_some_and(|row| row.0) {
        return Err(forbidden("only group members may fetch history"));
    }
    let mut messages = Vec::with_capacity(rows.len());
    for (_, sig, author, epoch, signed_at, cid) in rows {
        let (Some(sig), Some(author), Some(epoch), Some(signed_at), Some(cid)) =
            (sig, author, epoch, signed_at, cid)
        else {
            continue;
        };
        let ciphertext = load_blob(&state, &cid).await?;
        messages.push(json!({ "signature_hash": sig, "author": author, "epoch": epoch, "signed_at": signed_at, "ciphertext": b64(&ciphertext) }));
    }
    Ok(Json(json!({ "group_id": group_id, "messages": messages })))
}

pub async fn fetch_blob(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(hash): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let hash = hash.to_lowercase();
    let signer = auth(
        &state,
        &headers,
        "get",
        &SignedPath::at(&uri, format!("/mls/blobs/{hash}")),
    )
    .await?;
    let (allowed, cid): (bool, Option<String>) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM social.mls_message_refs r \
                        JOIN social.mls_group_members m ON m.group_id = r.group_id \
                        WHERE r.ciphertext_hash = $1 AND m.member = $2 AND m.removed_epoch IS NULL), \
                (SELECT ciphertext_cid FROM social.mls_message_refs WHERE ciphertext_hash = $1 LIMIT 1)",
    )
    .bind(&hash)
    .bind(&signer)
    .fetch_one(&state.pool)
    .await?;
    if !allowed {
        return Err(forbidden("not authorized for this blob"));
    }
    match cid {
        Some(cid) => Ok(Json(
            json!({ "hash": hash, "ciphertext": b64(&load_blob(&state, &cid).await?) }),
        )),
        None => Err(ApiError::not_found("blob not found")),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use openmls::prelude::*;
    use openmls_basic_credential::SignatureKeyPair;
    use openmls_rust_crypto::OpenMlsRustCrypto;
    use tls_codec::Serialize as _;
    use tower::ServiceExt;

    use super::*;
    use crate::comms::gatekeeper::{self as gatekeeper, CommsState};
    use crate::comms::interfaces::test_support::{StubPolicy, StubScenes};
    use crate::http::Outbound;
    use crate::shared::test_keys::{address_of, random_key, signed};

    struct Member {
        key: String,
        address: String,
        provider: OpenMlsRustCrypto,
        signer: SignatureKeyPair,
        credential: CredentialWithKey,
    }

    fn member() -> Member {
        let key = random_key();
        let address = address_of(&key);
        let provider = OpenMlsRustCrypto::default();
        let signer = SignatureKeyPair::new(mls::PINNED_CIPHERSUITE.signature_algorithm()).unwrap();
        signer.store(provider.storage()).unwrap();
        let credential = CredentialWithKey {
            credential: BasicCredential::new(address.clone().into_bytes()).into(),
            signature_key: signer.public().into(),
        };
        Member {
            key,
            address,
            provider,
            signer,
            credential,
        }
    }

    fn key_package(m: &Member) -> Vec<u8> {
        let bundle = KeyPackage::builder()
            .build(
                mls::PINNED_CIPHERSUITE,
                &m.provider,
                &m.signer,
                m.credential.clone(),
            )
            .unwrap();
        MlsMessageOut::from(bundle.key_package().clone())
            .tls_serialize_detached()
            .unwrap()
    }

    async fn app(peer: &str, pool: sqlx::PgPool, dir: &std::path::Path) -> axum::Router {
        app_with(peer, pool, dir, true).await
    }

    /// The gatekeeper router with these routes behind the social gate, as
    /// `serve` mounts them.
    async fn app_with(
        peer: &str,
        pool: sqlx::PgPool,
        dir: &std::path::Path,
        writes: bool,
    ) -> axum::Router {
        let storage = Arc::new(crate::storage::Storage::from_config(dir).await.unwrap());
        let messaging = Arc::new(MessagingState::new(
            pool.clone(),
            storage,
            None,
            Settings {
                fed_peer_id: Some(peer.to_string()),
                gatekeeper_auth_token: Some("svc".into()),
            },
        ));
        let comms = Arc::new(CommsState::new(
            pool,
            Outbound::for_tests(),
            gatekeeper::Settings {
                gatekeeper_auth_token: Some("svc".into()),
                ..gatekeeper::Settings::for_tests()
            },
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::comms::presence::Presence::new()),
            Arc::new(catalyst_comms::interfaces::test_support::StubDirectory::default()),
        ));
        crate::writes::guard(
            gatekeeper::router(comms.clone()).merge(router(messaging)),
            writes,
        )
    }

    /// A claim consumes the package it returns: off or held it answers the
    /// social 503 and consumes nothing.
    #[tokio::test]
    async fn a_key_package_claim_is_a_social_write() {
        use crate::shared::writes::restore::{
            lab::{HOUR, StampLab},
            scoped,
        };
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let dir =
            std::env::temp_dir().join(format!("mls-claim-gate-{}", uuid::Uuid::new_v4().simple()));
        let alice = member();
        let bob = member();
        let claim_path = format!("/mls/key-packages/{}", bob.address);
        let off = app_with("peer-off", pool.clone(), &dir, false).await;
        let (status, body) = call(&off, &alice.key, "GET", &claim_path, None, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["message"], crate::writes::DISABLED_MESSAGE);
        let on = app_with("peer-on", pool.clone(), &dir, true).await;
        let lab = StampLab::fresh();
        scoped(lab.stamps.clone(), async {
            let (status, _) = call(&on, &alice.key, "GET", &claim_path, None, None).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "fresh stamps reach the store");
            lab.advance(30 * HOUR);
            let (status, body) = call(&on, &alice.key, "GET", &claim_path, None, None).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(
                body["message"],
                "social writes are held on this node: the last complete backup (backup.json) is 31 h old, past WRITES_BACKUP_MAX_AGE_HOURS=26"
            );
        })
        .await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn call(
        app: &axum::Router,
        key: &str,
        method: &str,
        path: &str,
        body: Option<Value>,
        bearer: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        let signed_path = path.split('?').next().unwrap_or(path);
        for (k, v) in signed(key, &method.to_ascii_lowercase(), signed_path, "{}")
            .await
            .iter()
        {
            req = req.header(k, v);
        }
        if let Some(token) = bearer {
            req = req.header("authorization", format!("Bearer {token}"));
        }
        let body = body
            .map(|b| Body::from(b.to_string()))
            .unwrap_or_else(Body::empty);
        let resp = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 22)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// Unset `FED_PEER_ID` falls back to the id schema apply minted (the old
    /// 0009 migration's fix: never the literal "local" every instance shared).
    #[tokio::test]
    async fn the_epoch_author_defaults_to_the_persisted_instance_id() {
        let Some(pool) = crate::db::scratch::pool(1).await else {
            return;
        };
        let dir = std::env::temp_dir().join(format!(
            "mls-epoch-author-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let storage = Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
        let state = MessagingState::new(pool.clone(), storage.clone(), None, Settings::default());
        let persisted: String =
            sqlx::query_scalar("SELECT peer_id FROM social.fed_instance_identity")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(persisted.starts_with("instance-"), "{persisted}");
        assert_eq!(state.peer_id().await.unwrap(), persisted);
        let named = MessagingState::new(
            pool,
            storage,
            None,
            Settings {
                fed_peer_id: Some("peer.example".into()),
                ..Settings::default()
            },
        );
        assert_eq!(named.peer_id().await.unwrap(), "peer.example");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn mls_router_keeps_handler_auth_and_invalid_owner_precedence() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(20))
            .connect_lazy("postgres://postgres@127.0.0.1:1/postgres")
            .unwrap();
        let dir = std::env::temp_dir().join(format!("mls-route-auth-{}", uuid::Uuid::new_v4()));
        let app = app_with("fixture", pool.clone(), &dir, true).await;
        // These malformed owners previously hit an incidental substring token gate.
        for owner in ["voice-chat", "private-messages-privacy", "not-an-address"] {
            let response = app
                .clone()
                .oneshot(
                    Request::get(format!("/mls/key-packages/{owner}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        for path in [
            "/mls/key-packages/0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "/mls/blobs/voice-chat",
        ] {
            let response = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        let response = app
            .clone()
            .oneshot(
                Request::post("/mls/groups")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let off = app_with("fixture", pool, &dir, false).await;
        let (status, _) = call(
            &off,
            &random_key(),
            "GET",
            "/mls/key-packages/0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn community_binding_still_requires_its_own_platform_credential() {
        let pool = PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
        let dir = std::env::temp_dir().join(format!("mls-community-auth-{}", uuid::Uuid::new_v4()));
        let storage = Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
        let state = MessagingState::new(
            pool,
            storage,
            None,
            Settings {
                gatekeeper_auth_token: Some("svc".into()),
                ..Settings::default()
            },
        );
        let id = uuid::Uuid::new_v4().to_string();
        let mut headers = HeaderMap::new();
        assert_eq!(
            authorized_community_binding(&state, &headers, None).unwrap(),
            None
        );
        assert_eq!(
            authorized_community_binding(&state, &headers, Some(&id))
                .unwrap_err()
                .code,
            403
        );
        headers.insert("authorization", "Bearer wrong".parse().unwrap());
        assert_eq!(
            authorized_community_binding(&state, &headers, Some(&id))
                .unwrap_err()
                .code,
            403
        );
        headers.insert("authorization", "Bearer svc".parse().unwrap());
        assert_eq!(
            authorized_community_binding(&state, &headers, Some(&id)).unwrap(),
            Some(uuid::Uuid::parse_str(&id).unwrap())
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn history_bound_is_the_old_rounded_seconds_comparison() {
        assert_eq!(history_bound(None), None);
        assert_eq!(
            history_bound(Some(i64::MAX)),
            None,
            "the old default (no bound)"
        );
        let bound = history_bound(Some(1_700_000_000)).unwrap();
        assert_eq!(bound.and_utc().timestamp_millis(), 1_700_000_000_000 - 500);
    }

    /// Alice publishes key packages (only under her own identity), Bob claims
    /// one exactly once, Alice creates the DM, commits Bob in on this
    /// epoch-author node (another node refuses the commit), sends a real
    /// ciphertext; Bob reads the commits, the history and the blob, an
    /// outsider reads nothing; a community channel needs the service token.
    #[tokio::test]
    async fn a_dm_group_round_trips_key_packages_commits_and_messages() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let dir = std::env::temp_dir().join(format!(
            "catalyst-mls-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let here = app("peer-here", pool.clone(), &dir).await;
        let there = app("peer-there", pool.clone(), &dir).await;
        let alice = member();
        let bob = member();
        let outsider = random_key();

        let (status, _) = call(
            &here,
            &bob.key,
            "POST",
            "/mls/key-packages",
            Some(json!({ "key_packages": [b64(&key_package(&alice))] })),
            None,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "a key package is published only by the wallet it names"
        );
        let (status, _) = call(
            &here,
            &bob.key,
            "POST",
            "/mls/key-packages",
            Some(json!({ "key_packages": [b64(b"garbage")] })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, body) = call(
            &here,
            &bob.key,
            "POST",
            "/mls/key-packages",
            Some(json!({ "key_packages": [b64(&key_package(&bob))] })),
            None,
        )
        .await;
        assert_eq!(
            (status, body["published"].as_i64()),
            (StatusCode::OK, Some(1)),
            "{body}"
        );
        let count_path = format!("/mls/key-packages/{}/count", bob.address);
        let (_, body) = call(&here, &alice.key, "GET", &count_path, None, None).await;
        assert_eq!(body["available"], 1);
        let claim_path = format!("/mls/key-packages/{}", bob.address);
        let (status, claimed) = call(&here, &alice.key, "GET", &claim_path, None, None).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = call(&here, &alice.key, "GET", &claim_path, None, None).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "a key package is claimed once"
        );

        let bob_kp_bytes = base64::engine::general_purpose::STANDARD
            .decode(claimed["key_package"].as_str().unwrap())
            .unwrap();
        let (msg, _) = MlsMessageIn::tls_deserialize_bytes(&bob_kp_bytes).unwrap();
        let MlsMessageBodyIn::KeyPackage(kp_in) = msg.extract() else {
            panic!("a key package")
        };
        let bob_kp = kp_in
            .validate(alice.provider.crypto(), ProtocolVersion::Mls10)
            .unwrap();
        let config = MlsGroupCreateConfig::builder()
            .ciphersuite(mls::PINNED_CIPHERSUITE)
            .use_ratchet_tree_extension(true)
            .build();
        let mut group = MlsGroup::new(
            &alice.provider,
            &alice.signer,
            &config,
            alice.credential.clone(),
        )
        .unwrap();
        let group_id = hex::encode(group.group_id().as_slice());

        let dm = |members: Vec<&str>| json!({ "group_id": group_id, "group_kind": "dm", "initial_members": members });
        let (status, _) = call(
            &here,
            &alice.key,
            "POST",
            "/mls/groups",
            Some(dm(vec![&bob.address, &address_of(&outsider)])),
            None,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "the creator must be a member"
        );
        let (status, created) = call(
            &here,
            &alice.key,
            "POST",
            "/mls/groups",
            Some(dm(vec![&alice.address, &bob.address])),
            None,
        )
        .await;
        assert_eq!(
            (status, created["epoch_author"].as_str()),
            (StatusCode::OK, Some("peer-here")),
            "{created}"
        );
        let (status, _) = call(
            &here,
            &alice.key,
            "POST",
            "/mls/groups",
            Some(dm(vec![&alice.address, &bob.address])),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        let channel = json!({ "group_id": "ab".repeat(16), "group_kind": "channel", "community_id": uuid::Uuid::new_v4().to_string(), "initial_members": [alice.address] });
        let (status, _) = call(
            &here,
            &alice.key,
            "POST",
            "/mls/groups",
            Some(channel.clone()),
            None,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "a wallet signature proves no community membership"
        );
        let (status, _) = call(
            &here,
            &alice.key,
            "POST",
            "/mls/groups",
            Some(channel),
            Some("svc"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (commit, welcome, _) = group
            .add_members(&alice.provider, &alice.signer, &[bob_kp])
            .unwrap();
        let commit_body = |epoch: i64| json!({ "epoch": epoch, "commit": b64(&commit.tls_serialize_detached().unwrap()), "welcome": b64(&welcome.tls_serialize_detached().unwrap()), "added_members": [bob.address] });
        let commits_path = format!("/mls/groups/{group_id}/commits");
        let (status, body) = call(
            &there,
            &alice.key,
            "POST",
            &commits_path,
            Some(commit_body(1)),
            None,
        )
        .await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::CONFLICT, Some("wrong epoch author"))
        );
        let (status, body) = call(
            &here,
            &alice.key,
            "POST",
            &commits_path,
            Some(commit_body(2)),
            None,
        )
        .await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::CONFLICT, Some("epoch conflict"))
        );
        let (status, _) = call(
            &here,
            &outsider,
            "POST",
            &commits_path,
            Some(commit_body(1)),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, body) = call(
            &here,
            &alice.key,
            "POST",
            &commits_path,
            Some(commit_body(1)),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        group.merge_pending_commit(&alice.provider).unwrap();

        let ciphertext = group
            .create_message(&alice.provider, &alice.signer, b"hello bob")
            .unwrap()
            .tls_serialize_detached()
            .unwrap();
        let messages_path = format!("/mls/groups/{group_id}/messages");
        let (status, sent) = call(
            &here,
            &alice.key,
            "POST",
            &messages_path,
            Some(json!({ "ciphertext": b64(&ciphertext) })),
            None,
        )
        .await;
        assert_eq!(
            (status, sent["epoch"].as_i64()),
            (StatusCode::OK, Some(1)),
            "{sent}"
        );
        let (status, _) = call(
            &here,
            &alice.key,
            "POST",
            &format!("/mls/groups/{}/messages", "cd".repeat(16)),
            Some(json!({ "ciphertext": b64(&ciphertext) })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, commits) = call(&here, &bob.key, "GET", &commits_path, None, None).await;
        assert_eq!(status, StatusCode::OK, "the commit added bob to the roster");
        assert_eq!(
            commits["commits"][0]["commit"],
            b64(&commit.tls_serialize_detached().unwrap())
        );
        assert_eq!(
            commits["commits"][0]["welcome"],
            b64(&welcome.tls_serialize_detached().unwrap())
        );
        let (status, history) = call(&here, &bob.key, "GET", &messages_path, None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(history["messages"][0]["ciphertext"], b64(&ciphertext));
        assert_eq!(history["messages"][0]["author"], alice.address);
        let (_, older) = call(
            &here,
            &bob.key,
            "GET",
            &format!("{messages_path}?before=1000"),
            None,
            None,
        )
        .await;
        assert_eq!(older["messages"], json!([]));
        let blob_path = format!("/mls/blobs/{}", sent["ciphertext_hash"].as_str().unwrap());
        let (status, blob) = call(&here, &bob.key, "GET", &blob_path, None, None).await;
        assert_eq!(
            (status, &blob["ciphertext"]),
            (StatusCode::OK, &json!(b64(&ciphertext)))
        );
        for path in [&commits_path, &messages_path, &blob_path] {
            let (status, _) = call(&here, &outsider, "GET", path, None, None).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        }
        let cid: String = sqlx::query_scalar(
            "SELECT ciphertext_cid FROM social.mls_message_refs WHERE group_id = $1",
        )
        .bind(&group_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            cid.starts_with("baf"),
            "the ciphertext is in the blob store, the row keeps its CID"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
