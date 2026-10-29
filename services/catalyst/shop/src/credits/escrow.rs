//! The economy side of a checkout: the broker buy that delivers a line into
//! the Landiler escrow, the escrow reclaim/release of a lease, and the MANA
//! payment verification behind the top-up. The old crate reached the economy
//! service over HTTP with `CATALYRST_ECONOMY_ADMIN_TOKEN`; here [`Broker`] is
//! a module call into `economy` (its handlers, invoked with the economy
//! bearer the process holds, so the economy's own authorization check and
//! idempotency ledgers run unchanged), and a test hands in a fake. Without an
//! economy admin token, an escrow address or `DATA_WRITES_ENABLED`, nothing
//! is bought: the checkout refuses to debit and the outbox idles.
//!
//! The escrow release worker (`credits-escrow-release`) is the old
//! `ReleaseWorker`: every tick it releases the active `shop.usage_grants`
//! whose lock expired (a bounded batch) to their buyer through the broker.

use std::sync::Arc;

use async_trait::async_trait;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::IntoResponse;
use serde_json::Value;
use sqlx::Row;

use super::errors::ApiError;
use super::store::{Credits, now};
use crate::jobs::{Jobs, PeriodicCfg};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaPaymentVerification {
    Pending,
    Reverted,
    NoPayment,
    Confirmed {
        from: String,
        to: String,
        value_wei: String,
    },
}

pub fn parse_verification(v: &Value) -> Result<ManaPaymentVerification, ApiError> {
    match v.get("status").and_then(|s| s.as_str()) {
        Some("pending") => Ok(ManaPaymentVerification::Pending),
        Some("reverted") => Ok(ManaPaymentVerification::Reverted),
        Some("no_payment") => Ok(ManaPaymentVerification::NoPayment),
        Some("confirmed") => {
            let field = |name: &str| {
                v.get(name)
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_lowercase())
                    .ok_or_else(|| {
                        ApiError::Internal(format!(
                            "economy verify 'confirmed' response missing {name}"
                        ))
                    })
            };
            Ok(ManaPaymentVerification::Confirmed {
                from: field("from")?,
                to: field("to")?,
                value_wei: field("valueWei")?,
            })
        }
        other => Err(ApiError::Internal(format!(
            "economy verify returned unknown status {other:?}"
        ))),
    }
}

/// One broker buy: the line as the outbox pinned it.
#[derive(Debug, Clone)]
pub struct BuyRequest {
    pub collection: String,
    pub item_id: String,
    pub token_id: Option<String>,
    pub price_wei: String,
    pub mode: String,
    pub buyer: String,
    pub trade: Option<Value>,
    pub idempotency_key: String,
    /// The credits debit the buy settles, in US cents.
    pub charge_cents: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerError {
    Retryable(String),
    Terminal(String),
}

impl std::fmt::Display for BrokerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retryable(m) => write!(f, "retryable: {m}"),
            Self::Terminal(m) => write!(f, "terminal: {m}"),
        }
    }
}

#[async_trait]
pub trait Broker: Send + Sync {
    /// Whether a buy can be attempted at all (token, escrow and the write switch).
    fn configured(&self) -> bool;

    fn escrow_address(&self) -> Option<String>;

    /// Delivers the line into escrow; `Ok` is the forward transaction hash.
    async fn buy(&self, req: &BuyRequest) -> Result<String, BrokerError>;

    async fn reclaim(
        &self,
        collection: &str,
        token_id: &str,
        idempotency_key: &str,
    ) -> Result<String, ApiError>;

    async fn release(
        &self,
        collection: &str,
        token_id: &str,
        buyer: &str,
        idempotency_key: &str,
    ) -> Result<String, ApiError>;

    async fn verify_mana_payment(&self, tx_hash: &str)
    -> Result<ManaPaymentVerification, ApiError>;
}

/// The in-process economy module.
pub struct EconomyBroker {
    state: Arc<crate::evm::State>,
}

impl EconomyBroker {
    pub fn new(state: Arc<crate::evm::State>) -> Self {
        Self { state }
    }

    fn token(&self) -> Option<&str> {
        self.state
            .settings
            .admin_token
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
    }

