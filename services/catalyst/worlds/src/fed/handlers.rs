//! The four federation routes, at their old paths: `/federation/worlds/peers`,
//! `/federation/worlds/mirror` (public), `/admin/federation/worlds/refresh` and
//! `/admin/federation/worlds/{peer_id}/{world_name}/hidden` (bearer
//! `CATALYRST_WORLDS_ADMIN_TOKEN`). Errors render through
//! [`crate::shared::errors::ApiError`], the one copy of the old
//! `catalyrst_types::ApiErrorBody` envelope the worlds service answered with.
//!
//! Every response is a typed struct rather than a `json!` macro, so the guarantee
//! "**no `owner` key exists in the mirror response**" is checkable by a reviewer.

use std::sync::Arc;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};

use super::FedState;
use super::names::RemoteWorldName;
use super::peers::{PeerOmitted, WorldsFederationPeers};
use super::poll::PollOutcome;
use super::store::{RemotePeerStatus, RemoteWorld};
use crate::shared::errors::ApiError;

/// The old worlds admin gate: the configured bearer, timing-safe; an unset or empty
/// token matches nothing, and every refusal is the same 403.
pub(crate) fn authorize_admin(expected: Option<&str>, headers: &HeaderMap) -> Result<(), ApiError> {
    crate::auth::admin_bearer::check(expected, headers)
        .map_err(|_| ApiError::forbidden("You are not authorized to access this resource"))
}

/// Health of one peer's mirror.
///
/// `lastSuccessAt` is the field that keeps an unreachable peer honest. A peer whose
/// `lastAttemptAt` is recent and whose `lastSuccessAt` is old (or null) is a peer whose
/// mirrored rows are **stale or absent for lack of contact** -- which is a different
/// statement from "this peer holds no worlds", and the two must never render the same.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct FederationPeerStatusView {
    pub last_attempt_at: Option<String>,
    pub last_success_at: Option<String>,
    /// The last failure, or `null` after a clean success.
    ///
    /// This route is public, so the text is verbatim for anything the **peer** did -- an
    /// HTTP status, a content type, a message about their body -- and a bounded constant
    /// for a fault in **our** database, whose reason is in this server's logs. Either
    /// way the failure itself is reported; only its wording is withheld.
    ///
    /// It can be non-null *beside* a fresh `lastSuccessAt`, and that pairing is
    /// deliberate: a poll whose fetch and write succeeded but whose local-name collision
    /// probe could not run records
    /// [`super::poll::COLLISION_PROBE_UNAVAILABLE_PREFIX`] here. "Current rows, one
    /// unchecked thing about them" is a state the two fields express together and
    /// neither expresses alone.
    pub last_error: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub worlds_observed: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub entries_skipped: i64,
    pub truncated: bool,
    /// `false` until the peer has answered successfully at least once. A consumer that
    /// sees `hasEverSucceeded: false` is looking at no knowledge, not at an empty peer.
    pub has_ever_succeeded: bool,
}

impl FederationPeerStatusView {
    fn never_attempted() -> Self {
        Self {
            last_attempt_at: None,
            last_success_at: None,
            last_error: None,
            worlds_observed: 0,
            entries_skipped: 0,
            truncated: false,
            has_ever_succeeded: false,
        }
    }

    fn from_status(s: &RemotePeerStatus) -> Self {
        Self {
            last_attempt_at: s.last_attempt_rfc3339(),
            last_success_at: s.last_success_rfc3339(),
            last_error: s.last_error.clone(),
            worlds_observed: s.worlds_observed,
            entries_skipped: s.entries_skipped,
            truncated: s.truncated,
            has_ever_succeeded: s.last_success_at.is_some(),
        }
    }

    fn for_peer(statuses: &[RemotePeerStatus], peer_id: &str) -> Self {
        statuses
            .iter()
            .find(|s| s.peer_id == peer_id)
            .map(Self::from_status)
            .unwrap_or_else(Self::never_attempted)
    }
}

