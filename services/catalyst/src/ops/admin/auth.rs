//! Sign-in With Ethereum for the console, exactly as the old server did it:
//! `GET /admin/auth/nonce?address=` answers the SIWE message to sign, whose
//! nonce is this node's HMAC over `host|address|exp` (no nonce table; the
//! mac is what makes the message unforgeable and five minutes short);
//! `POST /admin/auth/verify {message, signature}` re-parses the message,
//! checks the host it was minted for, its expiry and its mac, recovers the
//! signer and mints the session cookie only when it is in
//! `ADMIN_ADDRESSES`. [`AdminSession`] is the extractor every admin handler
//! takes: same-origin (`Origin`/`Referer` host against the request host)
//! plus the cookie's mac, expiry and the live allowlist.

use axum::extract::{FromRef, FromRequestParts, Json, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use super::{SharedState, session};

const NONCE_TTL_SECS: i64 = 300;

/// Proof that the current request carried a valid admin session cookie.
///
/// `address` is deliberately private: a public field would let any code in
/// this crate mint an `AdminSession` from a bare string and hand it to the
/// admin handlers that trust it, bypassing the SIWE + HMAC + allowlist
/// check. The only constructor is the `FromRequestParts` impl below.
pub struct AdminSession {
    address: String,
}

impl AdminSession {
    /// Recovered from the SIWE sign-in and re-verified (HMAC signature + live
    /// allowlist) by `session::verify` on this request.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl<S> FromRequestParts<S> for AdminSession
where
    S: Send + Sync,
    SharedState: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let denied = (StatusCode::FORBIDDEN, "admin auth required");
        if !origin_ok(&parts.headers) {
            return Err((StatusCode::FORBIDDEN, "cross-origin request rejected"));
        }
        let Some(val) = cookie_value(&parts.headers, session::COOKIE_NAME) else {
            return Err(denied);
        };
        let state = SharedState::from_ref(state);
        match session::verify(&state.settings, &val) {
            Some(address) => Ok(AdminSession { address }),
            None => Err(denied),
        }
    }
}

pub(crate) fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for pair in raw.split(';') {
        if let Some((k, v)) = pair.trim().split_once('=')
            && k.trim() == name
        {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn request_host(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
}

fn is_https(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}

fn url_host(u: &str) -> Option<String> {
    let after = u.split_once("://").map(|(_, r)| r).unwrap_or(u);
    let host = after.split(['/', '?', '#']).next().unwrap_or("").trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_lowercase())
    }
}

