//! The Transak card on-ramp, as marketplace-server serves it (`transak-handler`
//! and `ports/transak` at 9c3683cb):
//! - **`POST /v1/transak/widget-url`** (unsigned): the caller's widget options,
//!   validated against upstream's schema, plus the server's own (the API key,
//!   `TRANSAK_REFERRER_DOMAIN`, both networks, MANA), sent last so they win.
//!   Answers `{ok:true, data: <widget URL>}`.
//! - **`GET /v1/transak/orders/{id}`** (signed): the order, if its wallet is the
//!   signer.
//! - **`PUT /v1/transak/access-token`** (`TRANSAK_REFRESH_ACCESS_TOKEN_AUTH`
//!   bearer): forces a token refresh.
//!
//! **The access token.** Transak invalidates its previous partner token when
//! it issues a new one ("When you call the endpoint again, the previously
//! issued token is automatically invalidated", Transak's widget URL migration
//! guide). Upstream therefore kept one token in Redis for every instance. Here
//! it is one row, `shop.transak_access_token`, that every replica reads:
//! - a refresh runs only under the `transak-access-token` advisory lock;
//! - a call Transak refuses with 401 refreshes only while the stored token is
//!   still the refused one, so replicas never invalidate each other's fresh
//!   token. The call is then sent once more.
//!
//! The token is never logged.
//!
//! **Differences from upstream:**
//! - The order read answers 401 to an unsigned caller *before* calling
//!   Transak; upstream reads the order first.
//! - The order read returns only the fields its clients read: `meta.orderId`
//!   and `data` with `id`, `status`, `transactionHash`, `walletAddress`,
//!   `errorMessage` and `isNFTOrder`. That's the union of decentraland-dapps'
//!   gateway saga (`status`, `transactionHash`, `errorMessage`), the Shop UI's
//!   `TransakOrderStatus`, and upstream's declared `OrderResponse`. Upstream
//!   passes Transak's whole order, with its payment and personal fields.
//! - An order id that is not `[A-Za-z0-9-]{1,64}` is 400. A Transak 404 is 404,
//!   and any other failure 502. Upstream answers 500.
//! - `x-user-ip` is the client address the server pinned (`client_ip`), never a
//!   caller-supplied `cf-connecting-ip`.
//! - An invalid widget body is 400 "Invalid JSON body" without upstream's
//!   validator error list.
//!
//! Without the six `TRANSAK_*` settings, every route answers the unavailable
//! envelope naming them.

use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::signed::optional_signer;
use super::wire::ApiError;
use super::{SharedState, Source, check_upstream, unavailable};
use crate::auth::admin_bearer::timing_safe_eq;
use crate::shared::client_ip::client_ip;

pub(super) const WIDGET_PATH: &str = "/v1/transak/widget-url";
pub(super) const ORDER_PATH: &str = "/v1/transak/orders/{id}";
pub(super) const TOKEN_PATH: &str = "/v1/transak/access-token";

/// How long each Transak call may take.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// The variables, in the order the startup error names a missing one.
const VARIABLES: [&str; 6] = [
    "TRANSAK_API_URL",
    "TRANSAK_API_GATEWAY_URL",
    "TRANSAK_API_KEY",
    "TRANSAK_API_SECRET",
    "TRANSAK_REFRESH_ACCESS_TOKEN_AUTH",
    "TRANSAK_REFERRER_DOMAIN",
];

/// The partner credentials and endpoints. No derived `Debug`: it would print
/// the key, the secret and the bearer.
#[derive(Clone)]
pub struct TransakSettings {
    api_url: String,
    gateway_url: String,
    api_key: String,
    api_secret: String,
    refresh_auth: String,
    referrer_domain: String,
}

impl std::fmt::Debug for TransakSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransakSettings")
            .field("api_url", &self.api_url)
            .field("gateway_url", &self.gateway_url)
            .field("referrer_domain", &self.referrer_domain)
            .finish_non_exhaustive()
    }
}

