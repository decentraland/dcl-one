//! The OpenZeppelin Defender relayer: submit, poll the status until a hash
//! is broadcast, cancel on the status-check budget. Every call goes through
//! the shared guarded client; the API key travels as a bearer and is never
//! logged.

use std::time::Duration;

use serde::Deserialize;

use super::errors::ApiError;
use super::transaction::TransactionData;
use crate::http::{Outbound, read_body_capped};

const CANCEL_REQUEST_TIMEOUT: Duration = Duration::from_millis(5000);
const OZ_BODY_CAP: usize = 256 * 1024;

const BROADCAST_STATUSES: [&str; 3] = ["submitted", "mined", "confirmed"];
const FAILED_STATUSES: [&str; 3] = ["canceled", "failed", "expired"];

#[derive(Debug, Deserialize)]
struct OzResponse {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    data: Option<OzTransactionData>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OzTransactionData {
    id: String,
    #[serde(default)]
    hash: Option<String>,
    #[serde(default)]
    status: String,
    #[serde(default)]
    status_reason: Option<String>,
}

pub struct Relayer {
    http: Outbound,
    base_url: String,
    relayer_id: String,
    api_key: String,
    speed: String,
    max_status_checks: u32,
    sleep: Duration,
}

impl Relayer {
    pub fn new(
        http: Outbound,
        base_url: &str,
        relayer_id: &str,
        api_key: &str,
        speed: &str,
        max_status_checks: u32,
        sleep: Duration,
    ) -> Self {
        Self {
            http,
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            relayer_id: relayer_id.to_string(),
            api_key: api_key.to_string(),
            speed: speed.to_string(),
            max_status_checks,
            sleep,
        }
    }

    fn transactions_url(&self) -> String {
        format!(
            "{}/api/v1/relayers/{}/transactions",
            self.base_url, self.relayer_id
        )
    }

