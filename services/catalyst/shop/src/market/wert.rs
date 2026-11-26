//! `POST /v1/wert/sign`: Wert's card on-ramp for a purchase into a contract,
//! as marketplace-server serves it (`wert-signer-and-session-creator-handler`
//! at 9c3683cb). The node signs the caller's contract call with the partner
//! key Wert checks, and creates the widget session on Wert's API.
//!
//! - **The signature** is `@wert-io/widget-sc-signer` 2.0.0's: Ed25519 over
//!   the six message fields as `key:value` lines in a fixed order, with
//!   `commodity` and `network` lowercased and a numeric `commodity_amount`
//!   written as JavaScript writes it. It is returned as uppercase hex.
//! - **The key:** a `publicationFees` target signs with
//!   `WERT_PUBLICATION_FEES_PRIVATE_KEY`, and any other with `WERT_PRIVATE_KEY`.
//!   Both are read from their `_FILE` alone.
//! - A request must be signed (a scene-signed chain counts as unsigned),
//!   and both request wallets must match the signer, else 401. An invalid
//!   target returns 400 "Invalid target".
//! - The startup allow-list binds the chain, contract address and key target.
//!   A tuple outside it returns 400 "Invalid contract" before signing or
//!   requesting a Wert session. `WERT_EXTRA_CONTRACTS` adds approved tuples.
//! - Each replica spends from its own wallet token bucket, shared across
//!   both mounts and all permitted contracts. `WERT_SIGNER_REQUESTS_PER_MINUTE`
//!   sets the initial burst and per-minute refill, defaulting to 10. A full
//!   burst can exhaust the bucket and subsequent requests get 429 with
//!   `Retry-After` until a token refills.
//! - **Differences from upstream:**
//!   - A message with a missing, extra or mistyped field is 400 "Invalid
//!     message". Upstream throws, which is a 500.
//!   - A session Wert does not create is 502, with nothing of Wert's reply
//!     passed on. Upstream surfaces it as a 500.
//! - **Logs:** signatures and policy refusals record the normalized network,
//!   contract and target, with the reason on refusal. Wallets, calldata and
//!   session fields stay out of the logs.
//!
//! With none of the four `WERT_*` settings, the route answers the unavailable
//! envelope naming them.

use std::collections::HashSet;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header::RETRY_AFTER};
use axum::response::{IntoResponse, Response};
use ed25519_dalek::{Signer as _, SigningKey};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::signed::optional_signer;
use super::wire::ApiError;
use super::{SharedState, Source, check_upstream, listings, unavailable};
use crate::credits::batch::collection_store;
use crate::evm::contracts_addrs::{DclContracts, NameContracts};
use crate::shared::limits::{RateLimitDecision, RateLimiter};
use crate::types::{ChainId, EvmAddress};

/// The route, as `DataWrites` sees it: it writes nothing here.
pub(super) const PATH: &str = "/v1/wert/sign";

/// How long Wert's API may take to create a session.
const SESSION_TIMEOUT: Duration = Duration::from_secs(10);

/// The variables, in the order the startup error names a missing one.
const VARIABLES: [&str; 4] = [
    "WERT_PRIVATE_KEY_FILE",
    "WERT_PUBLICATION_FEES_PRIVATE_KEY_FILE",
    "WERT_API_URL",
    "WERT_API_KEY",
];

const NETWORKS: [(ChainId, &str); 4] = [
    (ChainId::ETHEREUM, "ethereum"),
    (ChainId::POLYGON, "polygon"),
    (ChainId::AMOY, "amoy"),
    (ChainId::SEPOLIA, "sepolia"),
];
const RATE_WINDOW: Duration = Duration::from_secs(60);

fn network(value: &str) -> Option<(ChainId, &'static str)> {
    NETWORKS
        .into_iter()
        .find(|(_, name)| name.eq_ignore_ascii_case(value))
}

/// Signing policy built at startup and shared by both route mounts.
pub struct WertPolicy {
    contracts: HashSet<(ChainId, EvmAddress, Target)>,
    signer_limit: RateLimiter,
    retry_after: u64,
}