    fn headers(&self) -> Result<HeaderMap, ApiError> {
        let token = self.token().ok_or_else(|| {
            ApiError::not_implemented("escrow ops disabled (CATALYRST_ECONOMY_ADMIN_TOKEN unset)")
        })?;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
                ApiError::Internal("economy admin token is not a header value".into())
            })?,
        );
        Ok(headers)
    }

    fn status_and_message(e: crate::evm::errors::ApiError) -> (u16, String) {
        let message = e.to_string();
        let status = e.into_response().status().as_u16();
        (status, message)
    }

    fn tx_hash_of(v: &Value) -> Result<String, ApiError> {
        v.get("txHash")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| ApiError::Internal("economy 200 missing txHash".into()))
    }
}

#[async_trait]
impl Broker for EconomyBroker {
    fn configured(&self) -> bool {
        self.token().is_some()
            && self.escrow_address().is_some()
            && self.state.settings.data_writes_enabled
    }

    fn escrow_address(&self) -> Option<String> {
        self.state
            .settings
            .landiler_escrow_address
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    async fn buy(&self, req: &BuyRequest) -> Result<String, BrokerError> {
        if !self.state.settings.data_writes_enabled {
            return Err(BrokerError::Retryable(
                "DATA_WRITES_ENABLED=false: the economy broker accepts no buys on this node".into(),
            ));
        }
        let headers = self
            .headers()
            .map_err(|e| BrokerError::Retryable(e.to_string()))?;
        let Some(escrow) = self.escrow_address() else {
            return Err(BrokerError::Retryable(
                "LANDILER_ESCROW_ADDRESS unset".into(),
            ));
        };
        let body = crate::checkout::broker::BuyBody {
            collection: req.collection.clone(),
            item_id: Some(req.item_id.clone()),
            token_id: req.token_id.clone(),
            price_wei: req.price_wei.clone(),
            escrow_address: escrow,
            mode: req.mode.clone(),
            buyer_address: Some(req.buyer.clone()),
            trade: req.trade.clone(),
            quote_mana_wei: None,
            idempotency_key: Some(req.idempotency_key.clone()),
            charge_cents: req.charge_cents,
        };
        match crate::checkout::broker::buy(State(self.state.clone()), headers, Ok(Json(body))).await
        {
            Ok(Json(v)) => Self::tx_hash_of(&v).map_err(|e| BrokerError::Retryable(e.to_string())),
            Err(e) => {
                let (code, msg) = Self::status_and_message(e);
                let msg = format!("broker status {code}: {msg}");
                if code == 408 || code == 409 || code == 429 || (500..600).contains(&code) {
                    Err(BrokerError::Retryable(msg))
                } else {
                    Err(BrokerError::Terminal(msg))
                }
            }
        }
    }

    async fn reclaim(
        &self,
        collection: &str,
        token_id: &str,
        idempotency_key: &str,
    ) -> Result<String, ApiError> {
        let headers = self.headers()?;
        let body = crate::checkout::escrow::ReclaimBody {
            collection: collection.to_string(),
            token_id: token_id.to_string(),
            idempotency_key: Some(idempotency_key.to_string()),
        };
        match crate::checkout::escrow::reclaim(State(self.state.clone()), headers, Ok(Json(body)))
            .await
        {
            Ok(Json(v)) => Self::tx_hash_of(&v),
            Err(e) => {
                let (code, msg) = Self::status_and_message(e);
                Err(ApiError::Internal(format!("economy status {code}: {msg}")))
            }
        }
    }

    async fn release(
        &self,
        collection: &str,
        token_id: &str,
        buyer: &str,
        idempotency_key: &str,
    ) -> Result<String, ApiError> {
        let headers = self.headers()?;
        let body = crate::checkout::escrow::ReleaseBody {
            collection: collection.to_string(),
            token_id: token_id.to_string(),
            buyer: buyer.to_string(),
            idempotency_key: Some(idempotency_key.to_string()),
        };
        match crate::checkout::escrow::release(State(self.state.clone()), headers, Ok(Json(body)))
            .await
        {
            Ok(Json(v)) => Self::tx_hash_of(&v),
            Err(e) => {
                let (code, msg) = Self::status_and_message(e);
                Err(ApiError::Internal(format!("economy status {code}: {msg}")))
            }
        }
    }

    async fn verify_mana_payment(
        &self,
        tx_hash: &str,
    ) -> Result<ManaPaymentVerification, ApiError> {
        let headers = self.headers()?;
        let body = crate::treasury::payments::VerifyBody {
            tx_hash: tx_hash.to_string(),
        };
        match crate::treasury::payments::verify(State(self.state.clone()), headers, Ok(Json(body)))
            .await
        {
            Ok(Json(v)) => parse_verification(&v),
            Err(e) => {
                let (code, msg) = Self::status_and_message(e);
                if code == 400 {
                    return Err(ApiError::bad_request(format!(
                        "economy rejected the txHash: {msg}"
                    )));
                }
                Err(ApiError::Internal(format!(
                    "economy verify status {code}: {msg}"
                )))
            }
        }
    }
}

/// No economy state in this process (tests, or a node built without the
/// data plane): nothing is configured, every call says so.
pub struct NoBroker;

#[async_trait]
impl Broker for NoBroker {
    fn configured(&self) -> bool {
        false
    }

