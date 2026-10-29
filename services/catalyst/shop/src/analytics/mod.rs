// from: shop@94584a5c:app/src/lib/analytics.ts (track/identify/trackPage, the context props, the no-PII rule, itemProps/purchaseItemsProps), app/src/lib/searchAnalytics.ts and app/src/lib/suggestionEvents.ts (the search and rail events), the `track('Shop …')` call sites under app/src (the event names below); catalyrst-v2 src/ops/telemetry/segment.rs (the ONE bounded writer every sink pushes through)
//! The Shop's telemetry owner: the analytics and search events the UI
//! emits (`ui3/src/shop/discovery/lib/searchAnalytics.ts`, the purchase
//! funnel) land in the ops telemetry sink under the `shop` project, through
//! the same bounded writer, admission and daily quota as the Sentry and
//! Segment sinks (`ops::telemetry`), and read back on the same console.
//!
//! Upstream wrote to Segment from the browser with a write key in the
//! bundle; here the Shop posts to its own node (`POST
//! /shop-api/v1/analytics/events`), which stamps what the browser cannot
//! prove: a signed fetch sets `address` from the verified signer
//! (`addressVerified: true`); an unsigned request keeps the address it
//! claims, marked unverified. Event names outside the Shop contract
//! ([`EVENTS`]: the upstream `Shop …` names and the four rail events) are
//! stored flagged (`invalid_reason`), never dropped silently and never
//! trusted as one of ours, the ops contract's rule. Properties are bounded
//! (a batch of at most [`MAX_EVENTS`], a body of at most [`MAX_BODY_BYTES`])
//! and scrubbed of the keys upstream forbade (no PII, no secrets, no env
//! values: e-mail, phone, names, IPs, tokens), so a redacted row is the
//! most a client can plant. `TELEMETRY_INGEST_ENABLED=false` answers the
//! sinks' 503; a node without the telemetry state answers the unavailable
//! envelope naming this sink.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Map, Value, json};

use super::market::signed::optional_signer;
use super::market::wire::ApiError;
use super::market::{Source, unavailable};
use crate::shared::telemetry::{Ingest, Row};

pub const PROJECT: &str = "shop";
pub const SOURCE: &str = "shop";
pub const MAX_EVENTS: usize = 50;
pub const MAX_BODY_BYTES: usize = 64 * 1024;
pub const MAX_PROPERTY_BYTES: usize = 16 * 1024;
pub const CONTRACT_VERSION: &str = "shop-94584a5c";
pub const EVENTS_PATH: &str = "/v1/analytics/events";

/// The Shop's event names, as upstream tracked them at the pinned revision
/// (`track('Shop …')` and the search/rail helpers).
pub const EVENTS: &[&str] = &[
    "Shop Abandoned Sale",
    "Shop Added To Cart",
    "Shop Added To Favorites",
    "Shop Applied Filter",
    "Shop Buy Credits Cancelled",
    "Shop Buy Credits Failed",
    "Shop Buy Credits Pending",
    "Shop Buy Credits Prompted",
    "Shop Changed Store Period",
    "Shop Cleared Search",
    "Shop Clicked Banner",
    "Shop Clicked Buy Resale",
    "Shop Clicked Item",
    "Shop Clicked Store Action",
    "Shop Completed Buy Credits",
    "Shop Completed Purchase",
    "Shop Created Sale",
    "Shop Ended Sale",
    "Shop Expanded Store Collection",
    "Shop Followed Creator",
    "Shop Issued Item",
    "Shop Issue Failed",
    "Shop Listed Item",
    "Shop Listing Failed",
    "Shop Make Offer Tooltip Shown",
    "Shop Migrated Listing",
    "Shop Outfit Added To Cart",
    "Shop Outfit Published",
    "Shop Outfit Viewed",
    "Shop Paged Store Table",
    "Shop Preview Renderer Fallback",
    "Shop Purchase Cancelled",
    "Shop Purchase Failed",
    "Shop Redirected To Marketplace",
    "Shop Redirected To Stripe",
    "Shop Removed From Cart",
    "Shop Removed From Favorites",
    "Shop Requested Notify",
    "Shop Reviewed Sale",
    "Shop Sale Failed",
    "Shop Searched",
    "Shop Search No Results",
    "Shop Search Suggestion Clicked",
    "Shop Signed In",
    "Shop Signed Out",
    "Shop Sorted Store Collections",
    "Shop Started Buy Credits",
    "Shop Started Checkout",
    "Shop Started Listing",
    "Shop Started Sale",
    "Shop Transfer Failed",
    "Shop Transferred Item",
    "Shop Tried On Outfit",
    "Shop Unfollowed Creator",
    "Shop Viewed Item",
    "Shop Viewed Page",
    "Shop Viewed Search Suggestions",
    "viewed_suggestions",
    "clicked_suggestion",
    "paged_suggestions",
    "hidden_suggestions",
];