impl WertPolicy {
    pub fn new(
        credits_manager: Option<(ChainId, EvmAddress)>,
        polygon_publication_manager: EvmAddress,
        extras: Option<&str>,
        requests_per_minute: NonZeroU32,
    ) -> anyhow::Result<Self> {
        let mut contracts = HashSet::new();
        for (chain, _) in NETWORKS {
            for marketplace in listings::offchain_marketplaces(chain.0 as i64) {
                contracts.insert((
                    chain,
                    EvmAddress::parse(marketplace.address)?,
                    Target::Default,
                ));
            }
            if let Some(addresses) = DclContracts::for_chain(chain.0) {
                contracts.insert((chain, addresses.marketplace_v2.into(), Target::Default));
                contracts.insert((
                    chain,
                    addresses.offchain_marketplace.into(),
                    Target::Default,
                ));
            }
            if let Some(addresses) = NameContracts::for_chain(chain.0) {
                contracts.insert((chain, addresses.marketplace.into(), Target::Default));
            }
            if let Some(address) = collection_store(chain) {
                contracts.insert((chain, address, Target::Default));
            }
        }
        if let Some((chain, address)) = credits_manager {
            anyhow::ensure!(
                chain.is_polygon(),
                "CREDITS_MANAGER_CONTRACT requires a Polygon or Amoy COLLECTIONS_CHAIN_ID for Wert"
            );
            contracts.insert((chain, address, Target::Default));
        }
        contracts.insert((
            ChainId::POLYGON,
            polygon_publication_manager,
            Target::PublicationFees,
        ));

        if let Some(extras) = extras.map(str::trim).filter(|value| !value.is_empty()) {
            for (index, entry) in extras.split(',').enumerate() {
                let mut fields = entry.split(':').map(str::trim);
                let (Some(chain), Some(address), Some(target), None) =
                    (fields.next(), fields.next(), fields.next(), fields.next())
                else {
                    anyhow::bail!(
                        "WERT_EXTRA_CONTRACTS entry {} must be network:address:target",
                        index + 1
                    );
                };
                let Some((chain, _)) = network(chain) else {
                    anyhow::bail!(
                        "WERT_EXTRA_CONTRACTS entry {} has an unsupported network",
                        index + 1
                    );
                };
                let Some(target) = Target::from_name(target) else {
                    anyhow::bail!(
                        "WERT_EXTRA_CONTRACTS entry {} has an invalid target",
                        index + 1
                    );
                };
                let address = EvmAddress::parse(address).map_err(|_| {
                    anyhow::anyhow!(
                        "WERT_EXTRA_CONTRACTS entry {} has an invalid EVM address",
                        index + 1
                    )
                })?;
                contracts.insert((chain, address, target));
            }
        }

        let rate = requests_per_minute.get();
        Ok(Self {
            contracts,
            signer_limit: RateLimiter::new(rate, RATE_WINDOW),
            retry_after: RATE_WINDOW.as_secs().div_ceil(u64::from(rate)),
        })
    }
}

/// The partner keys, API and signing policy. Debug omits the credentials.
#[derive(Clone)]
pub struct WertSettings {
    default_key: Arc<SigningKey>,
    publication_fees_key: Arc<SigningKey>,
    api_url: String,
    api_key: String,
    policy: Arc<WertPolicy>,
}

impl std::fmt::Debug for WertSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WertSettings")
            .field("api_url", &self.api_url)
            .finish_non_exhaustive()
    }
}