    fn transaction_url(&self, tx_id: &str) -> String {
        format!("{}/{}", self.transactions_url(), tx_id)
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    pub async fn send_meta_transaction(&self, tx: &TransactionData) -> Result<String, ApiError> {
        let resp = self
            .http
            .post(self.transactions_url())
            .header(reqwest::header::AUTHORIZATION, self.bearer())
            .json(&serde_json::json!({ "to": tx.params[0], "data": tx.params[1], "speed": self.speed, "value": "0x0" }))
            .send()
            .await
            .map_err(|e| ApiError::RelayerFailed(format!("relayer request failed: {e}")))?;

        let status = resp.status();
        let body = read_body_capped(resp, OZ_BODY_CAP)
            .await
            .unwrap_or_default();
        if !status.is_success() {
            let text = String::from_utf8_lossy(&body).into_owned();
            if status.as_u16() == 422 || status.as_u16() == 400 {
                return Err(ApiError::RelayReverted(text));
            }
            return Err(ApiError::RelayerFailed(format!(
                "The relayer responded with a {} status code: {text}",
                status.as_u16()
            )));
        }
        let parsed: OzResponse = serde_json::from_slice(&body)
            .map_err(|e| ApiError::RelayerFailed(format!("relayer decode failed: {e}")))?;
        let tx_data = match (parsed.success, parsed.data) {
            (true, Some(d)) => d,
            _ => {
                return Err(ApiError::RelayerFailed(
                    parsed
                        .error
                        .unwrap_or_else(|| "Unexpected response from relayer".into()),
                ));
            }
        };
        if let Some(hash) = &tx_data.hash
            && BROADCAST_STATUSES.contains(&tx_data.status.as_str())
        {
            return Ok(hash.clone());
        }
        self.wait_for_broadcast(&tx_data.id).await
    }

    async fn wait_for_broadcast(&self, tx_id: &str) -> Result<String, ApiError> {
        let url = self.transaction_url(tx_id);
        // The checks run until the configured budget, `sleep × checks`: the
        // backoff starts under one interval, so a count of attempts would
        // give up early.
        let deadline =
            tokio::time::Instant::now() + self.sleep.saturating_mul(self.max_status_checks);
        let mut delays = Self::backoff(self.sleep);
        let mut first = true;
        loop {
            if !std::mem::take(&mut first) {
                let delay = delays.next().unwrap_or(self.sleep);
                tokio::time::sleep(
                    delay.min(deadline.saturating_duration_since(tokio::time::Instant::now())),
                )
                .await;
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
            }
            let Ok(resp) = self
                .http
                .get(&url)
                .timeout(
                    deadline
                        .saturating_duration_since(tokio::time::Instant::now())
                        .max(Duration::from_millis(1)),
                )
                .header(reqwest::header::AUTHORIZATION, self.bearer())
                .send()
                .await
            else {
                continue;
            };
            if !resp.status().is_success() {
                continue;
            }
            let Ok(body) = read_body_capped(resp, OZ_BODY_CAP).await else {
                continue;
            };
            let Ok(parsed) = serde_json::from_slice::<OzResponse>(&body) else {
                continue;
            };
            let Some(data) = parsed.data else { continue };
            if FAILED_STATUSES.contains(&data.status.as_str()) {
                let reason = data
                    .status_reason
                    .clone()
                    .unwrap_or_else(|| data.status.clone());
                return Err(ApiError::RelayReverted(format!(
                    "Transaction {}: {}",
                    data.status, reason
                )));
            }
            if BROADCAST_STATUSES.contains(&data.status.as_str())
                && let Some(hash) = data.hash
            {
                return Ok(hash);
            }
        }
        self.cancel_transaction(tx_id).await;
        Err(ApiError::RelayerTimeout(
            "The relayer took too long to respond: The limit of status checks was reached".into(),
        ))
    }

    /// The waits between status checks: from a quarter second, doubling up
    /// to the configured interval (`OZ_SLEEP_TIME_BETWEEN_CHECKS_MS`), so a
    /// transaction broadcast after the first checks is seen within one
    /// interval.
    fn backoff(sleep: Duration) -> impl Iterator<Item = Duration> {
        std::iter::successors(Some(sleep.min(Duration::from_millis(250))), move |d| {
            Some(d.saturating_mul(2).min(sleep))
        })
    }

    async fn cancel_transaction(&self, tx_id: &str) {
        let url = self.transaction_url(tx_id);
        let result = self
            .http
            .request(reqwest::Method::DELETE, &url)
            .header(reqwest::header::AUTHORIZATION, self.bearer())
            .timeout(CANCEL_REQUEST_TIMEOUT)
            .send()
            .await;
        if let Err(e) = result {
            tracing::warn!(tx_id, error = %e, "relayer cancel request failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broadcast_is_checked_immediately_and_pending_statuses_retry() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        for pending in [0, 1] {
            let calls = Arc::new(AtomicUsize::new(0));
            let observed = calls.clone();
            let app = axum::Router::new().route("/api/v1/relayers/rid/transactions/tx", axum::routing::get(move || {
                let check = observed.fetch_add(1, Ordering::SeqCst);
                async move {
                    axum::Json(serde_json::json!({"success": true, "data": {
                        "id": "tx", "status": if check < pending { "pending" } else { "submitted" },
                        "hash": if check < pending { None } else { Some("0xhash") }
                    }}))
                }
            }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let relayer = Relayer::new(
                Outbound::for_tests(),
                &url,
                "rid",
                "key",
                "fast",
                3,
                if pending == 0 {
                    Duration::from_secs(60)
                } else {
                    Duration::from_millis(10)
                },
            );
            let result =
                tokio::time::timeout(Duration::from_secs(2), relayer.wait_for_broadcast("tx"))
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(result, "0xhash");
            assert_eq!(calls.load(Ordering::SeqCst), pending + 1);
            server.abort();
        }
    }

    #[tokio::test]
    async fn exhausted_status_checks_cancel_the_transaction() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let checks = Arc::new(AtomicUsize::new(0));
        let cancellations = Arc::new(AtomicUsize::new(0));
        let observed = checks.clone();
        let cancelled = cancellations.clone();
        let app = axum::Router::new().route(
            "/api/v1/relayers/rid/transactions/tx",
            axum::routing::get(move || {
                observed.fetch_add(1, Ordering::SeqCst);
                async { axum::http::StatusCode::SERVICE_UNAVAILABLE }
            })
            .delete(move || {
                cancelled.fetch_add(1, Ordering::SeqCst);
                async { axum::http::StatusCode::NO_CONTENT }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let relayer = Relayer::new(
            Outbound::for_tests(),
            &url,
            "rid",
            "key",
            "fast",
            3,
            Duration::from_millis(10),
        );
        let result = tokio::time::timeout(Duration::from_secs(2), relayer.wait_for_broadcast("tx"))
            .await
            .unwrap();
        assert!(matches!(result, Err(ApiError::RelayerTimeout(_))));
        let checks = checks.load(Ordering::SeqCst);
        assert!(
            (1..=3).contains(&checks),
            "{checks} checks in a 30 ms budget"
        );
        assert_eq!(cancellations.load(Ordering::SeqCst), 1);
        server.abort();
    }

    #[test]
    fn the_status_checks_back_off_up_to_the_configured_interval() {
        let ms = |v: u64| Duration::from_millis(v);
        assert_eq!(
            Relayer::backoff(ms(800)).take(5).collect::<Vec<_>>(),
            [ms(250), ms(500), ms(800), ms(800), ms(800)]
        );
        assert_eq!(
            Relayer::backoff(ms(100)).take(3).collect::<Vec<_>>(),
            [ms(100), ms(100), ms(100)]
        );
    }

    #[test]
    fn urls_are_the_defender_relayer_api() {
        let r = Relayer::new(
            Outbound::for_tests(),
            "https://api.defender.example/",
            "rid",
            "key",
            "fast",
            1,
            Duration::ZERO,
        );
        assert_eq!(
            r.transactions_url(),
            "https://api.defender.example/api/v1/relayers/rid/transactions"
        );
        assert_eq!(
            r.transaction_url("tx-1"),
            "https://api.defender.example/api/v1/relayers/rid/transactions/tx-1"
        );
    }
}