impl TransakSettings {
    /// All six settings, or none (`None`: the routes are unavailable). A
    /// partial set, a URL that is not a public https endpoint, or a referrer
    /// that is not a bare host name fails startup naming the variable.
    pub fn parse(
        api_url: Option<&str>,
        gateway_url: Option<&str>,
        api_key: Option<&str>,
        api_secret: Option<&str>,
        refresh_auth: Option<&str>,
        referrer_domain: Option<&str>,
        self_hosts: &[String],
    ) -> anyhow::Result<Option<Self>> {
        let values = [
            api_url,
            gateway_url,
            api_key,
            api_secret,
            refresh_auth,
            referrer_domain,
        ]
        .map(|v| v.map(str::trim).filter(|v| !v.is_empty()));
        let missing: Vec<&str> = VARIABLES
            .iter()
            .zip(&values)
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| *name)
            .collect();
        if missing.len() == VARIABLES.len() {
            return Ok(None);
        }
        let [
            Some(api_url),
            Some(gateway_url),
            Some(api_key),
            Some(api_secret),
            Some(refresh_auth),
            Some(referrer_domain),
        ] = values
        else {
            anyhow::bail!(
                "the Transak on-ramp needs all of {}; missing: {}",
                VARIABLES.join(", "),
                missing.join(", ")
            );
        };
        let mut urls = Vec::new();
        for (name, url) in [
            ("TRANSAK_API_URL", api_url),
            ("TRANSAK_API_GATEWAY_URL", gateway_url),
        ] {
            let url = url.trim_end_matches('/');
            check_upstream(name, url, self_hosts)?;
            if !url.starts_with("https://") {
                anyhow::bail!(
                    "{name} is not https: the partner credentials are never sent in plaintext"
                );
            }
            urls.push(url.to_string());
        }
        let referrer_domain = referrer_domain.to_ascii_lowercase();
        if !is_host_name(&referrer_domain) {
            anyhow::bail!(
                "TRANSAK_REFERRER_DOMAIN must be the bare host Transak registered (for example shop.example.com), with no scheme, port or path"
            );
        }
        let [api_url, gateway_url] = [urls.remove(0), urls.remove(0)];
        Ok(Some(Self {
            api_url,
            gateway_url,
            api_key: api_key.to_string(),
            api_secret: api_secret.to_string(),
            refresh_auth: refresh_auth.to_string(),
            referrer_domain,
        }))
    }

    /// Obviously fake credentials, with the API and gateway at one test server.
    #[cfg(test)]
    pub(crate) fn for_tests(url: &str) -> Self {
        Self {
            api_url: url.to_string(),
            gateway_url: format!("{url}/gateway"),
            api_key: "transak-api-key-example".to_string(),
            api_secret: "transak-api-secret-example".to_string(),
            refresh_auth: "transak-refresh-bearer-example-0123456789".to_string(),
            referrer_domain: "shop.example.com".to_string(),
        }
    }
}