/// Keys a property bag never carries (upstream: no PII, secrets or env values).
pub const FORBIDDEN_KEYS: &[&str] = &[
    "email",
    "e_mail",
    "phone",
    "phone_number",
    "first_name",
    "last_name",
    "full_name",
    "ip",
    "ip_address",
    "password",
    "token",
    "access_token",
    "secret",
    "authorization",
    "cookie",
    "private_key",
    "seed",
    "mnemonic",
];

pub fn is_shop_event(name: &str) -> bool {
    EVENTS.contains(&name)
}

/// The telemetry state this sink writes through; `None` on a node without
/// the ops plane (the route then answers the unavailable envelope).
#[derive(Clone, Default)]
pub struct Sink(pub Option<Arc<dyn Ingest>>);

impl<T: Ingest + 'static> From<Arc<T>> for Sink {
    fn from(state: Arc<T>) -> Self {
        Self(Some(state))
    }
}

/// Removes the forbidden keys at every depth and refuses a bag past
/// [`MAX_PROPERTY_BYTES`] (replaced by `{truncated: true}`).
pub fn scrub(value: &mut Value) {
    fn walk(v: &mut Value, depth: usize) {
        match v {
            Value::Object(map) => {
                map.retain(|k, _| !FORBIDDEN_KEYS.contains(&k.to_ascii_lowercase().as_str()));
                if depth < 8 {
                    for child in map.values_mut() {
                        walk(child, depth + 1);
                    }
                }
            }
            Value::Array(items) if depth < 8 => {
                for child in items.iter_mut() {
                    walk(child, depth + 1);
                }
            }
            _ => {}
        }
    }
    walk(value, 0);
    if serde_json::to_vec(&*value)
        .map(|b| b.len())
        .unwrap_or(usize::MAX)
        > MAX_PROPERTY_BYTES
    {
        *value = json!({ "truncated": true });
    }
}

/// One accepted event as a telemetry row: the Segment `track` shape the
/// console already reads, under the `shop` project.
pub fn row_for(event: &Value, verified: Option<&str>, received_at_ms: i64) -> Row {
    let name = event
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut properties = event
        .get("properties")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    scrub(&mut properties);
    let mut context = event
        .get("context")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    scrub(&mut context);
    let claimed = properties
        .get("address")
        .and_then(Value::as_str)
        .map(str::to_lowercase);
    let address = verified.map(str::to_lowercase).or(claimed);
    if let Some(map) = properties.as_object_mut() {
        match &address {
            Some(a) => {
                map.insert("address".into(), json!(a));
            }
            None => {
                map.remove("address");
            }
        }
        map.insert("address_verified".into(), json!(verified.is_some()));
        map.insert(
            "is_signed_in".into(),
            json!(
                verified.is_some()
                    || map
                        .get("is_signed_in")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
            ),
        );
    }
    let timestamp = event
        .get("timestamp")
        .and_then(Value::as_i64)
        .filter(|t| *t > 0 && *t <= received_at_ms + 60_000)
        .unwrap_or(received_at_ms);
    let mut body = Map::new();
    body.insert("type".into(), json!("track"));
    body.insert("event".into(), json!(name));
    body.insert("properties".into(), properties);
    body.insert("context".into(), context);
    body.insert("timestamp".into(), json!(timestamp));
    body.insert("receivedAt".into(), json!(received_at_ms));
    if let Some(anon) = event
        .get("anonymousId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 64)
    {
        body.insert("anonymousId".into(), json!(anon));
    }
    if let Some(session) = event
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 64)
    {
        body.insert("sessionId".into(), json!(session));
    }
    if let Some(a) = &address {
        body.insert("userId".into(), json!(a));
    }
    let invalid_reason = if name.is_empty() {
        Some("event name missing".to_string())
    } else if !is_shop_event(&name) {
        Some(format!("event not in the Shop contract {CONTRACT_VERSION}"))
    } else {
        None
    };
    Row {
        source: SOURCE,
        project: PROJECT.to_string(),
        kind: "track".to_string(),
        body: Value::Object(body),
        invalid_reason,
    }
}

