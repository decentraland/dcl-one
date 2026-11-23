use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::{ConnectInfo, OriginalUri, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{AppState, admission_lock, bad_request, budgeted, internal_error, request_source};
use crate::api::json_error;
use crate::shared::signed_fetch::{self, AUTH_METADATA_HEADER, PayloadPolicy, SignedPath};

const MAX_AGE_SECONDS: i64 = 120;
const CLOCK_SKEW_SECONDS: i64 = 30;
const ADMISSION_LOCK: &str = "explore_api.magic_deletion_attempts:admission";
const ADDRESS_LIMIT: i64 = 3;
const NETWORK_LIMIT: i64 = 20;
const GLOBAL_LIMIT: i64 = 120;
const API_URL: &str = "https://api.magic.link/v1/admin/user/deletion/request";

#[derive(Clone)]
pub(crate) struct MagicDeletion {
    audience: String,
    secret: String,
    origins: Vec<String>,
    #[cfg(test)]
    test_url: Option<String>,
}

impl std::fmt::Debug for MagicDeletion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MagicDeletion")
            .field("origins", &self.origins)
            .finish_non_exhaustive()
    }
}

impl MagicDeletion {
    pub(crate) fn from_config(config: &catalyst_config::config::Config) -> Option<Self> {
        let audience = config
            .node
            .magic_client_id
            .as_ref()
            .filter(|value| !value.trim().is_empty())?;
        let secret = config
            .node
            .magic_secret_key
            .as_ref()
            .filter(|value| !value.trim().is_empty())?;
        Some(Self {
            audience: audience.clone(),
            secret: secret.clone(),
            origins: config.node.account_deletion_allowed_origins.clone(),
            #[cfg(test)]
            test_url: None,
        })
    }
}

#[derive(Deserialize)]
struct Claim {
    iss: String,
    aud: String,
    iat: i64,
    nbf: i64,
    ext: i64,
    tid: String,
}

fn verify_token(token: &str, audience: &str, now: i64) -> Result<(String, [u8; 32]), StatusCode> {
    let invalid = StatusCode::UNAUTHORIZED;
    let decoded = STANDARD
        .decode(token)
        .or_else(|_| STANDARD_NO_PAD.decode(token))
        .map_err(|_| invalid)?;
    let (proof, raw): (String, String) = serde_json::from_slice(&decoded).map_err(|_| invalid)?;
    let claim: Claim = serde_json::from_str(&raw).map_err(|_| invalid)?;
    let claimed = claim.iss.strip_prefix("did:ethr:").ok_or(invalid)?;
    if claim.aud != audience
        || claim.ext <= now
        || claim.nbf > now + CLOCK_SKEW_SECONDS
        || claim.iat > now + CLOCK_SKEW_SECONDS
        || claim.tid.is_empty()
        || claim.tid.len() > 128
        || !crate::auth::is_eth_address(claimed)
    {
        return Err(invalid);
    }
    let address = crate::auth::recover_address(raw.as_bytes(), &proof).map_err(|_| invalid)?;
    if !address.eq_ignore_ascii_case(claimed) {
        return Err(invalid);
    }
    if claim.iat < now - MAX_AGE_SECONDS {
        return Err(StatusCode::FORBIDDEN);
    }
    let address = address.to_ascii_lowercase();
    let mut hash = Sha256::new();
    hash.update(address.as_bytes());
    hash.update(claim.tid.as_bytes());
    Ok((address, hash.finalize().into()))
}

#[derive(Debug, PartialEq, Eq)]
enum Attempt {
    Claimed,
    Reused,
    RateLimited,
}

async fn claim_attempt(
    pool: &sqlx::PgPool,
    token_key: &[u8; 32],
    address: &str,
    source: Option<&str>,
) -> sqlx::Result<Attempt> {
    let address_key = Sha256::digest(address.as_bytes());
    let network_key = source.map(|key| Sha256::digest(key.as_bytes()));

    let mut tx = pool.begin().await?;
    admission_lock(&mut tx, ADMISSION_LOCK).await?;
    let (reused, global, address, network): (bool, i64, i64, i64) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM explore_api.magic_deletion_attempts WHERE token_key = $1),
                count(*), count(*) FILTER (WHERE address_key = $2),
                count(*) FILTER (WHERE network_key IS NOT DISTINCT FROM $3)
         FROM explore_api.magic_deletion_attempts
         WHERE requested_at >= statement_timestamp() - interval '1 hour'",
    )
    .bind(token_key.as_slice())
    .bind(address_key.as_slice())
    .bind(network_key.as_ref().map(|key| key.as_slice()))
    .fetch_one(&mut *tx)
    .await?;
    if reused {
        return Ok(Attempt::Reused);
    }
    if address >= ADDRESS_LIMIT || network >= NETWORK_LIMIT || global >= GLOBAL_LIMIT {
        return Ok(Attempt::RateLimited);
    }
    sqlx::query(
        "INSERT INTO explore_api.magic_deletion_attempts
             (token_key, address_key, network_key, requested_at)
         VALUES ($1, $2, $3, statement_timestamp())",
    )
    .bind(token_key.as_slice())
    .bind(address_key.as_slice())
    .bind(network_key.as_ref().map(|key| key.as_slice()))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Attempt::Claimed)
}