/// A DNS host name: labels of letters, digits and inner hyphens, at least two.
fn is_host_name(host: &str) -> bool {
    host.len() <= 253
        && host.split('.').count() >= 2
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

/// A Transak order id as it goes into Transak's URL path.
fn is_order_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Upstream's schema-validator test: `application/json` or a `+json` type.
fn is_json(headers: &HeaderMap) -> bool {
    let Some(value) = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let media = value
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    media == "application/json"
        || media.split_once('/').is_some_and(|(kind, sub)| {
            !kind.is_empty()
                && !kind.contains(char::is_whitespace)
                && sub.len() > 5
                && sub.ends_with("+json")
        })
}

/// What a failed step tells the caller and the log: the step and a status,
/// never a token or a reply.
type Failure = String;

fn store_failed(_: sqlx::Error) -> Failure {
    "the access token store failed".to_string()
}

fn sent(error: crate::http::SendError) -> Failure {
    if error.is_timeout() {
        "Transak did not answer in time".to_string()
    } else {
        "Transak could not be reached".to_string()
    }
}

/// How the shared token is wanted.
#[derive(Clone, Copy)]
enum Want<'a> {
    /// The stored token while it is fresh.
    Fresh,
    /// Transak refused this token: a new one, unless another replica already
    /// replaced it.
    Replacing(&'a str),
    /// A new token whatever is stored (the operator's `PUT`).
    New,
}

/// The shared access token (see the module doc).
async fn token(
    state: &SharedState,
    transak: &TransakSettings,
    want: Want<'_>,
) -> Result<String, Failure> {
    if let Want::Fresh = want {
        let stored: Option<String> = sqlx::query_scalar(
            "SELECT access_token FROM shop.transak_access_token \
             WHERE expires_at > timezone('UTC', now()) + interval '60 seconds'",
        )
        .fetch_optional(&state.pool)
        .await
        .map_err(store_failed)?;
        if let Some(token) = stored {
            return Ok(token);
        }
    }
    let mut tx = state.pool.begin().await.map_err(store_failed)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('transak-access-token', 0))")
        .execute(&mut *tx)
        .await
        .map_err(store_failed)?;
    let current: Option<(String, bool)> = sqlx::query_as(
        "SELECT access_token, expires_at > timezone('UTC', now()) + interval '60 seconds' \
         FROM shop.transak_access_token",
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(store_failed)?;
    if let Some((stored, fresh)) = current {
        let keep = match want {
            Want::Fresh => fresh,
            Want::Replacing(refused) => fresh && stored != refused,
            Want::New => false,
        };
        if keep {
            return Ok(stored);
        }
    }
    let (token, expires_at) = refresh(state, transak).await?;
    sqlx::query(
        "INSERT INTO shop.transak_access_token (singleton, access_token, expires_at, refreshed_at) \
         VALUES (true, $1, $2, timezone('UTC', now())) \
         ON CONFLICT (singleton) DO UPDATE SET access_token = EXCLUDED.access_token, \
             expires_at = EXCLUDED.expires_at, refreshed_at = EXCLUDED.refreshed_at",
    )
    .bind(&token)
    .bind(expires_at)
    .execute(&mut *tx)
    .await
    .map_err(store_failed)?;
    tx.commit().await.map_err(store_failed)?;
    tracing::info!(%expires_at, "refreshed the shared Transak access token");
    Ok(token)
}

/// `POST {TRANSAK_API_URL}/v2/refresh-token`, as upstream's `getAccessToken`.
async fn refresh(
    state: &SharedState,
    transak: &TransakSettings,
) -> Result<(String, chrono::NaiveDateTime), Failure> {
    let response = state
        .http
        .post(format!("{}/v2/refresh-token", transak.api_url))
        .max_redirects(0)
        .header("api-secret", transak.api_secret.as_str())
        .header("x-api-key", transak.api_key.as_str())
        .header("accept", "application/json")
        .json(&json!({ "apiKey": transak.api_key }))
        .timeout(CALL_TIMEOUT)
        .send()
        .await
        .map_err(sent)?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "Error refreshing access token, status: {}",
            status.as_u16()
        ));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| "Transak's token reply is not JSON".to_string())?;
    let token = body
        .pointer("/data/accessToken")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty());
    let expires_at = body
        .pointer("/data/expiresAt")
        .and_then(Value::as_i64)
        .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0));
    match (token, expires_at) {
        (Some(token), Some(expires_at)) => Ok((token.to_string(), expires_at.naive_utc())),
        _ => Err("Transak's token reply has no accessToken or expiresAt".to_string()),
    }
}

/// Sends `request(token)` with the shared token. A 401 replaces the token
/// (unless another replica already did) and sends once more.
async fn send_authorized<F>(
    state: &SharedState,
    transak: &TransakSettings,
    request: F,
) -> Result<reqwest::Response, Failure>
where
    F: Fn(&str) -> crate::http::GuardedRequest,
{
    let first = token(state, transak, Want::Fresh).await?;
    let response = request(&first).send().await.map_err(sent)?;
    if response.status() != reqwest::StatusCode::UNAUTHORIZED {
        return Ok(response);
    }
    let second = token(state, transak, Want::Replacing(&first)).await?;
    request(&second).send().await.map_err(sent)
}