fn events_of(payload: &Value) -> Vec<Value> {
    match payload.get("events") {
        Some(Value::Array(items)) => items.clone(),
        _ if payload.get("event").is_some() => vec![payload.clone()],
        _ => Vec::new(),
    }
}

/// `POST /v1/analytics/events` (signature optional): `{events: [{event,
/// properties?, timestamp?, anonymousId?, sessionId?, context?}]}` or one
/// event; `{ok, data: {accepted, flagged, dropped}}`.
pub async fn post_events(State(sink): State<Sink>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(telemetry) = sink.0.as_ref() else {
        return unavailable(
            Source::Deferred("shop-analytics"),
            "the ops telemetry sink is not running on this node, so Shop events have nowhere to land",
        );
    };
    if !telemetry.ingest_enabled() {
        return telemetry.ingest_disabled_response();
    }
    let verified = match optional_signer(&headers, "post", EVENTS_PATH).await {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    if body.len() > MAX_BODY_BYTES || !telemetry.admit_bytes(body.len()) {
        return ApiError::http(
            413,
            format!("an analytics batch is at most {MAX_BODY_BYTES} bytes"),
        )
        .into_response();
    }
    let payload: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return ApiError::bad_request(format!("invalid analytics body: {e}")).into_response();
        }
    };
    let mut events = events_of(&payload);
    if events.is_empty() {
        return ApiError::bad_request("events is required").into_response();
    }
    let dropped_over_max = events.len().saturating_sub(MAX_EVENTS);
    events.truncate(MAX_EVENTS);
    let admitted = telemetry.admit_n(PROJECT, events.len());
    let dropped_quota = events.len() - admitted;
    let now = chrono::Utc::now().timestamp_millis();
    let rows: Vec<Row> = events
        .iter()
        .take(admitted)
        .map(|e| row_for(e, verified.as_deref(), now))
        .collect();
    let flagged = rows.iter().filter(|r| r.invalid_reason.is_some()).count();
    let queued = telemetry.push_all(rows);
    let dropped_queue = admitted - queued;
    Json(json!({ "ok": true, "data": { "accepted": queued, "flagged": flagged.min(queued), "dropped": dropped_over_max + dropped_quota + dropped_queue } })).into_response()
}

/// `GET /v1/analytics/contract`: the event names this node accepts as the
/// Shop's, so the client seam can validate before it sends.
pub async fn get_contract() -> Response {
    ([(axum::http::header::CACHE_CONTROL, "public, max-age=3600")], Json(json!({ "ok": true, "data": { "version": CONTRACT_VERSION, "project": PROJECT, "events": EVENTS, "maxEvents": MAX_EVENTS, "maxBodyBytes": MAX_BODY_BYTES } }))).into_response()
}

