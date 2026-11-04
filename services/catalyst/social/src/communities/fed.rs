//! The communities federation: a write route that receives a wallet-signed
//! envelope of the `DecentralandCommunities` domain (`crate::fed`) instead
//! of its client body takes the old federated branch, route for route
//! ([`envelopes`], the middleware in front of the client handlers). Each
//! accepted envelope is appended once to ONE log,
//! `social.community_signed_actions` (the old `communities_local` and six
//! `*_log` tables are its `kind`), and projected into the keyed relations the
//! REST surface reads (`communities`, `community_members`, `community_bans`,
//! `community_places`, `community_posts`, `community_post_likes`,
//! `community_requests`) in the same transaction. Peers pull
//! `/federation/communities/{snapshot,changes}`; a post's body travels as
//! content (`/federation/communities/content[/{hash}]`, sha256-addressed,
//! kept in the ONE blob store and registered in `community_contents`).
//!
//! The preflight is the old one in its order: the request's signed fetch,
//! the envelope (400), its signer = the chain's inside the skew window
//! (401), the domain (400), a new nonce for the signer (409;
//! `social.community_seen_nonces`, swept by `community-nonce-sweep`), 60
//! actions a minute per signer (429). The authority is the client path's,
//! proven against `community_members` and `community_bans` (the old
//! federated twin, `community_role_current`, is not recreated; a role action
//! older than the member's latest one, by `(signed_at, signature_hash)`, is
//! logged and not projected, its tie-break).
//!
//! What changed from the old projection: a
//! federated post lands with its body as the text when the body is here
//! (the hash otherwise, as before) under the id its digest names, so its
//! delete, like and unlike reach the rows the REST surface reads (the old
//! ones only marked the log); a request update settles only a pending
//! request; the community gc keeps a body younger than the blob collector's
//! age floor.

use std::sync::LazyLock;
use std::time::Duration;

use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{OriginalUri, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use super::auth::SignedPath;
use super::http::err_json;
use super::standing::{
    CommunityBanAuthority, CommunityUnbanAuthority, load_standing, load_standing_pair,
};
use super::write_support::{ClientCommunityWriteAuthority, is_banned, refusal_response};
use super::{SharedState, State as CommunitiesState, validate};
use crate::comms::limits::{RateLimitDecision, RateLimiter};
use crate::fed::{FedError, Signed, TypedMessage};
use crate::identity::{can_act_on_member, can_delete_post, can_like_post};
use crate::jobs::{Jobs, PeriodicCfg};
use crate::{
    Address, AuthorityNotEstablished, CommunityId, CommunityPermission, CommunityRole,
    CommunityStanding,
};

pub const NONCES: &str = "social.community_seen_nonces";
const DOMAIN: &str = "DecentralandCommunities";
/// The router's body limit, which the envelope branch reads under.
const BODY_LIMIT: usize = 512 * 1024;
/// The old content store's `MAX_BODY_BYTES`.
pub const MAX_CONTENT_BYTES: usize = 256 * 1024;
const DEFAULT_CHANGES: i64 = 500;
const MAX_CHANGES: i64 = 5000;
const SNAPSHOT_COMMUNITIES: i64 = 1000;
const GC_CHUNK: i64 = 1000;

/// The old `RateLimiter::new(60, 60 s)`, per signer, process-wide.
static LIMITER: LazyLock<RateLimiter> =
    LazyLock::new(|| RateLimiter::new(60, Duration::from_secs(60)));

fn ec(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
}

macro_rules! message {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? } => |$s:ident, $b:ident| $body:block) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct $name {
            $(pub $field: $ty,)*
        }

        impl TypedMessage for $name {
            const PRIMARY_TYPE: &'static str = stringify!($name);
            fn encode_struct(&self) -> Vec<u8> {
                let $s = self;
                let mut $b = Vec::new();
                ec(&mut $b, Self::PRIMARY_TYPE);
                $body
                $b
            }
        }
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityCreate {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub unlisted: bool,
    #[serde(default)]
    pub flags: Vec<String>,
}

impl TypedMessage for CommunityCreate {
    const PRIMARY_TYPE: &'static str = "CommunityCreate";
    fn encode_struct(&self) -> Vec<u8> {
        let mut b = Vec::new();
        ec(&mut b, Self::PRIMARY_TYPE);
        ec(&mut b, &self.name);
        ec(&mut b, &self.description);
        b.push(self.private as u8);
        b.push(self.unlisted as u8);
        for f in &self.flags {
            ec(&mut b, f);
        }
        b
    }
}

message!(CommunityUpdate { community_id: String, name: Option<String>, description: Option<String>, private: Option<bool>, unlisted: Option<bool> } => |s, b| {
    ec(&mut b, &s.community_id);
    ec(&mut b, s.name.as_deref().unwrap_or(""));
    ec(&mut b, s.description.as_deref().unwrap_or(""));
    b.push(s.private.unwrap_or(false) as u8);
    b.push(s.unlisted.unwrap_or(false) as u8);
});
message!(CommunityDelete { community_id: String } => |s, b| { ec(&mut b, &s.community_id); });
message!(CommunityJoin { community_id: String } => |s, b| { ec(&mut b, &s.community_id); });
message!(CommunityLeave { community_id: String, member: String } => |s, b| { ec(&mut b, &s.community_id); ec(&mut b, &s.member); });
message!(CommunityBan { community_id: String, target: String, reason: Option<String> } => |s, b| {
    ec(&mut b, &s.community_id);
    ec(&mut b, &s.target);
    ec(&mut b, s.reason.as_deref().unwrap_or(""));
});
message!(CommunityUnban { community_id: String, target: String } => |s, b| { ec(&mut b, &s.community_id); ec(&mut b, &s.target); });
message!(CommunityPlacesAdd { community_id: String, place_ids: Vec<String> } => |s, b| {
    ec(&mut b, &s.community_id);
    for p in &s.place_ids {
        ec(&mut b, p);
    }
});
message!(CommunityPlaceRemove { community_id: String, place_id: String } => |s, b| { ec(&mut b, &s.community_id); ec(&mut b, &s.place_id); });
message!(CommunityPost { community_id: String, content_hash: String } => |s, b| { ec(&mut b, &s.community_id); ec(&mut b, &s.content_hash); });
message!(CommunityPostDelete { community_id: String, post_id: String } => |s, b| { ec(&mut b, &s.community_id); ec(&mut b, &s.post_id); });
message!(CommunityPostLike { community_id: String, post_id: String } => |s, b| { ec(&mut b, &s.community_id); ec(&mut b, &s.post_id); });
message!(CommunityPostUnlike { community_id: String, post_id: String } => |s, b| { ec(&mut b, &s.community_id); ec(&mut b, &s.post_id); });
message!(CommunityRequestStatusUpdate { community_id: String, request_id: String, status: String } => |s, b| {
    ec(&mut b, &s.community_id);
    ec(&mut b, &s.request_id);
    ec(&mut b, &s.status);
});

/// The old `CommunityRole` message (its primary type is its wire name; the
/// Rust name leaves `crate::CommunityRole` alone).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityRoleUpdate {
    pub community_id: String,
    pub target: String,
    pub role: String,
}

impl TypedMessage for CommunityRoleUpdate {
    const PRIMARY_TYPE: &'static str = "CommunityRole";
    fn encode_struct(&self) -> Vec<u8> {
        let mut b = Vec::new();
        ec(&mut b, Self::PRIMARY_TYPE);
        ec(&mut b, &self.community_id);
        ec(&mut b, &self.target);
        ec(&mut b, &self.role);
        b
    }
}

/// The id a created community takes: sha256 of the creator, the name and
/// the envelope's nonce.
pub fn community_id_hex(creator: &str, name: &str, nonce: &[u8; 16]) -> String {
    let mut h = Sha256::new();
    h.update(creator.to_ascii_lowercase().as_bytes());
    h.update([0u8]);
    h.update(name.as_bytes());
    h.update([0u8]);
    h.update(nonce);
    hex::encode(h.finalize())
}

/// A post's row id, from its digest the way a community's comes from its
/// hex id, so a federated delete, like or unlike names the row.
pub fn post_uuid(post_signature_hash: &str) -> Uuid {
    CommunityId::from_federation_hex(post_signature_hash).as_uuid()
}

/// The sixteen federated writes, by the client route they share.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FedRoute {
    Create,
    Update(String),
    Delete(String),
    Join(String),
    Leave(String, String),
    Role(String, String),
    Ban(String, String),
    Unban(String, String),
    PlacesAdd(String),
    PlaceRemove(String, String),
    Post(String),
    PostDelete(String, String),
    Like(String, String),
    Unlike(String, String),
    RequestStatus(String, String),
}

