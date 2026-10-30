//! `/admin/api/{camera-reel,builder,create}/*`.
//!
//! The camera reel's image delete and review and the builder's curation
//! status are the modules' own handlers with the bearer the node configured
//! (`CATALYRST_CAMERA_REEL_ADMIN_TOKEN`, `CATALYRST_BUILDER_ADMIN_TOKEN`),
//! behind `CREATE_WRITES_ENABLED` through each module's gate predicate.
//!
//! The asset-bundle registry's controls (`registry-reingest`,
//! `flush-ab-cache`, `queues-*`, `denylist-*`) belong to abgen, a prebuilt
//! external runtime with no source in this tree that the deployment runs
//! beside the node (the old `catalyrst-ab-cdn`). They stay HTTP
//! calls, as the old console made them, to [`Abgen`]'s base: a loopback URL
//! only (`ABGEN_ADMIN_URL`, refused at startup otherwise, so the registry
//! bearer `AB_REGISTRY_ADMIN_TOKEN` can never leave the host), through the
//! ONE outbound client's pooled connection (`http::Outbound::raw`, the
//! guard exempts nothing else: the target is fixed at startup, never taken
//! from a request). The mutating ones are registry writes behind
//! `OPS_WRITES_ENABLED`; `queues-status` is a read.

#![allow(clippy::result_large_err)]

use std::time::Duration;

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::SharedState;
use super::api::{AdminEnvelope, ops_refused};
use super::auth::AdminSession;
use super::console::{
    Call, body_as, configured, finish, not_configured, refused, segment_field, service_headers,
    str_field, token_missing,
};

async fn reel_for(
    state: &SharedState,
    session: &AdminSession,
    method: Method,
    path: &str,
) -> Result<(crate::camera_reel::SharedState, axum::http::HeaderMap), Response> {
    let Some(cr) = state.domains.camera_reel.clone() else {
        return Err(not_configured("camera-reel"));
    };
    if crate::camera_reel::writes::is_gated(&method, path)
        && let Some(r) = crate::shared::writes::refusal(cr.settings.writes_enabled)
    {
        return Err(refused(crate::camera_reel::writes::refused_response(&r)).await);
    }
    let Some(token) = configured(cr.settings.admin_token.as_deref()) else {
        return Err(token_missing("camera-reel"));
    };
    Ok((cr.clone(), service_headers(Some(token), session.address())))
}