pub fn router(sink: Sink) -> Router {
    Router::new()
        .route(
            EVENTS_PATH,
            post(post_events).layer(DefaultBodyLimit::max(MAX_BODY_BYTES + 1024)),
        )
        .route("/v1/analytics/contract", get(get_contract))
        .with_state(sink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[test]
    fn the_contract_is_the_upstream_event_list_and_nothing_twice() {
        let mut sorted: Vec<&str> = EVENTS.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), EVENTS.len(), "no duplicate event name");
        assert!(
            is_shop_event("Shop Completed Purchase")
                && is_shop_event("viewed_suggestions")
                && !is_shop_event("Shop Whatever")
        );
        assert_eq!(
            EVENTS.iter().filter(|e| e.starts_with("Shop ")).count(),
            57,
            "the 57 `Shop …` names tracked at shop@94584a5c"
        );
    }

    #[test]
    fn rows_are_scrubbed_stamped_and_flagged() {
        let now = 1_700_000_000_000;
        let event = json!({
            "event": "Shop Viewed Item", "timestamp": now - 5, "anonymousId": "anon-1", "sessionId": "s-1",
            "properties": { "item_id": "7", "address": "0xABC", "email": "who@example.com", "nested": { "Token": "x", "ok": 1 } },
            "context": { "ip": "1.2.3.4", "page": "/shop" }
        });
        let unsigned = row_for(&event, None, now);
        assert_eq!(
            (
                unsigned.source,
                unsigned.project.as_str(),
                unsigned.kind.as_str(),
                unsigned.invalid_reason.as_deref()
            ),
            ("shop", "shop", "track", None)
        );
        let p = &unsigned.body["properties"];
        assert_eq!(
            (
                p["address"].as_str(),
                p["address_verified"].as_bool(),
                p.get("email"),
                p["nested"].get("Token"),
                p["nested"]["ok"].as_i64()
            ),
            (Some("0xabc"), Some(false), None, None, Some(1))
        );
        assert!(
            unsigned.body["context"].get("ip").is_none()
                && unsigned.body["context"]["page"] == "/shop"
        );
        assert_eq!(
            (
                unsigned.body["timestamp"].as_i64(),
                unsigned.body["anonymousId"].as_str(),
                unsigned.body["userId"].as_str()
            ),
            (Some(now - 5), Some("anon-1"), Some("0xabc"))
        );
        let signed = row_for(&event, Some("0xDEF"), now);
        assert_eq!(
            (
                signed.body["properties"]["address"].as_str(),
                signed.body["properties"]["address_verified"].as_bool(),
                signed.body["properties"]["is_signed_in"].as_bool()
            ),
            (Some("0xdef"), Some(true), Some(true)),
            "the verified signer wins over the claim"
        );
        let future = row_for(
            &json!({ "event": "Shop Viewed Page", "timestamp": now + 3_600_000 }),
            None,
            now,
        );
        assert_eq!(
            future.body["timestamp"].as_i64(),
            Some(now),
            "a timestamp from the future is the arrival time"
        );
        let unknown = row_for(&json!({ "event": "Shop Whatever" }), None, now);
        assert!(
            unknown
                .invalid_reason
                .as_deref()
                .unwrap()
                .starts_with("event not in the Shop contract")
        );
        assert_eq!(
            row_for(&json!({ "properties": {} }), None, now)
                .invalid_reason
                .as_deref(),
            Some("event name missing")
        );
        let mut huge = json!({ "blob": "x".repeat(MAX_PROPERTY_BYTES + 1) });
        scrub(&mut huge);
        assert_eq!(huge, json!({ "truncated": true }));
    }

    #[tokio::test]
    async fn without_the_ops_plane_the_route_answers_the_envelope_and_the_contract_is_public() {
        let app = router(Sink::default());
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(EVENTS_PATH)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"events":[{"event":"Shop Viewed Page"}]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(res.into_body(), 1 << 16)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["source"], "shop-analytics");
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/v1/analytics/contract")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(res.into_body(), 1 << 16)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["data"]["project"], "shop");
        assert_eq!(
            body["data"]["events"].as_array().unwrap().len(),
            EVENTS.len()
        );
    }

    #[test]
    fn a_batch_or_one_event_are_both_read() {
        assert_eq!(
            events_of(&json!({ "events": [{ "event": "a" }, { "event": "b" }] })).len(),
            2
        );
        assert_eq!(events_of(&json!({ "event": "a" })).len(), 1);
        assert!(events_of(&json!({ "nope": 1 })).is_empty());
    }
}