/// A path segment as axum's `Path` hands it to the client handler.
fn percent_decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%' && i + 2 < bytes.len())
            .then(|| {
                std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
            })
            .flatten();
        match escaped {
            Some(v) => {
                out.push(v);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl FedRoute {
    fn of(method: &Method, path: &str) -> Option<Self> {
        let segs: Vec<String> = path
            .trim_matches('/')
            .split('/')
            .map(percent_decode)
            .collect();
        let s: Vec<&str> = segs.iter().map(String::as_str).collect();
        let owned = |i: usize| s[i].to_string();
        Some(match (method.as_str(), s.as_slice()) {
            ("POST", ["v1", "communities"]) => Self::Create,
            ("PUT" | "PATCH", ["v1", "communities", _]) => Self::Update(owned(2)),
            ("DELETE", ["v1", "communities", _]) => Self::Delete(owned(2)),
            ("POST", ["v1", "communities", _, "members"]) => Self::Join(owned(2)),
            ("DELETE", ["v1", "communities", _, "members", _]) => Self::Leave(owned(2), owned(4)),
            ("PATCH", ["v1", "communities", _, "members", _]) => Self::Role(owned(2), owned(4)),
            ("POST", ["v1", "communities", _, "members", _, "bans"]) => {
                Self::Ban(owned(2), owned(4))
            }
            ("DELETE", ["v1", "communities", _, "members", _, "bans"]) => {
                Self::Unban(owned(2), owned(4))
            }
            ("POST", ["v1", "communities", _, "places"]) => Self::PlacesAdd(owned(2)),
            ("DELETE", ["v1", "communities", _, "places", _]) => {
                Self::PlaceRemove(owned(2), owned(4))
            }
            ("POST", ["v1", "communities", _, "posts"]) => Self::Post(owned(2)),
            ("DELETE", ["v1", "communities", _, "posts", _]) => {
                Self::PostDelete(owned(2), owned(4))
            }
            ("POST", ["v1", "communities", _, "posts", _, "like"]) => {
                Self::Like(owned(2), owned(4))
            }
            ("DELETE", ["v1", "communities", _, "posts", _, "like"]) => {
                Self::Unlike(owned(2), owned(4))
            }
            ("PATCH", ["v1", "communities", _, "requests", _]) => {
                Self::RequestStatus(owned(2), owned(4))
            }
            _ => return None,
        })
    }
}

/// The old write handlers' first step: an envelope body takes the federated
/// branch, anything else the client handler, untouched.
pub async fn envelopes(State(state): State<SharedState>, request: Request, next: Next) -> Response {
    let Some(route) = FedRoute::of(request.method(), request.uri().path()) else {
        return next.run(request).await;
    };
    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, BODY_LIMIT).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::PAYLOAD_TOO_LARGE, "length limit exceeded").into_response(),
    };
    if !crate::fed::is_envelope(&serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null)) {
        return next
            .run(Request::from_parts(parts, Body::from(bytes)))
            .await;
    }
    let method = parts.method.as_str().to_ascii_lowercase();
    let request_path = parts
        .extensions
        .get::<OriginalUri>()
        .map(|u| u.0.path().to_string())
        .unwrap_or_else(|| parts.uri.path().to_string());
    let path = SignedPath::new(request_path, parts.uri.path().to_string());
    let ctx = Ctx {
        state: &state,
        headers: &parts.headers,
        method: &method,
        path: &path,
        body: &bytes,
    };
    match route {
        FedRoute::Create => create(&ctx).await,
        FedRoute::Update(id) => update(&ctx, &id).await,
        FedRoute::Delete(id) => delete(&ctx, &id).await,
        FedRoute::Join(id) => join(&ctx, &id).await,
        FedRoute::Leave(id, address) => leave(&ctx, &id, &address).await,
        FedRoute::Role(id, address) => role(&ctx, &id, &address).await,
        FedRoute::Ban(id, address) => ban(&ctx, &id, &address).await,
        FedRoute::Unban(id, address) => unban(&ctx, &id, &address).await,
        FedRoute::PlacesAdd(id) => places_add(&ctx, &id).await,
        FedRoute::PlaceRemove(id, place) => place_remove(&ctx, &id, &place).await,
        FedRoute::Post(id) => post(&ctx, &id).await,
        FedRoute::PostDelete(id, post) => post_delete(&ctx, &id, &post).await,
        FedRoute::Like(id, post) => like(&ctx, &id, &post, true).await,
        FedRoute::Unlike(id, post) => like(&ctx, &id, &post, false).await,
        FedRoute::RequestStatus(id, request) => request_status(&ctx, &id, &request).await,
    }
}

struct Ctx<'a> {
    state: &'a CommunitiesState,
    headers: &'a HeaderMap,
    method: &'a str,
    path: &'a SignedPath,
    body: &'a Bytes,
}

/// The communities module boxes a refusal (`write_support::Refusal`).
type Reply<T> = Result<T, Box<Response>>;

fn bad(message: impl Into<String>) -> Response {
    err_json(StatusCode::BAD_REQUEST, message)
}

fn db_err(e: sqlx::Error) -> Response {
    tracing::error!(error = %e, "communities federation apply database error");
    err_json(StatusCode::INTERNAL_SERVER_ERROR, "database error")
}

fn ok(signature_hash: String) -> Response {
    (
        StatusCode::OK,
        Json(json!({ "ok": true, "signature_hash": signature_hash })),
    )
        .into_response()
}

fn ok_with(signature_hash: String, extra: Value) -> Response {
    let mut base = json!({ "ok": true, "signature_hash": signature_hash });
    if let (Some(b), Some(e)) = (base.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            b.insert(k.clone(), v.clone());
        }
    }
    (StatusCode::OK, Json(base)).into_response()
}

/// The federation gates answer with the authority's own detail (the old
/// `map_refusal_using_its_own_detail`).
fn own_detail(refusal: &AuthorityNotEstablished) -> Response {
    match refusal {
        AuthorityNotEstablished::RefusedLacksAuthority { detail } => {
            err_json(StatusCode::FORBIDDEN, detail.clone())
        }
        other => refusal_response(other, StatusCode::FORBIDDEN, String::new),
    }
}

async fn preflight<T: TypedMessage + DeserializeOwned>(
    ctx: &Ctx<'_>,
) -> Reply<(Signed<T>, Address)> {
    let signer = ctx
        .state
        .require_signer(ctx.headers, ctx.method, ctx.path)
        .await
        .map_err(|e| Box::new(e.into_response()))?;
    let signed: Signed<T> = serde_json::from_slice(ctx.body)
        .map_err(|e| Box::new(bad(format!("invalid Signed<{}>: {e}", T::PRIMARY_TYPE))))?;
    signed
        .verify(signer.as_str(), chrono::Utc::now().timestamp())
        .map_err(|e| {
            Box::new(err_json(
                StatusCode::UNAUTHORIZED,
                format!("signature verify: {e}"),
            ))
        })?;
    if !signed.domain.name.eq_ignore_ascii_case(DOMAIN) {
        return Err(Box::new(bad(format!("domain mismatch: expected {DOMAIN}"))));
    }
    if let Err(e) = crate::fed::check_and_record(
        &ctx.state.pool,
        NONCES,
        signer.as_str(),
        &signed.nonce,
        signed.signed_at,
    )
    .await
    {
        return Err(Box::new(match e {
            FedError::DuplicateNonce { .. } => err_json(StatusCode::CONFLICT, e.to_string()),
            other => err_json(StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        }));
    }
    if LIMITER.check(signer.as_str()) == RateLimitDecision::Deny {
        return Err(Box::new(err_json(
            StatusCode::TOO_MANY_REQUESTS,
            "rate limit exceeded",
        )));
    }
    Ok((signed, signer))
}

/// The path's community: a uuid, and the envelope's hex id must name it.
fn path_community(id: &str, envelope_hex: &str) -> Reply<CommunityId> {
    let uuid = Uuid::parse_str(id).map_err(|_| Box::new(bad("invalid uuid")))?;
    if CommunityId::from_federation_hex(envelope_hex).as_uuid() != uuid {
        return Err(Box::new(bad("community_id mismatch")));
    }
    Ok(CommunityId::from_uuid(uuid))
}

fn parse_uuid(id: &str) -> Reply<()> {
    Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| Box::new(bad("invalid uuid")))
}

async fn require_permission(
    state: &CommunitiesState,
    community: CommunityId,
    signer: &Address,
    permission: CommunityPermission,
    action: &str,
) -> Reply<()> {
    ClientCommunityWriteAuthority::resolve_requiring_capability(
        state, community, signer, permission, action,
    )
    .await
    .map(|_| ())
}

async fn require_at_least(
    state: &CommunitiesState,
    community: CommunityId,
    signer: &Address,
    minimum: CommunityStanding,
) -> Reply<()> {
    ClientCommunityWriteAuthority::resolve_requiring_at_least(state, community, signer, minimum)
        .await
        .map(|_| ())
}

#[derive(Default)]
struct Entry {
    kind: &'static str,
    target: Option<String>,
    role: Option<&'static str>,
    action: Option<String>,
    content_hash: Option<String>,
}

#[allow(clippy::too_many_arguments)]
async fn record(
    conn: &mut PgConnection,
    signature_hash: &str,
    community_hex: &str,
    community: CommunityId,
    signer: &Address,
    signed_at: i64,
    payload: &Value,
    entry: &Entry,
) -> sqlx::Result<bool> {
    let res = sqlx::query(
        "INSERT INTO social.community_signed_actions \
           (signature_hash, community_id, community_uuid, kind, signer, target, role, action, content_hash, signed_at, payload) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) ON CONFLICT (signature_hash) DO NOTHING",
    )
    .bind(signature_hash)
    .bind(community_hex)
    .bind(community)
    .bind(entry.kind)
    .bind(signer)
    .bind(entry.target.as_deref())
    .bind(entry.role)
    .bind(entry.action.as_deref())
    .bind(entry.content_hash.as_deref())
    .bind(signed_at)
    .bind(payload)
    .execute(&mut *conn)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// The tie-break the old `community_role_current` applied: a role action
/// is projected only when no logged role action on the same member is newer
/// by `(signed_at, signature_hash)`.
async fn superseded(
    conn: &mut PgConnection,
    community: CommunityId,
    member: &str,
    signed_at: i64,
    signature_hash: &str,
) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM social.community_signed_actions \
           WHERE community_uuid = $1 AND target = $2 AND role IS NOT NULL AND (signed_at, signature_hash) > ($3, $4))",
    )
    .bind(community)
    .bind(member)
    .bind(signed_at)
    .bind(signature_hash)
    .fetch_one(&mut *conn)
    .await
}

fn payload<T: Serialize>(message: &T) -> Value {
    serde_json::to_value(message).unwrap_or(Value::Null)
}

async fn create(ctx: &Ctx<'_>) -> Response {
    let (signed, signer) = match preflight::<CommunityCreate>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let m = &signed.message;
    for check in [
        validate::validate_name(&m.name),
        validate::check_restricted_name(&m.name, &ctx.state.settings.restricted_names),
        validate::validate_description(&m.description),
    ] {
        if let Err(e) = check {
            return bad(e);
        }
    }
    if let Some(false) = ctx.state.profiles.has_owned_name(signer.as_str()).await {
        return err_json(
            StatusCode::UNAUTHORIZED,
            format!("The user {signer} doesn't have any names"),
        );
    }
    let community_hex = community_id_hex(signer.as_str(), &m.name, &signed.nonce);
    let community = CommunityId::from_federation_hex(&community_hex);
    let signature_hash = signed.signature_hash();
    let applied: sqlx::Result<bool> = async {
        let mut tx = ctx.state.pool.begin().await?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM social.communities WHERE id = $1)").bind(community).fetch_one(&mut *tx).await?;
        if exists {
            return Ok(false);
        }
        sqlx::query("INSERT INTO social.communities (id, name, description, owner_address, private, unlisted) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(community)
            .bind(&m.name)
            .bind(&m.description)
            .bind(&signer)
            .bind(m.private)
            .bind(m.unlisted)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, 'owner') ON CONFLICT (community_id, member_address) DO NOTHING")
            .bind(community)
            .bind(&signer)
            .execute(&mut *tx)
            .await?;
        record(&mut tx, &signature_hash, &community_hex, community, &signer, signed.signed_at, &payload(m), &Entry { kind: "create", target: Some(signer.to_string()), role: Some("owner"), ..Default::default() }).await?;
        tx.commit().await?;
        Ok(true)
    }
    .await;
    match applied {
        Ok(true) => ok_with(
            signature_hash,
            json!({ "community_id": community_hex, "id": community.as_uuid(), "expected_id": community_hex }),
        ),
        Ok(false) => err_json(StatusCode::CONFLICT, "community already exists"),
        Err(e) => db_err(e),
    }
}