impl WertSettings {
    /// All four settings, or none (`None`: the route is unavailable). A partial
    /// set, a key that is not 32 bytes of hex, or an API URL that is not a
    /// public https endpoint fails startup naming the variable, never its value.
    pub fn parse(
        private_key: Option<&str>,
        publication_fees_private_key: Option<&str>,
        api_url: Option<&str>,
        api_key: Option<&str>,
        self_hosts: &[String],
        policy: impl FnOnce() -> anyhow::Result<WertPolicy>,
    ) -> anyhow::Result<Option<Self>> {
        let values = [private_key, publication_fees_private_key, api_url, api_key]
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
            Some(private_key),
            Some(fees_key),
            Some(api_url),
            Some(api_key),
        ] = values
        else {
            anyhow::bail!(
                "the Wert on-ramp needs all of {}; missing: {}",
                VARIABLES.join(", "),
                missing.join(", ")
            );
        };
        let api_url = api_url.trim_end_matches('/');
        check_upstream("WERT_API_URL", api_url, self_hosts)?;
        if !api_url.starts_with("https://") {
            anyhow::bail!(
                "WERT_API_URL is not https: the partner API key is never sent in plaintext"
            );
        }
        Ok(Some(Self {
            default_key: Arc::new(seed("WERT_PRIVATE_KEY", private_key)?),
            publication_fees_key: Arc::new(seed("WERT_PUBLICATION_FEES_PRIVATE_KEY", fees_key)?),
            api_url: api_url.to_string(),
            api_key: api_key.to_string(),
            policy: Arc::new(policy()?),
        }))
    }

    fn key(&self, target: Target) -> &SigningKey {
        match target {
            Target::Default => &self.default_key,
            Target::PublicationFees => &self.publication_fees_key,
        }
    }
}

/// A 32-byte Ed25519 seed in hex, with or without `0x`, as the signer library
/// reads it.
fn seed(name: &str, value: &str) -> anyhow::Result<SigningKey> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    let mut bytes = [0u8; 32];
    hex::decode_to_slice(digits, &mut bytes).map_err(|_| {
        anyhow::anyhow!("{name} is not a 32-byte Ed25519 key in hex (64 digits, 0x optional)")
    })?;
    Ok(SigningKey::from_bytes(&bytes))
}

/// The key a signature uses, upstream's `Target`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Target {
    Default,
    PublicationFees,
}

impl Target {
    /// Upstream's check, `target && !Object.values(Target).includes(target)`:
    /// a falsy value is the default key.
    fn from_body(target: Option<&Value>) -> Option<Self> {
        match target {
            None | Some(Value::Null) | Some(Value::Bool(false)) => Some(Self::Default),
            Some(Value::Number(n)) if n.as_f64() == Some(0.0) => Some(Self::Default),
            Some(Value::String(s)) if s.is_empty() => Some(Self::Default),
            Some(Value::String(s)) => Self::from_name(s),
            _ => None,
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "default" => Some(Self::Default),
            "publicationFees" => Some(Self::PublicationFees),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::PublicationFees => "publicationFees",
        }
    }
}

/// The six fields the signer library signs, and no other.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    address: String,
    commodity: String,
    commodity_amount: Value,
    network: String,
    sc_address: String,
    sc_input_data: String,
}

impl Message {
    /// The text the signature covers. `commodity_amount` is a string as sent,
    /// or a number as JavaScript's `String(n)` writes it; anything else has
    /// no signature (`None`).
    fn signed_text(&self) -> Option<String> {
        let amount = match &self.commodity_amount {
            Value::String(amount) => amount.clone(),
            Value::Number(amount) => crate::shared::js::number_to_string(amount.as_f64()?),
            _ => return None,
        };
        Some(format!(
            "address:{}\ncommodity:{}\ncommodity_amount:{amount}\nnetwork:{}\nsc_address:{}\nsc_input_data:{}",
            self.address,
            self.commodity.to_lowercase(),
            self.network.to_lowercase(),
            self.sc_address,
            self.sc_input_data,
        ))
    }
}

#[derive(Deserialize)]
struct SignRequest {
    message: Message,
    session: Map<String, Value>,
    #[serde(default)]
    target: Option<Value>,
}

/// The signature Wert checks: Ed25519 over the text, as uppercase hex.
fn sign_text(key: &SigningKey, text: &str) -> String {
    hex::encode_upper(key.sign(text.as_bytes()).to_bytes())
}

