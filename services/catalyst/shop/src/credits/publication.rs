//! A collection's publication fee paid with credits: `POST
//! /credits/authorize-publication` answers one ephemeral credit sized for the
//! fee and the co-signature of the collection manager's `createCollection`
//! call, both spent by one `useCredits` on this node's CreditsManager.
//!
//! The request is credits-server's (`usdPriceCents`, `chainId`,
//! `creditsManagerAddress`, `externalCall`) and so is the answer (`credit`,
//! `externalCallSignature`, `usdCents`, `oracleRate`). The rules are this
//! node's: the chain and the manager are the configured ones, the call is
//! the chain's CollectionManager's `createCollection`, and the builder
//! ([`Publications`]) says whose collection the call publishes and what it
//! costs, so the signer is its creator, the collection is an unpublished
//! draft of theirs and a `usdPriceCents` other than the fee answers 409
//! `Price changed`. The flow then follows [`super::batch::authorize_group`]:
//! the lane, the keys, a retried request answered from its row, the trial,
//! the CreditUsed index, the issuance gate (with
//! [`super::settlement::IssuanceGate::check_publication`]), the reserve
//! under the wallet lock with one `publication_fee` line, and only then the
//! two signatures.
//!
//! The authorization keeps the hash the co-signature covers
//! (`request_digest`), and the relay accepts a `useCredits` whose call is
//! `createCollection` only for it ([`guard_relayed`]).

use alloy_primitives::{Address, B256, U256};
use alloy_sol_types::{SolCall, sol};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::State;
use super::authorize::{GroupLine, NewGroup, ReserveRefusal};
use super::batch::{AuthorizeError, AuthorizedCreditOut};
use super::digest::{
    CreditIdentity, ExternalCallIdentity, TreasuryWallet, bytes32_hex, parse_bytes32,
};
use super::errors::ApiError;
use super::usage;
use crate::policy_entry::{UseCreditsCall, manager_routes};
use crate::shared::signed_fetch::Signer;
use crate::types::{
    ChainId, CreditSizing, CustomEffect, EvmAddress, ExternalCall, ManaUsdRate, ManaWei, UsdCents,
};

sol! {
    struct PublicationItem { string rarity; uint256 price; address beneficiary; string metadata; }
    function createCollection(address forwarder, address factory, bytes32 salt, string name, string symbol, string baseURI, address creator, PublicationItem[] items);
}

/// The `sale_kind` of a publication fee's line.
pub const SALE_KIND: &str = "publication_fee";
/// The `asset_kind` of a publication fee's line.
pub const ASSET_KIND: &str = "collection";
pub const SOURCE: &str = "credits-publication";
pub const SIGNER_CONFIGURE: &str = "CREDITS_EXTERNAL_CALL_SIGNER_KEY_FILE";

/// What a `createCollection` call publishes and costs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationQuote {
    /// The address the collection deploys at.
    pub contract: EvmAddress,
    pub usd_cents: i64,
}

/// The builder's answer for a publication: `serve` attaches it.
#[async_trait]
pub trait Publications: Send + Sync {
    /// The collection the `createCollection` arguments `data` publish for
    /// `signer`, its creator, and its fee; a refusal for any other call.
    async fn quote(&self, signer: &Signer, data: &[u8]) -> Result<PublicationQuote, ApiError>;
}

/// A validated request body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub usd_price_cents: i64,
    pub chain_id: u64,
    pub manager: Address,
    pub target: Address,
    pub selector: [u8; 4],
    pub data: Vec<u8>,
    /// Unix seconds.
    pub expires_at: u64,
    pub salt: B256,
}

fn hex_field(object: &Value, key: &str, bytes: Option<usize>) -> Option<Vec<u8>> {
    let digits = object.get(key)?.as_str()?.strip_prefix("0x")?;
    let decoded = hex::decode(digits).ok()?;
    bytes.is_none_or(|n| decoded.len() == n).then_some(decoded)
}

fn only_keys(value: &Value, keys: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.keys().all(|key| keys.contains(&key.as_str())))
}