async fn update(ctx: &Ctx<'_>, id: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityUpdate>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let m = &signed.message;
    let community = match path_community(id, &m.community_id) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    for check in [
        validate::validate_name_opt(m.name.as_deref()),
        validate::check_restricted_name_opt(
            m.name.as_deref(),
            &ctx.state.settings.restricted_names,
        ),
        validate::validate_description_opt(m.description.as_deref()),
    ] {
        if let Err(e) = check {
            return bad(e);
        }
    }
    if let Err(r) = require_permission(
        ctx.state,
        community,
        &signer,
        CommunityPermission::EditInfo,
        "edit the community",
    )
    .await
    {
        return *r;
    }
    if m.name.is_some()
        && let Err(r) = require_permission(
            ctx.state,
            community,
            &signer,
            CommunityPermission::EditName,
            "edit the community name",
        )
        .await
    {
        return *r;
    }
    if (m.private.is_some() || m.unlisted.is_some())
        && let Err(r) = require_permission(
            ctx.state,
            community,
            &signer,
            CommunityPermission::EditSettings,
            "update the community privacy",
        )
        .await
    {
        return *r;
    }
    let signature_hash = signed.signature_hash();
    let applied: sqlx::Result<()> = async {
        let mut tx = ctx.state.pool.begin().await?;
        sqlx::query(
            "UPDATE social.communities SET name = COALESCE($2, name), description = COALESCE($3, description), \
               private = COALESCE($4, private), unlisted = COALESCE($5, unlisted), updated_at = timezone('UTC', now()) WHERE id = $1",
        )
        .bind(community)
        .bind(m.name.as_deref())
        .bind(m.description.as_deref())
        .bind(m.private)
        .bind(m.unlisted)
        .execute(&mut *tx)
        .await?;
        record(&mut tx, &signature_hash, &m.community_id, community, &signer, signed.signed_at, &payload(m), &Entry { kind: "update", ..Default::default() }).await?;
        tx.commit().await
    }
    .await;
    applied.map(|_| ok(signature_hash)).unwrap_or_else(db_err)
}

async fn delete(ctx: &Ctx<'_>, id: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityDelete>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let community = match path_community(id, &signed.message.community_id) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if let Err(r) = require_at_least(ctx.state, community, &signer, CommunityStanding::Owner).await
    {
        return *r;
    }
    let signature_hash = signed.signature_hash();
    let applied: sqlx::Result<bool> = async {
        let mut tx = ctx.state.pool.begin().await?;
        sqlx::query("UPDATE social.communities SET active = FALSE, updated_at = timezone('UTC', now()) WHERE id = $1").bind(community).execute(&mut *tx).await?;
        let fresh = record(&mut tx, &signature_hash, &signed.message.community_id, community, &signer, signed.signed_at, &payload(&signed.message), &Entry { kind: "delete", ..Default::default() }).await?;
        tx.commit().await?;
        Ok(fresh)
    }
    .await;
    if let Ok(true) = applied {
        ctx.state.community_closed_to_worlds(community);
    }
    applied.map(|_| ok(signature_hash)).unwrap_or_else(db_err)
}

/// Logs a role action and projects it into the member row unless a newer
/// one on the same member was logged first.
#[allow(clippy::too_many_arguments)]
async fn apply_role_action<T: TypedMessage>(
    state: &CommunitiesState,
    signed: &Signed<T>,
    community_hex: &str,
    community: CommunityId,
    signer: &Address,
    member: &Address,
    role: &'static str,
    extra: impl AsyncFnOnce(&mut PgConnection) -> sqlx::Result<()>,
) -> sqlx::Result<String> {
    let signature_hash = signed.signature_hash();
    let mut removed = false;
    let mut tx = state.pool.begin().await?;
    let fresh = record(
        &mut tx,
        &signature_hash,
        community_hex,
        community,
        signer,
        signed.signed_at,
        &payload(&signed.message),
        &Entry {
            kind: "role",
            target: Some(member.to_string()),
            role: Some(role),
            ..Default::default()
        },
    )
    .await?;
    if fresh
        && !superseded(
            &mut tx,
            community,
            member.as_str(),
            signed.signed_at,
            &signature_hash,
        )
        .await?
    {
        match role {
            "member" | "mod" | "owner" => {
                let stored = CommunityRole::parse(role)
                    .map(CommunityRole::as_stored_text)
                    .unwrap_or("member");
                let conflict = if T::PRIMARY_TYPE == "CommunityJoin" {
                    "DO NOTHING"
                } else {
                    "DO UPDATE SET role = EXCLUDED.role"
                };
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, $3) ON CONFLICT (community_id, member_address) {conflict}"
                )))
                .bind(community)
                .bind(member)
                .bind(stored)
                .execute(&mut *tx)
                .await?;
            }
            _ => {
                removed = sqlx::query("DELETE FROM social.community_members WHERE community_id = $1 AND member_address = $2").bind(community).bind(member).execute(&mut *tx).await?.rows_affected() > 0;
            }
        }
        extra(&mut tx).await?;
    }
    tx.commit().await?;
    if removed {
        state.member_removed_from_worlds(community, member);
    }
    Ok(signature_hash)
}

async fn join(ctx: &Ctx<'_>, id: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityJoin>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let community = match path_community(id, &signed.message.community_id) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    match super::queries::community_exists(&ctx.state.pool, community, false).await {
        Ok(true) => {}
        Ok(false) => return err_json(StatusCode::NOT_FOUND, "community not found"),
        Err(e) => return db_err(e),
    }
    match super::queries::is_private(&ctx.state.pool, community).await {
        Ok(true) => {
            return err_json(
                StatusCode::UNAUTHORIZED,
                format!(
                    "Cannot join private community {id} directly; a join request or invite is required"
                ),
            );
        }
        Ok(false) => {}
        Err(e) => return db_err(e),
    }
    match is_banned(ctx.state, community, &signer).await {
        Ok(true) => return err_json(StatusCode::FORBIDDEN, "banned from community"),
        Ok(false) => {}
        Err(r) => return *r,
    }
    apply_role_action(
        ctx.state,
        &signed,
        &signed.message.community_id,
        community,
        &signer,
        &signer,
        "member",
        async |_| Ok(()),
    )
    .await
    .map(ok)
    .unwrap_or_else(db_err)
}

async fn leave(ctx: &Ctx<'_>, id: &str, address: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityLeave>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let community = match path_community(id, &signed.message.community_id) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if !signed.message.member.eq_ignore_ascii_case(signer.as_str()) {
        return err_json(StatusCode::FORBIDDEN, "may only leave on behalf of self");
    }
    if !address.eq_ignore_ascii_case(signer.as_str()) {
        return err_json(StatusCode::FORBIDDEN, "path address must match signer");
    }
    match load_standing(&ctx.state.pool, community, &signer).await {
        Ok(s) if s.tier() == CommunityStanding::Owner => {
            return err_json(
                StatusCode::UNAUTHORIZED,
                format!("The owner cannot leave the community {id}"),
            );
        }
        Ok(_) => {}
        Err(refusal) => return own_detail(&refusal),
    }
    apply_role_action(
        ctx.state,
        &signed,
        &signed.message.community_id,
        community,
        &signer,
        &signer,
        "none",
        async |_| Ok(()),
    )
    .await
    .map(ok)
    .unwrap_or_else(db_err)
}

async fn role(ctx: &Ctx<'_>, id: &str, address: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityRoleUpdate>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let m = &signed.message;
    let community = match path_community(id, &m.community_id) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if !m.target.eq_ignore_ascii_case(address) {
        return bad("target must match path address");
    }
    let role: &'static str = match CommunityStanding::parse_supplied(&m.role) {
        Some(CommunityStanding::Member) => "member",
        Some(CommunityStanding::Moderator) => "mod",
        _ => return bad("invalid role"),
    };
    if m.target.eq_ignore_ascii_case(signer.as_str()) {
        return err_json(StatusCode::FORBIDDEN, "a user cannot update their own role");
    }
    let Ok(target) = Address::parse(&m.target) else {
        return bad("target must match path address");
    };
    let (actor_standing, target_standing) =
        match load_standing_pair(&ctx.state.pool, community, &signer, &target).await {
            Ok(pair) => pair,
            Err(refusal) => return own_detail(&refusal),
        };
    if !actor_standing.has(CommunityPermission::AssignRoles)
        || !can_act_on_member(actor_standing.tier(), target_standing.tier())
    {
        return err_json(
            StatusCode::FORBIDDEN,
            "actor cannot assign roles for this member",
        );
    }
    apply_role_action(
        ctx.state,
        &signed,
        &m.community_id,
        community,
        &signer,
        &target,
        role,
        async |_| Ok(()),
    )
    .await
    .map(ok)
    .unwrap_or_else(db_err)
}

async fn ban(ctx: &Ctx<'_>, id: &str, address: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityBan>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let m = &signed.message;
    let community = match path_community(id, &m.community_id) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if !m.target.eq_ignore_ascii_case(address) {
        return bad("target must match path address");
    }
    let Ok(target) = Address::parse(&m.target) else {
        return bad("target must match path address");
    };
    if let Err(refusal) =
        CommunityBanAuthority::resolve(&ctx.state.pool, &signer, community, &target).await
    {
        return own_detail(&refusal);
    }
    let (reason, banned_by) = (m.reason.clone(), signer.clone());
    let banned = target.clone();
    let result = apply_role_action(ctx.state, &signed, &m.community_id, community, &signer, &target, "banned", async move |tx: &mut PgConnection| {
        sqlx::query("DELETE FROM social.community_requests WHERE community_id = $1 AND member_address = $2 AND status = 'pending'").bind(community).bind(&banned).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO social.community_bans (community_id, banned_address, banned_by, reason, active, banned_at) \
             VALUES ($1, $2, $3, $4, TRUE, timezone('UTC', now())) \
             ON CONFLICT (community_id, banned_address) DO UPDATE \
               SET active = TRUE, banned_by = EXCLUDED.banned_by, reason = EXCLUDED.reason, banned_at = timezone('UTC', now()), \
                   unbanned_by = NULL, unbanned_at = NULL",
        )
        .bind(community)
        .bind(&banned)
        .bind(&banned_by)
        .bind(reason.as_deref())
        .execute(&mut *tx)
        .await?;
        Ok(())
    })
    .await;
    match result {
        Ok(sig) => {
            ctx.state
                .evict_from_private_community_voice(community, &target)
                .await;
            ok(sig)
        }
        Err(e) => db_err(e),
    }
}