/// One admitted peer.
///
/// Deliberately not fields of this type: `mtlsRootPem` and `gossipPubkey`. They are not
/// omitted at serialisation time -- they do not exist here, so no future edit to a
/// serialiser can start printing them.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct FederationPeerView {
    pub peer_id: String,
    pub worlds_url: String,
    pub dao_proposal: String,
    pub added_at: String,
    /// True when this peer was admitted through `WORLDS_FED_ALLOW_INSECURE_LOOPBACK_PEERS`
    /// and is therefore spoken to in **cleartext, authenticated by nothing**. Surfaced so
    /// a two-node test rig is never mistaken for a federation deployment.
    pub insecure_loopback: bool,
    pub status: FederationPeerStatusView,
}

/// A peer in the file that is not a worlds peer.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct FederationPeerOmissionView {
    pub peer_id: String,
    pub reason: String,
    pub detail: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct FederationPeersResponse {
    /// Always `true` on a 200. The unconfigured case is a 503, not `configured: false`
    /// with an empty list, so a client cannot fall through into treating "federation is
    /// off" as "federation is on and nobody is there".
    pub configured: bool,
    /// Absolute path of the peer file that was adjudicated.
    pub peers_file: String,
    pub peers: Vec<FederationPeerView>,
    pub omitted: Vec<FederationPeerOmissionView>,
}

/// `GET /federation/worlds/peers` -- public.
///
/// The allowlist is public by construction: every entry cites a DAO proposal.
pub async fn get_federation_peers(
    State(state): State<Arc<FedState>>,
) -> Result<Json<FederationPeersResponse>, ApiError> {
    let (path, peers, omitted) = match &state.peers {
        WorldsFederationPeers::NotConfigured => {
            return Err(ApiError::service_unavailable(
                WorldsFederationPeers::NOT_CONFIGURED_DETAIL,
            ));
        }
        WorldsFederationPeers::Admitted {
            path,
            peers,
            omitted,
        } => (path, peers, omitted),
    };

    let statuses = state.mirror.store().peer_statuses().await?;

    let peers = peers
        .iter()
        .map(|p| FederationPeerView {
            peer_id: p.peer_id().as_str().to_string(),
            worlds_url: p.worlds_url().to_string(),
            dao_proposal: p.dao_proposal().to_string(),
            added_at: p.added_at().to_string(),
            insecure_loopback: p.is_insecure_loopback(),
            status: FederationPeerStatusView::for_peer(&statuses, p.peer_id().as_str()),
        })
        .collect();

    let omitted = omitted
        .iter()
        .map(|o| {
            let PeerOmitted::NoWorldsUrl { peer_id } = o;
            FederationPeerOmissionView {
                peer_id: peer_id.clone(),
                reason: "noWorldsUrl".to_string(),
                detail: o.to_string(),
            }
        })
        .collect();

    Ok(Json(FederationPeersResponse {
        configured: true,
        peers_file: path.display().to_string(),
        peers,
        omitted,
    }))
}