/// credits-server's `AuthorizePublicationSchema`: every field required, no
/// other property.
pub fn parse(body: &Value) -> Result<Request, String> {
    const BODY: [&str; 4] = [
        "usdPriceCents",
        "chainId",
        "creditsManagerAddress",
        "externalCall",
    ];
    const CALL: [&str; 5] = ["target", "selector", "data", "expiresAt", "salt"];
    let call = body.get("externalCall").unwrap_or(&Value::Null);
    if !only_keys(body, &BODY) || !only_keys(call, &CALL) {
        return Err(
            "The body holds \"usdPriceCents\", \"chainId\", \"creditsManagerAddress\" and \"externalCall\" (\"target\", \"selector\", \"data\", \"expiresAt\", \"salt\") and nothing else"
                .to_string(),
        );
    }
    let usd_price_cents = body
        .get("usdPriceCents")
        .and_then(Value::as_i64)
        .filter(|cents| *cents > 0)
        .ok_or("A positive integer \"usdPriceCents\" is required")?;
    let chain_id = body
        .get("chainId")
        .and_then(Value::as_u64)
        .ok_or("An integer \"chainId\" is required")?;
    let address = |object: &Value, key: &str| {
        hex_field(object, key, Some(20)).map(|bytes| Address::from_slice(&bytes))
    };
    let manager = address(body, "creditsManagerAddress")
        .ok_or("\"creditsManagerAddress\" must be an address")?;
    let target = address(call, "target").ok_or("\"externalCall.target\" must be an address")?;
    let selector: [u8; 4] = hex_field(call, "selector", Some(4))
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or("\"externalCall.selector\" must be 4 bytes of hex")?;
    let data = hex_field(call, "data", None).ok_or("\"externalCall.data\" must be hex")?;
    let expires_at = call
        .get("expiresAt")
        .and_then(Value::as_u64)
        .filter(|seconds| *seconds > 0)
        .ok_or("A positive integer \"externalCall.expiresAt\" (unix seconds) is required")?;
    let salt = call
        .get("salt")
        .and_then(Value::as_str)
        .and_then(parse_bytes32)
        .ok_or("\"externalCall.salt\" must be 32 bytes of hex")?;
    Ok(Request {
        usd_price_cents,
        chain_id,
        manager,
        target,
        selector,
        data,
        expires_at,
        salt,
    })
}

/// The answer: credits-server's, field for field.
#[derive(Debug, Clone, Serialize)]
pub struct PublicationAuthorizationOut {
    pub credit: AuthorizedCreditOut,
    #[serde(rename = "externalCallSignature")]
    pub external_call_signature: String,
    #[serde(rename = "usdCents")]
    pub usd_cents: i64,
    #[serde(rename = "oracleRate")]
    pub oracle_rate: String,
}

fn unavailable(configure: &'static str, message: impl Into<String>) -> AuthorizeError {
    AuthorizeError::Unavailable {
        source: SOURCE,
        configure,
        message: message.into(),
    }
}

fn bad_request(message: &str) -> AuthorizeError {
    AuthorizeError::Api(ApiError::bad_request(message))
}

/// The key of a request: the signer, the call it asks a co-signature for and
/// the signed-fetch timestamp, so the same signed request replays its credit.
fn idempotency_key(signer: &str, call_hash: &str, auth_ts: &str) -> String {
    let mut h = Sha256::new();
    for part in [signer, call_hash, auth_ts] {
        h.update(part.as_bytes());
        h.update([0x1f]);
    }
    format!("authz-pub:{}", hex::encode(h.finalize()))
}

fn answer(
    wallet: &TreasuryWallet,
    co_signer: &TreasuryWallet,
    credit: &CreditIdentity,
    call: &ExternalCallIdentity,
    usd_cents: i64,
    oracle_rate: String,
) -> Result<PublicationAuthorizationOut, AuthorizeError> {
    let signature = wallet
        .sign_credit(credit)
        .map_err(|e| ApiError::Internal(format!("credit signing failed: {e}")))?;
    let external_call_signature = co_signer
        .sign_external_call(call)
        .map_err(|e| ApiError::Internal(format!("external call signing failed: {e}")))?;
    let amount = credit.value.to_string();
    Ok(PublicationAuthorizationOut {
        credit: AuthorizedCreditOut {
            id: bytes32_hex(credit.salt),
            amount: amount.clone(),
            available_amount: amount,
            expires_at: credit.expires_at as i64,
            signature,
            contract: format!("{:#x}", credit.manager),
        },
        external_call_signature,
        usd_cents,
        oracle_rate,
    })
}