async fn unban(ctx: &Ctx<'_>, id: &str, address: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityUnban>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let m = &signed.message;
    let community = match path_community(id, &m.community_id) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if !m.target.eq_ignore_ascii_case(address) {
        return bad("target must match path address");
    }
    let Ok(target) = Address::parse(&m.target) else {
        return bad("target must match path address");
    };
    if let Err(refusal) =
        CommunityUnbanAuthority::resolve(&ctx.state.pool, &signer, community, &target).await
    {
        return own_detail(&refusal);
    }
    let (lifted, unbanned_by) = (target.clone(), signer.clone());
    apply_role_action(ctx.state, &signed, &m.community_id, community, &signer, &target, "none", async move |tx: &mut PgConnection| {
        sqlx::query(
            "UPDATE social.community_bans SET active = FALSE, unbanned_by = $3, unbanned_at = timezone('UTC', now()) \
             WHERE community_id = $1 AND banned_address = $2 AND active",
        )
        .bind(community)
        .bind(&lifted)
        .bind(&unbanned_by)
        .execute(&mut *tx)
        .await?;
        Ok(())
    })
    .await
    .map(ok)
    .unwrap_or_else(db_err)
}

async fn places_add(ctx: &Ctx<'_>, id: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityPlacesAdd>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let m = &signed.message;
    let community = match path_community(id, &m.community_id) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if let Err(r) = require_permission(
        ctx.state,
        community,
        &signer,
        CommunityPermission::AddPlaces,
        "add places to the community",
    )
    .await
    {
        return *r;
    }
    if let Err(e) = ctx
        .state
        .validate_places_ownership(&m.place_ids, &signer)
        .await
    {
        return e.into_response();
    }
    let signature_hash = signed.signature_hash();
    let applied: sqlx::Result<()> = async {
        let mut tx = ctx.state.pool.begin().await?;
        for (i, place) in m.place_ids.iter().enumerate() {
            let entry = Entry { kind: "place", target: Some(place.clone()), action: Some("add".into()), ..Default::default() };
            record(&mut tx, &format!("{signature_hash}-add-{i}"), &m.community_id, community, &signer, signed.signed_at, &json!({ "place_id": place, "action": "add" }), &entry).await?;
        }
        sqlx::query(
            "INSERT INTO social.community_places (place_id, community_id, added_by) SELECT t.pid, $1, $2 FROM unnest($3::text[]) AS t(pid) \
             ON CONFLICT (place_id, community_id) DO NOTHING",
        )
        .bind(community)
        .bind(&signer)
        .bind(&m.place_ids)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }
    .await;
    applied.map(|_| ok(signature_hash)).unwrap_or_else(db_err)
}

async fn place_remove(ctx: &Ctx<'_>, id: &str, place: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityPlaceRemove>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let m = &signed.message;
    let community = match path_community(id, &m.community_id) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if m.place_id != place {
        return bad("place_id mismatch");
    }
    if let Err(r) = require_permission(
        ctx.state,
        community,
        &signer,
        CommunityPermission::RemovePlaces,
        "remove places from the community",
    )
    .await
    {
        return *r;
    }
    if let Err(e) = ctx
        .state
        .validate_places_ownership(std::slice::from_ref(&m.place_id), &signer)
        .await
    {
        return e.into_response();
    }
    let signature_hash = signed.signature_hash();
    let applied: sqlx::Result<()> = async {
        let mut tx = ctx.state.pool.begin().await?;
        let entry = Entry {
            kind: "place",
            target: Some(m.place_id.clone()),
            action: Some("remove".into()),
            ..Default::default()
        };
        record(
            &mut tx,
            &signature_hash,
            &m.community_id,
            community,
            &signer,
            signed.signed_at,
            &payload(m),
            &entry,
        )
        .await?;
        sqlx::query(
            "DELETE FROM social.community_places WHERE place_id = $1 AND community_id = $2",
        )
        .bind(&m.place_id)
        .bind(community)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }
    .await;
    applied.map(|_| ok(signature_hash)).unwrap_or_else(db_err)
}

fn is_valid_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

async fn post(ctx: &Ctx<'_>, id: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityPost>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let m = &signed.message;
    let community = match path_community(id, &m.community_id) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if let Err(r) = require_permission(
        ctx.state,
        community,
        &signer,
        CommunityPermission::CreatePosts,
        "create posts in the community",
    )
    .await
    {
        return *r;
    }
    let body = registered_body(ctx.state, &m.content_hash.to_ascii_lowercase()).await;
    if body.is_none() {
        tracing::debug!(community_id = %m.community_id, content_hash = %m.content_hash, %signer, "CommunityPost accepted before its content body is here");
    }
    let text = body
        .as_ref()
        .and_then(|b| std::str::from_utf8(b).ok())
        .map(str::to_string)
        .unwrap_or_else(|| m.content_hash.clone());
    let signature_hash = signed.signature_hash();
    let applied: sqlx::Result<()> = async {
        let mut tx = ctx.state.pool.begin().await?;
        let entry = Entry { kind: "post", content_hash: Some(m.content_hash.clone()), ..Default::default() };
        if record(&mut tx, &signature_hash, &m.community_id, community, &signer, signed.signed_at, &payload(m), &entry).await? {
            sqlx::query("INSERT INTO social.community_posts (id, community_id, author_address, content) VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO NOTHING")
                .bind(post_uuid(&signature_hash))
                .bind(community)
                .bind(&signer)
                .bind(&text)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await
    }
    .await;
    applied
        .map(|_| {
            ok_with(
                signature_hash,
                json!({ "content_body_local": body.is_some() }),
            )
        })
        .unwrap_or_else(db_err)
}

/// `(community hex id, author)` of a logged post.
async fn post_meta(
    pool: &PgPool,
    post_signature_hash: &str,
) -> sqlx::Result<Option<(String, String)>> {
    sqlx::query_as("SELECT community_id, signer FROM social.community_signed_actions WHERE signature_hash = $1 AND kind = 'post'").bind(post_signature_hash).fetch_optional(pool).await
}

async fn post_delete(ctx: &Ctx<'_>, id: &str, post: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityPostDelete>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let m = &signed.message;
    let meta = match post_meta(&ctx.state.pool, &m.post_id).await {
        Ok(meta) => meta,
        Err(e) => return db_err(e),
    };
    let (post_community, author) = meta.map(|(c, a)| (c, Some(a))).unwrap_or_default();
    if m.community_id != post_community || m.post_id != post {
        return bad("post_id / community_id mismatch");
    }
    let community = CommunityId::from_federation_hex(&m.community_id);
    let is_author = author
        .as_deref()
        .is_some_and(|a| a.eq_ignore_ascii_case(signer.as_str()));
    let standing = match load_standing(&ctx.state.pool, community, &signer).await {
        Ok(s) => s,
        Err(refusal) => return own_detail(&refusal),
    };
    if !can_delete_post(standing.tier(), is_author) {
        return err_json(
            StatusCode::UNAUTHORIZED,
            format!("The user {signer} doesn't have permission to delete posts from the community"),
        );
    }
    let signature_hash = signed.signature_hash();
    let applied: sqlx::Result<()> = async {
        let mut tx = ctx.state.pool.begin().await?;
        record(
            &mut tx,
            &signature_hash,
            &m.community_id,
            community,
            &signer,
            signed.signed_at,
            &payload(m),
            &Entry {
                kind: "post_delete",
                target: Some(m.post_id.clone()),
                ..Default::default()
            },
        )
        .await?;
        sqlx::query("DELETE FROM social.community_posts WHERE id = $1 AND community_id = $2")
            .bind(post_uuid(&m.post_id))
            .bind(community)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    }
    .await;
    applied.map(|_| ok(signature_hash)).unwrap_or_else(db_err)
}

async fn like(ctx: &Ctx<'_>, id: &str, post: &str, liking: bool) -> Response {
    let community = match Uuid::parse_str(id) {
        Ok(u) => CommunityId::from_uuid(u),
        Err(_) => return bad("invalid uuid"),
    };
    let (community_hex, post_id, signature_hash, signed_at, message, signer) = if liking {
        let (signed, signer) = match preflight::<CommunityPostLike>(ctx).await {
            Ok(x) => x,
            Err(r) => return *r,
        };
        let m = signed.message.clone();
        (
            m.community_id.clone(),
            m.post_id.clone(),
            signed.signature_hash(),
            signed.signed_at,
            payload(&m),
            signer,
        )
    } else {
        let (signed, signer) = match preflight::<CommunityPostUnlike>(ctx).await {
            Ok(x) => x,
            Err(r) => return *r,
        };
        let m = signed.message.clone();
        (
            m.community_id.clone(),
            m.post_id.clone(),
            signed.signature_hash(),
            signed.signed_at,
            payload(&m),
            signer,
        )
    };
    if CommunityId::from_federation_hex(&community_hex) != community {
        return bad("community_id mismatch");
    }
    if post_id != post {
        return bad("post_id mismatch");
    }
    let private =
        match sqlx::query_scalar::<_, bool>("SELECT private FROM social.communities WHERE id = $1")
            .bind(community)
            .fetch_optional(&ctx.state.pool)
            .await
        {
            Ok(Some(p)) => p,
            Ok(None) => return err_json(StatusCode::NOT_FOUND, "community not found"),
            Err(e) => return db_err(e),
        };
    let standing = match load_standing(&ctx.state.pool, community, &signer).await {
        Ok(s) => s,
        Err(refusal) => return own_detail(&refusal),
    };
    let tier = match is_banned(ctx.state, community, &signer).await {
        Ok(true) => CommunityStanding::Banned,
        Ok(false) => standing.tier(),
        Err(r) => return *r,
    };
    if !can_like_post(tier, private) {
        return err_json(
            StatusCode::UNAUTHORIZED,
            format!("{signer} cannot like/unlike posts in community {community_hex}"),
        );
    }
    match post_meta(&ctx.state.pool, &post_id).await {
        Ok(Some((c, _))) if c == community_hex => {}
        Ok(_) => {
            return err_json(
                StatusCode::NOT_FOUND,
                "post does not belong to this community",
            );
        }
        Err(e) => return db_err(e),
    }
    let kind = if liking { "post_like" } else { "post_unlike" };
    let applied: sqlx::Result<()> = async {
        let mut tx = ctx.state.pool.begin().await?;
        record(&mut tx, &signature_hash, &community_hex, community, &signer, signed_at, &message, &Entry { kind, target: Some(post_id.clone()), ..Default::default() }).await?;
        let sql = if liking {
            "INSERT INTO social.community_post_likes (post_id, user_address) SELECT id, $2 FROM social.community_posts WHERE id = $1 ON CONFLICT (post_id, user_address) DO NOTHING"
        } else {
            "DELETE FROM social.community_post_likes WHERE post_id = $1 AND user_address = $2"
        };
        sqlx::query(sql).bind(post_uuid(&post_id)).bind(&signer).execute(&mut *tx).await?;
        tx.commit().await
    }
    .await;
    applied.map(|_| ok(signature_hash)).unwrap_or_else(db_err)
}

async fn request_status(ctx: &Ctx<'_>, id: &str, request: &str) -> Response {
    if let Err(r) = parse_uuid(id) {
        return *r;
    }
    let (signed, signer) = match preflight::<CommunityRequestStatusUpdate>(ctx).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let m = &signed.message;
    let community = match path_community(id, &m.community_id) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if m.request_id != request {
        return bad("request_id mismatch");
    }
    if let Err(r) =
        require_at_least(ctx.state, community, &signer, CommunityStanding::Moderator).await
    {
        return *r;
    }
    let status = match m.status.as_str() {
        s @ ("accepted" | "rejected" | "cancelled") => s,
        _ => return bad("invalid request status"),
    };
    let signature_hash = signed.signature_hash();
    let applied: sqlx::Result<Option<Response>> = async {
        let mut tx = ctx.state.pool.begin().await?;
        if let Ok(request_uuid) = Uuid::parse_str(&m.request_id) {
            let owner: Option<Uuid> = sqlx::query_scalar("SELECT community_id FROM social.community_requests WHERE id = $1").bind(request_uuid).fetch_optional(&mut *tx).await?;
            if owner.is_some_and(|c| c != community.as_uuid()) {
                return Ok(Some(err_json(StatusCode::NOT_FOUND, "request does not belong to this community")));
            }
            sqlx::query("UPDATE social.community_requests SET status = $2, updated_at = timezone('UTC', now()) WHERE id = $1 AND community_id = $3 AND status = 'pending'")
                .bind(request_uuid)
                .bind(status)
                .bind(community)
                .execute(&mut *tx)
                .await?;
        }
        let entry = Entry { kind: "request_status", target: Some(m.request_id.clone()), action: Some(status.to_string()), ..Default::default() };
        record(&mut tx, &signature_hash, &m.community_id, community, &signer, signed.signed_at, &payload(m), &entry).await?;
        tx.commit().await?;
        Ok(None)
    }
    .await;
    match applied {
        Ok(Some(refused)) => refused,
        Ok(None) => ok(signature_hash),
        Err(e) => db_err(e),
    }
}

fn hash_all(hashes: &[String]) -> String {
    let mut h = Sha256::new();
    for s in hashes {
        h.update(s.as_bytes());
    }
    hex::encode(h.finalize())
}

pub async fn snapshot_view(pool: &PgPool) -> sqlx::Result<Value> {
    let latest = |kinds: &'static [&'static str]| async move {
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT max(seq) FROM social.community_signed_actions WHERE kind = ANY($1)",
        )
        .bind(kinds)
        .fetch_one(pool)
        .await
        .map(|v| v.unwrap_or(0))
    };
    let (role, posts, places) = (
        latest(&["create", "role"]).await?,
        latest(&["post"]).await?,
        latest(&["place"]).await?,
    );
    let communities: Vec<(String, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT a.community_id, a.signer, COALESCE(c.name, a.payload->>'name'), a.signed_at FROM social.community_signed_actions a \
         LEFT JOIN social.communities c ON c.id = a.community_uuid WHERE a.kind = 'create' ORDER BY a.seq LIMIT $1",
    )
    .bind(SNAPSHOT_COMMUNITIES)
    .fetch_all(pool)
    .await?;
    let role_hashes: Vec<String> = sqlx::query_scalar("SELECT signature_hash FROM social.community_signed_actions WHERE kind IN ('create', 'role') ORDER BY signature_hash").fetch_all(pool).await?;
    Ok(json!({
        "latest_role_seq": role,
        "latest_posts_seq": posts,
        "latest_places_seq": places,
        "communities": communities.into_iter().map(|(community_id, creator, name, signed_at)| json!({ "community_id": community_id, "creator": creator, "name": name.unwrap_or_default(), "signed_at": signed_at })).collect::<Vec<_>>(),
        "role_log_hash": hash_all(&role_hashes),
    }))
}