/// `POST /admin/api/camera-reel/image-delete` `{image_id}`.
pub async fn reel_image_delete(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let image_id = match segment_field(&body, "image_id", "image-id") {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let (cr, headers) = match reel_for(
        &state,
        &session,
        Method::DELETE,
        &format!("/admin/images/{image_id}"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let answer = crate::camera_reel::handlers::images::admin_delete_image(
        State(cr),
        headers,
        Path(image_id.clone()),
    )
    .await
    .into_response();
    finish(
        &state,
        Call::new(&session, "create", "camera-reel.image.delete")
            .target(image_id.clone())
            .detail(json!({ "image_id": image_id })),
        answer,
    )
    .await
}

/// The old form's `action` (approve/reject/flag) for the route's `review_status`.
fn review_status(body: &Value) -> Option<String> {
    str_field(body, "reviewStatus")
        .or_else(|| str_field(body, "review_status"))
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            str_field(body, "action").map(|a| match a.trim() {
                "approve" | "approved" => "ok".to_string(),
                "reject" => "rejected".to_string(),
                "flag" => "flagged".to_string(),
                other => other.to_string(),
            })
        })
}

/// `POST /admin/api/camera-reel/image-review` `{image_id, reviewStatus}`
/// (`ok`, `flagged` or `rejected`; `review_status` and the old form's
/// `action` are accepted).
pub async fn reel_image_review(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let image_id = match segment_field(&body, "image_id", "image-id") {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let (cr, headers) = match reel_for(
        &state,
        &session,
        Method::PATCH,
        &format!("/admin/images/{image_id}/review"),
    )
    .await
    {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let review = json!({ "reviewStatus": review_status(&body).unwrap_or_default() });
    let typed = match body_as::<crate::camera_reel::dto::UpdateReview>(review.clone()) {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let answer = crate::camera_reel::handlers::images::admin_update_image_review(
        State(cr),
        headers,
        Path(image_id.clone()),
        Json(typed),
    )
    .await
    .into_response();
    finish(
        &state,
        Call::new(&session, "create", "camera-reel.image.review")
            .target(image_id)
            .detail(review),
        answer,
    )
    .await
}

async fn builder_for(
    state: &SharedState,
    session: &AdminSession,
) -> Result<(crate::builder::SharedState, axum::http::HeaderMap), Response> {
    let Some(bs) = state.domains.builder.clone() else {
        return Err(not_configured("builder"));
    };
    if let Some(r) = crate::shared::writes::refusal(bs.settings.writes_enabled) {
        return Err(refused(crate::builder::writes::refused_response(&r)).await);
    }
    let Some(token) = configured(bs.settings.admin_token.as_deref()) else {
        return Err(token_missing("builder"));
    };
    Ok((bs.clone(), service_headers(Some(token), session.address())))
}

fn uri(path: &str) -> Uri {
    path.parse().unwrap_or_else(|_| Uri::from_static("/"))
}

/// `POST /admin/api/builder/item-status` `{collection_id, item_id, status}`.
pub async fn builder_item_status(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let (collection_id, item_id) = match (
        segment_field(&body, "collection_id", "collection-id"),
        segment_field(&body, "item_id", "item-id"),
    ) {
        (Ok(c), Ok(i)) => (c, i),
        (Err(resp), _) | (_, Err(resp)) => return resp,
    };
    let (bs, headers) = match builder_for(&state, &session).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let status = json!({ "status": str_field(&body, "status").unwrap_or_default() });
    let typed = match body_as::<crate::builder::handlers::curation::ItemStatusBody>(status.clone())
    {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let path = uri(&format!(
        "/v1/collections/{collection_id}/items/{item_id}/status"
    ));
    let answer = crate::builder::handlers::curation::patch_item_status(
        State(bs),
        Path((collection_id, item_id.clone())),
        OriginalUri(path.clone()),
        path,
        headers,
        Json(typed),
    )
    .await
    .into_response();
    finish(
        &state,
        Call::new(&session, "create", "builder.item.status")
            .target(item_id)
            .detail(status),
        answer,
    )
    .await
}

/// `POST /admin/api/builder/items-status` `{collection_id, status, itemIds}`
/// (`itemIds` an array, or `item_ids` a comma-separated string).
pub async fn builder_items_status(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let collection_id = match segment_field(&body, "collection_id", "collection-id") {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    let (bs, headers) = match builder_for(&state, &session).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let item_ids: Vec<String> = match (body.get("itemIds"), str_field(&body, "item_ids")) {
        (Some(Value::Array(ids)), _) => ids
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        (_, Some(csv)) => csv
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    };
    let request =
        json!({ "status": str_field(&body, "status").unwrap_or_default(), "itemIds": item_ids });
    let typed =
        match body_as::<crate::builder::handlers::curation::BulkItemStatusBody>(request.clone()) {
            Ok(b) => b,
            Err(resp) => return resp,
        };
    let path = uri(&format!("/v1/collections/{collection_id}/items/status"));
    let answer = crate::builder::handlers::curation::patch_items_status_bulk(
        State(bs),
        Path(collection_id.clone()),
        OriginalUri(path.clone()),
        path,
        headers,
        Json(typed),
    )
    .await
    .into_response();
    finish(
        &state,
        Call::new(&session, "create", "builder.items.status")
            .target(collection_id)
            .detail(request),
        answer,
    )
    .await
}

/// The abgen runtime's admin endpoint: a loopback base URL, its bearer and
/// the ONE client's pooled connection.
#[derive(Clone)]
pub struct Abgen {
    base: String,
    token: Option<String>,
    client: reqwest::Client,
}

impl std::fmt::Debug for Abgen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Abgen")
            .field("base", &self.base)
            .field("token", &self.token.as_ref().map(|_| "<set>"))
            .finish()
    }
}

/// Every abgen admin call is bounded; the old console's client used 2 s,
/// the registry re-ingest answers after it queued the work.
const ABGEN_TIMEOUT: Duration = Duration::from_secs(10);

impl Abgen {
    /// `ABGEN_ADMIN_URL` must be `http(s)://` to a loopback address
    /// (`127.0.0.0/8`, `::1`, `localhost`): the runtime is the deployment's,
    /// on this host. The error names the variable, never the value.
    pub fn parse(
        url: &str,
        token: Option<String>,
        http: &crate::http::Outbound,
    ) -> anyhow::Result<Self> {
        let trimmed = url.trim().trim_end_matches('/');
        let parsed = url::Url::parse(trimmed)
            .map_err(|_| anyhow::anyhow!("ABGEN_ADMIN_URL is not a URL"))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            anyhow::bail!("ABGEN_ADMIN_URL must be http(s)");
        }
        let loopback = match parsed.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
            None => false,
        };
        if !loopback {
            anyhow::bail!(
                "ABGEN_ADMIN_URL must name a loopback address: abgen runs on this host and the registry bearer never leaves it"
            );
        }
        Ok(Self {
            base: trimmed.to_string(),
            token: token
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty()),
            client: http.raw().clone(),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// One call, the old `proxy_envelope`: any HTTP answer is the
    /// downstream's (status, body, `Retry-After`); a transport failure is
    /// `Err` with the error text, which carries no URL.
    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        operator: &str,
    ) -> Result<Response, String> {
        let mut req = self
            .client
            .request(method, format!("{}{path}", self.base))
            .timeout(ABGEN_TIMEOUT);
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        if super::auth::valid_eth_address(operator) {
            req = req.header("x-catalyrst-admin", operator);
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req.send().await.map_err(|e| e.without_url().to_string())?;
        let status =
            StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let retry = resp.headers().get(header::RETRY_AFTER).cloned();
        let bytes = crate::http::read_body_capped(resp, super::console::MAX_ANSWER_BYTES)
            .await
            .map_err(|e| e.to_string())?;
        let mut out = (status, bytes).into_response();
        if let Some(r) = retry {
            out.headers_mut().insert(header::RETRY_AFTER, r);
        }
        Ok(out)
    }
}

/// One `/admin/api/create/*` call: the registry-writes gate, the runtime
/// and (for the queue and denylist routes) its bearer, then the call. A
/// transport failure is the old `502 {ok: false, status: 0, body: {error:
/// request-failed}}`, audited as an error with status 0.
#[allow(clippy::too_many_arguments)]
async fn abgen_call(
    state: &SharedState,
    session: &AdminSession,
    call: Call<'_>,
    write: bool,
    token_required: bool,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Response {
    let Some(abgen) = state.domains.abgen.clone() else {
        return not_configured("abgen");
    };
    if write && let Some(refused) = ops_refused(state.settings.writes_enabled) {
        return refused;
    }
    if token_required && abgen.token.is_none() {
        return token_missing("ab-registry");
    }
    match abgen.call(method, path, body, session.address()).await {
        Ok(answer) => finish(state, call, answer).await,
        Err(detail) => {
            let mut audit_detail = call.detail;
            if let Some(obj) = audit_detail.as_object_mut() {
                obj.insert("status".into(), json!(0));
            }
            let entry = super::audit::Entry::new(call.actor, call.domain, call.action)
                .target(call.target.as_deref())
                .detail(audit_detail)
                .result("error");
            super::audit::record_after(&state.pool, &entry).await;
            (
                StatusCode::BAD_GATEWAY,
                Json(AdminEnvelope {
                    ok: false,
                    status: 0,
                    body: json!({ "error": "request-failed", "detail": detail }),
                }),
            )
                .into_response()
        }
    }
}

const DOMAIN: &str = "registry";

/// `POST /admin/api/create/registry-reingest` (body forwarded).
pub async fn registry_reingest(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let call = Call::new(&session, DOMAIN, "create.registry-reingest").detail(body.clone());
    abgen_call(
        &state,
        &session,
        call,
        true,
        false,
        Method::POST,
        "/registry",
        Some(body),
    )
    .await
}

/// `POST /admin/api/create/flush-ab-cache`.
pub async fn flush_ab_cache(session: AdminSession, State(state): State<SharedState>) -> Response {
    let call = Call::new(&session, DOMAIN, "create.flush-ab-cache");
    abgen_call(
        &state,
        &session,
        call,
        true,
        false,
        Method::DELETE,
        "/flush-cache",
        None,
    )
    .await
}

/// `POST /admin/api/create/queues-retry` (body forwarded).
pub async fn queues_retry(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let call = Call::new(&session, DOMAIN, "create.queues.retry").detail(body.clone());
    abgen_call(
        &state,
        &session,
        call,
        true,
        true,
        Method::POST,
        "/queues/retry",
        Some(body),
    )
    .await
}

/// `POST /admin/api/create/queues-pause`.
pub async fn queues_pause(session: AdminSession, State(state): State<SharedState>) -> Response {
    let call = Call::new(&session, DOMAIN, "create.queues.pause");
    abgen_call(
        &state,
        &session,
        call,
        true,
        true,
        Method::POST,
        "/queues/pause",
        None,
    )
    .await
}

/// `POST /admin/api/create/queues-resume`.
pub async fn queues_resume(session: AdminSession, State(state): State<SharedState>) -> Response {
    let call = Call::new(&session, DOMAIN, "create.queues.resume");
    abgen_call(
        &state,
        &session,
        call,
        true,
        true,
        Method::POST,
        "/queues/resume",
        None,
    )
    .await
}

/// `POST /admin/api/create/queues-status`: a read.
pub async fn queues_status(session: AdminSession, State(state): State<SharedState>) -> Response {
    let call = Call::new(&session, DOMAIN, "create.queues.status");
    abgen_call(
        &state,
        &session,
        call,
        false,
        true,
        Method::GET,
        "/queues/status",
        None,
    )
    .await
}

fn entity_id(body: &Value) -> Result<String, Response> {
    let raw = str_field(body, "entity_id")
        .or_else(|| str_field(body, "entityId"))
        .unwrap_or_default();
    if super::console::valid_path_segment(&raw) {
        Ok(raw)
    } else {
        Err(super::console::bad_segment("entity-id"))
    }
}

/// `POST /admin/api/create/denylist-add` `{entity_id, ...}` (the rest forwarded).
pub async fn denylist_add(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let id = match entity_id(&body) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let rest = super::console::body_without(&body, &["entity_id", "entityId"])
        .unwrap_or_else(|| json!({}));
    let call = Call::new(&session, DOMAIN, "create.denylist.add")
        .target(id.clone())
        .detail(rest.clone());
    abgen_call(
        &state,
        &session,
        call,
        true,
        true,
        Method::POST,
        &format!("/denylist/{id}"),
        Some(rest),
    )
    .await
}

/// `POST /admin/api/create/denylist-remove` `{entity_id}`.
pub async fn denylist_remove(
    session: AdminSession,
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Response {
    let id = match entity_id(&body) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let call = Call::new(&session, DOMAIN, "create.denylist.remove")
        .target(id.clone())
        .detail(json!({ "entityId": id }));
    abgen_call(
        &state,
        &session,
        call,
        true,
        true,
        Method::DELETE,
        &format!("/denylist/{id}"),
        None,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abgen_takes_a_loopback_url_only() {
        let http = crate::http::Outbound::for_tests();
        for ok in [
            "http://127.0.0.1:5147",
            "http://localhost:5147/",
            "http://[::1]:5147",
            "https://127.0.0.2",
        ] {
            assert!(Abgen::parse(ok, None, &http).is_ok(), "{ok}");
        }
        for bad in [
            "http://10.0.0.1:5147",
            "https://abgen.example.org",
            "ftp://127.0.0.1",
            "not a url",
            "http://169.254.169.254",
        ] {
            let err = Abgen::parse(bad, Some("secret".into()), &http)
                .unwrap_err()
                .to_string();
            assert!(
                !err.contains(bad) || bad == "not a url",
                "the value is not echoed: {err}"
            );
        }
        let a = Abgen::parse(
            "http://127.0.0.1:5147/",
            Some(" s3cr3t-bearer ".into()),
            &http,
        )
        .unwrap();
        assert_eq!(a.base(), "http://127.0.0.1:5147");
        assert!(
            !format!("{a:?}").contains("s3cr3t"),
            "Debug never prints the bearer"
        );
    }
}