/// The partner headers of a call, with the client's address when there is one.
fn partner_request(
    request: crate::http::GuardedRequest,
    transak: &TransakSettings,
    token: &str,
    user_ip: &str,
) -> crate::http::GuardedRequest {
    let request = request
        .max_redirects(0)
        .header("access-token", token)
        .header("x-api-key", transak.api_key.as_str())
        .header("accept", "application/json")
        .timeout(CALL_TIMEOUT);
    if user_ip.is_empty() {
        request
    } else {
        request.header("x-user-ip", user_ip)
    }
}

fn not_configured() -> Response {
    unavailable(
        Source::Transak,
        "Transak card purchases are not configured on this node",
    )
}

fn failed(status: u16, message: impl Into<String>) -> Response {
    ApiError::http(status, message).into_response()
}

/// `GET /v1/transak/orders/{id}`.
pub async fn order(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(transak) = state.settings.market.transak.as_ref() else {
        return not_configured();
    };
    let path = format!("/v1/transak/orders/{id}");
    let signer = match optional_signer(state.eip1654.as_deref(), &headers, "get", &path).await {
        Ok(Some(signer)) => signer,
        Ok(None) => return ApiError::unauthorized("Unauthorized").into_response(),
        Err(refusal) => return refusal.into_response(),
    };
    if !is_order_id(&id) {
        return ApiError::bad_request("Invalid order id").into_response();
    }
    let user_ip = client_ip(&headers);
    let url = format!("{}/v2/order/{id}", transak.api_url);
    let response = match send_authorized(&state, transak, |token| {
        partner_request(state.http.get(url.as_str()), transak, token, &user_ip)
    })
    .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, "a Transak order read failed");
            return failed(502, "Transak order could not be read");
        }
    };
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return failed(404, "Order not found");
    }
    if !status.is_success() {
        tracing::warn!(status = status.as_u16(), "Transak refused an order read");
        return failed(502, "Transak order could not be read");
    }
    let Ok(body) = response.json::<Value>().await else {
        return failed(502, "Transak order could not be read");
    };
    let data = &body["data"];
    let wallet = data["walletAddress"].as_str().map(str::to_lowercase);
    if wallet.as_deref() != Some(signer.as_str()) {
        return ApiError::unauthorized("Unauthorized").into_response();
    }
    let field = |name: &str| data.get(name).cloned().unwrap_or(Value::Null);
    let order_id = body
        .pointer("/meta/orderId")
        .cloned()
        .unwrap_or_else(|| Value::String(id.clone()));
    let projected = json!({
        "meta": { "orderId": order_id },
        "data": {
            "id": field("id"),
            "status": field("status"),
            "transactionHash": field("transactionHash"),
            "walletAddress": field("walletAddress"),
            "errorMessage": field("errorMessage"),
            "isNFTOrder": field("isNFTOrder"),
        },
    });
    (
        StatusCode::OK,
        Json(json!({ "ok": true, "data": projected })),
    )
        .into_response()
}

/// Upstream's `WidgetOptionsSchema`: every field optional and nullable, no
/// other field allowed.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[allow(
    dead_code,
    reason = "parsed to validate; the original object is what is sent"
)]
struct WidgetOptions {
    fiat_amount: Option<f64>,
    fiat_currency: Option<String>,
    default_network: Option<String>,
    wallet_address: Option<String>,
    estimated_gas_limit: Option<f64>,
    contract_id: Option<String>,
    email: Option<String>,
    #[serde(rename = "redirectURL")]
    redirect_url: Option<String>,
    contract_address: Option<String>,
    trade_type: Option<String>,
    products_availed: Option<String>,
    #[serde(rename = "isNFT")]
    is_nft: Option<bool>,
    crypto_currency_code: Option<String>,
    nft_data: Option<Vec<NftData>>,
    calldata: Option<String>,
}