/// `GET /federation/communities/snapshot`.
pub async fn snapshot(State(state): State<SharedState>) -> Response {
    snapshot_view(&state.pool)
        .await
        .map(|v| Json(v).into_response())
        .unwrap_or_else(db_err)
}

#[derive(Debug, Deserialize)]
pub struct ChangesQuery {
    #[serde(default)]
    pub since: i64,
    #[serde(default)]
    pub limit: Option<i64>,
}

pub async fn changes_view(pool: &PgPool, since: i64, limit: i64) -> sqlx::Result<Value> {
    let limit = limit.clamp(1, MAX_CHANGES);
    let rows = |kinds: &'static [&'static str]| async move {
        sqlx::query(
            "SELECT signature_hash, community_id, kind, signer, target, role, action, content_hash, signed_at, payload, seq \
             FROM social.community_signed_actions WHERE kind = ANY($1) AND seq > $2 ORDER BY seq ASC LIMIT $3",
        )
        .bind(kinds)
        .bind(since)
        .bind(limit)
        .fetch_all(pool)
        .await
    };
    let role: Vec<Value> = rows(&["create", "role"])
        .await?
        .iter()
        .map(|r| {
            let (community_id, signer, kind): (String, String, String) = (r.get("community_id"), r.get("signer"), r.get("kind"));
            let payload = if kind == "create" { json!({ "type": "CommunityCreate", "community_id": community_id, "creator": signer, "role": "owner" }) } else { r.get("payload") };
            json!({
                "kind": "role",
                "signature_hash": r.get::<String, _>("signature_hash"),
                "community_id": community_id,
                "signer": signer,
                "target": r.get::<Option<String>, _>("target"),
                "role": r.get::<Option<String>, _>("role"),
                "signed_at": r.get::<i64, _>("signed_at"),
                "payload": payload,
                "seq": r.get::<i64, _>("seq"),
            })
        })
        .collect();
    let posts: Vec<Value> = rows(&["post"])
        .await?
        .iter()
        .map(|r| {
            json!({
                "kind": "post",
                "signature_hash": r.get::<String, _>("signature_hash"),
                "community_id": r.get::<String, _>("community_id"),
                "author": r.get::<String, _>("signer"),
                "content_hash": r.get::<Option<String>, _>("content_hash"),
                "signed_at": r.get::<i64, _>("signed_at"),
                "seq": r.get::<i64, _>("seq"),
            })
        })
        .collect();
    let places: Vec<Value> = rows(&["place"])
        .await?
        .iter()
        .map(|r| {
            json!({
                "kind": "place",
                "signature_hash": r.get::<String, _>("signature_hash"),
                "community_id": r.get::<String, _>("community_id"),
                "place_id": r.get::<Option<String>, _>("target"),
                "action": r.get::<Option<String>, _>("action"),
                "signer": r.get::<String, _>("signer"),
                "signed_at": r.get::<i64, _>("signed_at"),
                "seq": r.get::<i64, _>("seq"),
            })
        })
        .collect();
    Ok(json!({ "role": role, "posts": posts, "places": places }))
}

/// `GET /federation/communities/changes?since=&limit=` (500 by default, 1..=5000 per feed).
pub async fn changes(State(state): State<SharedState>, Query(q): Query<ChangesQuery>) -> Response {
    changes_view(&state.pool, q.since, q.limit.unwrap_or(DEFAULT_CHANGES))
        .await
        .map(|v| Json(v).into_response())
        .unwrap_or_else(db_err)
}

fn content_hash(body: &[u8]) -> String {
    hex::encode(Sha256::digest(body))
}

/// The blob a registered body is stored as (the ONE store keys by CID).
async fn registered_cid(pool: &PgPool, hash: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT cid FROM social.community_contents WHERE content_hash = $1")
        .bind(hash)
        .fetch_optional(pool)
        .await
}

async fn registered_body(state: &CommunitiesState, hash: &str) -> Option<Bytes> {
    if !is_valid_hash(hash) {
        return None;
    }
    let cid = registered_cid(&state.pool, hash).await.ok().flatten()?;
    state.storage.retrieve(&cid).await.ok().flatten()
}

/// `POST /federation/communities/content`: stores a post body (<= 256 KiB)
/// and registers its sha256 (the wire's content hash) against the blob.
pub async fn put_content(State(state): State<SharedState>, body: Bytes) -> Response {
    if body.len() > MAX_CONTENT_BYTES {
        return err_json(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("body exceeds {MAX_CONTENT_BYTES} bytes"),
        );
    }
    let (hash, size) = (content_hash(&body), body.len());
    let for_cid = body.clone();
    let cid = match tokio::task::spawn_blocking(move || crate::auth::hash::hash_bytes_v1(&for_cid))
        .await
    {
        Ok(cid) => cid,
        Err(_) => return err_json(StatusCode::INTERNAL_SERVER_ERROR, "io error"),
    };
    let stored = async {
        if !state.storage.exist(&cid).await? {
            state.storage.store(&cid, body).await?;
        }
        Ok::<_, crate::storage::StorageError>(())
    }
    .await;
    if let Err(e) = stored {
        tracing::error!(error = %e, "community content store io error");
        return err_json(StatusCode::INTERNAL_SERVER_ERROR, "io error");
    }
    let registered = sqlx::query("INSERT INTO social.community_contents (content_hash, cid, size) VALUES ($1, $2, $3) ON CONFLICT (content_hash) DO NOTHING")
        .bind(&hash)
        .bind(&cid)
        .bind(size as i32)
        .execute(&state.pool)
        .await;
    if let Err(e) = registered {
        return db_err(e);
    }
    Json(json!({ "ok": true, "content_hash": hash, "size": size })).into_response()
}