pub(super) async fn sweep_attempts(conn: &mut sqlx::PgConnection) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM explore_api.magic_deletion_attempts WHERE requested_at < now() - interval '1 day'")
        .execute(conn)
        .await?;
    Ok(())
}

pub(super) async fn delete_account(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    let Some(magic) = state.settings.magic_deletion.as_ref() else {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Account deletion is unavailable",
        );
    };
    let allowed = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|origin| {
            !origin.is_empty()
                && magic
                    .origins
                    .iter()
                    .any(|allowed| origin.eq_ignore_ascii_case(allowed))
        });
    if !allowed {
        return json_error(StatusCode::FORBIDDEN, "Origin not allowed");
    }

    let metadata: Option<Value> = headers
        .get(AUTH_METADATA_HEADER)
        .and_then(|value| serde_json::from_slice(value.as_bytes()).ok());
    let Some(token) = metadata
        .as_ref()
        .and_then(|body| body.get("didToken"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 4096)
    else {
        return bad_request("A Magic DID token is required");
    };
    let (address, token_key) =
        match verify_token(token, &magic.audience, chrono::Utc::now().timestamp()) {
            Ok(token) => token,
            Err(StatusCode::FORBIDDEN) => {
                return json_error(StatusCode::FORBIDDEN, "Magic DID token is too old");
            }
            Err(_) => return json_error(StatusCode::UNAUTHORIZED, "Invalid Magic DID token"),
        };
    if signed_fetch::auth_chain_claimed_address(&headers).as_deref() != Some(address.as_str()) {
        return json_error(
            StatusCode::FORBIDDEN,
            "Magic wallet does not match the request signer",
        );
    }
    let source = request_source(
        &headers,
        peer.map(|Extension(ConnectInfo(peer))| peer),
        &state.settings.trusted_proxy_cidrs,
    );
    let budget = budgeted(&state, source.as_deref());
    let signer = signed_fetch::require_signer_at(
        &headers,
        "delete",
        &SignedPath::at(&uri, "/accounts"),
        PayloadPolicy::V6Only,
        super::as_validator(&budget),
    )
    .await;
    match signer {
        Ok(signer) if signer.as_str() == address => {}
        Ok(_) => {
            return json_error(
                StatusCode::FORBIDDEN,
                "Magic wallet does not match the request signer",
            );
        }
        Err(err) => {
            if let Some(spent) = budget.as_ref().and_then(|budget| budget.spent()) {
                return super::chain_refused(super::validation::ChainRefusal::Throttled(spent));
            }
            let (status, _) = err.http_status_and_message();
            return json_error(
                StatusCode::from_u16(status).unwrap_or(StatusCode::UNAUTHORIZED),
                "Account deletion requires a valid signed request",
            );
        }
    }

    // Persist the one-use claim before dispatching the external deletion request.
    match claim_attempt(&state.pool, &token_key, &address, source.as_deref()).await {
        Ok(Attempt::Claimed) => {}
        Ok(Attempt::Reused) => {
            return json_error(StatusCode::FORBIDDEN, "Magic DID token was already used");
        }
        Ok(Attempt::RateLimited) => {
            return json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Too many account deletion requests",
            );
        }
        Err(err) => return internal_error("Account deletion could not be recorded", err),
    }

    let (status, answer) = match request_deletion(&state, magic, &address).await {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!(%err, "Magic account deletion provider request failed");
            (
                "upstream_error",
                json_error(
                    StatusCode::BAD_GATEWAY,
                    "The outcome of the Magic deletion request is unknown",
                ),
            )
        }
    };
    if let Err(error) = sqlx::query(
        "UPDATE explore_api.magic_deletion_attempts SET status = $2 WHERE token_key = $1",
    )
    .bind(token_key.as_slice())
    .bind(status)
    .execute(&state.pool)
    .await
    {
        tracing::error!(%error, status, "Magic account deletion result could not be recorded");
    }
    answer
}