/// The whole flow for `request` on behalf of `signer`; `auth_ts` is the
/// signed-fetch timestamp.
pub async fn authorize(
    state: &State,
    signer: &Signer,
    request: &Request,
    auth_ts: &str,
) -> Result<PublicationAuthorizationOut, AuthorizeError> {
    let settings = &state.settings;
    if !settings.credits_onchain_authorize_enabled {
        return Err(unavailable(
            "CREDITS_ONCHAIN_AUTHORIZE_ENABLED",
            "on-chain credit authorization is disabled; set CREDITS_ONCHAIN_AUTHORIZE_ENABLED=true to enable it",
        ));
    }
    let (Some(signer_key), Some(contract)) = (
        settings.credits_signer_key.as_ref(),
        settings.credits_manager_contract.as_ref(),
    ) else {
        return Err(unavailable(
            "CREDITS_SIGNER_KEY_FILE",
            "credit authorization is unavailable (signer key or CreditsManager contract unset)",
        ));
    };
    let Some(co_signer_key) = settings.credits_external_call_signer_key.as_ref() else {
        return Err(unavailable(
            SIGNER_CONFIGURE,
            "a publication cannot be paid with credits on this node: no key co-signs its createCollection call",
        ));
    };
    let Some(publications) = state.publications.get() else {
        return Err(AuthorizeError::Api(ApiError::service_unavailable(
            "a publication cannot be paid with credits on this node: no builder prices it",
        )));
    };
    let wallet = TreasuryWallet::from_hex(signer_key)
        .map_err(|e| ApiError::Internal(format!("invalid credits signer key: {e}")))?;
    let co_signer = TreasuryWallet::from_hex(co_signer_key).map_err(|e| {
        ApiError::Internal(format!("invalid credits external call signer key: {e}"))
    })?;
    let buyer = EvmAddress::parse(signer.as_str())
        .map_err(|_| ApiError::bad_request("Invalid or missing signed-fetch authentication"))?;
    let chain_id = ChainId(settings.usage.chain_id as u64);
    let manager = EvmAddress::parse(contract).map_err(|_| {
        ApiError::Internal("CREDITS_MANAGER_CONTRACT is not an address".to_string())
    })?;

    if request.chain_id != chain_id.0 {
        return Err(bad_request("Unsupported chain"));
    }
    if request.manager != manager.address() {
        return Err(bad_request("Invalid credits manager address"));
    }
    let collection_manager = manager_routes(chain_id)
        .map(|routes| routes.collection_manager)
        .ok_or_else(|| bad_request("Unsupported chain"))?;
    if request.target != collection_manager {
        return Err(bad_request(
            "Invalid external call target: must be CollectionManager",
        ));
    }
    if request.selector != createCollectionCall::SELECTOR {
        return Err(bad_request(
            "Invalid function selector: must be createCollection",
        ));
    }
    let now = chrono::Utc::now();
    if request.expires_at <= now.timestamp() as u64 {
        return Err(bad_request("The external call has expired"));
    }
    let call = ExternalCallIdentity {
        sender: buyer.address(),
        chain_id: chain_id.0,
        manager: manager.address(),
        target: request.target,
        selector: request.selector,
        data: request.data.clone(),
        expires_at: U256::from(request.expires_at),
        salt: request.salt,
    };
    let call_hash = bytes32_hex(call.hash());
    let key = idempotency_key(signer.as_str(), &call_hash, auth_ts);

    if let Some(prior) = state
        .credits
        .authorization_for_key(&key, signer.as_str())
        .await?
    {
        if prior.request_digest.as_deref() != Some(call_hash.as_str()) {
            return Err(AuthorizeError::OperationReused);
        }
        return replay(state, &prior.id, &wallet, &co_signer, &call).await;
    }

    if let Some(trial) = &settings.trial
        && !trial.admits(signer.as_str())
    {
        return Err(AuthorizeError::NotATrialBuyer);
    }
    if state.usage.ready_at(now).await.is_none() {
        return Err(AuthorizeError::Unavailable {
            source: usage::SOURCE,
            configure: usage::CONFIGURE,
            message: "credit authorization is unavailable: the CreditUsed index that consumes a pledge is not ready (not configured, catching up, or its last fold is older than 10 minutes)".to_string(),
        });
    }
    let gate = match state.gate.check(manager.address(), wallet.signer()).await {
        Ok(()) => {
            state
                .gate
                .check_publication(manager.address(), co_signer.signer(), collection_manager)
                .await
        }
        refused => refused,
    };
    if let Err(why) = gate {
        return Err(AuthorizeError::Unavailable {
            source: super::settlement::GATE_SOURCE,
            configure: super::settlement::GATE_CONFIGURE,
            message: format!("credit issuance is refused: {why}"),
        });
    }

    let quote = publications.quote(signer, &request.data).await?;
    if quote.usd_cents != request.usd_price_cents {
        return Err(AuthorizeError::PriceChanged {
            lines: vec![(0, quote.usd_cents, request.usd_price_cents)],
        });
    }
    let quoted = UsdCents::new(quote.usd_cents);
    let charged = quoted
        .charge_for_quote()
        .ok_or_else(|| ApiError::bad_request("The publication fee is out of range"))?;
    let mana_usd = state.pricing.fetch_mana_usd().await?;
    let rate = ManaUsdRate::parse_usd_per_mana(&mana_usd).ok_or_else(|| {
        ApiError::Internal(format!(
            "oracle rate {mana_usd:?} is not a positive decimal"
        ))
    })?;
    let sizing = CreditSizing::new(
        rate,
        settings.credit_buffer_bps,
        settings.credit_cap_headroom_pct,
    )
    .ok_or_else(|| {
        ApiError::Internal(
            "CREDITS_CAP_HEADROOM_PCT or CREDITS_ORACLE_BUFFER_BPS out of range".to_string(),
        )
    })?;
    let value: ManaWei = sizing
        .cap(quoted)
        .ok_or_else(|| ApiError::bad_request("The publication fee is out of range"))?;
    let recorded_rate = sizing.recorded_rate();

    let salt = B256::from_slice(
        &super::batch::getrandom_32()
            .ok_or_else(|| ApiError::Internal("CSPRNG unavailable".into()))?,
    );
    let salt_hex = bytes32_hex(salt);
    let expires_at_s = now.timestamp() as u64 + settings.credit_ttl_seconds;
    let expires_at = chrono::DateTime::from_timestamp(expires_at_s as i64, 0)
        .ok_or_else(|| ApiError::Internal("expiry out of range".into()))?
        .naive_utc();
    let credit = CreditIdentity {
        sender: buyer.address(),
        chain_id: chain_id.0,
        manager: manager.address(),
        value: value.get(),
        expires_at: expires_at_s,
        salt,
    };
    let collection = quote.contract.to_string();
    let lines = [GroupLine {
        quoted_cents: quoted.get(),
        charged_cents: charged.get(),
        sale_kind: SALE_KIND,
        asset_kind: ASSET_KIND,
        asset_chain_id: chain_id.0 as i64,
        asset_contract: collection.clone(),
        asset_item_id: None,
        asset_token_id: None,
        trade_id: None,
        contract_address: Some(collection),
        item_id: None,
        registered_name: None,
    }];
    let rate_text = recorded_rate.rate().to_string();
    let group = NewGroup {
        id: &salt_hex,
        address: signer.as_str(),
        chain_id: chain_id.0 as i64,
        contract: &manager.to_string(),
        credit_hash: &bytes32_hex(credit.credit_hash()),
        quoted_cents: quoted.get(),
        charged_cents: charged.get(),
        amount_wei: &value.get().to_string(),
        oracle_rate: &rate_text,
        oracle_decimals: recorded_rate.decimals() as i16,
        source: Some("builder"),
        expires_at,
        lines: &lines,
    };
    let reserved = match state
        .credits
        .reserve_credit_group(&group, &key, Some(&call_hash))
        .await?
    {
        Ok(reserved) => reserved,
        Err(ReserveRefusal::Insufficient {
            balance_cents,
            required_cents,
        }) => {
            return Err(AuthorizeError::Insufficient {
                balance_cents,
                required_cents,
            });
        }
        Err(ReserveRefusal::TooManyLive { balance_cents }) => {
            return Err(AuthorizeError::TooManyLive { balance_cents });
        }
        Err(ReserveRefusal::OperationReused) => return Err(AuthorizeError::OperationReused),
    };
    if reserved.replayed {
        return replay(state, &reserved.id, &wallet, &co_signer, &call).await;
    }
    tracing::info!(address = %signer.as_str(), salt = %salt_hex, usd_cents = charged.get(), value = %value.get(), collection = %quote.contract, "authorized a publication credit");
    answer(
        &wallet,
        &co_signer,
        &credit,
        &call,
        charged.get(),
        rate_text,
    )
}

