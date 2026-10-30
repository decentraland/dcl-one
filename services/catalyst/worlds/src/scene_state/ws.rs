//! `GET /ws/{scene}`: the client socket. Handshake, then one signed auth
//! frame within the deadline, then admission by the permission core, then
//! the `Init` frame (the entity range and the state snapshot) and the CRDT
//! relay until either side closes or the client is removed.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{OriginalUri, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::NaiveDateTime;
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::policy::{Access, AccessGrant, Ban, WorldStanding};
use crate::shared::identity::{Address, AuthorityNotEstablished};

use super::auth::{candidate_paths, verify_auth_frame};
use super::protocol::{MessageType, decode_message, encode_init_message, encode_message};
use super::scene::Scene;
use super::{SceneHostState, SceneKey, SharedState};

/// Why a verified identity is not let into a scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionRefused {
    Banned(Ban),
    InvalidSecret,
    NotAllowed,
    /// The world is unknown, operator-disabled or deny-listed.
    InvalidWorld,
    /// The policy could not answer (a store fault): refused, fail closed.
    Undetermined(String),
}

impl std::fmt::Display for AdmissionRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Banned(b) => write!(f, "banned ({b:?})"),
            Self::InvalidSecret => f.write_str("shared secret missing or invalid"),
            Self::NotAllowed => f.write_str("not allowed by the world's access setting"),
            Self::InvalidWorld => f.write_str("world unknown, disabled or deny-listed"),
            Self::Undetermined(why) => write!(f, "permission undetermined: {why}"),
        }
    }
}

fn undetermined(e: AuthorityNotEstablished) -> AdmissionRefused {
    AdmissionRefused::Undetermined(e.to_string())
}

/// May `address` be in `scene` at `now`? A world scene needs a known,
/// enabled, non-deny-listed world, then asks
/// `Policy::can_access` (bans, owner, the access setting; `secret` from the
/// signed metadata), a Genesis scene `Policy::is_banned`, the local
/// development scene nobody. `prior` is how the client was admitted before
/// (a re-check without the secret): a shared-secret world keeps a client it
/// admitted by secret until a ban or an access-type change refuses it.
pub async fn admit(
    state: &SceneHostState,
    scene: &Scene,
    address: &Address,
    secret: Option<&str>,
    prior: Option<AccessGrant>,
    now: NaiveDateTime,
) -> Result<Option<AccessGrant>, AdmissionRefused> {
    let ban_key = scene.identity.ban_key();
    match &scene.identity.key {
        SceneKey::Local => Ok(None),
        SceneKey::World(world)
            if !matches!(
                state
                    .policy
                    .world_standing(world)
                    .await
                    .map_err(undetermined)?,
                WorldStanding::Open(_)
            ) =>
        {
            Err(AdmissionRefused::InvalidWorld)
        }
        SceneKey::World(world) => match state
            .policy
            .can_access(world, address, secret, ban_key.as_ref(), None, now)
            .await
            .map_err(undetermined)?
        {
            Access::Allowed(grant) => Ok(Some(grant)),
            Access::Banned(ban) => Err(AdmissionRefused::Banned(ban)),
            Access::InvalidSecret
                if secret.is_none() && prior == Some(AccessGrant::SharedSecret) =>
            {
                Ok(prior)
            }
            Access::InvalidSecret => Err(AdmissionRefused::InvalidSecret),
            Access::NotAllowed => Err(AdmissionRefused::NotAllowed),
        },
        SceneKey::Genesis(_) | SceneKey::GenesisParcel(_) => match state
            .policy
            .is_banned(address, ban_key.as_ref(), None, now)
            .await
            .map_err(undetermined)?
        {
            Some(ban) => Err(AdmissionRefused::Banned(ban)),
            None => Ok(None),
        },
    }
}

pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    Path(scene_name): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: axum::http::HeaderMap,
    State(state): State<SharedState>,
) -> axum::response::Response {
    let Ok(key) = SceneKey::parse(&scene_name) else {
        return (
            StatusCode::NOT_FOUND,
            format!("{scene_name} is not currently loaded in the server"),
        )
            .into_response();
    };
    let mut scene = state.scenes.get(&key.room_key());
    if scene.is_none() && state.settings.autoload && key != SceneKey::Local {
        if let Err(e) = state.load(&key).await {
            tracing::info!(scene = %key, error = %e, "autoload refused");
            let status = if e.to_string().contains("scene host is full") {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::NOT_FOUND
            };
            return (
                status,
                format!("{scene_name} is not currently loaded in the server: {e}"),
            )
                .into_response();
        }
        scene = state.scenes.get(&key.room_key());
    }
    let Some(scene) = scene else {
        return (
            StatusCode::NOT_FOUND,
            format!("{scene_name} is not currently loaded in the server"),
        )
            .into_response();
    };

    let route = format!("/ws/{scene_name}");
    let paths = candidate_paths(&headers, uri.path(), &route);
    let ws = ws
        .max_frame_size(state.settings.ws_max_frame_bytes)
        .max_message_size(state.settings.ws_max_frame_bytes);
    let mgr = Arc::clone(&state);
    ws.on_upgrade(move |socket| async move {
        mgr.scenes.on_ws_connected();
        handle_socket(&mgr, socket, scene, paths).await;
        mgr.scenes.on_ws_closed();
    })
    .into_response()
}

async fn handle_socket(
    state: &SceneHostState,
    socket: WebSocket,
    scene: Arc<Scene>,
    paths: Vec<String>,
) {
    let (mut sink, mut stream) = socket.split();

    let auth = tokio::time::timeout(state.settings.auth_timeout, async {
        while let Some(Ok(msg)) = stream.next().await {
            if let Message::Binary(bytes) = msg
                && let Some((MessageType::Auth, body)) = decode_message(&bytes)
            {
                let now = chrono::Utc::now().timestamp();
                return Some(verify_auth_frame(body, "GET", &paths, now, state.eip1654()).await);
            }
        }
        None
    })
    .await;

    let authed = match auth {
        Ok(Some(Ok(a))) => a,
        Ok(Some(Err(e))) => {
            tracing::debug!(scene = %scene.name, error = %e, "ws auth failed");
            let _ = sink.close().await;
            return;
        }
        _ => {
            tracing::debug!(scene = %scene.name, "ws auth missing or timed out");
            let _ = sink.close().await;
            return;
        }
    };

    let now = chrono::Utc::now().naive_utc();
    let grant = match admit(
        state,
        &scene,
        &authed.signer,
        authed.presented_secret(),
        None,
        now,
    )
    .await
    {
        Ok(grant) => grant,
        Err(reason) => {
            tracing::info!(scene = %scene.name, address = %authed.signer, %reason, "ws admission refused");
            let _ = sink.close().await;
            return;
        }
    };

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(state.settings.limits.client_outbound_max.max(1));
    let (client, init) = scene.add_client_admitted(authed.signer.as_str().to_string(), tx, grant);
    let index = client.index;

    if init.size == 0 {
        tracing::error!(scene = %scene.name, index, address = %authed.signer, "no entity range available; refusing the connection");
        scene.remove_client(index);
        let _ = sink.close().await;
        return;
    }

    let init_frame = encode_init_message(
        &init.crdt_state,
        init.start,
        init.size,
        init.reserved_local_entities,
    );
    if sink.send(Message::Binary(init_frame.into())).await.is_err() {
        scene.remove_client(index);
        return;
    }
    tracing::info!(scene = %scene.name, index, address = %authed.signer, ?grant, "ws authenticated");

    loop {
        tokio::select! {
            _ = client.cancel.cancelled() => {
                tracing::debug!(scene = %scene.name, index, "ws closed by the host");
                let _ = sink.close().await;
                return;
            }

            queued = rx.recv() => {
                let Some(frame) = queued else { break };
                if sink.send(Message::Binary(frame.into())).await.is_err() {
                    break;
                }
            }

            incoming = stream.next() => {
                let Some(Ok(msg)) = incoming else { break };
                match msg {
                    Message::Binary(bytes) => {
                        if let Some((MessageType::Crdt, body)) = decode_message(&bytes) {
                            if body.is_empty() { continue; }
                            let outbound = scene.runtime.on_client_crdt(index, body);
                            for out in outbound {
                                let frame = encode_message(MessageType::Crdt, &out);
                                scene.broadcast(&frame, index);
                            }
                        }
                    }
                    Message::Ping(p) => { let _ = sink.send(Message::Pong(p)).await; }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        }
    }

    scene.remove_client(index);
    tracing::debug!(scene = %scene.name, index, "ws closed");
}