async fn request_deletion(
    state: &AppState,
    magic: &MagicDeletion,
    address: &str,
) -> anyhow::Result<(&'static str, Response)> {
    #[cfg(test)]
    let url = magic.test_url.as_deref().unwrap_or(API_URL);
    #[cfg(not(test))]
    let url = API_URL;
    let response = state
        .http
        .post(url)
        .header("X-Magic-Secret-Key", &magic.secret)
        .timeout(Duration::from_secs(10))
        .max_redirects(0)
        .json(&json!({ "public_addresses": [address] }))
        .send()
        .await?;
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Ok((
            "rate_limited",
            json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Magic account deletion is rate limited",
            ),
        ));
    }
    let result: Value = response.error_for_status()?.json().await?;
    let processed = result
        .get("processed")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.as_str()
                    .is_some_and(|value| value.eq_ignore_ascii_case(address))
            })
        });
    if !processed {
        return Ok((
            "unprocessed",
            json_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Magic did not process the deletion",
            ),
        ));
    }
    Ok((
        "processed",
        Json(json!({ "deleted": true, "address": address })).into_response(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_signer::SignerSync;

    fn signed_token(now: i64, issued: i64, token_id: &str) -> String {
        let wallet = crate::shared::signed_fetch::test_support::wallet();
        let address = format!("{:#x}", wallet.address());
        let claim = json!({
            "iss": format!("did:ethr:{address}"),
            "aud": "decentraland",
            "iat": issued,
            "nbf": issued,
            "ext": now + 120,
            "tid": token_id,
        })
        .to_string();
        let proof = wallet.sign_message_sync(claim.as_bytes()).unwrap();
        STANDARD.encode(json!([format!("0x{}", hex::encode(proof.as_bytes())), claim]).to_string())
    }

    #[test]
    fn magic_token_matches_the_audience_and_recovers_a_fresh_wallet() {
        let now = chrono::Utc::now().timestamp();
        let token = signed_token(now, now, "one");
        let (address, key) = verify_token(&token, "decentraland", now).unwrap();
        assert_eq!(
            address,
            crate::shared::signed_fetch::test_support::address()
        );
        assert_eq!(
            verify_token(&token, "other-app", now),
            Err(StatusCode::UNAUTHORIZED)
        );
        assert_eq!(
            verify_token(&token, "decentraland", now + 120),
            Err(StatusCode::UNAUTHORIZED)
        );
        assert_eq!(
            verify_token(&signed_token(now, now - 121, "one"), "decentraland", now),
            Err(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            verify_token(&signed_token(now, now - 1, "one"), "decentraland", now)
                .unwrap()
                .1,
            key
        );
    }

    #[tokio::test]
    async fn only_one_replica_can_use_a_magic_token() {
        let Some(db) = crate::api::tests_support::scratch_database().await else {
            return;
        };
        let other = db.second_pool().await;
        let key = [13u8; 32];
        let (a, b) = tokio::join!(
            claim_attempt(&db.pool, &key, "wallet", Some("network")),
            claim_attempt(&other, &key, "wallet", Some("network"))
        );
        assert_ne!(a.unwrap(), b.unwrap());
        assert_eq!(
            claim_attempt(&other, &key, "wallet", Some("network"))
                .await
                .unwrap(),
            Attempt::Reused
        );
        other.close().await;
        db.teardown().await;
    }

    #[tokio::test]
    async fn deletion_claims_respect_wallet_network_and_global_limits() {
        let Some(db) = crate::api::tests_support::scratch_database().await else {
            return;
        };
        let mut next_id = 0u16;
        let mut key = || {
            next_id += 1;
            let mut key = [0; 32];
            key[..2].copy_from_slice(&next_id.to_be_bytes());
            key
        };
        for _ in 0..ADDRESS_LIMIT {
            assert_eq!(
                claim_attempt(&db.pool, &key(), "wallet", Some("network"))
                    .await
                    .unwrap(),
                Attempt::Claimed
            );
        }
        assert_eq!(
            claim_attempt(&db.pool, &key(), "wallet", Some("another-network"))
                .await
                .unwrap(),
            Attempt::RateLimited
        );
        for wallet in ADDRESS_LIMIT..NETWORK_LIMIT {
            assert_eq!(
                claim_attempt(&db.pool, &key(), &wallet.to_string(), Some("network"))
                    .await
                    .unwrap(),
                Attempt::Claimed
            );
        }
        assert_eq!(
            claim_attempt(&db.pool, &key(), "different-wallet", Some("network"))
                .await
                .unwrap(),
            Attempt::RateLimited
        );
        for i in NETWORK_LIMIT..GLOBAL_LIMIT {
            assert_eq!(
                claim_attempt(&db.pool, &key(), &i.to_string(), Some(&i.to_string()))
                    .await
                    .unwrap(),
                Attempt::Claimed
            );
        }
        assert_eq!(
            claim_attempt(&db.pool, &key(), "new-wallet", Some("new-network"))
                .await
                .unwrap(),
            Attempt::RateLimited
        );
        let rows: i64 =
            sqlx::query_scalar("SELECT count(*) FROM explore_api.magic_deletion_attempts")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(rows, GLOBAL_LIMIT);
        db.teardown().await;
    }

    #[tokio::test]
    async fn account_deletion_refuses_a_foreign_origin_and_wallet() {
        let settings = with_magic(None);
        let app = crate::api::router(crate::api::tests::node_without_db_state(settings).await);
        let owner = crate::shared::signed_fetch::test_support::wallet();
        assert_eq!(
            call(&app, "https://unlisted.example", "one", &owner)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        let other = alloy_signer_local::PrivateKeySigner::random();
        let (status, body) = call(&app, ORIGIN, "one", &other).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            body["error"],
            "Magic wallet does not match the request signer"
        );
    }

    #[tokio::test]
    async fn account_deletion_handles_provider_failures_and_rejects_token_reuse() {
        use std::future::IntoFuture;
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let Some(db) = crate::api::tests_support::scratch_database().await else {
            return;
        };
        let address = crate::shared::signed_fetch::test_support::address();
        let owner = crate::shared::signed_fetch::test_support::wallet();
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = axum::Router::new().fallback({
            let address = address.clone();
            let calls = Arc::clone(&calls);
            move || {
                let address = address.clone();
                let calls = Arc::clone(&calls);
                async move {
                    match calls.fetch_add(1, Ordering::Relaxed) {
                        0 => StatusCode::TOO_MANY_REQUESTS.into_response(),
                        1 => StatusCode::SERVICE_UNAVAILABLE.into_response(),
                        _ => Json(json!({ "processed": [address], "internal": "redacted" }))
                            .into_response(),
                    }
                }
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(axum::serve(listener, provider).into_future());
        let state =
            crate::api::tests::node_on_pool_state(with_magic(Some(url)), db.pool.clone()).await;
        let app = crate::api::router(state);

        assert_eq!(
            call(&app, ORIGIN, "one", &owner).await.0,
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            call(&app, ORIGIN, "one", &owner).await.0,
            StatusCode::FORBIDDEN
        );
        let (status, body) = call(&app, ORIGIN, "two", &owner).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(
            body["error"],
            "The outcome of the Magic deletion request is unknown"
        );
        assert_eq!(
            call(&app, ORIGIN, "three", &owner).await,
            (
                StatusCode::OK,
                json!({ "deleted": true, "address": address })
            ),
        );
        assert_eq!(calls.load(Ordering::Relaxed), 3);
        server.abort();
        db.teardown().await;
    }

    const ORIGIN: &str = "https://decentraland.org";
    const ACCOUNT_PATH: &str = "/auth-api/accounts";

    fn with_magic(test_url: Option<String>) -> crate::api::Settings {
        let mut settings = crate::api::Settings::for_tests(crate::api::Sources::defaults());
        settings.magic_deletion = Some(MagicDeletion {
            audience: "decentraland".into(),
            secret: "dummy-key".into(),
            origins: vec![ORIGIN.into()],
            test_url,
        });
        settings
    }

    async fn call(
        app: &axum::Router,
        origin: &str,
        token_id: &str,
        wallet: &alloy_signer_local::PrivateKeySigner,
    ) -> (StatusCode, Value) {
        use axum::body::{Body, to_bytes};
        use axum::extract::Request;
        use tower::ServiceExt;

        let now = chrono::Utc::now();
        let token = signed_token(now.timestamp(), now.timestamp(), token_id);
        let metadata = json!({ "didToken": token }).to_string();
        let headers = signed_fetch::test_support::signed_headers_by(
            wallet,
            "delete",
            ACCOUNT_PATH,
            &metadata,
            true,
            &now.timestamp_millis().to_string(),
        )
        .await;
        let mut request = Request::builder()
            .method("DELETE")
            .uri(ACCOUNT_PATH)
            .body(Body::empty())
            .unwrap();
        *request.headers_mut() = headers;
        request
            .headers_mut()
            .insert(header::ORIGIN, origin.parse().unwrap());
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        (status, body)
    }
}
