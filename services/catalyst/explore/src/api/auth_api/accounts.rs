use std::time::Duration;

use axum::Json;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{AppState, bad_request, budgeted, internal_error};
use crate::api::json_error;
use crate::shared::signed_fetch::{self, AUTH_METADATA_HEADER, PayloadPolicy, SignedPath};

const MAX_AGE_SECONDS: i64 = 120;
const CLOCK_SKEW_SECONDS: i64 = 30;
const API_URL: &str = "https://api.magic.link/v1/admin/user/deletion/request";

#[derive(Clone)]
pub(crate) struct MagicDeletion {
    audience: String,
    secret: String,
    origins: Vec<String>,
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

async fn claim_attempt(pool: &sqlx::PgPool, token_key: &[u8]) -> sqlx::Result<bool> {
    let inserted = sqlx::query(
        "INSERT INTO explore_api.magic_deletion_attempts (token_key) VALUES ($1)
         ON CONFLICT DO NOTHING",
    )
    .bind(token_key)
    .execute(pool)
    .await?;
    Ok(inserted.rows_affected() == 1)
}

pub(super) async fn sweep_attempts(conn: &mut sqlx::PgConnection) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM explore_api.magic_deletion_attempts WHERE requested_at < now() - interval '1 day'")
        .execute(conn)
        .await?;
    Ok(())
}

pub(super) async fn delete_account(
    State(state): State<AppState>,
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
    let budget = budgeted(&state, None);
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
    match claim_attempt(&state.pool, token_key.as_slice()).await {
        Ok(true) => {}
        Ok(false) => return json_error(StatusCode::FORBIDDEN, "Magic DID token was already used"),
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
                    "Magic account deletion request failed",
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
    let response = state
        .http
        .post(API_URL)
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
        Json(json!({ "deleted": true, "address": address, "magic": result })).into_response(),
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
        let (a, b) = tokio::join!(claim_attempt(&db.pool, &key), claim_attempt(&other, &key));
        assert_ne!(a.unwrap(), b.unwrap());
        assert!(!claim_attempt(&other, &key).await.unwrap());
        other.close().await;
        db.teardown().await;
    }
}