    fn escrow_address(&self) -> Option<String> {
        None
    }

    async fn buy(&self, _req: &BuyRequest) -> Result<String, BrokerError> {
        Err(BrokerError::Retryable(
            "the economy broker is not configured on this node".into(),
        ))
    }

    async fn reclaim(
        &self,
        _collection: &str,
        _token_id: &str,
        _idempotency_key: &str,
    ) -> Result<String, ApiError> {
        Err(ApiError::not_implemented(
            "escrow ops disabled (CATALYRST_ECONOMY_ADMIN_TOKEN unset)",
        ))
    }

    async fn release(
        &self,
        _collection: &str,
        _token_id: &str,
        _buyer: &str,
        _idempotency_key: &str,
    ) -> Result<String, ApiError> {
        Err(ApiError::not_implemented(
            "escrow ops disabled (CATALYRST_ECONOMY_ADMIN_TOKEN unset)",
        ))
    }

    async fn verify_mana_payment(
        &self,
        _tx_hash: &str,
    ) -> Result<ManaPaymentVerification, ApiError> {
        Err(ApiError::not_implemented(
            "MANA top-ups are disabled (CATALYRST_ECONOMY_ADMIN_TOKEN unset)",
        ))
    }
}

const RELEASE_BATCH_LIMIT: i64 = 50;

pub const RELEASE_JOB_NAME: &str = "credits-escrow-release";

struct DueGrant {
    id: i64,
    buyer: String,
    urn: String,
    token_id: Option<String>,
    collection: Option<String>,
}

/// Releases the escrowed items whose lease expired to their buyer.
#[derive(Clone)]
pub struct ReleaseWorker {
    pub credits: Credits,
    pub broker: Arc<dyn Broker>,
}

impl ReleaseWorker {
    /// Attached only when the broker is configured: without it there is
    /// nothing to poll (authorization expiry runs on the fulfilment tick).
    pub fn spawn(self, jobs: &Jobs, interval: std::time::Duration) {
        if !self.broker.configured() {
            tracing::warn!(
                "escrow releases disabled: the economy broker is not configured (CATALYRST_ECONOMY_ADMIN_TOKEN / LANDILER_ESCROW_ADDRESS / DATA_WRITES_ENABLED)"
            );
            return;
        }
        jobs.spawn_periodic_leased(
            RELEASE_JOB_NAME,
            PeriodicCfg::every(interval.max(std::time::Duration::from_secs(1)))
                .with_error_backoff(std::time::Duration::from_secs(600)),
            move |ctx| {
                let worker = self.clone();
                async move {
                    let released = worker.run_once().await?;
                    ctx.progress(format!("released {released}"));
                    Ok(())
                }
            },
        );
    }

    pub async fn run_once(&self) -> Result<usize, ApiError> {
        if !self.broker.configured() {
            return Ok(0);
        }
        let due: Vec<DueGrant> = sqlx::query(
            "SELECT id, grantee_address, urn, token_id, collection \
             FROM shop.usage_grants \
             WHERE status = 'active' AND unlock_at <= $1 \
             ORDER BY id LIMIT $2",
        )
        .bind(now())
        .bind(RELEASE_BATCH_LIMIT)
        .fetch_all(&self.credits.pool)
        .await?
        .into_iter()
        .map(|r| DueGrant {
            id: r.get("id"),
            buyer: r.get("grantee_address"),
            urn: r.get("urn"),
            token_id: r.get("token_id"),
            collection: r.get("collection"),
        })
        .collect();

        let mut released = 0usize;
        for g in due {
            if self.release_one(&g).await {
                released += 1;
            }
        }
        Ok(released)
    }