fn origin_ok(headers: &HeaderMap) -> bool {
    let stated = headers
        .get(header::ORIGIN)
        .or_else(|| headers.get(header::REFERER))
        .and_then(|v| v.to_str().ok())
        .and_then(url_host);
    match (stated, request_host(headers)) {
        (Some(o), Some(h)) => o == h,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

pub(crate) fn valid_eth_address(s: &str) -> bool {
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
    matches!(s, Some(rest) if rest.len() == 40 && rest.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn chain_id(network: &str) -> u64 {
    match network {
        "mainnet" => 1,
        "sepolia" => 11_155_111,
        "goerli" => 5,
        _ => 1,
    }
}

fn rfc3339(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default()
}

fn build_message(
    host: &str,
    scheme: &str,
    address: &str,
    chain: u64,
    nonce: &str,
    issued: i64,
    exp: i64,
) -> String {
    format!(
        "{host} wants you to sign in with your Ethereum account:\n{address}\n\n\
         Sign in to the catalyrst admin console.\n\n\
         URI: {scheme}://{host}/admin\nVersion: 1\nChain ID: {chain}\n\
         Nonce: {nonce}\nIssued At: {issued_at}\nExpiration Time: {exp_at}",
        issued_at = rfc3339(issued),
        exp_at = rfc3339(exp),
    )
}

#[derive(Deserialize)]
pub struct NonceQuery {
    pub address: String,
}

pub async fn nonce(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<NonceQuery>,
) -> Response {
    let settings = &state.settings;
    if !settings.admin_enabled() {
        return (StatusCode::FORBIDDEN, "admin disabled").into_response();
    }
    let address = q.address.trim().to_lowercase();
    if !valid_eth_address(&address) {
        return (StatusCode::BAD_REQUEST, "invalid address").into_response();
    }
    let Some(host) = request_host(&headers) else {
        return (StatusCode::BAD_REQUEST, "missing host").into_response();
    };
    let issued = session::now_unix();
    let exp = issued + NONCE_TTL_SECS;
    let Some(nonce) = session::mac_b64(settings, &challenge_payload(&host, &address, exp)) else {
        return (StatusCode::FORBIDDEN, "admin disabled").into_response();
    };
    let scheme = if is_https(&headers) { "https" } else { "http" };
    let message = build_message(
        &host,
        scheme,
        &address,
        chain_id(&settings.eth_network),
        &nonce,
        issued,
        exp,
    );
    Json(json!({ "message": message })).into_response()
}

fn challenge_payload(host: &str, address: &str, exp: i64) -> String {
    format!("{host}|{address}|{exp}")
}

#[derive(Deserialize)]
pub struct VerifyReq {
    pub message: String,
    pub signature: String,
}

struct ParsedMessage {
    host: String,
    address: String,
    nonce: String,
    exp: i64,
}

fn line_after<'a>(message: &'a str, label: &str) -> Option<&'a str> {
    message
        .lines()
        .find_map(|l| l.trim().strip_prefix(label).map(|r| r.trim()))
        .filter(|s| !s.is_empty())
}

fn parse_message(message: &str) -> Option<ParsedMessage> {
    let host = message
        .lines()
        .next()?
        .split(" wants you to sign in")
        .next()?
        .trim()
        .to_lowercase();
    if host.is_empty() {
        return None;
    }
    let address = message
        .lines()
        .map(|l| l.trim())
        .find(|l| valid_eth_address(l))?
        .to_lowercase();
    let nonce = line_after(message, "Nonce:")?.to_string();
    let exp = chrono::DateTime::parse_from_rfc3339(line_after(message, "Expiration Time:")?)
        .ok()?
        .timestamp();
    Some(ParsedMessage {
        host,
        address,
        nonce,
        exp,
    })
}

pub async fn verify(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<VerifyReq>,
) -> Response {
    let settings = &state.settings;
    let denied = || (StatusCode::FORBIDDEN, "sign-in failed").into_response();

    if !settings.admin_enabled() {
        return denied();
    }
    let Some(host) = request_host(&headers) else {
        return denied();
    };
    let Some(parsed) = parse_message(&req.message) else {
        return denied();
    };
    if parsed.host != host || parsed.exp <= session::now_unix() {
        return denied();
    }
    if !session::mac_verify(
        settings,
        &challenge_payload(&parsed.host, &parsed.address, parsed.exp),
        &parsed.nonce,
    ) {
        return denied();
    }
    let recovered = match crate::auth::recover_address(req.message.as_bytes(), &req.signature) {
        Ok(addr) => addr.to_lowercase(),
        Err(_) => return denied(),
    };
    if recovered != parsed.address {
        return denied();
    }
    let Some(cookie) = session::mint(settings, &recovered) else {
        return denied();
    };
    // The allowlist check: mint signs any address, verify admits only the allowed ones.
    if session::verify(settings, &cookie).is_none() {
        return denied();
    }
    let set_cookie = session::set_cookie_header(settings, &cookie, !settings.cookie_insecure);
    let mut resp = Json(json!({ "authenticated": true, "address": recovered })).into_response();
    match set_cookie.parse() {
        Ok(h) => {
            resp.headers_mut().insert(header::SET_COOKIE, h);
            resp
        }
        Err(_) => denied(),
    }
}

pub async fn logout() -> Response {
    let mut resp = Json(json!({ "authenticated": false })).into_response();
    if let Ok(h) = session::clear_cookie_header().parse() {
        resp.headers_mut().insert(header::SET_COOKIE, h);
    }
    resp
}

pub async fn me(State(state): State<SharedState>, headers: HeaderMap) -> Response {
    match cookie_value(&headers, session::COOKIE_NAME)
        .and_then(|v| session::verify(&state.settings, &v))
    {
        Some(address) => (
            StatusCode::OK,
            Json(json!({ "authenticated": true, "address": address })),
        )
            .into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "authenticated": false })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_eth_address_checks_shape() {
        assert!(valid_eth_address(
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ));
        assert!(valid_eth_address(
            "0xAbC0000000000000000000000000000000000000"
        ));
        assert!(!valid_eth_address("0x123"));
        assert!(!valid_eth_address(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ));
        assert!(!valid_eth_address("0x../../etc/passwd"));
        assert!(!valid_eth_address(
            "0xzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"
        ));
    }

    #[test]
    fn url_host_extracts_host() {
        assert_eq!(
            url_host("https://realm.example.com/admin").as_deref(),
            Some("realm.example.com")
        );
        assert_eq!(
            url_host("http://127.0.0.1:5141").as_deref(),
            Some("127.0.0.1:5141")
        );
        assert_eq!(
            url_host("https://h.test/a/b?c=d").as_deref(),
            Some("h.test")
        );
    }

    #[test]
    fn origin_ok_matches_host() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "realm.test".parse().unwrap());
        assert!(origin_ok(&h));
        h.insert(header::ORIGIN, "https://realm.test".parse().unwrap());
        assert!(origin_ok(&h));
        h.insert(header::ORIGIN, "https://evil.test".parse().unwrap());
        assert!(!origin_ok(&h));
        // The proxy's forwarded host wins over the upstream Host header.
        h.insert("x-forwarded-host", "evil.test".parse().unwrap());
        assert!(origin_ok(&h));
    }

    #[test]
    fn parse_message_roundtrips_build() {
        let msg = build_message(
            "realm.test",
            "https",
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
            "the-nonce-value",
            1_900_000_000,
            1_900_000_300,
        );
        let p = parse_message(&msg).expect("parses");
        assert_eq!(p.host, "realm.test");
        assert_eq!(p.address, "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(p.nonce, "the-nonce-value");
        assert_eq!(p.exp, 1_900_000_300);
    }

    #[test]
    fn parse_message_rejects_garbage() {
        assert!(parse_message("not a real message").is_none());
    }

    #[test]
    fn cookie_value_finds_named_cookie() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            "other=1; cat_admin=the-value; x=2".parse().unwrap(),
        );
        assert_eq!(cookie_value(&h, "cat_admin").as_deref(), Some("the-value"));
        assert!(cookie_value(&h, "missing").is_none());
    }
}