/// An `nftData` item: every field required; others are allowed, as upstream.
#[derive(Deserialize)]
#[allow(
    dead_code,
    reason = "parsed to validate; the original object is what is sent"
)]
struct NftData {
    #[serde(rename = "imageURL")]
    image_url: String,
    #[serde(rename = "nftName")]
    nft_name: String,
    #[serde(rename = "collectionAddress")]
    collection_address: String,
    #[serde(rename = "tokenID")]
    token_id: Vec<String>,
    price: Vec<f64>,
    quantity: f64,
    #[serde(rename = "nftType")]
    nft_type: NftType,
}

#[derive(Deserialize)]
enum NftType {
    #[serde(rename = "ERC721")]
    Erc721,
    #[serde(rename = "ERC1155")]
    Erc1155,
}

/// `POST /v1/transak/widget-url`.
pub async fn widget_url(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(transak) = state.settings.market.transak.as_ref() else {
        return not_configured();
    };
    if !is_json(&headers) {
        return failed(415, "Content-Type must be application/json");
    }
    let (Ok(Value::Object(mut params)), Ok(_)) = (
        serde_json::from_slice::<Value>(&body),
        serde_json::from_slice::<WidgetOptions>(&body),
    ) else {
        return ApiError::bad_request("Invalid JSON body").into_response();
    };
    // Last, so the server's values win over the caller's (`cyptoCurrencyList`
    // is upstream's spelling, sent as is).
    for (key, value) in [
        ("apiKey", transak.api_key.as_str()),
        ("referrerDomain", transak.referrer_domain.as_str()),
        ("networks", "ethereum,polygon"),
        ("cryptoCurrencyCode", "MANA"),
        ("defaultCryptoCurrency", "MANA"),
        ("cyptoCurrencyList", "MANA"),
    ] {
        params.insert(key.to_string(), Value::String(value.to_string()));
    }
    let request_body = json!({ "widgetParams": Value::Object(params) });
    let user_ip = client_ip(&headers);
    let url = format!("{}/v2/auth/session", transak.gateway_url);
    let response = match send_authorized(&state, transak, |token| {
        partner_request(state.http.post(url.as_str()), transak, token, &user_ip).json(&request_body)
    })
    .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, "a Transak widget URL request failed");
            return failed(500, error);
        }
    };
    let status = response.status();
    if !status.is_success() {
        tracing::warn!(status = status.as_u16(), "Transak refused a widget URL");
        return failed(
            500,
            format!("Error getting widget, status: {}", status.as_u16()),
        );
    }
    let reply: Option<Value> = response.json().await.ok();
    match reply
        .as_ref()
        .and_then(|reply| reply.pointer("/data/widgetUrl"))
        .and_then(Value::as_str)
    {
        Some(url) => (StatusCode::OK, Json(json!({ "ok": true, "data": url }))).into_response(),
        None => failed(500, "Transak's widget reply has no widgetUrl"),
    }
}

/// `PUT /v1/transak/access-token`, with upstream's `bearerTokenMiddleware`
/// answers.
pub async fn refresh_token(State(state): State<SharedState>, headers: HeaderMap) -> Response {
    let Some(transak) = state.settings.market.transak.as_ref() else {
        return not_configured();
    };
    let Some(authorization) = headers.get(header::AUTHORIZATION) else {
        return ApiError::unauthorized("Authorization header is missing").into_response();
    };
    let mut parts = authorization.to_str().unwrap_or("").split(' ');
    let kind = parts.next().unwrap_or("");
    let presented = parts.next().unwrap_or("");
    if kind != "Bearer" || !timing_safe_eq(presented.as_bytes(), transak.refresh_auth.as_bytes()) {
        return ApiError::unauthorized("Invalid authorization header").into_response();
    }
    match token(&state, transak, Want::New).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => {
            tracing::warn!(%error, "a forced Transak token refresh failed");
            failed(500, error)
        }
    }
}

#[cfg(test)]
#[path = "transak_tests.rs"]
mod tests;
