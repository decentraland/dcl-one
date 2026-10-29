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
        for _ in 0..self.max_status_checks {
            tokio::time::sleep(self.sleep).await;
            let Ok(resp) = self
                .http
                .get(&url)
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
