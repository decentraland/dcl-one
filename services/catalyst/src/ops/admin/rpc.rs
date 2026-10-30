//! `/admin/api/rpc/*`: the EVM relay's method allowlist and network map
//! (`ops::rpc::admin`, bearer `CATALYRST_RPC_ADMIN_TOKEN`), each the relay's
//! own handler with the bearer this node configured. Process state the
//! relay audits itself (`domain` `rpc`); not gated by `OPS_WRITES_ENABLED`,
//! as the relay's own routes are not. An upstream URL may carry a gateway key in its path,
//! so the console's row keeps its host only, the relay's rule.

#![allow(clippy::result_large_err)]

use axum::Json;
use axum::extract::{Path, State};
use axum::response::Response;
use serde_json::{Value, json};

use super::SharedState;
use super::auth::AdminSession;
use super::console::{
    Call, body_as, configured, finish, not_configured, segment_field, service_headers, str_field,
    token_missing,
};
use crate::ops::rpc::admin as radmin;

const DOMAIN: &str = "rpc";

fn relay_for(
    state: &SharedState,
    session: &AdminSession,
) -> Result<(crate::ops::rpc::Shared, axum::http::HeaderMap), Response> {
    let Some(rs) = state.domains.rpc.clone() else {
        return Err(not_configured(DOMAIN));
    };
    let Some(token) = configured(rs.settings.admin_token.as_deref()) else {
        return Err(token_missing(DOMAIN));
    };
    let headers = service_headers(Some(token), session.address());
    Ok((rs, headers))
}

macro_rules! relay_read {
    ($name:ident, $action:literal, $handler:path) => {
        pub async fn $name(session: AdminSession, State(state): State<SharedState>) -> Response {
            let (rs, headers) = match relay_for(&state, &session) {
                Ok(v) => v,
                Err(resp) => return resp,
            };
            let answer = $handler(State(rs), headers).await;
            finish(&state, Call::new(&session, DOMAIN, $action), answer).await
        }
    };
}

relay_read!(config, "rpc.config", radmin::get_config);
relay_read!(methods_list, "rpc.methods.list", radmin::list_methods);
relay_read!(methods_reset, "rpc.methods.reset", radmin::reset_methods);
relay_read!(networks_list, "rpc.networks.list", radmin::list_networks);

fn method_body(body: &Value) -> Result<(String, radmin::MethodBody), Response> {
    let method = str_field(body, "method").unwrap_or_default();
    Ok((
        method.clone(),
        body_as::<radmin::MethodBody>(json!({ "method": method }))?,
    ))
}

/// `POST /admin/api/rpc/methods-add` `{method}`.
pub async fn methods_add(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (method, typed) = match method_body(&body) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let (rs, headers) = match relay_for(&state, &session) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = radmin::add_method(State(rs), headers, Json(typed)).await;
    finish(
        &state,
        Call::new(&session, DOMAIN, "rpc.methods.add")
            .target(method.clone())
            .detail(json!({ "method": method })),
        answer,
    )
    .await
}

/// `POST /admin/api/rpc/methods-remove` `{method}`.
pub async fn methods_remove(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (method, typed) = match method_body(&body) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let (rs, headers) = match relay_for(&state, &session) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = radmin::remove_method(State(rs), headers, Json(typed)).await;
    finish(
        &state,
        Call::new(&session, DOMAIN, "rpc.methods.remove")
            .target(method.clone())
            .detail(json!({ "method": method })),
        answer,
    )
    .await
}

/// `POST /admin/api/rpc/networks-set` `{network, url}`.
pub async fn networks_set(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let network = str_field(&body, "network").unwrap_or_default();
    let url = str_field(&body, "url").unwrap_or_default();
    let typed = match body_as::<radmin::NetworkBody>(json!({ "network": network, "url": url })) {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let (rs, headers) = match relay_for(&state, &session) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = radmin::upsert_network(State(rs), headers, Json(typed)).await;
    let detail = json!({ "network": network, "host": radmin::host_of(url.trim()) });
    finish(
        &state,
        Call::new(&session, DOMAIN, "rpc.networks.set")
            .target(network.trim().to_ascii_lowercase())
            .detail(detail),
        answer,
    )
    .await
}

/// `POST /admin/api/rpc/networks-delete` `{network}`.
pub async fn networks_delete(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let network = match segment_field(&body, "network", "network") {
        Ok(n) => n.to_lowercase(),
        Err(resp) => return resp,
    };
    let (rs, headers) = match relay_for(&state, &session) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = radmin::remove_network(State(rs), headers, Path(network.clone())).await;
    finish(
        &state,
        Call::new(&session, DOMAIN, "rpc.networks.delete")
            .target(network.clone())
            .detail(json!({ "network": network })),
        answer,
    )
    .await
}
