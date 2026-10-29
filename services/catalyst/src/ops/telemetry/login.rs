//! The console sign-in: the form posts the admin token once and a 30-day
//! `HttpOnly; Secure; SameSite=Strict` session cookie opens the GET reads
//! and pages (`admin::authorize_read`); the JSON writes still need the
//! bearer. The cookie is `<exp>.<mac>`: the unix expiry and an HMAC-SHA256
//! of it under the admin token ([`session_value`], [`session_valid`]), so
//! the cookie never carries the bearer (the old form stored the token
//! itself in the browser for 30 days) and rotating the token logs every
//! session out. Scoped to the base path the console is served under.

use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;

use super::SharedState;

pub(crate) const COOKIE_NAME: &str = "telemetry_admin";
pub(crate) const COOKIE_MAX_AGE_SECS: u32 = 30 * 24 * 3600;
const SESSION_CONTEXT: &str = "catalyst telemetry admin session\n";

fn mac(token: &str, exp: i64) -> Vec<u8> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(token.as_bytes()).expect("HMAC accepts any key length");
    mac.update(SESSION_CONTEXT.as_bytes());
    mac.update(exp.to_string().as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// The cookie value for a session expiring at `exp` (unix seconds).
pub(crate) fn session_value(token: &str, exp: i64) -> String {
    format!("{exp}.{}", URL_SAFE_NO_PAD.encode(mac(token, exp)))
}

/// Whether `value` is an unexpired session minted under `token` (a
/// constant-time mac check; a raw token pasted as the cookie is refused).
pub(crate) fn session_valid(token: &str, value: &str, now: i64) -> bool {
    let Some((exp, sig)) = value.split_once('.') else {
        return false;
    };
    let Ok(exp) = exp.parse::<i64>() else {
        return false;
    };
    if exp <= now {
        return false;
    }
    let Ok(sig) = URL_SAFE_NO_PAD.decode(sig) else {
        return false;
    };
    let mut expected =
        Hmac::<Sha256>::new_from_slice(token.as_bytes()).expect("HMAC accepts any key length");
    expected.update(SESSION_CONTEXT.as_bytes());
    expected.update(exp.to_string().as_bytes());
    expected.verify_slice(&sig).is_ok()
}

pub(crate) fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

fn cookie(base: &str, value: &str, max_age: u32) -> String {
    let path = if base.is_empty() { "/" } else { base };
    format!(
        "{COOKIE_NAME}={value}; Path={path}; HttpOnly; Secure; SameSite=Strict; Max-Age={max_age}"
    )
}

#[derive(Deserialize)]
pub struct LoginQuery {
    #[serde(default)]
    pub err: Option<String>,
}

pub async fn page(
    State(state): State<SharedState>,
    Query(q): Query<LoginQuery>,
    headers: HeaderMap,
) -> Response {
    if super::admin::authorize_read(&state, &headers).is_ok() {
        return Redirect::to(&format!("{}/", state.settings.base_path)).into_response();
    }
    let notice = if q.err.is_some() {
        r#"<p class="err">That token was not accepted.</p>"#
    } else {
        ""
    };
    Html(LOGIN_TEMPLATE.replace("<!--NOTICE-->", notice)).into_response()
}

#[derive(Deserialize)]
pub struct LoginForm {
    #[serde(default)]
    pub token: String,
}

pub async fn submit(State(state): State<SharedState>, Form(form): Form<LoginForm>) -> Response {
    let base = state.settings.base_path.as_str();
    let token = form.token.trim();
    if token.is_empty() {
        return (
            StatusCode::SEE_OTHER,
            [
                (header::SET_COOKIE, cookie(base, "", 0)),
                (header::LOCATION, format!("{base}/login")),
            ],
        )
            .into_response();
    }
    if super::admin::token_matches(&state, token) {
        let session = session_value(token, now_unix() + i64::from(COOKIE_MAX_AGE_SECS));
        (
            StatusCode::SEE_OTHER,
            [
                (
                    header::SET_COOKIE,
                    cookie(base, &session, COOKIE_MAX_AGE_SECS),
                ),
                (header::LOCATION, format!("{base}/")),
            ],
        )
            .into_response()
    } else {
        Redirect::to(&format!("{base}/login?err=1")).into_response()
    }
}

const LOGIN_TEMPLATE: &str = r#"<!doctype html>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Telemetry sign-in</title>
<style>
  body{margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;
    background:#101014;color:#e8e8ee;font:15px/1.5 system-ui,sans-serif}
  form{background:#17171d;border:1px solid #2a2a33;border-radius:10px;padding:28px 32px;
    display:flex;flex-direction:column;gap:12px;min-width:320px}
  h1{font-size:1.05rem;margin:0}
  p{margin:0;opacity:.7;font-size:.85rem}
  p.err{color:#ff7a7a;opacity:1}
  input{background:#0d0d11;border:1px solid #2a2a33;border-radius:6px;color:#e8e8ee;
    padding:9px 11px;font-size:.95rem}
  input:focus{outline:none;border-color:#6a5acd}
  button{background:#6a5acd;border:0;border-radius:6px;color:#fff;padding:9px 0;
    font-size:.95rem;font-weight:600;cursor:pointer}
  button:hover{filter:brightness(1.1)}
</style>
<form method="post" action="login">
  <h1>Telemetry admin</h1>
  <p>Paste the admin token once &#x2014; a browser session cookie keeps you signed in for 30 days.</p>
  <!--NOTICE-->
  <input type="password" name="token" placeholder="admin token" autofocus autocomplete="current-password">
  <button type="submit">Sign in</button>
</form>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cookie_is_scoped_to_the_base_path() {
        assert!(cookie("", "t", 5).starts_with(
            "telemetry_admin=t; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=5"
        ));
        assert!(cookie("/telemetry", "t", 5).contains("Path=/telemetry;"));
    }

    #[test]
    fn the_session_is_an_hmac_of_its_expiry_not_the_token() {
        let now = 1_800_000_000;
        let value = session_value("tok", now + 60);
        assert!(!value.contains("tok"), "{value}");
        assert!(session_valid("tok", &value, now));
        assert!(!session_valid("tok", &value, now + 60), "expired at exp");
        assert!(
            !session_valid("other", &value, now),
            "a rotated token logs the session out"
        );
        assert!(
            !session_valid("tok", "tok", now),
            "the raw token is not a session"
        );
        let (exp, sig) = value.split_once('.').unwrap();
        assert!(
            !session_valid("tok", &format!("{}.{sig}", now + 3600), now),
            "a moved expiry fails the mac"
        );
        assert!(!session_valid(
            "tok",
            &format!("{exp}.{}", URL_SAFE_NO_PAD.encode([0u8; 32])),
            now
        ));
        assert!(!session_valid("tok", "", now) && !session_valid("tok", "x.y", now));
    }
}