    async fn release_one(&self, g: &DueGrant) -> bool {
        let Some(token_id) = g.token_id.as_deref() else {
            tracing::warn!(grant_id = g.id, urn = %g.urn, "release skipped: usage_grant has no token_id (primary mint); the minted tokenId must be resolved from the escrow Leased event / buy receipt (follow-up)");
            return false;
        };
        let Some(collection) = g.collection.as_deref() else {
            tracing::warn!(grant_id = g.id, urn = %g.urn, "release skipped: usage_grant has no collection (manual grant)");
            return false;
        };

        let idem = format!("release:grant:{}", g.id);
        let tx_hash = match self
            .broker
            .release(collection, token_id, &g.buyer, &idem)
            .await
        {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(grant_id = g.id, urn = %g.urn, error = %e, "escrow release call failed; grant stays 'active' and retries next tick");
                return false;
            }
        };

        let flipped = sqlx::query(
            "UPDATE shop.usage_grants SET status = 'released' WHERE id = $1 AND status = 'active'",
        )
        .bind(g.id)
        .execute(&self.credits.pool)
        .await;
        match flipped {
            Ok(res) if res.rows_affected() > 0 => {
                tracing::info!(grant_id = g.id, urn = %g.urn, tx_hash = %tx_hash, "escrow grant released to buyer at unlock");
                true
            }
            Ok(_) => {
                tracing::info!(grant_id = g.id, urn = %g.urn, "release: grant already released or revoked; idempotent no-op");
                true
            }
            Err(e) => {
                tracing::error!(grant_id = g.id, error = %e, "release broadcast OK but marking grant 'released' FAILED; retries next tick (the broker call is idempotent on its key)");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_every_documented_status() {
        assert_eq!(
            parse_verification(&json!({"status": "pending"})).unwrap(),
            ManaPaymentVerification::Pending
        );
        assert_eq!(
            parse_verification(&json!({"status": "reverted"})).unwrap(),
            ManaPaymentVerification::Reverted
        );
        assert_eq!(
            parse_verification(&json!({"status": "no_payment", "valueWei": "0"})).unwrap(),
            ManaPaymentVerification::NoPayment
        );
        assert_eq!(
            parse_verification(&json!({
                "status": "confirmed",
                "from": "0xAAAA567890123456789012345678901234567890",
                "to": "0xbbbb567890123456789012345678901234567890",
                "valueWei": "2500000000000000000",
            }))
            .unwrap(),
            ManaPaymentVerification::Confirmed {
                from: "0xaaaa567890123456789012345678901234567890".into(),
                to: "0xbbbb567890123456789012345678901234567890".into(),
                value_wei: "2500000000000000000".into(),
            }
        );
    }

    #[test]
    fn rejects_unknown_or_incomplete_shapes() {
        assert!(parse_verification(&json!({"status": "???"})).is_err());
        assert!(parse_verification(&json!({})).is_err());
        assert!(parse_verification(&json!({"status": "confirmed"})).is_err());
        assert!(
            parse_verification(&json!({"status": "confirmed", "from": "0x1", "to": "0x2"}))
                .is_err()
        );
    }

    #[tokio::test]
    async fn an_economy_without_a_token_or_writes_is_not_a_broker() {
        let mut settings = crate::evm::Settings::defaults();
        settings.landiler_escrow_address =
            Some("0x0000000000000000000000000000000000000001".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://nobody:nowhere@127.0.0.1:1/never")
            .unwrap();
        let state = Arc::new(
            crate::evm::State::new(pool, crate::http::Outbound::for_tests(), settings).unwrap(),
        );
        let broker = EconomyBroker::new(state.clone());
        assert!(!broker.configured(), "no admin token");
        assert_eq!(
            broker.escrow_address().as_deref(),
            Some("0x0000000000000000000000000000000000000001")
        );
        let req = BuyRequest {
            collection: "0xeede64bfaf8055492aa500846ec7c6e6a9f533d5".into(),
            item_id: "1".into(),
            token_id: None,
            price_wei: "1".into(),
            mode: "primary".into(),
            buyer: "0x2222222222222222222222222222222222222222".into(),
            trade: None,
            idempotency_key: "k".into(),
            charge_cents: None,
        };
        assert!(
            matches!(broker.buy(&req).await, Err(BrokerError::Retryable(_))),
            "writes off: nothing is bought"
        );
        let err = broker
            .reclaim("0xeede64bfaf8055492aa500846ec7c6e6a9f533d5", "1", "k")
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::NotImplemented(_)), "{err:?}");
    }
}