/// One mirrored world, as published.
///
/// **There is no `owner` key, and no field it could be renamed from.** There is no
/// `access`, no `permissions`, no `blockedSince`, no `deployer`. `peerId` is
/// mandatory: a mirrored world without the peer that reported it would be a claim
/// about a world, which is exactly what this slice refuses to make.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct RemoteWorldView {
    pub peer_id: String,
    /// The name **as the peer reported it**, after shape admission. Not a name we have
    /// resolved, and not a name this server answers for.
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub content_rating: Option<String>,
    pub categories: Vec<String>,
    /// A label the peer printed. We hold no bytes for it; `/contents/{hash}` on this
    /// server will not serve it.
    pub thumbnail_hash: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub deployed_scenes: i64,
    /// As reported by the peer. Read by no branch here.
    pub last_deployed_at: Option<String>,
    /// **Our** clock, at the poll that produced this row. This is the staleness signal
    /// on a per-row basis.
    pub observed_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct FederationMirrorResponse {
    pub worlds: Vec<RemoteWorldView>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
    /// Health of every admitted peer, returned **with the listing**, so a consumer
    /// cannot read `worlds: []` without also being told whether anybody answered. An
    /// empty list next to `hasEverSucceeded: false` is an absence of knowledge; an
    /// empty list next to a fresh `lastSuccessAt` is knowledge of an absence.
    pub peers: Vec<FederationPeerStatusLine>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct FederationPeerStatusLine {
    pub peer_id: String,
    pub status: FederationPeerStatusView,
}

#[derive(Debug, Deserialize)]
pub struct MirrorQuery {
    #[serde(default)]
    pub peer: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// `GET /federation/worlds/mirror` -- public.
///
/// Serves what peers already publish on their own public `/worlds`, qualified by the
/// peer that said it. Vetoed rows (`hidden_since IS NOT NULL`) are excluded.
pub async fn get_federation_mirror(
    State(state): State<Arc<FedState>>,
    Query(q): Query<MirrorQuery>,
) -> Result<Json<FederationMirrorResponse>, ApiError> {
    if !state.peers.is_configured() {
        return Err(ApiError::service_unavailable(
            WorldsFederationPeers::NOT_CONFIGURED_DETAIL,
        ));
    }

    let peer = match q.peer.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => match state.peers.get(raw) {
            Some(p) => Some(p.peer_id().clone()),
            None => {
                return Err(ApiError::not_found(format!(
                    "no admitted worlds federation peer {raw:?}"
                )));
            }
        },
        None => None,
    };

    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    let offset = q.offset.unwrap_or(0).max(0);

    let ((rows, total), statuses) = tokio::try_join!(
        state
            .mirror
            .store()
            .list_mirror(&state.peers, peer.as_ref(), limit, offset),
        state.mirror.store().peer_statuses(),
    )?;
    let peers = state
        .peers
        .peers()
        .iter()
        .map(|p| FederationPeerStatusLine {
            peer_id: p.peer_id().as_str().to_string(),
            status: FederationPeerStatusView::for_peer(&statuses, p.peer_id().as_str()),
        })
        .collect();

    Ok(Json(FederationMirrorResponse {
        worlds: rows
            .into_iter()
            .map(RemoteWorld::into_published_view)
            .collect(),
        total,
        peers,
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct FederationRefreshPeerResult {
    pub peer_id: String,
    pub ok: bool,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub worlds_observed: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub entries_skipped: i64,
    pub truncated: bool,
    /// Local world names this peer also publishes. The local record wins everywhere;
    /// this is the report, not a resolution.
    ///
    /// `null` -- never `[]` -- when the probe did not run, either because this poll
    /// failed before there was anything to probe with or because the probe itself
    /// errored. `[]` means we asked and nothing collided. The two are different facts
    /// and a consumer that reads `.length` off the second must not silently get `0` for
    /// the first.
    pub local_name_collisions: Option<Vec<String>>,
    /// Why `localNameCollisions` is `null`. Non-null exactly when it is -- the same
    /// pairing `lastError` has with a missing `lastSuccessAt`.
    pub local_name_collisions_error: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct FederationRefreshResponse {
    pub polled: Vec<FederationRefreshPeerResult>,
}

/// The reason attached to a `null` collision list on a peer whose poll failed. A
/// constant so a test asserts the contract rather than a literal.
pub const NOT_PROBED_POLL_FAILED: &str =
    "not checked: the poll failed before the local name collision probe ran";

/// `POST /admin/federation/worlds/refresh` -- **admin**.
///
/// This handler can make this server contact every admitted peer, so it authenticates
/// its own caller *as its first statement*, before reading config and before touching
/// any peer. A route that holds privileged outbound reach and does not authenticate its
/// caller is a confused deputy; the federated version of that mistake crosses a trust
/// boundary and is worse.
pub async fn refresh_federation_mirror(
    State(state): State<Arc<FedState>>,
    headers: HeaderMap,
) -> Result<Json<FederationRefreshResponse>, ApiError> {
    authorize_admin(state.admin_token.as_deref(), &headers)?;

    if !state.peers.is_configured() {
        return Err(ApiError::service_unavailable(
            WorldsFederationPeers::NOT_CONFIGURED_DETAIL,
        ));
    }

    let results = state.mirror.poll_all(&state.peers).await;
    let polled = results
        .into_iter()
        .map(|(peer_id, outcome)| match outcome {
            PollOutcome::Polled(r) => FederationRefreshPeerResult {
                peer_id: peer_id.as_str().to_string(),
                ok: true,
                worlds_observed: r.worlds_observed,
                entries_skipped: r.entries_skipped as i64,
                truncated: r.truncated,
                local_name_collisions: r.collisions.checked().map(<[String]>::to_vec),
                local_name_collisions_error: r.collisions.unavailable_reason().map(str::to_string),
                error: None,
            },
            PollOutcome::Failed(e) => FederationRefreshPeerResult {
                peer_id: peer_id.as_str().to_string(),
                ok: false,
                worlds_observed: 0,
                entries_skipped: 0,
                truncated: false,
                local_name_collisions: None,
                local_name_collisions_error: Some(NOT_PROBED_POLL_FAILED.to_string()),
                error: Some(e),
            },
        })
        .collect();

    Ok(Json(FederationRefreshResponse { polled }))
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct SetMirrorHiddenRequest {
    pub hidden: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct SetMirrorHiddenResponse {
    pub peer_id: String,
    pub world_name: String,
    pub hidden: bool,
}

/// `PUT /admin/federation/worlds/{peer_id}/{world_name}/hidden` -- **admin**.
///
/// A **local** veto over our own publication. It sends nothing to the peer, makes no
/// claim about the peer, and is not reversible by a subsequent poll: the poller's
/// `DELETE` spares vetoed rows and its `UPDATE` arm does not name `hidden_since`, so a
/// peer cannot un-hide itself by re-listing.
///
/// Answers 403 to any caller without admin credentials **before** the request body is
/// looked at (the body is `Bytes`, parsed after the check), so the shape of the body is
/// never a signal to an anonymous caller.
pub async fn set_mirror_world_hidden(
    State(state): State<Arc<FedState>>,
    Path((peer_id, world_name)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SetMirrorHiddenResponse>, ApiError> {
    authorize_admin(state.admin_token.as_deref(), &headers)?;

    let body: SetMirrorHiddenRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::bad_request(format!("request body is not valid JSON: {e}")))?;

    if !state.peers.is_configured() {
        return Err(ApiError::service_unavailable(
            WorldsFederationPeers::NOT_CONFIGURED_DETAIL,
        ));
    }

    let Some(peer) = state.peers.get(&peer_id) else {
        return Err(ApiError::not_found(format!(
            "no admitted worlds federation peer {peer_id:?}"
        )));
    };
    let Some(name) = RemoteWorldName::from_operator_veto_path(&world_name) else {
        return Err(ApiError::bad_request(
            "world_name is not a shape this mirror can hold",
        ));
    };

    let now = chrono::Utc::now().naive_utc();
    let touched = state
        .mirror
        .store()
        .set_hidden(peer.peer_id(), &name, body.hidden, now)
        .await?;
    if !touched {
        return Err(ApiError::not_found(format!(
            "peer {} does not have a mirrored world by that name",
            peer.peer_id()
        )));
    }

    tracing::info!(
        peer = %peer.peer_id(),
        world = %name.as_peer_reported_str(),
        hidden = body.hidden,
        "local operator veto applied to a mirrored world; nothing was sent to the peer"
    );

    Ok(Json(SetMirrorHiddenResponse {
        peer_id: peer.peer_id().as_str().to_string(),
        world_name: name.as_peer_reported_str().to_string(),
        hidden: body.hidden,
    }))
}

/// Returned by every federation route when `WORLDS_FED_PEERS_FILE` is unset. Named so
/// tests assert the contract rather than a literal.
pub const NOT_CONFIGURED_STATUS: StatusCode = StatusCode::SERVICE_UNAVAILABLE;

#[cfg(test)]
mod tests {
    use super::*;

    fn with_auth(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("authorization", value.parse().unwrap());
        h
    }

    #[test]
    fn admin_gate_needs_a_configured_token_and_the_exact_bearer() {
        assert!(authorize_admin(Some("s3cret"), &with_auth("Bearer s3cret")).is_ok());
        assert!(authorize_admin(Some("s3cret"), &with_auth("Bearer wrong")).is_err());
        assert!(authorize_admin(Some("s3cret"), &with_auth("Basic s3cret")).is_err());
        assert!(authorize_admin(Some("s3cret"), &HeaderMap::new()).is_err());
        assert!(
            authorize_admin(None, &with_auth("Bearer anything")).is_err(),
            "no token configured admits nobody"
        );
        assert!(
            authorize_admin(Some(""), &with_auth("Bearer ")).is_err(),
            "an empty token matches nothing"
        );
    }
}