/// A reserved publication, answered from its row: the stored value, salt,
/// expiry and rate, signed again (both signatures are deterministic).
async fn replay(
    state: &State,
    id: &str,
    wallet: &TreasuryWallet,
    co_signer: &TreasuryWallet,
    call: &ExternalCallIdentity,
) -> Result<PublicationAuthorizationOut, AuthorizeError> {
    let row = state
        .credits
        .get_authorization(id)
        .await?
        .ok_or_else(|| ApiError::Internal("the replayed authorization vanished".into()))?;
    let stored = || ApiError::Internal("a replayed publication authorization is incomplete".into());
    let credit = CreditIdentity {
        sender: call.sender,
        chain_id: row.chain_id.ok_or_else(stored)? as u64,
        manager: row
            .contract
            .as_deref()
            .and_then(|contract| contract.parse().ok())
            .ok_or_else(stored)?,
        value: U256::from_str_radix(&row.amount_wei, 10).map_err(|_| stored())?,
        expires_at: row.expires_at.and_utc().timestamp() as u64,
        salt: parse_bytes32(&row.id).ok_or_else(stored)?,
    };
    answer(
        wallet,
        co_signer,
        &credit,
        call,
        row.usd_cents,
        row.oracle_rate.ok_or_else(stored)?,
    )
}