/// `GET /federation/communities/content/{hash}`: the registered body.
pub async fn get_content(State(state): State<SharedState>, Path(hash): Path<String>) -> Response {
    let hash = hash.to_ascii_lowercase();
    if !is_valid_hash(&hash) {
        return bad(format!("invalid content hash: {hash}"));
    }
    let cid = match registered_cid(&state.pool, &hash).await {
        Ok(Some(cid)) => cid,
        Ok(None) => return err_json(StatusCode::NOT_FOUND, "content not found"),
        Err(e) => return db_err(e),
    };
    match state.storage.retrieve(&cid).await {
        Ok(Some(bytes)) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            if let Ok(v) = HeaderValue::from_str(&hash) {
                headers.insert("x-content-hash", v);
            }
            (StatusCode::OK, headers, bytes).into_response()
        }
        Ok(None) => err_json(StatusCode::NOT_FOUND, "content not found"),
        Err(e) => {
            tracing::error!(error = %e, "community content store io error");
            err_json(StatusCode::INTERNAL_SERVER_ERROR, "io error")
        }
    }
}

/// `POST /federation/communities/content/gc` (the admin bearer): drops the
/// registered bodies no logged post names, once older than the blob
/// collector's age floor, and their blobs when no other domain holds them.
pub async fn gc_content(State(state): State<SharedState>, headers: HeaderMap) -> Response {
    if !state.admin_bearer(&headers) {
        return err_json(StatusCode::UNAUTHORIZED, "admin bearer token required");
    }
    match collect(&state).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "community content gc failed");
            err_json(StatusCode::INTERNAL_SERVER_ERROR, "db error")
        }
    }
}

async fn collect(state: &CommunitiesState) -> anyhow::Result<Value> {
    let floor = chrono::Utc::now().naive_utc()
        - chrono::Duration::from_std(crate::storage::gc::MIN_ORPHAN_AGE)?;
    let scanned: i64 = sqlx::query_scalar("SELECT count(*) FROM social.community_contents")
        .fetch_one(&state.pool)
        .await?;
    let referenced: i64 = sqlx::query_scalar("SELECT count(DISTINCT content_hash) FROM social.community_signed_actions WHERE content_hash IS NOT NULL").fetch_one(&state.pool).await?;
    let mut removed = 0i64;
    loop {
        let dropped: Vec<String> = sqlx::query_scalar(
            "DELETE FROM social.community_contents WHERE content_hash IN ( \
               SELECT cc.content_hash FROM social.community_contents cc \
               WHERE cc.stored_at < $1 \
                 AND NOT EXISTS (SELECT 1 FROM social.community_signed_actions a WHERE a.content_hash = cc.content_hash) \
               ORDER BY cc.content_hash LIMIT $2) \
             RETURNING cid",
        )
        .bind(floor)
        .bind(GC_CHUNK)
        .fetch_all(&state.pool)
        .await?;
        if dropped.is_empty() {
            break;
        }
        removed += dropped.len() as i64;
        let held = crate::storage::gc::referenced_among(&state.pool, &dropped).await?;
        for cid in dropped.iter().filter(|c| !held.contains(*c)) {
            let registered_again: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM social.community_contents WHERE cid = $1)",
            )
            .bind(cid)
            .fetch_one(&state.pool)
            .await?;
            if !registered_again {
                state.storage.delete_strict(cid).await?;
            }
        }
    }
    Ok(
        json!({ "ok": true, "scanned": scanned, "kept": scanned - removed, "removed": removed, "referenced": referenced }),
    )
}