pub async fn sign(State(state): State<SharedState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(wert) = state.settings.market.wert.as_ref() else {
        return unavailable(
            Source::Wert,
            "Wert card purchases are not configured on this node",
        );
    };
    let signer = match optional_signer(state.eip1654.as_deref(), &headers, "post", PATH).await {
        Ok(Some(signer)) => signer,
        Ok(None) => return ApiError::unauthorized("Unauthorized").into_response(),
        Err(refusal) => return refusal.into_response(),
    };
    let Ok(request) = serde_json::from_slice::<SignRequest>(&body) else {
        return ApiError::bad_request("Invalid message").into_response();
    };
    let wallet = request
        .session
        .get("wallet_address")
        .and_then(Value::as_str);
    if request.message.address.to_lowercase() != signer
        || wallet.map(str::to_lowercase).as_deref() != Some(signer.as_str())
    {
        return ApiError::unauthorized("Unauthorized").into_response();
    }
    let Some(target) = Target::from_body(request.target.as_ref()) else {
        return ApiError::bad_request("Invalid target").into_response();
    };
    let Some(text) = request.message.signed_text() else {
        return ApiError::bad_request("Invalid message").into_response();
    };
    let network = network(&request.message.network);
    let contract = EvmAddress::parse(&request.message.sc_address).ok();
    let (network_name, contract) = match (network, contract) {
        (Some((chain, name)), Some(address))
            if wert.policy.contracts.contains(&(chain, address, target)) =>
        {
            (name, address)
        }
        _ => {
            let reason = match (network, contract) {
                (None, _) => "unsupported_network",
                (_, None) => "invalid_address",
                _ => "contract_not_allowed",
            };
            log_refusal(network.map(|(_, name)| name), contract, target, reason);
            return ApiError::bad_request("Invalid contract").into_response();
        }
    };
    if wert.policy.signer_limit.check(&signer) == RateLimitDecision::Deny {
        log_refusal(
            Some(network_name),
            Some(contract),
            target,
            "signer_rate_limit",
        );
        return (
            [(RETRY_AFTER, wert.policy.retry_after.to_string())],
            ApiError::http(429, "Too many Wert requests"),
        )
            .into_response();
    }
    let signature = sign_text(wert.key(target), &text);
    let session_id = match create_session(&state, wert, &request.session).await {
        Ok(session_id) => session_id,
        Err(error) => {
            tracing::warn!(
                sc_address = %contract,
                network = network_name,
                wert_target = target.name(),
                reason = %error,
                "Wert did not create a card purchase session"
            );
            return ApiError::http(502, "Wert session could not be created").into_response();
        }
    };
    tracing::info!(
        sc_address = %contract,
        network = network_name,
        wert_target = target.name(),
        "signed a Wert card purchase"
    );
    (
        StatusCode::OK,
        Json(json!({ "ok": true, "data": { "signature": signature, "sessionId": session_id } })),
    )
        .into_response()
}

fn log_refusal(network: Option<&str>, contract: Option<EvmAddress>, target: Target, reason: &str) {
    tracing::warn!(
        sc_address = contract
            .map(|address| address.to_string())
            .as_deref()
            .unwrap_or("invalid"),
        network = network.unwrap_or("unknown"),
        wert_target = target.name(),
        reason,
        "refused a Wert card purchase"
    );
}

/// `POST {WERT_API_URL}/create-session` with the session as the caller sent
/// it; the error names what failed, never the reply.
async fn create_session(
    state: &SharedState,
    wert: &WertSettings,
    session: &Map<String, Value>,
) -> Result<String, String> {
    let response = state
        .http
        .post(format!("{}/create-session", wert.api_url))
        .max_redirects(0)
        .header("x-api-key", wert.api_key.as_str())
        .json(session)
        .timeout(SESSION_TIMEOUT)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                "the request timed out".to_string()
            } else {
                "the request failed".to_string()
            }
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Wert answered {status}"));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| "Wert's reply is not JSON".to_string())?;
    body.get("sessionId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "Wert's reply has no sessionId".to_string())
}

#[cfg(test)]
#[path = "wert_tests.rs"]
mod tests;