/// Whether a relayed `useCredits` whose call is `createCollection` spends
/// the publication credit this node issued for exactly that call: one
/// credit, no MANA of the sender's beside it, and a live authorization of
/// the sender's on this chain and `manager` with a `publication_fee` line,
/// the credit's value and expiry, and the hash of the call as sent.
/// `Err` names why it is refused.
pub async fn guard_relayed(
    pool: &sqlx::PgPool,
    chain_id: ChainId,
    sender: EvmAddress,
    manager: Address,
    call: &UseCreditsCall,
) -> Result<(), String> {
    let ExternalCall::Custom {
        effect: CustomEffect::CollectionPublication { creator },
        ..
    } = &call.call
    else {
        return Err("the call is not a collection publication".to_string());
    };
    if *creator != sender {
        return Err("the publication names another creator than its sender".to_string());
    }
    let [credit] = call.credits.as_slice() else {
        return Err("a publication spends exactly one credit".to_string());
    };
    if call.max_uncredited != ManaWei::ZERO || call.max_credited.get() > credit.value.get() {
        return Err("a publication paid with credits moves no MANA beyond its credit".to_string());
    }
    let expires_at = i64::try_from(credit.expires_at)
        .ok()
        .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
        .ok_or("the credit's expiry is out of range")?
        .naive_utc();
    let call_hash = ExternalCallIdentity {
        sender: sender.address(),
        chain_id: chain_id.0,
        manager,
        target: call.sent.target,
        selector: call.sent.selector,
        data: call.sent.data.clone(),
        expires_at: call.sent.expires_at,
        salt: B256::from(call.sent.salt),
    }
    .hash();
    let issued: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM credits.authorizations a \
         WHERE a.id = $1 AND a.address = $2 AND a.status = 'authorized' AND a.chain_id = $3 AND a.contract = $4 \
           AND a.amount_wei = $5::numeric AND a.expires_at = $6 AND a.expires_at > $7 AND a.request_digest = $8 \
           AND EXISTS (SELECT 1 FROM credits.authorization_lines l \
                       WHERE l.authorization_id = a.id AND l.sale_kind = $9))",
    )
    .bind(bytes32_hex(B256::from(credit.salt)))
    .bind(sender.to_string())
    .bind(chain_id.0 as i64)
    .bind(format!("{manager:#x}"))
    .bind(credit.value.get().to_string())
    .bind(expires_at)
    .bind(super::store::now())
    .bind(bytes32_hex(call_hash))
    .bind(SALE_KIND)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "the publication authorization of a relayed useCredits could not be read");
        "the publication authorization could not be read; try again".to_string()
    })?;
    if issued {
        Ok(())
    } else {
        Err(
            "no live publication authorization of this node covers this credit and call"
                .to_string(),
        )
    }
}