/// `community-nonce-sweep`: drops the nonces whose replay window closed.
pub fn spawn_job(state: &SharedState, jobs: &Jobs) {
    let pool = state.pool.clone();
    let cfg = PeriodicCfg::every(Duration::from_secs(300))
        .with_jitter(Duration::from_secs(30))
        .with_error_backoff(Duration::from_secs(900));
    jobs.spawn_periodic_leased("community-nonce-sweep", cfg, move |_ctx| {
        let pool = pool.clone();
        async move {
            crate::fed::sweep_nonces(&pool, NONCES, chrono::Utc::now().timestamp()).await?;
            Ok(())
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::signers::local::PrivateKeySigner;
    use axum::Router;
    use tower::ServiceExt;

    use crate::fed::domains;
    use crate::fed::test_support::sign;
    use crate::shared::signed_fetch::test_support::signed_headers_by;

    const ADMIN: &str = "fed-admin";

    fn now() -> i64 {
        chrono::Utc::now().timestamp()
    }

    fn address(w: &PrivateKeySigner) -> String {
        format!("{:#x}", w.address())
    }

    async fn call(
        app: &Router,
        method: &str,
        path: &str,
        headers: HeaderMap,
        body: Body,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(path);
        for (k, v) in headers.iter() {
            req = req.header(k, v);
        }
        let resp = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned())),
        )
    }

    async fn send<T: TypedMessage>(
        app: &Router,
        w: &PrivateKeySigner,
        method: &str,
        path: &str,
        signed: &Signed<T>,
    ) -> (StatusCode, Value) {
        let headers = signed_headers_by(
            w,
            &method.to_lowercase(),
            path,
            "{}",
            true,
            &chrono::Utc::now().timestamp_millis().to_string(),
        )
        .await;
        call(
            app,
            method,
            path,
            headers,
            Body::from(serde_json::to_vec(signed).unwrap()),
        )
        .await
    }

    fn env<T: TypedMessage>(w: &PrivateKeySigner, message: T) -> Signed<T> {
        sign(w, domains::communities(), message, now())
    }

    /// Signed at `at`: the role actions on one member carry increasing
    /// seconds, as a person's do, since the tie-break orders by them.
    fn env_at<T: TypedMessage>(w: &PrivateKeySigner, message: T, at: i64) -> Signed<T> {
        sign(w, domains::communities(), message, at)
    }

    async fn claimed_name(pool: &PgPool, w: &PrivateKeySigner) {
        let metadata = json!({ "v": { "avatars": [{ "name": "fed", "hasClaimedName": true, "avatar": { "snapshots": { "face256": "bafkface" } } }] } });
        sqlx::query(
            "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
             VALUES ($1, 'v3', 'profile', $2, $3, timezone('UTC', now()), ARRAY[$1]::text[], timezone('UTC', now()), '[]'::json)",
        )
        .bind(address(w))
        .bind(format!("bafkprofile{}", Uuid::new_v4().simple()))
        .bind(metadata)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn role_of(pool: &PgPool, community: Uuid, w: &PrivateKeySigner) -> Option<String> {
        sqlx::query_scalar("SELECT role FROM social.community_members WHERE community_id = $1 AND member_address = $2").bind(community).bind(address(w)).fetch_optional(pool).await.unwrap()
    }

    struct Recorded(std::sync::Mutex<Vec<(CommunityId, String)>>);

    impl super::super::VoiceEviction for Recorded {
        fn evict<'a>(
            &'a self,
            community_id: CommunityId,
            target: &'a str,
        ) -> futures::future::BoxFuture<'a, Result<(), String>> {
            self.0
                .lock()
                .unwrap()
                .push((community_id, target.to_string()));
            Box::pin(async { Ok(()) })
        }
    }

    /// A ban in a private community takes the seat in its voice room
    /// (upstream #482) through the comms module's eviction; a public one's
    /// is left alone.
    #[tokio::test]
    async fn a_ban_in_a_private_community_evicts_the_voice_seat() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("catalyst-community-voice-{}", Uuid::new_v4()));
        let storage =
            std::sync::Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
        let state = std::sync::Arc::new(CommunitiesState::new(
            pool.clone(),
            storage,
            super::super::Settings::for_tests(),
            None,
            &crate::http::Outbound::for_tests(),
            CommunitiesState::explore_for_tests(&pool),
        ));
        let recorded = std::sync::Arc::new(Recorded(std::sync::Mutex::new(Vec::new())));
        state.set_voice_eviction(recorded.clone());
        let app = Router::new().merge(super::super::router(state.clone()));
        let (owner, member) = (PrivateKeySigner::random(), PrivateKeySigner::random());
        claimed_name(&pool, &owner).await;
        for private in [true, false] {
            let name = format!("voice {}", &Uuid::new_v4().simple().to_string()[..8]);
            let (status, body) = send(
                &app,
                &owner,
                "POST",
                "/v1/communities",
                &env(
                    &owner,
                    CommunityCreate {
                        name,
                        description: "d".into(),
                        private,
                        unlisted: false,
                        flags: vec![],
                    },
                ),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            let (hex_id, id) = (
                body["community_id"].as_str().unwrap().to_string(),
                body["id"].as_str().unwrap().parse::<Uuid>().unwrap(),
            );
            sqlx::query("INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, 'member')").bind(id).bind(address(&member)).execute(&pool).await.unwrap();
            let (status, body) = send(
                &app,
                &owner,
                "POST",
                &format!("/v1/communities/{id}/members/{}/bans", address(&member)),
                &env(
                    &owner,
                    CommunityBan {
                        community_id: hex_id,
                        target: address(&member),
                        reason: None,
                    },
                ),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            let evicted = recorded
                .0
                .lock()
                .unwrap()
                .iter()
                .any(|(c, t)| c.as_uuid() == id && *t == address(&member));
            assert_eq!(evicted, private, "private={private}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    use super::super::tests::world_access::WorldExits;

    /// A federated leave and ban re-decide world admission and a delete
    /// closes the community; a superseded ban or an already-recorded delete
    /// tells the worlds nothing.
    #[tokio::test]
    async fn federated_exits_reach_the_worlds() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("catalyst-community-exits-{}", Uuid::new_v4()));
        let storage =
            std::sync::Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
        let state = std::sync::Arc::new(CommunitiesState::new(
            pool.clone(),
            storage,
            super::super::Settings::for_tests(),
            None,
            &crate::http::Outbound::for_tests(),
            CommunitiesState::explore_for_tests(&pool),
        ));
        let recorded = std::sync::Arc::new(WorldExits::default());
        state.set_world_access_revocation(recorded.clone());
        let app = Router::new().merge(super::super::router(state.clone()));
        let (owner, leaver, banned, late) = (
            PrivateKeySigner::random(),
            PrivateKeySigner::random(),
            PrivateKeySigner::random(),
            PrivateKeySigner::random(),
        );
        claimed_name(&pool, &owner).await;
        let create = async || {
            let name = format!("exits {}", &Uuid::new_v4().simple().to_string()[..8]);
            let create = CommunityCreate {
                name,
                description: "d".into(),
                private: false,
                unlisted: false,
                flags: vec![],
            };
            let (status, body) = send(
                &app,
                &owner,
                "POST",
                "/v1/communities",
                &env(&owner, create),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            (
                body["community_id"].as_str().unwrap().to_string(),
                body["id"].as_str().unwrap().parse::<Uuid>().unwrap(),
            )
        };
        let (hex_id, id) = create().await;
        for w in [&leaver, &banned] {
            sqlx::query("INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, 'member')").bind(id).bind(address(w)).execute(&pool).await.unwrap();
        }
        let base = format!("/v1/communities/{id}");
        let t0 = now();
        let join = CommunityJoin {
            community_id: hex_id.clone(),
        };
        let (status, body) = send(
            &app,
            &late,
            "POST",
            &format!("{base}/members"),
            &env_at(&late, join, t0),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let leave = CommunityLeave {
            community_id: hex_id.clone(),
            member: address(&leaver),
        };
        let (status, body) = send(
            &app,
            &leaver,
            "DELETE",
            &format!("{base}/members/{}", address(&leaver)),
            &env(&leaver, leave),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        for (target, at) in [(&banned, t0), (&late, t0 - 1)] {
            let ban = CommunityBan {
                community_id: hex_id.clone(),
                target: address(target),
                reason: None,
            };
            let (status, body) = send(
                &app,
                &owner,
                "POST",
                &format!("{base}/members/{}/bans", address(target)),
                &env_at(&owner, ban, at),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
        }
        assert_eq!(
            role_of(&pool, id, &late).await.as_deref(),
            Some("member"),
            "the ban signed before the join is superseded"
        );

        // A delete whose record already landed (a replay) is not fresh.
        let delete = env(
            &owner,
            CommunityDelete {
                community_id: hex_id.clone(),
            },
        );
        let mut conn = pool.acquire().await.unwrap();
        assert!(
            record(
                &mut conn,
                &delete.signature_hash(),
                &hex_id,
                CommunityId::from_uuid(id),
                &Address::parse(&address(&owner)).unwrap(),
                delete.signed_at,
                &payload(&delete.message),
                &Entry {
                    kind: "delete",
                    ..Default::default()
                },
            )
            .await
            .unwrap()
        );
        drop(conn);
        let (status, body) = send(&app, &owner, "DELETE", &base, &delete).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (fresh_hex, fresh) = create().await;
        let delete = CommunityDelete {
            community_id: fresh_hex,
        };
        let (status, body) = send(
            &app,
            &owner,
            "DELETE",
            &format!("/v1/communities/{fresh}"),
            &env(&owner, delete),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            *recorded.0.lock().unwrap(),
            vec![
                format!("member {id} {}", address(&leaver)),
                format!("member {id} {}", address(&banned)),
                format!("closed {fresh}"),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn routes_and_messages_keep_the_old_wire() {
        assert_eq!(
            FedRoute::of(&Method::POST, "/v1/communities"),
            Some(FedRoute::Create)
        );
        assert_eq!(
            FedRoute::of(&Method::PATCH, "/v1/communities/c/members/0xA"),
            Some(FedRoute::Role("c".into(), "0xA".into()))
        );
        assert_eq!(
            FedRoute::of(&Method::DELETE, "/v1/communities/c/posts/p/like"),
            Some(FedRoute::Unlike("c".into(), "p".into()))
        );
        assert_eq!(
            FedRoute::of(&Method::DELETE, "/v1/communities/c/places/a%20b"),
            Some(FedRoute::PlaceRemove("c".into(), "a b".into()))
        );
        assert_eq!(FedRoute::of(&Method::GET, "/v1/communities/c"), None);
        assert_eq!(
            FedRoute::of(&Method::POST, "/v1/communities/c/requests"),
            None,
            "join requests were never federated"
        );
        assert_eq!(CommunityRoleUpdate::PRIMARY_TYPE, "CommunityRole");
        let r = CommunityRoleUpdate {
            community_id: "c".into(),
            target: "t".into(),
            role: "mod".into(),
        };
        assert_eq!(r.encode_struct(), b"CommunityRole\0c\0t\0mod\0".to_vec());
        assert_eq!(
            CommunityJoin {
                community_id: "c".into()
            }
            .encode_struct(),
            b"CommunityJoin\0c\0".to_vec()
        );
        assert_eq!(
            community_id_hex("0xAB", "n", &[1; 16]),
            community_id_hex("0xab", "n", &[1; 16])
        );
        assert_eq!(
            post_uuid(&"ab".repeat(32)),
            CommunityId::from_federation_hex(&"ab".repeat(32)).as_uuid()
        );
    }

    /// Every federated write through the client route it shares, its old
    /// checks and answers, the projection the REST surface reads, the tie-
    /// break, the feeds and the content routes.
    #[tokio::test]
    async fn envelopes_take_the_federated_branch_and_project() {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("catalyst-community-fed-{}", Uuid::new_v4()));
        let storage =
            std::sync::Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
        let settings = super::super::Settings {
            admin_token: Some(ADMIN.into()),
            ..super::super::Settings::for_tests()
        };
        let state = std::sync::Arc::new(CommunitiesState::new(
            pool.clone(),
            storage,
            settings,
            None,
            &crate::http::Outbound::for_tests(),
            CommunitiesState::explore_for_tests(&pool),
        ));
        let app = Router::new()
            .merge(super::super::router(state.clone()))
            .nest("/social-api", super::super::router(state.clone()));
        let (owner, member, other) = (
            PrivateKeySigner::random(),
            PrivateKeySigner::random(),
            PrivateKeySigner::random(),
        );
        claimed_name(&pool, &owner).await;
        let (_, before) = call(
            &app,
            "GET",
            "/federation/communities/snapshot",
            HeaderMap::new(),
            Body::empty(),
        )
        .await;
        let since = ["latest_role_seq", "latest_posts_seq", "latest_places_seq"]
            .iter()
            .map(|k| before[*k].as_i64().unwrap())
            .max()
            .unwrap();

        let name = format!("fed {}", &Uuid::new_v4().simple().to_string()[..8]);
        let create = env(
            &owner,
            CommunityCreate {
                name: name.clone(),
                description: "d".into(),
                private: false,
                unlisted: false,
                flags: vec![],
            },
        );
        let (status, body) = send(&app, &owner, "POST", "/v1/communities", &create).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let hex_id = body["community_id"].as_str().unwrap().to_string();
        let id: Uuid = body["id"].as_str().unwrap().parse().unwrap();
        assert_eq!(
            (hex_id.clone(), id),
            (
                community_id_hex(&address(&owner), &name, &create.nonce),
                CommunityId::from_federation_hex(&hex_id).as_uuid()
            )
        );
        assert_eq!(role_of(&pool, id, &owner).await.as_deref(), Some("owner"));
        let (status, _) = send(&app, &owner, "POST", "/v1/communities", &create).await;
        assert_eq!(status, StatusCode::CONFLICT, "a replayed nonce");
        let (status, body) = send(
            &app,
            &other,
            "POST",
            "/social-api/v1/communities",
            &env(
                &other,
                CommunityCreate {
                    name: "x".into(),
                    description: "d".into(),
                    private: false,
                    unlisted: false,
                    flags: vec![],
                },
            ),
        )
        .await;
        assert_eq!(
            (status, body["message"].as_str()),
            (
                StatusCode::UNAUTHORIZED,
                Some(format!("The user {} doesn't have any names", address(&other)).as_str())
            ),
            "nested, and the owned-name gate"
        );
        let (status, body) = send(
            &app,
            &owner,
            "POST",
            "/v1/communities",
            &sign(
                &owner,
                domains::places(),
                CommunityCreate {
                    name: "y".into(),
                    description: "d".into(),
                    private: false,
                    unlisted: false,
                    flags: vec![],
                },
                now(),
            ),
        )
        .await;
        assert_eq!(
            (status, body["message"].as_str()),
            (
                StatusCode::BAD_REQUEST,
                Some("domain mismatch: expected DecentralandCommunities")
            )
        );
        let unsigned = call(
            &app,
            "POST",
            "/v1/communities",
            HeaderMap::new(),
            Body::from(
                serde_json::to_vec(&env(
                    &owner,
                    CommunityJoin {
                        community_id: hex_id.clone(),
                    },
                ))
                .unwrap(),
            ),
        )
        .await;
        let client_unsigned = call(
            &app,
            "POST",
            "/v1/communities",
            HeaderMap::new(),
            Body::from("{}"),
        )
        .await;
        assert_eq!(
            unsigned, client_unsigned,
            "an envelope still needs the signed fetch, refused as the client route refuses it"
        );
        assert!(unsigned.0.is_client_error());

        let base = format!("/v1/communities/{id}");
        let t0 = now() - 200;
        let (status, body) = send(
            &app,
            &member,
            "POST",
            &format!("{base}/members"),
            &env_at(
                &member,
                CommunityJoin {
                    community_id: hex_id.clone(),
                },
                t0,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(role_of(&pool, id, &member).await.as_deref(), Some("member"));
        let (status, body) = send(
            &app,
            &member,
            "POST",
            &format!("{base}/members"),
            &env(
                &member,
                CommunityJoin {
                    community_id: "00".repeat(32),
                },
            ),
        )
        .await;
        assert_eq!(
            (status, body["message"].as_str()),
            (StatusCode::BAD_REQUEST, Some("community_id mismatch"))
        );

        // The role, with the old tie-break: an older action arriving later
        // is logged and not projected.
        let promote = env_at(
            &owner,
            CommunityRoleUpdate {
                community_id: hex_id.clone(),
                target: address(&member),
                role: "mod".into(),
            },
            t0 + 1,
        );
        let (status, body) = send(
            &app,
            &owner,
            "PATCH",
            &format!("{base}/members/{}", address(&member)),
            &promote,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            role_of(&pool, id, &member).await.as_deref(),
            Some("moderator")
        );
        let stale = sign(
            &owner,
            domains::communities(),
            CommunityRoleUpdate {
                community_id: hex_id.clone(),
                target: address(&member),
                role: "member".into(),
            },
            promote.signed_at - 10,
        );
        let (status, _) = send(
            &app,
            &owner,
            "PATCH",
            &format!("{base}/members/{}", address(&member)),
            &stale,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            role_of(&pool, id, &member).await.as_deref(),
            Some("moderator"),
            "the newer promotion stands"
        );
        let (status, body) = send(
            &app,
            &owner,
            "PATCH",
            &format!("{base}/members/{}", address(&owner)),
            &env(
                &owner,
                CommunityRoleUpdate {
                    community_id: hex_id.clone(),
                    target: address(&owner),
                    role: "member".into(),
                },
            ),
        )
        .await;
        assert_eq!(
            (status, body["message"].as_str()),
            (
                StatusCode::FORBIDDEN,
                Some("a user cannot update their own role")
            )
        );

        let (status, _) = send(
            &app,
            &member,
            "PUT",
            &base,
            &env(
                &member,
                CommunityUpdate {
                    community_id: hex_id.clone(),
                    name: Some("renamed".into()),
                    description: None,
                    private: None,
                    unlisted: None,
                },
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "a moderator cannot rename"
        );
        let (status, body) = send(
            &app,
            &owner,
            "PATCH",
            &base,
            &env(
                &owner,
                CommunityUpdate {
                    community_id: hex_id.clone(),
                    name: Some(format!("{name} 2")),
                    description: None,
                    private: None,
                    unlisted: None,
                },
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");

        let (status, body) = send(
            &app,
            &member,
            "POST",
            &format!("{base}/places"),
            &env(
                &member,
                CommunityPlacesAdd {
                    community_id: hex_id.clone(),
                    place_ids: vec!["p-1".into(), "p-2".into()],
                },
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, _) = send(
            &app,
            &owner,
            "DELETE",
            &format!("{base}/places/p-2"),
            &env(
                &owner,
                CommunityPlaceRemove {
                    community_id: hex_id.clone(),
                    place_id: "p-2".into(),
                },
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let places: Vec<String> = sqlx::query_scalar(
            "SELECT place_id FROM social.community_places WHERE community_id = $1",
        )
        .bind(id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(places, ["p-1"]);

        // A post whose body was uploaded first lands with its text.
        let (status, stored) = call(
            &app,
            "POST",
            "/federation/communities/content",
            HeaderMap::new(),
            Body::from("hello peers"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{stored}");
        let content = stored["content_hash"].as_str().unwrap().to_string();
        assert_eq!(content, hex::encode(Sha256::digest(b"hello peers")));
        let (status, got) = call(
            &app,
            "GET",
            &format!("/federation/communities/content/{content}"),
            HeaderMap::new(),
            Body::empty(),
        )
        .await;
        assert_eq!(
            (status, got),
            (StatusCode::OK, Value::String("hello peers".into()))
        );
        let post_env = env(
            &owner,
            CommunityPost {
                community_id: hex_id.clone(),
                content_hash: content.clone(),
            },
        );
        let (status, body) = send(&app, &owner, "POST", &format!("{base}/posts"), &post_env).await;
        assert_eq!(
            (status, body["content_body_local"].clone()),
            (StatusCode::OK, json!(true)),
            "{body}"
        );
        let post_id = post_env.signature_hash();
        let text: String =
            sqlx::query_scalar("SELECT content FROM social.community_posts WHERE id = $1")
                .bind(post_uuid(&post_id))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(text, "hello peers");

        let (status, _) = send(
            &app,
            &member,
            "POST",
            &format!("{base}/posts/{post_id}/like"),
            &env(
                &member,
                CommunityPostLike {
                    community_id: hex_id.clone(),
                    post_id: post_id.clone(),
                },
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let likes = || async {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM social.community_post_likes WHERE post_id = $1",
            )
            .bind(post_uuid(&post_id))
            .fetch_one(&pool)
            .await
            .unwrap()
        };
        assert_eq!(likes().await, 1);
        let (status, _) = send(
            &app,
            &member,
            "DELETE",
            &format!("{base}/posts/{post_id}/like"),
            &env(
                &member,
                CommunityPostUnlike {
                    community_id: hex_id.clone(),
                    post_id: post_id.clone(),
                },
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(likes().await, 0);
        let (status, _) = send(
            &app,
            &member,
            "DELETE",
            &format!("{base}/posts/{post_id}"),
            &env(
                &member,
                CommunityPostDelete {
                    community_id: hex_id.clone(),
                    post_id: post_id.clone(),
                },
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "a moderator deletes only their own posts"
        );
        let (status, _) = send(
            &app,
            &owner,
            "DELETE",
            &format!("{base}/posts/{post_id}"),
            &env(
                &owner,
                CommunityPostDelete {
                    community_id: hex_id.clone(),
                    post_id: post_id.clone(),
                },
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let left: i64 =
            sqlx::query_scalar("SELECT count(*) FROM social.community_posts WHERE id = $1")
                .bind(post_uuid(&post_id))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(left, 0);

        // A request settles once; bans and unbans reach the member and ban rows.
        let request = Uuid::new_v4();
        sqlx::query("INSERT INTO social.community_requests (id, community_id, member_address, type) VALUES ($1, $2, $3, 'request_to_join')").bind(request).bind(id).bind(address(&other)).execute(&pool).await.unwrap();
        let (status, body) = send(
            &app,
            &member,
            "PATCH",
            &format!("{base}/requests/{request}"),
            &env(
                &member,
                CommunityRequestStatusUpdate {
                    community_id: hex_id.clone(),
                    request_id: request.to_string(),
                    status: "rejected".into(),
                },
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, _) = send(
            &app,
            &owner,
            "PATCH",
            &format!("{base}/requests/{request}"),
            &env(
                &owner,
                CommunityRequestStatusUpdate {
                    community_id: hex_id.clone(),
                    request_id: request.to_string(),
                    status: "accepted".into(),
                },
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let settled: String =
            sqlx::query_scalar("SELECT status FROM social.community_requests WHERE id = $1")
                .bind(request)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(settled, "rejected", "a settled request is not reopened");

        let (status, body) = send(
            &app,
            &owner,
            "POST",
            &format!("{base}/members/{}/bans", address(&member)),
            &env_at(
                &owner,
                CommunityBan {
                    community_id: hex_id.clone(),
                    target: address(&member),
                    reason: Some("spam".into()),
                },
                t0 + 2,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(role_of(&pool, id, &member).await, None);
        let (status, _) = send(
            &app,
            &member,
            "POST",
            &format!("{base}/members"),
            &env(
                &member,
                CommunityJoin {
                    community_id: hex_id.clone(),
                },
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "banned from community");
        let (status, _) = send(
            &app,
            &owner,
            "DELETE",
            &format!("{base}/members/{}/bans", address(&member)),
            &env_at(
                &owner,
                CommunityUnban {
                    community_id: hex_id.clone(),
                    target: address(&member),
                },
                t0 + 3,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let active: bool = sqlx::query_scalar("SELECT active FROM social.community_bans WHERE community_id = $1 AND banned_address = $2").bind(id).bind(address(&member)).fetch_one(&pool).await.unwrap();
        assert!(!active);

        let (status, _) = send(
            &app,
            &member,
            "POST",
            &format!("{base}/members"),
            &env_at(
                &member,
                CommunityJoin {
                    community_id: hex_id.clone(),
                },
                t0 + 4,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, body) = send(
            &app,
            &member,
            "DELETE",
            &format!("{base}/members/{}", address(&owner)),
            &env(
                &member,
                CommunityLeave {
                    community_id: hex_id.clone(),
                    member: address(&member),
                },
            ),
        )
        .await;
        assert_eq!(
            (status, body["message"].as_str()),
            (
                StatusCode::FORBIDDEN,
                Some("path address must match signer")
            )
        );
        let (status, _) = send(
            &app,
            &member,
            "DELETE",
            &format!("{base}/members/{}", address(&member)),
            &env_at(
                &member,
                CommunityLeave {
                    community_id: hex_id.clone(),
                    member: address(&member),
                },
                t0 + 5,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(role_of(&pool, id, &member).await, None);
        let (status, _) = send(
            &app,
            &owner,
            "DELETE",
            &format!("{base}/members/{}", address(&owner)),
            &env(
                &owner,
                CommunityLeave {
                    community_id: hex_id.clone(),
                    member: address(&owner),
                },
            ),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "the owner cannot leave");

        // The feeds peers pull.
        let (_, snap) = call(
            &app,
            "GET",
            "/federation/communities/snapshot",
            HeaderMap::new(),
            Body::empty(),
        )
        .await;
        let listed = snap["communities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["community_id"] == json!(hex_id))
            .unwrap()
            .clone();
        assert_eq!(
            (listed["creator"].clone(), listed["name"].clone()),
            (json!(address(&owner)), json!(format!("{name} 2")))
        );
        assert_eq!(snap["role_log_hash"].as_str().map(str::len), Some(64));
        let (_, feed) = call(
            &app,
            "GET",
            &format!("/federation/communities/changes?since={since}"),
            HeaderMap::new(),
            Body::empty(),
        )
        .await;
        let mine: Vec<&Value> = feed["role"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["community_id"] == json!(hex_id))
            .collect();
        let roles: Vec<&str> = mine.iter().map(|r| r["role"].as_str().unwrap()).collect();
        assert_eq!(
            roles,
            [
                "owner", "member", "mod", "member", "banned", "none", "member", "none"
            ]
        );
        assert_eq!(
            mine[0]["payload"],
            json!({ "type": "CommunityCreate", "community_id": hex_id, "creator": address(&owner), "role": "owner" })
        );
        let posts: Vec<&Value> = feed["posts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["community_id"] == json!(hex_id))
            .collect();
        assert_eq!(
            (
                posts.len(),
                posts[0]["content_hash"].clone(),
                posts[0]["author"].clone()
            ),
            (1, json!(content), json!(address(&owner)))
        );
        let places: Vec<(String, String)> = feed["places"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["community_id"] == json!(hex_id))
            .map(|p| {
                (
                    p["place_id"].as_str().unwrap().to_string(),
                    p["action"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(
            places,
            [
                ("p-1".to_string(), "add".to_string()),
                ("p-2".into(), "add".into()),
                ("p-2".into(), "remove".into())
            ]
        );
        assert!(
            feed["places"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["signature_hash"].as_str().unwrap().ends_with("-add-1"))
        );

        let (status, _) = call(
            &app,
            "POST",
            "/federation/communities/content/gc",
            HeaderMap::new(),
            Body::empty(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let mut bearer = HeaderMap::new();
        bearer.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {ADMIN}")).unwrap(),
        );
        let (status, report) = call(
            &app,
            "POST",
            "/federation/communities/content/gc",
            bearer,
            Body::empty(),
        )
        .await;
        assert_eq!(
            (status, report["ok"].clone()),
            (StatusCode::OK, json!(true)),
            "{report}"
        );
        assert_eq!(
            call(
                &app,
                "GET",
                &format!("/federation/communities/content/{content}"),
                HeaderMap::new(),
                Body::empty()
            )
            .await
            .0,
            StatusCode::OK,
            "a referenced, fresh body stays"
        );
        assert_eq!(
            call(
                &app,
                "GET",
                "/federation/communities/content/nothex",
                HeaderMap::new(),
                Body::empty()
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );

        let (status, _) = send(
            &app,
            &owner,
            "DELETE",
            &base,
            &env(
                &owner,
                CommunityDelete {
                    community_id: hex_id.clone(),
                },
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let active: bool =
            sqlx::query_scalar("SELECT active FROM social.communities WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!active);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
